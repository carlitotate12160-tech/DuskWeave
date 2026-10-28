#!/usr/bin/env python3
"""Bounded two-worker scheduling for the instrumented Rust CI suite.

rust-build compiles the full instrumented inventory once (cargo-llvm-cov
0.9.1 `show-env` flow) and ships the compiled object set plus a manifest;
rust-tests workers replay an assigned partition of test executables through
the existing native bootstrap/supervisor; the retained `rust` job
re-validates the complete worker evidence, then `cargo llvm-cov report`
merges raw LLVM profiles against that same object set for the unchanged
coverage gate.

Subcommands: enumerate, manifest, plan, verify, receipt, aggregate.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

# Build-time enumeration only: libtest `--list` prints names without running
# them, and `--list --ignored` prints exactly the ignored subset.
LIST_ARGS = ("--list",)
LIST_IGNORED_ARGS = ("--list", "--ignored")
# The reservation-stall qualification keeps its distinct supervised prelude;
# it always runs in addition to the ordinary full-suite inventory.
PRELUDE_TARGET = "tests/m1_provider_fixture.rs"
PRELUDE_TEST = ("stalled_reservation_reply_hard_stops_at_the_"
                "original_decision_envelope")
WORKER_COUNTS = (1, 2)
# Policy weight for a target with no measured baseline (seconds); new targets
# enter automatically and stay visible, so the estimate is deliberately
# conservative rather than fitted.
DEFAULT_WEIGHT = 30.0
# Measured per-target wall seconds from the serial baseline run
# (job 122251064388, 2026-10-27, instrumented). Scheduling hints only.
WEIGHTS = {
    "tests/m1_inference_lifecycle.rs": 331.4, "src/lib.rs": 176.3, "tests/m1_run_failures.rs": 155.3,
    "tests/m1_inference_budget_limits.rs": 106.1, "tests/m1_provider_fixture.rs": 99.3, "tests/m1_run_diagnostics.rs": 87.8,
    "tests/m1_session_cli.rs": 67.8, "tests/m1_dns_selection_postgres.rs": 51.3, "tests/m1_prepared_withdrawal_commit_ack.rs": 40.2,
    "tests/m1_terrain_postgres.rs": 36.1, "tests/database_wait_postgres.rs": 36.0, "tests/database_wait_commit.rs": 33.5,
    "tests/m1_run_postgres.rs": 26.8, "tests/m1_terrain_context_postgres.rs": 25.2, "tests/m1_context_prepare_publication.rs": 25.0,
    "tests/m1_prepared_withdrawal_postgres.rs": 23.6, "src/main.rs": 20.6, "tests/m1_native_commit_ack.rs": 18.4,
    "tests/m1_inference_permission_postgres.rs": 18.0, "tests/m1_permission_postgres.rs": 16.3, "tests/m1_policy_postgres.rs": 14.7,
    "tests/m1_terrain_context_commit_ack.rs": 14.1, "tests/m1_session_postgres.rs": 13.0, "tests/m1_session_privilege.rs": 12.8,
    "tests/m1_context_prepare_authority.rs": 12.4, "tests/planning_bounds_cli.rs": 12.2, "tests/planning_history_cli.rs": 11.9,
    "tests/m1_prepared_withdrawal_concurrency.rs": 11.5, "tests/authority_confirmation_cli.rs": 11.1, "tests/database_wait_cli.rs": 10.0,
    "tests/planning_cli.rs": 9.8, "tests/m1_inference_budget_concurrency.rs": 9.5, "tests/m1_terrain_cli.rs": 9.4,
    "tests/withdrawal_cli.rs": 9.3, "tests/planning_history_postgres.rs": 9.1, "tests/m1_native_privilege.rs": 8.6,
    "tests/m1_provider_terminal_metadata.rs": 7.9, "tests/m1_inference_permission_cli.rs": 7.8, "tests/m1_context_prepare_postgres.rs": 7.7,
    "tests/planning_baseline_convergence.rs": 7.6, "tests/m1_terrain_context_cli.rs": 7.1, "tests/planning_journal_postgres.rs": 6.9,
    "tests/m1_prepared_withdrawal_cli.rs": 6.7, "tests/m1_terrain_commit_ack.rs": 6.7, "tests/planning_withdrawal_history.rs": 6.6,
    "tests/m1_inference_budget_postgres.rs": 6.5, "tests/m1_context_prepare_snapshot.rs": 5.8, "tests/m1_native_cli.rs": 5.8,
    "tests/fixture_supervisor.rs": 5.5, "tests/m1_session_concurrency.rs": 5.5, "tests/m1_native_release.rs": 5.4,
    "tests/m1_inference_budget_commit_ack.rs": 5.2, "tests/m1_context_prepare_capacity.rs": 4.8, "tests/m1_inference_permission_commit_ack.rs": 4.8,
    "tests/planning_postgres.rs": 4.4, "src/bin/dw-fixture-supervisor/main.rs": 4.2, "tests/planning_positive_cli.rs": 4.2,
    "tests/m1_context_prepare_commit_ack.rs": 4.1, "tests/mission_producer_transactions.rs": 4.1, "tests/planning_bounds_postgres.rs": 4.0,
    "tests/withdrawal_history.rs": 4.0, "tests/m1_inference_budget_cli.rs": 3.9, "tests/planning_history_concurrency.rs": 3.7,
    "tests/registration_commit_ack.rs": 3.7, "tests/m1_inference_budget_privilege.rs": 3.4, "tests/m1_permission_cli.rs": 3.4,
    "tests/planning_positive_postgres.rs": 3.4, "tests/planning_history_conflicts.rs": 3.1, "tests/m1_policy_cli.rs": 2.9,
    "tests/m1_context_prepare_cli.rs": 2.7, "tests/planning_history_recovery.rs": 2.7, "tests/registration_postgres.rs": 2.7,
    "tests/registration_recovery.rs": 2.7, "tests/planning_withdrawal_postgres.rs": 2.3, "tests/planning_c1b_integration.rs": 2.1,
    "tests/planning_withdrawal_cli.rs": 2.1, "tests/withdrawal_concurrency.rs": 1.9, "tests/planning_concurrency.rs": 1.8,
    "tests/m1_native_continuation.rs": 1.7, "tests/withdrawal_recovery.rs": 1.7, "tests/planning_recovery.rs": 1.6,
    "tests/withdrawal_postgres.rs": 1.6, "tests/database_config_postgres.rs": 1.4, "tests/registration_cli.rs": 1.2,
    "tests/registration_permissions.rs": 1.2, "tests/authority_confirmation_stream_cli.rs": 0.9, "tests/m1_context_prepare_limits.rs": 0.9,
    "tests/planning_v1_compatibility.rs": 0.8, "tests/planning_withdrawal_compatibility.rs": 0.8, "tests/database_config_cli.rs": 0.4,
    "tests/m1_autonomous_wire.rs": 0.1, "tests/m1_run_cli.rs": 0.1, "tests/m1_terrain_context.rs": 0.1,
    "tests/m1_autonomous_permission.rs": 0.0, "tests/m1_context_prepare.rs": 0.0, "tests/m1_context_prepare_integrity.rs": 0.0,
    "tests/m1_inference_budget.rs": 0.0, "tests/m1_inference_permission.rs": 0.0, "tests/m1_native_admission.rs": 0.0,
    "tests/m1_permission.rs": 0.0, "tests/m1_policy.rs": 0.0, "tests/m1_prepared_withdrawal.rs": 0.0,
    "tests/m1_run.rs": 0.0, "tests/m1_schema_contract.rs": 0.0, "tests/m1_session.rs": 0.0,
    "tests/m1_terrain.rs": 0.0, "tests/mission_registration.rs": 0.0, "tests/parser_mutation.rs": 0.0,
    "tests/planning.rs": 0.0, "tests/planning_bounds.rs": 0.0, "tests/planning_history.rs": 0.0,
    "tests/planning_positive.rs": 0.0, "tests/planning_withdrawal.rs": 0.0, "tests/withdrawal.rs": 0.0,
}
TEST_LINE = re.compile(
    r"^test (\S+)(?: - should panic)? \.\.\. (ok|FAILED|ignored|measured)\b")
# Child processes bypass libtest's capture and write to the same stream, so a
# header can be separated from its verdict by arbitrary child output lines.
TEST_HEAD = re.compile(r"^test (\S+)(?: - should panic)? \.\.\.")
VERDICT_LINE = re.compile(r"^(ok|FAILED|ignored|measured)\b")
# A spawned test binary writes its own libtest transcript into the captured
# stream: `running N tests` opens a nested harness scope and its `test result`
# summary closes it. Only depth-1 lines are outer-harness evidence.
HARNESS_OPEN = re.compile(r"^running \d+ tests?")
HARNESS_DONE = re.compile(r"^test result: ")
RESULT_LINE = re.compile(
    r"^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
    r"(\d+) measured; (\d+) filtered out; finished in ([\d.]+)s")
LABEL_RE = re.compile(r"[a-z0-9_-]+")
ELF_MAGIC = b"\x7fELF"


def fail(message):
    print(f"ci_rust_shards: {message}", file=sys.stderr)
    raise SystemExit(1)


def load_json(path, what):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        fail(f"cannot read {what} {path}: {exc}")


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def is_elf(path):
    try:
        with open(path, "rb") as fh:
            return fh.read(4) == ELF_MAGIC
    except OSError:
        return False


def rel(root, path):
    return Path(os.path.relpath(path, root)).as_posix()


def file_hash(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def artifact_entries(path):
    entries = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") == "compiler-artifact" and msg.get("executable"):
            entries.append(msg)
    return entries


def enumerate_tests(exe, env):
    """List libtest names without executing tests; returns (all, ignored)."""
    out = {}
    for key, extra in (("tests", LIST_ARGS), ("ignored", LIST_IGNORED_ARGS)):
        result = subprocess.run([str(exe), *extra], capture_output=True,
                                text=True, env=env, timeout=120)
        if result.returncode != 0:
            fail(f"test enumeration failed for {exe}: {result.stderr[-200:]}")
        names = []
        for line in result.stdout.splitlines():
            match = re.match(r"^(\S+): (test|benchmark)$", line)
            if match:
                names.append(match.group(1))
        out[key] = names
    return out["tests"], out["ignored"]


def cmd_enumerate(args):
    """Emit per-target libtest name inventories for `manifest --tests`."""
    root = Path(args.workspace).resolve()
    entries = artifact_entries(args.artifacts)
    env = dict(os.environ)
    # Enumeration runs instrumented binaries; route their throwaway profiles
    # to a scratch directory so no stale profraw can enter the merge set.
    with tempfile.TemporaryDirectory() as scratch:
        env["LLVM_PROFILE_FILE"] = str(Path(scratch) / "enum-%p-%m.profraw")
        listed = {}
        for msg in entries:
            if not msg["profile"].get("test"):
                continue
            src = rel(root, msg["target"]["src_path"])
            tests, ignored = enumerate_tests(msg["executable"], env)
            listed[src] = {"tests": tests, "ignored": ignored}
    Path(args.out).write_text(json.dumps(listed, indent=1) + "\n", encoding="utf-8")
    return 0


def slug(target_id):
    stem = re.sub(r"\.rs$", "", target_id)
    cleaned = re.sub(r"[^a-z0-9_-]+", "-", stem.lower()).strip("-")
    if not cleaned or not LABEL_RE.fullmatch(cleaned):
        fail(f"target id {target_id} has no valid action label")
    return cleaned


def label_for(position, target_id):
    return f"t{position:02d}-{slug(target_id)}"


def cmd_manifest(args):
    root = Path(args.workspace).resolve()
    entries = artifact_entries(args.artifacts)
    if not entries:
        fail(f"no compiler artifacts with executables in {args.artifacts}")
    tests = load_json(args.tests, "test inventory")
    cov = root / args.cov_dir
    if list(cov.rglob("*.profraw")):
        fail(f"stale profraw under {args.cov_dir}; archive must be profile-free")

    objects, targets = [], []
    seen_exes = set()
    for msg in entries:
        exe = Path(msg["executable"])
        if str(exe) in seen_exes:
            continue
        seen_exes.add(str(exe))
        exe_rel = rel(root, exe)
        if not exe.is_file():
            fail(f"artifact executable missing: {exe_rel}")
        src_rel = rel(root, msg["target"]["src_path"])
        entry = {"path": exe_rel, "sha256": sha256_file(exe),
                 "kind": "test" if msg["profile"].get("test") else "operational",
                 "target": src_rel}
        objects.append(entry)
        if entry["kind"] == "test":
            listed = tests.get(src_rel)
            if not isinstance(listed, dict) or not isinstance(listed.get("tests"), list):
                fail(f"no libtest listing for target {src_rel}")
            targets.append({"id": src_rel, "exe": exe_rel,
                            "sha256": entry["sha256"], "tests": listed["tests"],
                            "ignored": listed.get("ignored", [])})
    known = {t["id"] for t in targets}
    if set(tests) - known:
        fail(f"listed targets without executables: {sorted(set(tests) - known)}")
    # Every other ELF object under the instrumented build dir is part of the
    # coverage-object set (dependency shared objects, build scripts).
    for path in sorted((cov / "debug").rglob("*")):
        if path.is_file() and not path.is_symlink() and is_elf(path):
            p = rel(root, path)
            if p not in {o["path"] for o in objects}:
                objects.append({"path": p, "sha256": sha256_file(path),
                                "kind": "object", "target": None})
    for extra in args.extra_object:
        path = Path(extra)
        if not path.is_file():
            fail(f"extra shipped object missing: {extra}")
        objects.append({"path": rel(root, path), "sha256": sha256_file(path),
                        "kind": "support", "target": None})
    operational = {o["path"] for o in objects if o["kind"] == "operational"}
    for required in (f"{args.cov_dir}/debug/duskweave",
                     f"{args.cov_dir}/debug/dw-fixture-supervisor"):
        if required not in operational:
            fail(f"required operational binary absent from build: {required}")
    manifest = {
        "schema": 1, "checkout": args.sha,
        "tools": {"rustc": args.rustc, "cargo_llvm_cov": args.llvm_cov},
        "selection": {"locked": True, "features": ["fixture-supervisor"],
                      "all_targets": True, "profile": "test"},
        "cov_dir": args.cov_dir,
        "objects": objects, "targets": targets,
        "prelude": {"target": PRELUDE_TARGET, "test": PRELUDE_TEST},
    }
    manifest["sha256"] = hashlib.sha256(
        json.dumps(manifest, sort_keys=True).encode()).hexdigest()
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(manifest, indent=1) + "\n", encoding="utf-8")
    print(f"manifest: {len(targets)} test targets, {len(objects)} objects")
    return 0


def partition_targets(manifest, workers):
    order = sorted(manifest["targets"],
                   key=lambda t: (-WEIGHTS.get(t["id"], DEFAULT_WEIGHT), t["id"]))
    loads = [0.0] * workers
    assignments = {str(i): [] for i in range(workers)}
    for target in order:
        index = min(range(workers), key=lambda i: (loads[i], i))
        assignments[str(index)].append(target["id"])
        loads[index] += WEIGHTS.get(target["id"], DEFAULT_WEIGHT)
    return assignments, loads


def write_session(path, manifest, target_ids):
    exe_by_id = {t["id"]: t["exe"] for t in manifest["targets"]}
    lines = []
    for pos, target_id in enumerate(target_ids):
        exe = str(Path(manifest["workspace_root"]) / exe_by_id[target_id])
        lines.append(json.dumps({"label": label_for(pos, target_id),
                                 "args": [exe, "--test-threads=1"]}))
    lines.append(json.dumps({"exit": True}))
    Path(path).write_text("\n".join(lines) + "\n", encoding="utf-8")


def cmd_plan(args):
    manifest = load_json(args.manifest, "manifest")
    manifest["workspace_root"] = str(Path(args.workspace).resolve())
    manifest_sha = file_hash(args.manifest)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    prelude_exe = next((t for t in manifest["targets"]
                        if t["id"] == PRELUDE_TARGET), None)
    if prelude_exe is None:
        fail(f"prelude target {PRELUDE_TARGET} absent from inventory")
    prelude_args = [str(Path(manifest["workspace_root"]) / prelude_exe["exe"]),
                    PRELUDE_TEST, "--exact", "--test-threads=1", "--nocapture"]
    (out / "prelude.jsonl").write_text(
        json.dumps({"label": "prelude", "args": prelude_args}) + "\n"
        + json.dumps({"exit": True}) + "\n", encoding="utf-8")
    for workers in args.counts:
        assignments, loads = partition_targets(manifest, workers)
        doc = {"schema": 1, "workers": workers, "manifest_sha256": manifest_sha,
               "assignments": assignments,
               "est_seconds": {str(i): round(v, 1) for i, v in enumerate(loads)}}
        (out / f"partition-{workers}.json").write_text(
            json.dumps(doc, indent=1) + "\n", encoding="utf-8")
        for index, ids in assignments.items():
            write_session(out / f"session-{workers}-{index}.jsonl",
                          manifest, ids)
        print(f"partition workers={workers} est={doc['est_seconds']}")
    return 0


def check_objects(manifest, root):
    problems = []
    for obj in manifest["objects"]:
        path = root / obj["path"]
        if not path.is_file():
            problems.append(f"missing object {obj['path']}")
        elif sha256_file(path) != obj["sha256"]:
            problems.append(f"hash mismatch {obj['path']}")
        if obj["kind"] in ("test", "operational") and not os.access(path, os.X_OK):
            problems.append(f"object not executable {obj['path']}")
    return problems


def cmd_verify(args):
    """Worker/aggregate check: same candidate, tools, objects, profile-free."""
    root = Path(args.workspace).resolve()
    manifest = load_json(args.manifest, "manifest")
    if args.sha and manifest["checkout"] != args.sha:
        fail(f"candidate mismatch: manifest {manifest['checkout']} != {args.sha}")
    problems = check_objects(manifest, root)
    for key, actual in (("rustc", args.rustc), ("cargo_llvm_cov", args.llvm_cov)):
        if actual and manifest["tools"].get(key) != actual:
            problems.append(f"tool {key}: built with "
                            f"{manifest['tools'].get(key)!r}, running {actual!r}")
    stale = list((root / manifest["cov_dir"]).rglob("*.profraw"))
    if stale:
        problems.append(f"{len(stale)} stale profraw under {manifest['cov_dir']}")
    for problem in problems:
        print(f"verify FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"verify OK: {len(manifest['objects'])} objects, "
          f"candidate {manifest['checkout']}")
    return 0


def _record_test(name, status, names, ignored, failed, problems):
    if name in names:
        problems.append(f"duplicate outer test execution: {name}")
    names.add(name)
    if status == "ignored":
        ignored.add(name)
    elif status != "ok":
        failed.add(name)


def parse_libtest_log(path):
    """Parse a libtest transcript, tolerating nested child-harness scopes.

    Outer evidence lives at depth 1 only: the outer `running` banner opens the
    outer harness, spawned test binaries nest additional `running`/`test
    result` scopes inside a pending parent's output, and a parent's verdict
    may land on its own line after all nested scopes close. Child test names
    and summaries are consumed without entering the outer inventory.
    """
    names, ignored, failed, result = set(), set(), set(), None
    problems = []
    depth = 0
    pending = None
    for raw in Path(path).read_text(
            encoding="utf-8", errors="replace").splitlines():
        line = raw.rstrip("\r")
        if HARNESS_DONE.match(line):
            match = RESULT_LINE.match(line)
            if depth == 1:
                if match:
                    result = {"status": match.group(1),
                              "passed": int(match.group(2)),
                              "failed": int(match.group(3)),
                              "ignored": int(match.group(4)),
                              "measured": int(match.group(5)),
                              "filtered": int(match.group(6)),
                              "secs": float(match.group(7))}
                depth = 0
            elif depth > 1:
                depth -= 1
            elif match:
                result = {"status": match.group(1),
                          "passed": int(match.group(2)),
                          "failed": int(match.group(3)),
                          "ignored": int(match.group(4)),
                          "measured": int(match.group(5)),
                          "filtered": int(match.group(6)),
                          "secs": float(match.group(7))}
            continue
        if HARNESS_OPEN.match(line):
            depth += 1
            continue
        if depth != 1:
            continue
        match = TEST_LINE.match(line)
        if match:
            if pending is not None:
                problems.append(f"unresolved outer test verdict: {pending}")
            _record_test(match.group(1), match.group(2),
                         names, ignored, failed, problems)
            pending = None
            continue
        head = TEST_HEAD.match(line)
        if head:
            if pending is not None:
                problems.append(f"unresolved outer test verdict: {pending}")
            pending = head.group(1)
            continue
        verdict = VERDICT_LINE.match(line)
        if verdict and pending is not None:
            _record_test(pending, verdict.group(1),
                         names, ignored, failed, problems)
            pending = None
    if pending is not None:
        problems.append(f"unresolved outer test verdict: {pending}")
    if depth > 0:
        problems.append(f"unclosed harness scope at depth {depth}")
    return names, ignored, failed, result, problems


def validate_result_fields(record):
    return (record.get("status") == "completed" and record.get("exit") == 0
            and record.get("child_exit") == 0 and record.get("capture") == "complete"
            and record.get("cleanup") == "completed" and record.get("omissions") == []
            and record.get("timing_origin") == "request_admission")


def validate_report(manifest, assignment, report_dir):
    """Recompute per-target receipts from raw supervisor evidence."""
    targets = {t["id"]: t for t in manifest["targets"]}
    outcomes = []
    for pos, target_id in enumerate(assignment):
        label = label_for(pos, target_id)
        rpath = Path(report_dir) / f"{label}.result.json"
        lpath = Path(report_dir) / f"{label}.log"
        if not rpath.is_file() or not lpath.is_file():
            fail(f"missing supervisor evidence for {label}")
        record = load_json(rpath, "action result")
        if not validate_result_fields(record):
            fail(f"{label}: supervisor result not a clean success: {record}")
        names, ignored, failed, result, problems = parse_libtest_log(lpath)
        expected = targets[target_id]
        for problem in problems:
            fail(f"{label}: {problem}")
        if result is None:
            fail(f"{label}: no libtest result line")
        if (result["status"] != "ok" or result["failed"] or failed
                or result["filtered"] or result["measured"]):
            fail(f"{label}: unsuccessful or filtered libtest result {result}")
        if names != set(expected["tests"]):
            fail(f"{label}: executed tests != inventory "
                 f"(missing={sorted(set(expected['tests']) - names)}, "
                 f"extra={sorted(names - set(expected['tests']))})")
        if ignored != set(expected["ignored"]):
            fail(f"{label}: ignored set {sorted(ignored)} != "
                 f"expected {sorted(expected['ignored'])}")
        if result["passed"] + result["ignored"] != len(expected["tests"]):
            fail(f"{label}: result count does not cover inventory")
        outcomes.append({"target": target_id, "label": label,
                         "exit": record["exit"],
                         "elapsed_ms": record.get("elapsed_ms"),
                         "passed": result["passed"], "ignored": result["ignored"],
                         "secs": result["secs"]})
    return outcomes


def check_prelude(report_dir):
    rpath = Path(report_dir) / "prelude.result.json"
    lpath = Path(report_dir) / "prelude.log"
    if not rpath.is_file() or not lpath.is_file():
        fail("worker 0 missing reservation-stall prelude evidence")
    record = load_json(rpath, "prelude result")
    if not validate_result_fields(record):
        fail(f"prelude: supervisor result not clean: {record}")
    # Route the prelude log through the real scope-aware parser: the stall
    # probe's children run nested libtest harnesses, so only depth-1 evidence
    # (the parent name and the outer summary) may count.
    names, ignored, failed, result, problems = parse_libtest_log(lpath)
    if problems:
        fail(f"prelude: {problems[0]}")
    if (PRELUDE_TEST not in names or failed or result is None
            or result["status"] != "ok" or result["failed"]):
        fail("prelude: reservation-stall test did not run successfully")


def check_partition(partition, manifest):
    """Union must equal the discovered inventory with empty intersection."""
    all_ids = {t["id"] for t in manifest["targets"]}
    seen = [i for ids in partition["assignments"].values() for i in ids]
    if set(seen) != all_ids:
        fail("partition union != inventory: "
             f"missing={sorted(all_ids - set(seen))}, "
             f"extra={sorted(set(seen) - all_ids)}")
    if len(seen) != len(set(seen)):
        fail("partition assigns a target to more than one worker")


def cmd_receipt(args):
    manifest = load_json(args.manifest, "manifest")
    partition = load_json(args.partition, "partition")
    if partition.get("manifest_sha256") != file_hash(args.manifest):
        fail("partition is not bound to this manifest")
    check_partition(partition, manifest)
    worker = str(args.worker)
    if worker not in partition["assignments"]:
        fail(f"worker {worker} has no assignment")
    if args.prelude:
        check_prelude(args.prelude)
    outcomes = validate_report(manifest, partition["assignments"][worker],
                               args.report)
    receipt = {"schema": 1, "worker": int(worker),
               "checkout": manifest["checkout"],
               "manifest_sha256": file_hash(args.manifest),
               "partition_sha256": file_hash(args.partition),
               "targets": outcomes}
    Path(args.out).write_text(json.dumps(receipt, indent=1) + "\n",
                              encoding="utf-8")
    print(f"receipt worker={worker}: {len(outcomes)} targets verified")
    return 0


def cmd_aggregate(args):
    root = Path(args.workspace).resolve() if args.workspace else Path.cwd()
    manifest = load_json(args.manifest, "manifest")
    partition = load_json(args.partition, "partition")
    if args.sha and manifest["checkout"] != args.sha:
        fail(f"candidate mismatch: manifest {manifest['checkout']} != {args.sha}")
    if partition["workers"] != args.workers or partition["manifest_sha256"] != file_hash(args.manifest):
        fail("partition does not match this manifest/worker count")
    check_partition(partition, manifest)
    if len(args.evidence) != args.workers:
        fail(f"expected {args.workers} evidence dirs, got {len(args.evidence)}")

    seen_workers, total_secs = set(), 0.0
    for ev in args.evidence:
        receipt = load_json(Path(ev) / "receipt.json", "worker receipt")
        worker = receipt.get("worker")
        if worker in seen_workers or str(worker) not in partition["assignments"]:
            fail(f"duplicate or unassigned worker evidence: {ev}")
        seen_workers.add(worker)
        if (receipt.get("manifest_sha256") != file_hash(args.manifest)
                or receipt.get("partition_sha256") != file_hash(args.partition)
                or receipt.get("checkout") != manifest["checkout"]):
            fail(f"worker {worker} receipt does not bind this candidate")
        outcomes = validate_report(manifest, partition["assignments"][str(worker)],
                                   Path(ev) / "suite")
        if outcomes != receipt["targets"]:
            fail(f"worker {worker} receipt does not match raw evidence")
        if worker == 0:
            check_prelude(Path(ev) / "prelude")
        total_secs += sum(o["secs"] for o in outcomes)
    if seen_workers != set(range(args.workers)):
        fail(f"evidence covers workers {sorted(seen_workers)}, "
             f"expected 0..{args.workers - 1}")

    cov = root / manifest["cov_dir"]
    profiles = list(cov.glob("*.profraw"))
    for index in range(args.workers):
        if not any(p.name.startswith(f"w{index}-") for p in profiles):
            fail(f"no merged profraw from worker {index}")
    if any(p.name.startswith("prelude-") for p in profiles):
        fail("prelude profiles must not enter the merged coverage set")
    problems = check_objects(manifest, root)
    if problems:
        for problem in problems:
            print(f"aggregate FAIL: {problem}", file=sys.stderr)
        return 1
    print(json.dumps({"workers": args.workers,
                      "targets": sum(len(v) for v in partition["assignments"].values()),
                      "profiles": len(profiles),
                      "suite_seconds": round(total_secs, 1)}))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("enumerate")
    p.add_argument("--artifacts", required=True)
    p.add_argument("--workspace", required=True)
    p.add_argument("--out", required=True)

    p = sub.add_parser("manifest")
    p.add_argument("--artifacts", required=True)
    p.add_argument("--workspace", required=True)
    p.add_argument("--cov-dir", required=True)
    p.add_argument("--sha", required=True)
    p.add_argument("--rustc", default="")
    p.add_argument("--llvm-cov", default="")
    p.add_argument("--tests", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--extra-object", action="append", default=[],
                   help="additional shipped executable (e.g. the normal "
                        "fixture-supervisor copy), recorded and hash-verified")

    p = sub.add_parser("plan")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace", required=True)
    p.add_argument("--out-dir", required=True)
    p.add_argument("--counts", type=int, nargs="+", default=list(WORKER_COUNTS))

    p = sub.add_parser("verify")
    p.add_argument("--manifest", required=True)
    p.add_argument("--workspace", default=".")
    p.add_argument("--sha", default=None)
    p.add_argument("--rustc", default=None)
    p.add_argument("--llvm-cov", default=None)

    p = sub.add_parser("receipt")
    p.add_argument("--manifest", required=True)
    p.add_argument("--partition", required=True)
    p.add_argument("--worker", type=int, required=True)
    p.add_argument("--report", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--prelude", default=None)

    p = sub.add_parser("aggregate")
    p.add_argument("--manifest", required=True)
    p.add_argument("--partition", required=True)
    p.add_argument("--evidence", nargs="+", required=True)
    p.add_argument("--cov-dir", required=True)
    p.add_argument("--workers", type=int, required=True)
    p.add_argument("--workspace", default=".")
    p.add_argument("--sha", default=None)

    args = parser.parse_args()
    return {"enumerate": cmd_enumerate, "manifest": cmd_manifest,
            "plan": cmd_plan, "verify": cmd_verify,
            "receipt": cmd_receipt, "aggregate": cmd_aggregate}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())

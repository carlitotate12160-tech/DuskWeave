#!/usr/bin/env python3
"""McCabe own-complexity ratchet for tracked DuskWeave Rust sources.

CLI owner of the gate: pinned analyzer qualification, tracked src/**/*.rs
inventory (``git ls-files`` candidate worktree; ``git ls-tree`` at the
validated ``--base`` commit), analyzer invocation
(``-p src -m -O json -o <raw> -j 1 -w``), provenance, reference point
assignment, reviewed pure-dispatch classification, and the base-to-candidate
ratchet verdict delegated to ``complexity_ratchet.py``. Report codec,
validation and projection live in sibling ``complexity_reports.py``.

Gate (QUALITY_BAR.md): threshold 7 per function/closure; the reviewed
pure-dispatch ``src/main.rs::run`` direct unit child has 10. A new function
above threshold fails, a previously compliant function crossing above it
fails, and retained debt must not increase. Spans >50 emit REVIEW_TRIGGER.
Exit 0 = gate pass (debt reported), 1 = McCabe regression,
2 = tool/input/report/base failure.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile

import complexity_ratchet as ratchet
import complexity_reports as reports

CAP_DEFAULT = 7
CAP_REVIEWED_DISPATCH = 10
REVIEW_TRIGGER_SPAN = 50
SUBPROCESS_TIMEOUT_SECONDS = 300.0
TOOL_NAME = "rust-code-analysis-cli"
REQUIRED_VERSION = "rust-code-analysis-cli 0.0.25"
DISPATCH_FILE = "src/main.rs"
DISPATCH_NAME = "run"


def _subprocess_timeout():
    """Bounded wait for analyzer/git children; overridable for tests."""
    raw = os.environ.get("DW_COMPLEXITY_TIMEOUT")
    if raw is None:
        return SUBPROCESS_TIMEOUT_SECONDS
    try:
        value = float(raw)
    except ValueError:
        raise reports.CheckError(
            f"DW_COMPLEXITY_TIMEOUT must be a number of seconds, got {raw!r}"
        ) from None
    if not 0 < value <= SUBPROCESS_TIMEOUT_SECONDS:
        raise reports.CheckError(
            f"DW_COMPLEXITY_TIMEOUT must be within "
            f"(0, {SUBPROCESS_TIMEOUT_SECONDS:g}] seconds, got {value!r}"
        )
    return value


def _run(argv, cwd=None):
    try:
        return subprocess.run(
            argv, capture_output=True, timeout=_subprocess_timeout(), cwd=cwd
        )
    except subprocess.TimeoutExpired as exc:
        raise reports.CheckError(
            f"{argv[0]!r} timed out after {exc.timeout}s"
        ) from exc
    except OSError as exc:
        raise reports.CheckError(f"cannot execute {argv[0]!r}: {exc}") from exc


def _git(*args):
    proc = _run(["git", *args])
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip()
        raise reports.CheckError(
            f"git {' '.join(args)} failed: {detail or proc.returncode}"
        )
    return proc.stdout


def _require_absolute(path, flag):
    if not os.path.isabs(path):
        raise reports.CheckError(f"{flag} must be an absolute path")


def check_analyzer(analyzer):
    _require_absolute(analyzer, "--analyzer")
    if not os.path.isfile(analyzer):
        raise reports.CheckError(f"analyzer binary not found: {analyzer}")
    proc = _run([analyzer, "--version"])
    version = proc.stdout.decode("utf-8", "replace").strip()
    if proc.returncode != 0 or version != REQUIRED_VERSION:
        raise reports.CheckError(
            f"analyzer --version must be exactly {REQUIRED_VERSION!r}, "
            f"got {version!r} (exit {proc.returncode})"
        )
    return version


def inventory():
    out = _git("ls-files", "-z", "--", "src/*.rs", "src/**/*.rs")
    files = sorted(
        {
            p.replace("\\", "/")
            for p in out.decode("utf-8", "replace").split("\0")
            if p.startswith("src/") and p.endswith(".rs")
        }
    )
    if not files:
        raise reports.CheckError("empty tracked src/**/*.rs inventory")
    return files


def candidate_sha():
    proc = _run(["git", "rev-parse", "--verify", "HEAD"])
    if proc.returncode != 0:
        return None
    return proc.stdout.decode("ascii", "replace").strip() or None


def run_analyzer(analyzer, raw_dir, diag_dir, cwd=None, tag="head"):
    if os.path.exists(raw_dir):
        shutil.rmtree(raw_dir)
    os.makedirs(raw_dir)
    proc = _run(
        [analyzer, "-p", "src", "-m", "-O", "json", "-o", raw_dir, "-j", "1", "-w"],
        cwd=cwd,
    )
    for name, data in (
        (f"analyzer-{tag}.stdout.log", proc.stdout),
        (f"analyzer-{tag}.stderr.log", proc.stderr),
    ):
        with open(os.path.join(diag_dir, name), "wb") as fh:
            fh.write(data)
    if proc.returncode != 0:
        tail = proc.stderr.decode("utf-8", "replace").strip().splitlines()
        raise reports.CheckError(
            f"analyzer ({tag}) exited {proc.returncode}: "
            f"{tail[-1] if tail else 'no stderr'}"
        )


def _measure_all(raw_dir, files, src_root=None):
    report_paths = reports.reconcile_reports(raw_dir, files)
    measurements = []
    for rel in files:
        report = reports.load_report(report_paths[rel])
        source = os.path.join(src_root, *rel.split("/")) if src_root else rel
        for m in reports.measure_report(report, rel, source):
            m["cap"] = CAP_DEFAULT
            m["allowance"] = None
            measurements.append(m)
    return measurements


def _eligible_runs(measurements):
    return [
        m
        for m in measurements
        if m["file"] == DISPATCH_FILE
        and m["name"] == DISPATCH_NAME
        and m["depth"] == 1
    ]


def resolve_run_allowance(measurements):
    """Grant cap 10 only to the unique reviewed pure-dispatch run.

    Only a single direct unit-child ``run`` in src/main.rs receives the
    reviewed-dispatch reference point. Its location is verified here; its
    semantic purity (discriminator -> owned handler forwarding, no
    authority, eligibility, mutation, I/O or retry decisions) is re-proven
    in review whenever the function changes. A nested, wrong-file or
    duplicated ``run`` keeps cap 7.
    """
    eligible = _eligible_runs(measurements)
    if not eligible:
        return {
            "status": "absent",
            "detail": "no direct unit-child function run in src/main.rs",
        }
    if len(eligible) > 1:
        return {
            "status": "ambiguous",
            "detail": f"{len(eligible)} unit-child run candidates in "
            f"{DISPATCH_FILE}; none receive cap {CAP_REVIEWED_DISPATCH}",
        }
    node = eligible[0]
    node["cap"] = CAP_REVIEWED_DISPATCH
    node["allowance"] = "reviewed-dispatch"
    return {
        "status": "granted",
        "identity": node["identity"],
        "detail": f"src/main.rs::run cap {CAP_REVIEWED_DISPATCH}; "
        "reviewer re-proves pure routing when the function changes",
    }


def evaluate(measurements):
    over_cap = [m for m in measurements if m["own"] > m["cap"]]
    triggers = [m for m in measurements if m["span"] > REVIEW_TRIGGER_SPAN]
    return over_cap, triggers


def _emit(
    version, sha, base_sha, files, base_files, measurements, base_count,
    triggers, allowance, verdict,
):
    print(
        f"tool={version} candidate={sha or 'unknown'} base={base_sha} "
        f"candidate_source=worktree files={len(files)} "
        f"functions={len(measurements)} base_files={len(base_files)} "
        f"base_functions={base_count}"
    )
    for line in reports.measurement_lines(measurements):
        print(line)
    for m in triggers:
        print(
            f"REVIEW_TRIGGER {m['identity']} span {m['span']} "
            f"> {REVIEW_TRIGGER_SPAN}"
        )
    detail = allowance.get("detail", "")
    print(f"run allowance: {allowance['status']} {detail}".rstrip())
    c = verdict["counts"]
    print(
        f"ratchet new={c['new']} regressed={c['regressed']} "
        f"retained={c['retained']} improved={c['improved']} "
        f"resolved={c['resolved']} removed={c['removed']} "
        f"ambiguous={c['ambiguous']} compliant={c['compliant']}"
    )
    for m in verdict["failures"]:
        print(
            f"FAIL {m['status'].upper()} {m['identity']} own={m['own']} "
            f"cap={m['cap']} base_own={m.get('base_own') or '-'}"
        )
    for g in verdict["ambiguous_groups"]:
        tag = (
            "FAIL" if g["blocked"]
            else "DEBT" if g["disposition"] == "retained"
            else "OK"
        )
        print(
            f"{tag} AMBIGUOUS {g['identity']} members={g['members']} "
            f"head_over={g['head_over']} base_over={g['base_over']} "
            f"disposition={g['disposition']} "
            f"owner={g['owner'] or '-'} "
            f"owner_base_span={g['owner_base_span'] or '-'} "
            f"owner_head_span={g['owner_head_span'] or '-'} "
            f"reason={g['reason']}"
        )
    for m in verdict["debt"]:
        print(
            f"DEBT {m['status'].upper()} {m['identity']} own={m['own']} "
            f"cap={m['cap']} base_own={m.get('base_own') or '-'}"
        )
    for m in verdict["resolved"]:
        print(
            f"RESOLVED {m['identity']} own={m['own']} "
            f"base_own={m.get('base_own') or '-'}"
        )


def main(argv=None):
    parser = argparse.ArgumentParser(description="McCabe complexity ratchet")
    parser.add_argument("--analyzer", required=True)
    parser.add_argument("--report-dir", required=True)
    parser.add_argument("--base", required=True)
    args = parser.parse_args(argv)
    try:
        _require_absolute(args.report_dir, "--report-dir")
        version = check_analyzer(args.analyzer)
        base_sha = ratchet.resolve_base(_git, args.base)
        files = inventory()
        sha = candidate_sha()
        os.makedirs(args.report_dir, exist_ok=True)
        head_raw = os.path.join(args.report_dir, "raw-head")
        run_analyzer(args.analyzer, head_raw, args.report_dir, tag="head")
        head_ms = _measure_all(head_raw, files)
        allowance = resolve_run_allowance(head_ms)
        with tempfile.TemporaryDirectory(prefix="dw-ratchet-base-") as root:
            base_files = ratchet.base_inventory(_git, base_sha)
            ratchet.extract_base(_git, base_sha, base_files, root)
            base_raw = os.path.join(args.report_dir, "raw-base")
            run_analyzer(
                args.analyzer, base_raw, args.report_dir, cwd=root, tag="base"
            )
            base_ms = _measure_all(base_raw, base_files, src_root=root)
            resolve_run_allowance(base_ms)
            verdict = ratchet.classify(
                base_ms, head_ms, base_root=root, head_root=None
            )
        over_cap, triggers = evaluate(head_ms)
        _emit(
            version, sha, base_sha, files, base_files, head_ms,
            len(base_ms), triggers, allowance, verdict,
        )
        summary = reports.build_summary(
            TOOL_NAME, "0.0.25", sha, base_sha, files, base_files, head_ms,
            len(base_ms), over_cap, triggers, allowance, verdict,
        )
        reports.write_summaries(args.report_dir, summary)
        group_failures = sum(
            1 for g in verdict["ambiguous_groups"] if g["blocked"]
        )
        if verdict["failures"] or group_failures:
            print(
                f"Gate FAIL: {len(verdict['failures']) + group_failures} "
                "complexity regressions (exit 1)."
            )
            return 1
        c = verdict["counts"]
        print(
            f"Gate PASS: {c['retained']} retained, {c['improved']} "
            f"improved, {c['resolved']} resolved debt items (exit 0)."
        )
        return 0
    except (reports.CheckError, OSError) as exc:
        print(f"complexity check failed: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())

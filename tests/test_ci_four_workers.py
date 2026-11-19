"""Four-worker CI fanout, pure planner, and artifact-to-aggregate regressions."""
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest

import test_ci_rust_shards as shards
import test_ci_scope as ci_scope
import test_ci_worker_artifacts as worker_art

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "doc-check.yml"
SHARDS_SCRIPT = ROOT / "scripts" / "ci_rust_shards.py"
PLAN_SCRIPT = ROOT / "scripts" / "ci_rust_shard_plan.py"
SELECTOR_SCRIPT = ROOT / "scripts" / "ci_worker_artifacts.py"
SCOPE_SCRIPT = ROOT / "scripts" / "ci_scope.py"

EXTRA_HEAVY_TARGETS = (
    ("tests/m1_inference_lifecycle.rs", ["il_a", "il_b"], []), ("tests/m1_live_dns_sql.rs", ["dns_a"], []),
    ("tests/m1_run_failures.rs", ["rf_a"], []), ("tests/m1_inference_budget_limits.rs", ["ib_a"], []),
    ("tests/m1_genai_integration.rs", ["ga_a"], []), ("tests/m1_run_diagnostics.rs", ["rd_a"], []),
    ("tests/m1_session_cli.rs", ["sc_a"], []),
)
EXPECTED_HEAVY_HINTS = {
    "tests/m1_inference_lifecycle.rs": 231.7, "src/lib.rs": 181.6, "tests/m1_live_dns_sql.rs": 161.8,
    "tests/m1_run_failures.rs": 143.0, "tests/m1_provider_fixture.rs": 97.1,
    "tests/m1_genai_integration.rs": 94.3, "tests/m1_run_diagnostics.rs": 85.6, "tests/m1_session_cli.rs": 64.4,
}


def populated_four_worker_workspace(root):
    """Extend Workspace with real heavy target IDs so all 4 workers get targets."""
    ws = shards.Workspace(root)
    ws.TARGET_TESTS = dict(shards.Workspace.TARGET_TESTS)
    tests = shards.read_json(ws.root / "tests.json")
    lines = (ws.root / "artifacts.jsonl").read_text(encoding="utf-8").splitlines()
    for idx, (tid, names, ignored) in enumerate(EXTRA_HEAVY_TARGETS):
        stem = Path(tid).stem
        exe_rel = f"{shards.COV}/debug/deps/{stem}-extra{idx}"
        exe = ws.root / exe_rel
        exe.write_bytes(b"\x7fELF-object-" + exe_rel.encode())
        os.chmod(exe, 0o755)
        ws.TARGET_TESTS[tid] = (names, ignored)
        tests[tid] = {"tests": names, "ignored": ignored}
        lines.append(shards.artifact_line(ws.root, tid, exe_rel, True, ["test"]))
    (ws.root / "tests.json").write_text(json.dumps(tests), encoding="utf-8")
    (ws.root / "artifacts.jsonl").write_text("\n".join(lines), encoding="utf-8")
    return ws


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, str(path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def dispatch_inputs(workflow_text):
    match = re.search(r"^on:\r?\n(.*?)^permissions:", workflow_text, re.M | re.S)
    assert match, "workflow 'on:' block not found"
    block = match.group(1)
    default_m = re.search(r'^\s+default:\s*"([^"]+)"', block, re.M)
    options_m = re.search(r'^\s+options:\s*\[([^\]]+)\]', block, re.M)
    assert default_m and options_m, "rust_workers dispatch metadata missing"
    opts = [item.strip().strip('"').strip("'") for item in options_m.group(1).split(",")]
    return default_m.group(1), opts


class WorkflowFanoutAndClosedCountsTest(unittest.TestCase):
    BASH = shutil.which("bash")

    @classmethod
    def setUpClass(cls):
        cls.workflow_text = WORKFLOW.read_text(encoding="utf-8")
        cls.jobs = ci_scope.job_blocks(WORKFLOW)
        cls.fanout_body = ci_scope.step_body(cls.jobs["changes"], "Rust worker fan-out")

    def _run_fanout(self, requested=None):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            out = root / "ghout"
            out.write_text("", encoding="utf-8")
            changes_block = self.jobs["changes"]
            step_chunk = changes_block[changes_block.index("- name: Rust worker fan-out"):]
            m = re.search(r"REQUESTED:\s*\$\{\{\s*github\.event\.inputs\.rust_workers\s*\|\|\s*'([^']+)'\s*\}\}", step_chunk)
            default_expr = m.group(1) if m else ""
            val = default_expr if requested is None else requested
            script = root / "fanout.sh"
            script.write_bytes(self.fanout_body.encode("utf-8"))
            env = dict(os.environ, REQUESTED=val, GITHUB_OUTPUT=shards._posix(out))
            res = subprocess.run([self.BASH, script.as_posix()], cwd=root, env=env, capture_output=True, text=True)
            parsed = dict(line.split("=", 1) for line in out.read_text(encoding="utf-8").splitlines() if "=" in line)
            return res, parsed

    def test_workflow_fanout_defaults_to_four_and_handles_closed_counts(self):
        for requested, expected in (
            ("4", {"matrix": "[0,1,2,3]", "count": "4"}),
            ("2", {"matrix": "[0,1]", "count": "2"}),
            ("1", {"matrix": "[0]", "count": "1"}),
            (None, {"matrix": "[0,1,2,3]", "count": "4"}),
        ):
            with self.subTest(requested=requested):
                res, parsed = self._run_fanout(requested)
                self.assertEqual(res.returncode, 0, res.stderr)
                self.assertEqual(parsed, expected)
        default_val, opts = dispatch_inputs(self.workflow_text)
        self.assertEqual(default_val, "4")
        self.assertEqual(opts, ["4", "2", "1"])

    def test_workflow_fanout_rejects_unsupported_counts(self):
        for bad in ("0", "3", "5", "abc", ""):
            with self.subTest(bad=bad):
                res, parsed = self._run_fanout(bad)
                self.assertNotEqual(res.returncode, 0)
                self.assertEqual(parsed, {})

    def test_cross_consumer_closed_count_agreement_and_refusals(self):
        self.assertTrue(PLAN_SCRIPT.is_file(), f"missing {PLAN_SCRIPT}")
        shards_mod = load_module("ci_rust_shards_check", SHARDS_SCRIPT)
        sel_mod = load_module("ci_worker_artifacts_check", SELECTOR_SCRIPT)
        _, opts = dispatch_inputs(self.workflow_text)
        self.assertEqual(shards_mod.WORKER_COUNTS, (1, 2, 4))
        self.assertEqual(sel_mod.WORKER_COUNTS, (1, 2, 4))
        self.assertEqual(set(opts), {"1", "2", "4"})

        with tempfile.TemporaryDirectory() as tmp:
            ws = shards.Workspace(tmp)
            m = ws.manifest()
            out_dir = ws.root / "ci-shards"
            r = ws.run("plan", "--manifest", str(m), "--workspace", str(ws.root), "--out-dir", str(out_dir))
            self.assertEqual(r.returncode, 0, r.stderr)
            for c in (1, 2, 4):
                self.assertTrue((out_dir / f"partition-{c}.json").is_file())
                for w in range(c):
                    self.assertTrue((out_dir / f"session-{c}-{w}.jsonl").is_file())
            inv = ws.root / "inv.json"
            inv.write_text(json.dumps([worker_art.page(worker_art.artifact(worker_art.worker_name(0, 1), 1))]))
            for bad in ("0", "3", "5"):
                self.assertNotEqual(ws.run("plan", "--manifest", str(m), "--workspace", str(ws.root),
                                           "--out-dir", str(out_dir), "--counts", bad).returncode, 0)
                self.assertNotEqual(ws.run("aggregate", "--manifest", str(m), "--partition", str(out_dir / "partition-4.json"),
                                           "--evidence", str(ws.root), "--cov-dir", shards.COV, "--workers", bad).returncode, 0)
                r_sel = subprocess.run([sys.executable, str(SELECTOR_SCRIPT), "select", "--inventory", str(inv),
                                        "--sha", worker_art.SHA, "--run-id", str(worker_art.RUN), "--attempt", "1",
                                        "--workers", bad], capture_output=True, text=True)
                self.assertNotEqual(r_sel.returncode, 0)

    def test_workflow_downloads_and_preserved_step_wiring(self):
        rust = self.jobs["rust"]
        pin = "9000827ccba6bdab643e8b6fd33ac0654aef8333"
        guards = {
            0: None,
            1: "needs.changes.outputs.rust_workers == '2' || needs.changes.outputs.rust_workers == '4'",
            2: "needs.changes.outputs.rust_workers == '4'",
            3: "needs.changes.outputs.rust_workers == '4'",
        }
        for w in range(4):
            sec = re.search(rf"- name: Download worker {w} evidence and raw profiles\r?\n(.*?)- name:", rust, re.S)
            self.assertIsNotNone(sec, f"missing download step for worker {w}")
            block = sec.group(1)
            self.assertIn(pin, block)
            self.assertIn(f"artifact-ids: ${{{{ steps.select.outputs.worker_{w}_id }}}}", block)
            self.assertIn(f"/dw-evidence/dw-rust-worker-{w}", block)
            if guards[w] is None:
                self.assertNotIn("if:", block)
            else:
                self.assertIn(guards[w], block)

        tests_job = self.jobs["rust-tests"]
        self.assertIn("fail-fast: false", tests_job)
        for step_name in (
            "Reservation-stall qualification prelude (designated worker)",
            "Candidate real-PG qualification witness (designated worker)",
            "HTTPS-loop PostgreSQL end-to-end matrix (designated worker)",
        ):
            sec = re.search(rf"- name: {re.escape(step_name)}\r?\n(.*?)- name:", tests_job, re.S).group(1)
            self.assertIn("if: matrix.worker == 0", sec)
        self.assertIn('"${{ needs.rust-tests.result }}" != "success"', rust)
        self.assertIn("tests/check_coverage_gate.py", rust)


class PurePlannerAndIsolationTest(unittest.TestCase):
    def test_planner_constants_and_eight_refreshed_heavy_hints(self):
        plan_mod = load_module("ci_rust_shard_plan_test", PLAN_SCRIPT)
        self.assertEqual(plan_mod.DEFAULT_WEIGHT, 30.0)
        self.assertEqual(plan_mod.WORKER0_DESIGNATED_SECONDS, 380.0)
        for target_id, seconds in EXPECTED_HEAVY_HINTS.items():
            self.assertEqual(plan_mod.WEIGHTS.get(target_id), seconds)
        self.assertEqual(plan_mod.WEIGHTS.get("src/main.rs"), 20.6)
        self.assertEqual(plan_mod.WEIGHTS.get("tests/m1_inference_budget_limits.rs"), 106.1)

    def test_shards_script_works_standalone_without_plan_module_for_non_plan_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            copied = root / "ci_rust_shards.py"
            shutil.copy2(SHARDS_SCRIPT, copied)
            sys.modules.pop("ci_rust_shard_plan", None)
            mod = load_module("standalone_shards", copied)
            self.assertNotIn("ci_rust_shard_plan", sys.modules)
            self.assertFalse(hasattr(mod, "WEIGHTS"))
            self.assertFalse(hasattr(mod, "partition_targets"))
            log = root / "sample.log"
            log.write_text("running 1 test\ntest alpha ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n")
            names, ignored, failed, result, problems = mod.parse_libtest_log(log)
            self.assertEqual(names, {"alpha"})
            self.assertEqual((ignored, failed, problems), (set(), set(), []))
            self.assertEqual(result["passed"], 1)

    def test_partitions_1_2_4_are_deterministic_disjoint_complete_and_include_unknown(self):
        with tempfile.TemporaryDirectory() as tmp:
            root1 = Path(tmp) / "ws1"
            root2 = Path(tmp) / "ws2"
            root1.mkdir()
            root2.mkdir()
            ws1 = populated_four_worker_workspace(root1)
            ws2 = populated_four_worker_workspace(root2)

            unknown_exe = ws1.root / f"{shards.COV}/debug/deps/new_unknown-0001"
            unknown_exe.write_bytes(b"\x7fELF-new")
            os.chmod(unknown_exe, 0o755)
            tests = shards.read_json(ws1.root / "tests.json")
            tests["tests/new_unknown_target.rs"] = {"tests": ["u1"], "ignored": []}
            (ws1.root / "tests.json").write_text(json.dumps(tests))
            lines = (ws1.root / "artifacts.jsonl").read_text().splitlines()
            lines.append(shards.artifact_line(ws1.root, "tests/new_unknown_target.rs",
                                             f"{shards.COV}/debug/deps/new_unknown-0001", True, ["test"]))
            (ws1.root / "artifacts.jsonl").write_text("\n".join(lines))

            ws1.plan(counts=("1", "2", "4"))
            ws2.plan(counts=("1", "2", "4"))
            manifest1 = shards.read_json(ws1.root / "ci-shards" / "manifest.json")
            expected_ids = set(ws1.TARGET_TESTS) | {"tests/new_unknown_target.rs"}
            expected_tests = {n for t in manifest1["targets"] for n in t["tests"]}
            by_id = {t["id"]: t for t in manifest1["targets"]}
            for count in (1, 2, 4):
                part = shards.read_json(ws1.root / "ci-shards" / f"partition-{count}.json")
                self.assertEqual(part["workers"], count)
                self.assertEqual(set(part["assignments"]), {str(i) for i in range(count)})
                assigned = [tid for ids in part["assignments"].values() for tid in ids]
                self.assertEqual(set(assigned), expected_ids)
                self.assertEqual(len(assigned), len(expected_ids))
                self.assertEqual({n for tid in assigned for n in by_id[tid]["tests"]}, expected_tests)
                for w in range(count):
                    actions = shards.session_actions(ws1.root / "ci-shards" / f"session-{count}-{w}.jsonl")
                    self.assertEqual(actions[-1], {"exit": True})
                    for act in actions[:-1]:
                        self.assertEqual(act["args"][1:], ["--test-threads=1"])
                if count == 4:
                    for w in range(4):
                        self.assertTrue(part["assignments"][str(w)], f"worker {w} unexpectedly empty")
            prelude = shards.session_actions(ws1.root / "ci-shards" / "prelude.jsonl")
            self.assertEqual(len(prelude), 2)
            root2b = Path(tmp) / "ws2b"
            root2b.mkdir()
            ws2b = populated_four_worker_workspace(root2b)
            ws2b.plan(counts=("1", "2", "4"))
            for count in (1, 2, 4):
                self.assertEqual(shards.read_json(ws2.root / f"ci-shards/partition-{count}.json"),
                                 shards.read_json(ws2b.root / f"ci-shards/partition-{count}.json"))

    def test_380s_allowance_shifts_load_only_and_tiny_empty_worker0_requires_evidence(self):
        plan_mod = load_module("ci_rust_shard_plan_test", PLAN_SCRIPT)
        synthetic = {"targets": [{"id": f"tests/{x}.rs"} for x in ("a", "b", "c", "d")]}
        a4, l4 = plan_mod.partition_targets(synthetic, 4)
        self.assertEqual((l4[0], a4), (380.0, {"0": [], "1": ["tests/a.rs", "tests/d.rs"], "2": ["tests/b.rs"], "3": ["tests/c.rs"]}))
        self.assertEqual(plan_mod.partition_targets(synthetic, 1)[1], [500.0])

        with tempfile.TemporaryDirectory() as tmp:
            ws = shards.Workspace(tmp)
            ws.plan(counts=("1", "2", "4"))
            p4 = shards.read_json(ws.root / "ci-shards" / "partition-4.json")
            self.assertEqual((p4["assignments"]["0"], p4["est_seconds"]["0"]), ([], 380.0))
            self.assertEqual(shards.read_json(ws.root / "ci-shards" / "partition-1.json")["est_seconds"]["0"], 743.5)
            self.assertEqual(shards.session_actions(ws.root / "ci-shards" / "session-4-0.jsonl"), [{"exit": True}])
            ws.evidence(0, [], prelude=False)
            self.assertNotEqual(ws.run("receipt", "--manifest", str(ws.root / "ci-shards/manifest.json"),
                                       "--partition", str(ws.root / "ci-shards/partition-4.json"), "--worker", "0",
                                       "--report", str(ws.root / "evidence-w0/suite"),
                                       "--prelude", str(ws.root / "evidence-w0/prelude"),
                                       "--out", str(ws.root / "evidence-w0/receipt.json")).returncode, 0)
            for w, ids in p4["assignments"].items():
                ev = ws.evidence(int(w), ids)
                extra = ["--prelude", str(ev / "prelude")] if int(w) == 0 else []
                self.assertEqual(ws.run("receipt", "--manifest", str(ws.root / "ci-shards/manifest.json"),
                                        "--partition", str(ws.root / "ci-shards/partition-4.json"),
                                        "--worker", str(w), "--report", str(ev / "suite"), *extra,
                                        "--out", str(ev / "receipt.json")).returncode, 0)
            cov = ws.root / shards.COV
            for w in range(4):
                (cov / f"w{w}-1-2.profraw").write_bytes(b"p")
            evs = [str(ws.root / f"evidence-w{w}") for w in range(4)]
            agg = ("aggregate", "--manifest", str(ws.root / "ci-shards/manifest.json"), "--partition",
                   str(ws.root / "ci-shards/partition-4.json"), "--evidence", *evs, "--cov-dir", shards.COV, "--workers", "4")
            self.assertEqual(ws.run(*agg).returncode, 0)
            for missing in (cov / "w0-1-2.profraw", ws.root / "evidence-w0" / "receipt.json"):
                saved = missing.read_bytes()
                missing.unlink()
                self.assertNotEqual(ws.run(*agg).returncode, 0)
                missing.write_bytes(saved)
            shutil.rmtree(ws.root / "evidence-w0" / "prelude")
            self.assertNotEqual(ws.run(*agg).returncode, 0)


class FourWorkerEndToEndAndNegativeTest(unittest.TestCase):
    BASH = shutil.which("bash")

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.ws = populated_four_worker_workspace(self.root / "ws")
        self.ws.plan(counts=("1", "2", "4"))
        (self.ws.root / "scripts").mkdir(exist_ok=True)
        shutil.copy2(SHARDS_SCRIPT, self.ws.root / "scripts" / "ci_rust_shards.py")
        self.part4 = self.ws.root / "ci-shards" / "partition-4.json"
        self.manifest = self.ws.root / "ci-shards" / "manifest.json"
        self.assign4 = {int(k): v for k, v in shards.read_json(self.part4)["assignments"].items()}
        for w, ids in self.assign4.items():
            ev = self.ws.evidence(w, ids)
            extra = ["--prelude", str(ev / "prelude")] if w == 0 else []
            r = self.ws.run("receipt", "--manifest", str(self.manifest), "--partition", str(self.part4),
                            "--worker", str(w), "--report", str(ev / "suite"), *extra, "--out", str(ev / "receipt.json"))
            self.assertEqual(r.returncode, 0, r.stderr)
        self.ev = self.root / "dw-evidence"
        self.sha = "a" * 40

    def _content(self, name, worker, *, bad_receipt=False, sha_override=None, part_override=None, profraw=True):
        dst = self.root / name
        if dst.exists():
            shutil.rmtree(dst)
        shutil.copytree(self.ws.root / f"evidence-w{worker}", dst)
        if bad_receipt or sha_override or part_override:
            rc = shards.read_json(dst / "receipt.json")
            if bad_receipt:
                rc["manifest_sha256"] = "0" * 64
            if sha_override:
                rc["checkout"] = sha_override
            if part_override:
                rc["partition_sha256"] = part_override
            (dst / "receipt.json").write_text(json.dumps(rc))
        if profraw:
            (dst / "profraw").mkdir(exist_ok=True)
            (dst / "profraw" / f"w{worker}-1-2.profraw").write_bytes(b"p")
        return dst

    def _shim(self, with_gh=False):
        shim = self.root / "shim"
        shim.mkdir(exist_ok=True)
        if os.name == "nt":
            py3 = shim / "python3.exe"
            if not py3.exists():
                shutil.copy(sys.executable, py3)
        else:
            py3 = shim / "python3"
            py3.write_bytes(f"#!/usr/bin/env sh\nexec {shlex.quote(sys.executable)} \"$@\"\n".encode("utf-8"))
            os.chmod(py3, 0o755)
        if with_gh:
            gh = shim / "gh"
            gh.write_bytes(b'#!/usr/bin/env bash\ncat "$GH_INVENTORY"\n')
            os.chmod(gh, 0o755)
        return shim

    def _select_via_workflow(self, payload, attempt=3, workers=4):
        rust = ci_scope.job_blocks(WORKFLOW)["rust"]
        body = (ci_scope.step_body(rust, "List this run's artifact inventory")
                + ci_scope.step_body(rust, "Select worker evidence artifact IDs"))
        for k, v in {
            "${{ github.repository }}": "o/r", "${{ github.run_id }}": str(worker_art.RUN),
            "${{ github.run_attempt }}": str(attempt), "${{ github.sha }}": self.sha,
            "${{ needs.changes.outputs.rust_workers }}": str(workers), "${{ secrets.GITHUB_TOKEN }}": "tok",
        }.items():
            body = body.replace(k, v)
        shim = self._shim(with_gh=True)
        inv = self.root / "inv.json"
        inv.write_text(json.dumps(payload), encoding="utf-8")
        out = self.root / "ghout"
        out.write_text("", encoding="utf-8")
        script = self.root / "sel.sh"
        script.write_bytes(body.encode("utf-8"))
        env = dict(os.environ, PATH=str(shim) + os.pathsep + os.environ["PATH"],
                   RUNNER_TEMP=shards._posix(self.root), GITHUB_OUTPUT=shards._posix(out),
                   GH_INVENTORY=shards._posix(inv))
        r = subprocess.run([self.BASH, script.as_posix()], cwd=ROOT, env=env, capture_output=True, text=True)
        parsed = dict(ln.split("=", 1) for ln in out.read_text(encoding="utf-8").splitlines() if "=" in ln)
        return r, parsed

    def _stage(self, workers=4):
        body = (shards._staging_body()
                .replace("${{ needs.changes.outputs.rust_workers }}", str(workers))
                .replace("$(git rev-parse HEAD)", self.sha))
        script = self.root / "stage.sh"
        script.write_bytes(body.encode("utf-8"))
        shim = self._shim()
        env = dict(os.environ, PATH=str(shim) + os.pathsep + os.environ["PATH"],
                   RUNNER_TEMP=shards._posix(self.root), GITHUB_WORKSPACE=shards._posix(self.ws.root))
        return subprocess.run([self.BASH, script.as_posix()], cwd=self.ws.root, env=env, capture_output=True, text=True)

    def _materialize(self, content, selected, count=4):
        if self.ev.exists():
            shutil.rmtree(self.ev)
        for w in range(count):
            shutil.copytree(content[int(selected[f"worker_{w}_id"])], self.ev / f"dw-rust-worker-{w}")

    def test_four_worker_happy_and_mixed_attempt_partial_rerun_end_to_end(self):
        content = {100 + w: self._content(f"ok-{w}", w) for w in range(4)}
        inv_same = [worker_art.page(*(worker_art.artifact(worker_art.worker_name(w, 1, sha=self.sha), 100 + w) for w in range(4)))]
        r, sel = self._select_via_workflow(inv_same, attempt=1, workers=4)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(sel, {f"worker_{w}_id": str(100 + w) for w in range(4)})
        self._materialize(content, sel)
        r_stage = self._stage(4)
        self.assertEqual(r_stage.returncode, 0, r_stage.stderr)
        summary = json.loads(r_stage.stdout.strip().splitlines()[-1])
        self.assertEqual(summary["workers"], 4)
        self.assertEqual(summary["targets"], len(self.ws.TARGET_TESTS))

        # Clean profraw staged into cov_dir, then verify mixed-attempt partial rerun
        for f in (self.ws.root / shards.COV).glob("*.profraw"):
            f.unlink()
        inv_mixed = [
            worker_art.page(
                worker_art.artifact(worker_art.worker_name(0, 2, sha=self.sha), 901),
                worker_art.artifact(worker_art.worker_name(1, 2, sha=self.sha), 101),
                worker_art.artifact(worker_art.worker_name(2, 1, sha=self.sha), 902),
                worker_art.artifact(worker_art.worker_name(3, 1, sha=self.sha), 103),
            ),
            worker_art.page(
                worker_art.artifact(worker_art.worker_name(0, 3, sha=self.sha), 100),
                worker_art.artifact(worker_art.worker_name(2, 3, sha=self.sha), 102),
            ),
        ]
        r2, sel2 = self._select_via_workflow(inv_mixed, attempt=3, workers=4)
        self.assertEqual(r2.returncode, 0, r2.stderr)
        self.assertEqual(sel2, {f"worker_{w}_id": str(100 + w) for w in range(4)})
        self._materialize(content, sel2)
        self.assertEqual(self._stage(4).returncode, 0)

    def test_four_worker_negative_controls(self):
        # 1. Selector failures: missing w3, duplicate w2, unassigned w4, wrong-run, expired newest (no fallback)
        base_records = [worker_art.artifact(worker_art.worker_name(w, 2, sha=self.sha), 200 + w) for w in range(4)]
        bad_inventories = [
            [worker_art.page(*base_records[:3])],
            [worker_art.page(*base_records, worker_art.artifact(worker_art.worker_name(2, 2, sha=self.sha), 296))],
            [worker_art.page(*base_records, worker_art.artifact(worker_art.worker_name(4, 2, sha=self.sha), 204))],
            [worker_art.page(*base_records[:3], worker_art.artifact(worker_art.worker_name(3, 2, sha=self.sha, run=worker_art.RUN + 1), 299))],
            [worker_art.page(*base_records, worker_art.artifact(worker_art.worker_name(2, 3, sha=self.sha), 298, expired=True))],
        ]
        for inv in bad_inventories:
            with self.subTest(inv=inv):
                r, out = self._select_via_workflow(inv, attempt=3, workers=4)
                self.assertNotEqual(r.returncode, 0)
                self.assertEqual(out, {})
        # 2. Newer selected invalid receipt does NOT fall back to older good artifact
        content = {200 + w: self._content(f"base-{w}", w) for w in range(4)}
        content[297] = self._content("bad-w2", 2, bad_receipt=True)
        inv_nofb = [worker_art.page(*base_records, worker_art.artifact(worker_art.worker_name(2, 3, sha=self.sha), 297))]
        r, sel = self._select_via_workflow(inv_nofb, attempt=3, workers=4)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(sel["worker_2_id"], "297")
        self._materialize(content, sel)
        self.assertNotEqual(self._stage(4).returncode, 0)
        # 3. Wrong checkout, wrong partition, duplicate worker receipt, missing w3 profiles, dir count, prelude leak
        dup_rc = self._content("dup-rc", 0)
        (dup_rc / "profraw" / "w3-1-2.profraw").write_bytes(b"p")
        bad_w3 = {
            "wrong_sha": self._content("bad-sha", 3, sha_override="b" * 40),
            "wrong_part": self._content("bad-part", 3, part_override="f" * 64),
            "dup_worker": dup_rc,
            "no_profraw": self._content("no-prof", 3, profraw=False),
        }
        for label, bad_dir in bad_w3.items():
            with self.subTest(case=label):
                c_map = {**{200 + w: content[200 + w] for w in range(3)}, 203: bad_dir}
                self._materialize(c_map, {f"worker_{w}_id": str(200 + w) for w in range(4)})
                self.assertNotEqual(self._stage(4).returncode, 0)
        self._materialize({200 + w: content[200 + w] for w in range(4)}, {f"worker_{w}_id": str(200 + w) for w in range(4)})
        self.assertNotEqual(self._stage(2).returncode, 0)
        shutil.rmtree(self.ev / "dw-rust-worker-3")
        self.assertNotEqual(self._stage(4).returncode, 0)
        (self.ws.root / shards.COV / "prelude-0-1.profraw").write_bytes(b"p")
        self._materialize({200 + w: content[200 + w] for w in range(4)}, {f"worker_{w}_id": str(200 + w) for w in range(4)})
        self.assertNotEqual(self._stage(4).returncode, 0)

    def test_scope_classifier_selects_full_for_changed_ci_paths(self):
        scope_mod = load_module("ci_scope_test", SCOPE_SCRIPT)
        for path in (".github/workflows/doc-check.yml", "scripts/ci_rust_shard_plan.py",
                     "scripts/ci_rust_shards.py", "scripts/ci_worker_artifacts.py", "unknown/path.md"):
            self.assertFalse(scope_mod.document(path), f"{path} must require full CI")


if __name__ == "__main__":
    unittest.main()

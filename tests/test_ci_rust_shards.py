"""Synthetic shard-plan/evidence fixtures; no services, network or credentials."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/ci_rust_shards.py"
COV = "target/llvm-cov-target"

ARTIFACTS = [
    # (src_path, exe rel path, profile.test, kind)
    ("src/lib.rs", f"{COV}/debug/deps/duskweave-aaa1", True, ["lib"]),
    ("src/main.rs", f"{COV}/debug/deps/duskweave-bbb2", True, ["bin"]),
    ("src/bin/dw-fixture-supervisor/main.rs",
     f"{COV}/debug/deps/dw_fixture_supervisor-ccc3", True, ["bin"]),
    ("tests/alpha.rs", f"{COV}/debug/deps/alpha-ddd4", True, ["test"]),
    ("tests/beta.rs", f"{COV}/debug/deps/beta-eee5", True, ["test"]),
    ("tests/m1_provider_fixture.rs",
     f"{COV}/debug/deps/m1_provider_fixture-fff6", True, ["test"]),
    ("src/bin/dw-fixture-supervisor/main.rs",
     f"{COV}/debug/dw-fixture-supervisor", False, ["bin"]),
    ("src/main.rs", f"{COV}/debug/duskweave", False, ["bin"]),
]


def artifact_line(root, src_path, exe_rel, is_test, kind):
    return json.dumps({
        "reason": "compiler-artifact",
        "profile": {"test": is_test},
        "target": {"name": Path(src_path).stem, "kind": kind,
                   "src_path": str(Path(root) / src_path)},
        "executable": str(Path(root) / exe_rel),
    })


def result_json(exit_code=0, status="completed", child_exit=0, capture="complete",
                cleanup="completed", omissions=(), elapsed_ms=1):
    return {"status": status, "exit": exit_code, "child_exit": child_exit,
            "capture": capture, "cleanup": cleanup, "omissions": list(omissions),
            "timing_origin": "request_admission", "elapsed_ms": elapsed_ms}


def test_log(names, ignored=(), failures=(), extra_lines=(), result=True,
             filtered=0, status="ok"):
    lines = [f"running {len(names)} tests"]
    for name in names:
        if name in failures:
            lines.append(f"test {name} ... FAILED")
        elif name in ignored:
            lines.append(f"test {name} ... ignored, synthetic reason")
        else:
            lines.append(f"test {name} ... ok")
    lines += list(extra_lines)
    if result:
        passed = len(names) - len(failures) - len(ignored)
        failed = len(failures)
        lines.append(f"test result: {status}. {passed} passed; {failed} failed; "
                     f"{len(ignored)} ignored; 0 measured; {filtered} filtered out; "
                     "finished in 1.23s")
    return "\n".join(lines) + "\n"


class Workspace:
    """One synthetic workspace: cov dir objects, artifacts jsonl, tests.json."""

    TARGET_TESTS = {
        "src/lib.rs": (["lib_a", "lib_b"], ["lib_b"]),
        "src/main.rs": (["main_a"], []),
        "src/bin/dw-fixture-supervisor/main.rs": (["sup_a"], []),
        "tests/alpha.rs": (["a1", "a2"], []),
        "tests/beta.rs": (["b1"], []),
        "tests/m1_provider_fixture.rs": (["stall", "other"], []),
    }

    def __init__(self, root):
        self.root = Path(root)
        (self.root / COV / "debug" / "deps").mkdir(parents=True)
        (self.root / COV / "debug").mkdir(parents=True, exist_ok=True)
        (self.root / "ci-shards").mkdir(exist_ok=True)
        for _src, exe, _t, _k in ARTIFACTS:
            p = self.root / exe
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(b"\x7fELF-object-" + exe.encode())
            os.chmod(p, 0o755)
        tests = {k: {"tests": v[0], "ignored": v[1]}
                 for k, v in self.TARGET_TESTS.items()}
        (self.root / "tests.json").write_text(json.dumps(tests), encoding="utf-8")
        lines = [artifact_line(self.root, *row) for row in ARTIFACTS]
        lines.insert(0, '{"reason":"build-finished","success":true}')
        lines.insert(0, "not-json-noise")
        (self.root / "artifacts.jsonl").write_text("\n".join(lines), encoding="utf-8")

    def run(self, *args, env=None, cwd=None):
        return subprocess.run([sys.executable, str(SCRIPT), *args],
                              cwd=cwd or self.root, env=env or dict(os.environ),
                              capture_output=True, text=True)

    def manifest(self, *extra):
        out = self.root / "ci-shards" / "manifest.json"
        r = self.run("manifest", "--artifacts", "artifacts.jsonl",
                     "--workspace", str(self.root), "--cov-dir", COV,
                     "--sha", "a" * 40, "--rustc", "rustc 1.94.1 (x)",
                     "--llvm-cov", "cargo-llvm-cov 0.9.1",
                     "--tests", "tests.json", "--out", str(out))
        assert r.returncode == 0, r.stderr + r.stdout
        return out

    def plan(self, manifest=None, counts=("1", "2")):
        manifest = manifest or self.manifest()
        out = self.root / "ci-shards"
        r = self.run("plan", "--manifest", str(manifest), "--workspace",
                     str(self.root), "--out-dir", str(out), "--counts", *counts)
        assert r.returncode == 0, r.stderr + r.stdout
        return out

    def evidence(self, worker, assignment, report="suite", prelude=True):
        """Worker evidence dir with per-action result+log artifacts."""
        ev = self.root / f"evidence-w{worker}"
        suite = ev / report
        suite.mkdir(parents=True, exist_ok=True)
        for pos, tid in enumerate(assignment):
            label = action_label(pos, tid)
            (suite / f"{label}.result.json").write_text(json.dumps(result_json()))
            names, ignored = self.TARGET_TESTS[tid]
            (suite / f"{label}.log").write_text(test_log(names, ignored))
        if prelude and worker == 0:
            pre = ev / "prelude"
            pre.mkdir(exist_ok=True)
            (pre / "prelude.result.json").write_text(json.dumps(result_json()))
            test = ("stalled_reservation_reply_hard_stops_at_the_"
                    "original_decision_envelope")
            (pre / "prelude.log").write_text(
                f"running 1 test\ntest {test} ... child output interleaved\n"
                "more child output\nok\r\n\n"
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                "5 filtered out; finished in 3.01s\n")
        return ev


def action_label(pos, target_id):
    slug = target_id.replace("/", "-").rsplit(".", 1)[0]
    slug = "".join(c if c.isalnum() or c in "_-" else "-" for c in slug.lower())
    return f"t{pos:02d}-{slug}"


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def session_actions(path):
    return [json.loads(ln) for ln in Path(path).read_text(encoding="utf-8").splitlines()]


class ManifestTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.ws = Workspace(self.tmp.name)

    def test_manifest_records_objects_targets_and_config(self):
        m = read_json(self.ws.manifest())
        self.assertEqual(m["checkout"], "a" * 40)
        self.assertEqual(m["selection"]["features"], ["fixture-supervisor"])
        ids = {t["id"] for t in m["targets"]}
        self.assertEqual(ids, set(self.ws.TARGET_TESTS))
        for t in m["targets"]:
            names, ignored = self.ws.TARGET_TESTS[t["id"]]
            self.assertEqual(t["tests"], names)
            self.assertEqual(t["ignored"], ignored)
            self.assertTrue(t["exe"].startswith(COV + "/"))
            self.assertEqual(len(t["sha256"]), 64)
        operational = [o["path"] for o in m["objects"] if o["kind"] != "test"]
        self.assertIn(f"{COV}/debug/duskweave", operational)
        self.assertIn(f"{COV}/debug/dw-fixture-supervisor", operational)
        self.assertIn("sha256", m)

    def test_manifest_refuses_missing_executable_and_stale_profraw(self):
        (self.ws.root / f"{COV}/debug/deps/alpha-ddd4").unlink()
        out = self.ws.root / "m.json"
        r = self.ws.run("manifest", "--artifacts", "artifacts.jsonl",
                        "--workspace", str(self.ws.root), "--cov-dir", COV,
                        "--sha", "a" * 40, "--tests", "tests.json", "--out", str(out))
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("alpha-ddd4", r.stderr + r.stdout)

        ws2 = Workspace(tempfile.mkdtemp(dir=self.tmp.name))
        (ws2.root / COV / "stale-1-2.profraw").write_bytes(b"x")
        r = ws2.run("manifest", "--artifacts", "artifacts.jsonl",
                    "--workspace", str(ws2.root), "--cov-dir", COV,
                    "--sha", "a" * 40, "--tests", "tests.json", "--out", "m.json")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("profraw", r.stderr + r.stdout)

    def test_manifest_refuses_target_without_listing(self):
        tests = read_json(self.ws.root / "tests.json")
        del tests["tests/beta.rs"]
        (self.ws.root / "tests.json").write_text(json.dumps(tests))
        r = self.ws.run("manifest", "--artifacts", "artifacts.jsonl",
                        "--workspace", str(self.ws.root), "--cov-dir", COV,
                        "--sha", "a" * 40, "--tests", "tests.json", "--out", "m.json")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("beta", r.stderr + r.stdout)


class PlanTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.ws = Workspace(self.tmp.name)

    def test_partition_is_complete_disjoint_and_deterministic(self):
        self.ws.plan()
        p = read_json(self.ws.root / "ci-shards" / "partition-2.json")
        a, b = p["assignments"]["0"], p["assignments"]["1"]
        self.assertEqual(set(a) | set(b), set(self.ws.TARGET_TESTS))
        self.assertEqual(set(a) & set(b), set())
        p1 = read_json(self.ws.root / "ci-shards" / "partition-1.json")
        self.assertEqual(set(p1["assignments"]["0"]), set(self.ws.TARGET_TESTS))
        self.assertEqual(p["workers"], 2)
        self.assertTrue(p["manifest_sha256"])

        ws2 = Workspace(tempfile.mkdtemp(dir=self.tmp.name))
        ws2.plan()
        self.assertEqual(p, read_json(ws2.root / "ci-shards" / "partition-2.json"))

    def test_new_target_is_included_automatically(self):
        ws = self.ws
        p = ws.root / COV / "debug/deps/gamma-9999"
        p.write_bytes(b"\x7fELF-new")
        tests = read_json(ws.root / "tests.json")
        tests["tests/gamma.rs"] = {"tests": ["g1"], "ignored": []}
        (ws.root / "tests.json").write_text(json.dumps(tests))
        lines = (ws.root / "artifacts.jsonl").read_text().splitlines()
        lines.append(artifact_line(ws.root, "tests/gamma.rs",
                                   f"{COV}/debug/deps/gamma-9999", True, ["test"]))
        (ws.root / "artifacts.jsonl").write_text("\n".join(lines))
        ws.plan()
        part = read_json(ws.root / "ci-shards" / "partition-2.json")
        union = set().union(*part["assignments"].values())
        self.assertIn("tests/gamma.rs", union)

    def test_session_files_supervised_actions_and_prelude(self):
        self.ws.plan()
        part = read_json(self.ws.root / "ci-shards" / "partition-2.json")
        for idx, assignment in part["assignments"].items():
            actions = session_actions(
                self.ws.root / "ci-shards" / f"session-2-{idx}.jsonl")
            self.assertEqual(actions[-1], {"exit": True})
            labels = [a["label"] for a in actions[:-1]]
            self.assertEqual(len(labels), len(set(labels)))
            for a in actions[:-1]:
                self.assertRegex(a["label"], r"^[a-z0-9_-]+$")
                exe = a["args"][0]
                self.assertTrue(exe.startswith(str(self.ws.root)))
                self.assertEqual(a["args"][1:], ["--test-threads=1"])
        prelude = session_actions(self.ws.root / "ci-shards" / "prelude.jsonl")
        self.assertEqual(prelude[-1], {"exit": True})
        self.assertEqual(len(prelude), 2)
        args = prelude[0]["args"]
        self.assertIn("m1_provider_fixture", args[0])
        self.assertIn("--exact", args)
        self.assertIn("stalled_reservation_reply_hard_stops_at_the_"
                      "original_decision_envelope", args)


class VerifyTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.ws = Workspace(self.tmp.name)
        self.m = self.ws.manifest()
        self.sha = "a" * 40

    def verify(self, sha=None):
        return self.ws.run("verify", "--manifest", str(self.m),
                           "--sha", sha or self.sha)

    def test_clean_tree_passes(self):
        r = self.verify()
        self.assertEqual(r.returncode, 0, r.stderr + r.stdout)

    def test_tampered_or_missing_object_fails(self):
        p = self.ws.root / f"{COV}/debug/deps/alpha-ddd4"
        p.write_bytes(b"\x7fELF-tampered")
        self.assertNotEqual(self.verify().returncode, 0)
        p.unlink()
        self.assertNotEqual(self.verify().returncode, 0)
        (self.ws.root / f"{COV}/debug/duskweave").unlink()
        self.assertNotEqual(self.verify().returncode, 0)

    def test_wrong_candidate_and_stale_profile_fail(self):
        self.assertNotEqual(self.verify("b" * 40).returncode, 0)
        (self.ws.root / COV / "w0-1-2.profraw").write_bytes(b"x")
        self.assertNotEqual(self.verify().returncode, 0)


class ReceiptAggregateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.ws = Workspace(self.tmp.name)
        self.ws.plan()
        self.part = self.ws.root / "ci-shards" / "partition-2.json"
        self.manifest = self.ws.root / "ci-shards" / "manifest.json"
        p = read_json(self.part)
        self.assign = {int(k): v for k, v in p["assignments"].items()}

    def receipt(self, worker, report="suite", out=None):
        r = self.ws.run("receipt", "--manifest", str(self.manifest),
                        "--partition", str(self.part), "--worker", str(worker),
                        "--report", str(self.ws.root / f"evidence-w{worker}" / report),
                        "--out", out or str(self.ws.root / f"evidence-w{worker}" / "receipt.json"))
        return r

    def aggregate(self, extra=(), profraw=("w0", "w1"), prelude_profraw=()):
        cov = self.ws.root / COV
        for w in profraw:
            (cov / f"{w}-111-222.profraw").write_bytes(b"prof")
        for w in prelude_profraw:
            (cov / f"prelude-{w}-1-2.profraw").write_bytes(b"prof")
        evs = [str(self.ws.root / f"evidence-w{i}") for i in self.assign]
        r = self.ws.run("aggregate", "--manifest", str(self.manifest),
                        "--partition", str(self.part),
                        "--evidence", *evs, "--cov-dir", COV,
                        "--workers", "2", *extra)
        for w in profraw + prelude_profraw:
            for f in cov.glob(f"{w}*.profraw"):
                f.unlink()
        return r

    def make_workers(self, prelude=True):
        for i, assignment in self.assign.items():
            ev = self.ws.evidence(i, assignment)
            self.assertEqual(self.receipt(i).returncode, 0)

    def test_happy_path_receipts_and_aggregate(self):
        self.make_workers()
        for i in self.assign:
            rec = read_json(self.ws.root / f"evidence-w{i}" / "receipt.json")
            self.assertEqual(rec["worker"], i)
            self.assertEqual(len(rec["targets"]), len(self.assign[i]))
        r = self.aggregate()
        self.assertEqual(r.returncode, 0, r.stderr + r.stdout)

    def test_dropped_worker_and_missing_result_fail(self):
        self.ws.evidence(0, self.assign[0])
        self.receipt(0)
        self.assertNotEqual(self.aggregate(profraw=("w0",)).returncode, 0)

        ws2 = Workspace(tempfile.mkdtemp(dir=self.tmp.name))
        self.ws = ws2
        self.ws.plan()
        p = read_json(ws2.root / "ci-shards" / "partition-2.json")
        self.part = ws2.root / "ci-shards" / "partition-2.json"
        self.manifest = ws2.root / "ci-shards" / "manifest.json"
        self.assign = {int(k): v for k, v in p["assignments"].items()}
        ev = self.ws.evidence(1, self.assign[1])
        (ev / "suite" / f"{action_label(0, self.assign[1][0])}.result.json").unlink()
        self.assertNotEqual(self.receipt(1).returncode, 0)

    def test_action_failures_and_incomplete_capture_fail_receipt(self):
        ws = self.ws
        ws.evidence(0, self.assign[0])
        ev = ws.root / "evidence-w0" / "suite"
        label = action_label(0, self.assign[0][0])

        cases = [
            ({"exit": 1, "status": "action_failed"}, "nonzero exit"),
            ({"capture": "incomplete", "omissions": ["stderr"]}, "capture"),
            ({"cleanup": "unknown"}, "cleanup"),
        ]
        for patch, name in cases:
            with self.subTest(name=name):
                good = result_json()
                good.update(patch)
                if "exit" in patch:
                    good["exit"] = patch["exit"]
                    good["status"] = patch["status"]
                (ev / f"{label}.result.json").write_text(json.dumps(good))
                self.assertNotEqual(self.receipt(0).returncode, 0)
                (ev / f"{label}.result.json").write_text(json.dumps(result_json()))

    def test_wrong_execution_failed_and_ignored_mismatch_fail(self):
        ws = self.ws
        tid = self.assign[0][0]
        names, ignored = ws.TARGET_TESTS[tid]
        ws.evidence(0, self.assign[0])
        log = ws.root / "evidence-w0" / "suite" / f"{action_label(0, tid)}.log"

        extra_ignored = ignored + [n for n in names if n not in ignored][:1]
        variants = [
            test_log(names[:-1], ignored),            # dropped test
            test_log(names, ignored, failures=names[:1]),
            test_log(names, extra_ignored),           # newly ignored test
            test_log([n for n in names if n not in ignored]),
            test_log(names, ignored, filtered=2),
            "garbage without result\n",
        ]
        expect_fail = [True, True, len(extra_ignored) > len(ignored),
                       len(ignored) > 0, True, True]
        # Omitting ignored entries breaks the required ignored-set when the
        # target declares ignored tests; the filtered run is always invalid.
        for body, fail in zip(variants, expect_fail):
            log.write_text(body)
            r = self.receipt(0)
            self.assertEqual(r.returncode != 0, fail, msg=body[:60])
            if fail:
                continue
        # extra executed test (duplicated coverage of another shard's target)
        log.write_text(test_log(names + ["surprise_test"], ignored))
        self.assertNotEqual(self.receipt(0).returncode, 0)

    def test_should_panic_result_line_is_recognized(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("shards", str(SCRIPT))
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        p = Path(self.ws.root) / "panic.log"
        p.write_text(
            "running 2 tests\n"
            "test panics_ok - should panic ... ok\n"
            "test normal_ok ... ok\n\n"
            "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; "
            "0 filtered out; finished in 0.01s\n")
        names, ignored, failed, result = mod.parse_libtest_log(p)
        self.assertEqual(names, {"panics_ok", "normal_ok"})
        self.assertEqual(failed, set())
        self.assertEqual(ignored, set())
        self.assertEqual(result["passed"], 2)

    def test_aggregate_rejects_evidence_mismatch(self):
        self.make_workers()
        # tamper evidence vs receipt: change a receipt after the fact
        rec_path = self.ws.root / "evidence-w0" / "receipt.json"
        rec = read_json(rec_path)
        rec["targets"][0]["exit"] = 1
        rec_path.write_text(json.dumps(rec))
        self.assertNotEqual(self.aggregate().returncode, 0)

        self.make_workers()
        # prelude profile leaking into merge set fails
        self.assertNotEqual(self.aggregate(prelude_profraw=("prelude",)).returncode, 0)

    def test_aggregate_rejects_missing_object_and_assignment_drift(self):
        self.make_workers()
        (self.ws.root / f"{COV}/debug/deps/beta-eee5").unlink()
        self.assertNotEqual(self.aggregate().returncode, 0)


if __name__ == "__main__":
    unittest.main()

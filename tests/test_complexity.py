"""Negative-control regression tests for scripts/check_complexity.py."""
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"
CHECKER = SCRIPTS / "check_complexity.py"
ANALYZER = os.environ.get("DW_COMPLEXITY_ANALYZER")

REVIEWED_RUN_SHA = (
    "558499af2367afeb051d01e4c31e82e20087d491fee936c1fa83549bd1bd6d7a"
)
REVIEWED_RUN_SRC = """fn run(args: &[String]) -> Res<()> {
    match args
        .get(1)
        .map(String::as_str)
        .ok_or(Fail::Input("missing_command"))?
    {
        "prepare-operation" => cmd_prepare(args),
        "register" => cmd_register(args),
        "assess" => planning_cli::cmd_assess(args),
        "withdraw" => cmd_withdraw(args),
        "planning-history" => planning_cli::cmd_planning_history(args),
        "inspect" => cmd_inspect(args),
        "reconcile" => cmd_reconcile(args),
        _ => Err(Fail::Input("unknown_command")),
    }
}
"""

OK_RS = "fn gate() {\n" + "    if true {}\n" * 6 + "}\n"
OVER_RS = "fn gate() {\n" + "    if true {}\n" * 7 + "}\n"
DECL_RS = "struct A;\nenum E { X }\n"
UNPARSED_RS = ("fn a() {\n    let x = 1;\n" * 60) + "}\n"
LONG_RS = "fn long() {\n" + "    let x = 1;\n" * 60 + "}\n"
SAME_LINE_RS = (
    "fn f() {\n    let a = || { if true {} }; let b = || { if true {} };\n}\n"
)
NESTED_RS = """fn parent() {
    if a {}
    fn child() {
        if b {}
        if b {}
        let _z = || {
            if c {}
        };
    }
    let _c = || {
        if d {}
    };
}
struct S;
impl S {
    fn method(&self) {
        if a {}
    }
}
fn m(x: u8) -> u8 {
    match x {
        0 => 1,
        1 | 2 => 2,
        _ => 3,
    }
}
fn b(a: bool, b: bool, c: bool) -> bool {
    a && b || c
}
"""


def load_module(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def load_checker():
    load_module("complexity_reports")
    return load_module("check_complexity")


def git(repo, *args):
    return subprocess.run(
        ["git", "-C", repo, *args], check=True, capture_output=True, text=True
    ).stdout.strip()


def make_repo(td, files):
    for rel, text in files.items():
        path = os.path.join(td, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)
    git(td, "init", "-q")
    git(td, "add", "-A")


def commit(td):
    git(td, "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "f")


def run_checker(td, analyzer=None, report_dir=None):
    analyzer = ANALYZER if analyzer is None else analyzer
    rd = report_dir or os.path.join(td, "report-out")
    return subprocess.run(
        [
            sys.executable,
            str(CHECKER),
            "--analyzer",
            analyzer,
            "--report-dir",
            os.path.abspath(rd),
        ],
        cwd=td,
        capture_output=True,
        text=True,
    )


def summary_of(td, report_dir=None):
    rd = report_dir or os.path.join(td, "report-out")
    with open(os.path.join(rd, "summary.json"), encoding="utf-8") as fh:
        return json.load(fh)


def mk(file="src/main.rs", name="f", depth=1, own=1, span=3):
    return {
        "file": file,
        "name": name,
        "identity": f"{file} {file}@0::{name}@0",
        "start_line": 1,
        "end_line": span,
        "span": span,
        "own": own,
        "depth": depth,
        "cap": 7,
        "allowance": None,
    }


class CliFixtures(unittest.TestCase):
    def setUp(self):
        if not ANALYZER:
            self.fail("DW_COMPLEXITY_ANALYZER is required for these tests")

    def test_cap_boundary_7_passes_8_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK_RS})
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            make_repo(td, {"src/a.rs": OK_RS, "src/b.rs": OVER_RS})
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout)
            self.assertIn("own=8 cap=7", res.stdout)
            self.assertIn("Failed: 1 complexity violations", res.stdout)

    def test_real_reviewed_dispatch_passes_at_10(self):
        self.assertEqual(
            hashlib.sha256(REVIEWED_RUN_SRC.encode()).hexdigest(),
            REVIEWED_RUN_SHA,
            "embedded reviewed run text no longer matches the pinned digest",
        )
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/main.rs": REVIEWED_RUN_SRC})
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout)
            self.assertIn("cap=10 [reviewed-dispatch]", res.stdout)
            self.assertEqual(summary_of(td)["run_allowance"]["status"], "granted")

    def test_changed_dispatch_body_loses_allowance(self):
        changed = REVIEWED_RUN_SRC.replace('"inspect"', '"inspect2"')
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/main.rs": changed})
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout)
            self.assertIn("[digest-mismatch]", res.stdout)
            self.assertEqual(
                summary_of(td)["run_allowance"]["status"], "digest_mismatch"
            )

    def test_same_named_run_elsewhere_stays_7(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/other.rs": REVIEWED_RUN_SRC})
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout)
            self.assertIn("src/other.rs", res.stdout)
            self.assertIn("own=10 cap=7", res.stdout)

    def test_nested_run_in_main_stays_7(self):
        nested = "fn wrapper() -> Res<()> {\n" + "".join(
            f"    {line}" for line in REVIEWED_RUN_SRC.splitlines(keepends=True)
        ) + "}\n"
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/main.rs": nested})
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout)
            self.assertIn("run allowance: absent", res.stdout)

    def test_ambiguous_dispatch_never_gains_10(self):
        two = OVER_RS.replace("gate", "run") + OVER_RS.replace("gate", "run")
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/main.rs": two})
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout)
            self.assertIn("ambiguous", res.stdout)
            self.assertEqual(summary_of(td)["violation_count"], 2)

    def test_missing_dispatch_is_absent_not_granted(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/main.rs": "fn main() {}\n"})
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout)
            self.assertEqual(summary_of(td)["run_allowance"]["status"], "absent")

    def test_nested_functions_measured_independently(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/nested.rs": NESTED_RS})
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            s = summary_of(td)
            self.assertEqual(s["functions"], 7)
            own = {m["identity"].split("::")[-1].rsplit("@", 1)[0]: m["own"]
                   for m in s["measurements"]}
            self.assertEqual(own["parent"], 2)
            self.assertEqual(own["child"], 3)
            self.assertEqual(own["method"], 2)
            self.assertEqual(own["m"], 4)
            self.assertEqual(own["b"], 3)

    def test_same_line_closures_are_distinct(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/s.rs": SAME_LINE_RS})
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            s = summary_of(td)
            self.assertEqual(s["functions"], 3)
            ids = [m["identity"] for m in s["measurements"]]
            self.assertEqual(len(set(ids)), 3)
            anon = [m for m in s["measurements"] if m["name"] == "<anonymous>"]
            self.assertEqual(len(anon), 2)

    def test_declarations_only_file_passes(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/decl.rs": DECL_RS})
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            self.assertEqual(summary_of(td)["functions"], 0)

    def test_non_unit_report_root_fails_closed(self):
        # rca emits a function-kind root for source it cannot fully parse;
        # schema qualification must reject it rather than measure it.
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/bad.rs": UNPARSED_RS})
            res = run_checker(td)
            self.assertEqual(res.returncode, 2, res.stdout + res.stderr)
            self.assertIn("root kind", res.stderr)

    def test_long_span_review_trigger_passes(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/long.rs": LONG_RS})
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout)
            self.assertIn("REVIEW_TRIGGER", res.stdout)
            self.assertEqual(summary_of(td)["review_trigger_count"], 1)

    def test_cli_binds_candidate_sha_and_reports(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK_RS})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout)
            self.assertIn(git(td, "rev-parse", "HEAD"), res.stdout)
            s = summary_of(td)
            self.assertEqual(s["candidate_sha"], git(td, "rev-parse", "HEAD"))
            self.assertEqual(s["tool"], "rust-code-analysis-cli")
            self.assertTrue(os.path.exists(os.path.join(
                td, "report-out", "summary.txt")))
            self.assertTrue(os.path.exists(os.path.join(
                td, "report-out", "analyzer.stderr.log")))

    def test_untracked_source_is_unexpected_report(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK_RS})
            extra = os.path.join(td, "src", "extra.rs")
            with open(extra, "w", encoding="utf-8") as fh:
                fh.write(OK_RS)
            res = run_checker(td)
            self.assertEqual(res.returncode, 2, res.stdout + res.stderr)
            self.assertIn("unexpected", res.stderr)

    def test_empty_inventory_fails_closed(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"README.md": "x\n"})
            res = run_checker(td)
            self.assertEqual(res.returncode, 2, res.stdout + res.stderr)
            self.assertIn("empty tracked", res.stderr)

    def test_not_a_git_repo_fails_closed(self):
        with tempfile.TemporaryDirectory() as td:
            os.makedirs(os.path.join(td, "src"))
            res = run_checker(td)
            self.assertEqual(res.returncode, 2, res.stdout + res.stderr)

    def test_bad_analyzer_inputs_fail_closed(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK_RS})
            for bad in (
                os.path.join(td, "missing-analyzer"),
                td,
                sys.executable,
            ):
                res = run_checker(td, analyzer=bad)
                self.assertEqual(res.returncode, 2, f"{bad}: {res.stderr}")

    def test_relative_args_fail_closed(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK_RS})
            res = subprocess.run(
                [sys.executable, str(CHECKER), "--analyzer", "rel/an",
                 "--report-dir", os.path.join(td, "r")],
                cwd=td, capture_output=True, text=True)
            self.assertEqual(res.returncode, 2)
            res = subprocess.run(
                [sys.executable, str(CHECKER), "--analyzer", ANALYZER,
                 "--report-dir", "rel/dir"],
                cwd=td, capture_output=True, text=True)
            self.assertEqual(res.returncode, 2)


class PolicyEvaluator(unittest.TestCase):
    def setUp(self):
        self.checker = load_checker()

    def allowance(self, ms):
        return self.checker.resolve_run_allowance(ms, lambda m: REVIEWED_RUN_SHA)

    def test_dispatch_cap_10_passes_11_fails(self):
        ms = [mk(name="run", own=10)]
        self.assertEqual(self.allowance(ms)["status"], "granted")
        self.assertEqual(ms[0]["cap"], 10)
        self.assertEqual(self.checker.evaluate(ms)[0], [])
        ms = [mk(name="run", own=11)]
        self.allowance(ms)
        self.assertEqual(len(self.checker.evaluate(ms)[0]), 1)

    def test_dispatch_identity_not_digest_alone(self):
        for field in ({"depth": 2}, {"file": "src/other.rs"}, {"name": "runner"}):
            ms = [mk(own=10, **{"name": "run", **field})]
            status = self.allowance(ms)["status"]
            self.assertIn(status, ("absent", "digest_mismatch", "ambiguous"))
            self.assertEqual(ms[0]["cap"], 7, field)

    def test_ambiguous_and_missing_dispatch(self):
        ms = [mk(name="run"), mk(name="run")]
        self.assertEqual(self.allowance(ms)["status"], "ambiguous")
        self.assertTrue(all(m["cap"] == 7 for m in ms))
        ms = [mk(name="main")]
        self.assertEqual(self.allowance(ms)["status"], "absent")

    def test_digest_mismatch_keeps_cap_7(self):
        ms = [mk(name="run", own=10)]
        allow = self.checker.resolve_run_allowance(ms, lambda m: "0" * 64)
        self.assertEqual(allow["status"], "digest_mismatch")
        self.assertEqual(ms[0]["cap"], 7)
        self.assertEqual(len(self.checker.evaluate(ms)[0]), 1)

    def test_body_digest_matches_reviewed_binding(self):
        c = self.checker
        with tempfile.TemporaryDirectory() as td:
            path = os.path.join(td, "main.rs")
            with open(path, "w", encoding="utf-8", newline="") as fh:
                fh.write(REVIEWED_RUN_SRC.replace("\n", "\r\n"))
            self.assertEqual(c.body_digest(path, 1, 16), REVIEWED_RUN_SHA)


if __name__ == "__main__":
    unittest.main()

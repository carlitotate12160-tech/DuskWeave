"""Ratchet gate tests for scripts/check_complexity.py --base behavior.

Exercises the real CLI on disposable Git histories with the pinned analyzer:
exit 0 gate pass, 1 McCabe regression, 2 measurement/input/base failure.
Statuses: NEW, REGRESSED, RETAINED, IMPROVED, RESOLVED, REMOVED, AMBIGUOUS.
"""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"
CHECKER = SCRIPTS / "check_complexity.py"
ANALYZER = os.environ.get("DW_COMPLEXITY_ANALYZER")


def fn(name, ifs):
    return f"fn {name}() {{\n" + "    if true {}\n" * ifs + "}\n"


OK7 = fn("gate", 6)            # own = 7, at threshold
OVER8 = fn("gate", 7)          # own = 8, over threshold
DEBT24 = fn("debt", 23)        # own = 24
DEBT23 = fn("debt", 22)        # own = 23
DEBT25 = fn("debt", 24)        # own = 25
TINY = "fn tiny() {}\n"

RUN_SRC = """fn run(command: &str, args: &[String]) -> Res<()> {
    match command {
        "a" => cmd_a(args),
        "b" => cmd_b(args),
        "c" => cmd_c(args),
        "d" => cmd_d(args),
        "e" => cmd_e(args),
        "f" => cmd_f(args),
        "g" => cmd_g(args),
        "h" => cmd_h(args),
        _ => other(command, args),
    }
}
"""

RUN11_SRC = RUN_SRC.replace('"h" => cmd_h(args),', '"h" => cmd_h(args),\n        "i" => cmd_i(args),')

TWO_CLOSURES = """fn parent() {
    let _a = || {
""" + "        if true {}\n" * 7 + """    };
    let _b = || {
""" + "        if true {}\n" * 7 + """    };
}
"""

TWO_CLOSURES_WORSE = TWO_CLOSURES.replace(
    "let _b = || {", "let _b = || {\n        if true {}", 1
)

ONE_CLOSURE = """fn parent() {
    let _a = || {
""" + "        if true {}\n" * 7 + """    };
}
"""

SCOPE_PAIR = """struct A;
struct B;
impl A {
""" + fn("    shared", 7).replace("fn ", "fn ", 1).replace("\n", "\n    ") + """}
impl B {
""" + fn("    shared", 7).replace("\n", "\n    ") + """}
"""


def git(repo, *args):
    return subprocess.run(
        ["git", "-C", repo, *args], check=True, capture_output=True, text=True
    ).stdout.strip()


def write(repo, rel, text):
    path = os.path.join(repo, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)


def make_repo(td, files):
    for rel, text in files.items():
        write(td, rel, text)
    git(td, "init", "-q")
    git(td, "add", "-A")


def commit(td):
    git(td, "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "f")
    return git(td, "rev-parse", "HEAD")


def run_checker(td, base="HEAD", analyzer=None, report_dir=None, extra=()):
    analyzer = ANALYZER if analyzer is None else analyzer
    rd = report_dir or os.path.join(td, "report-out")
    argv = [
        sys.executable, str(CHECKER), "--analyzer", analyzer,
        "--report-dir", os.path.abspath(rd), *extra,
    ]
    if base is not None:
        argv += ["--base", base]
    return subprocess.run(argv, cwd=td, capture_output=True, text=True)


def summary_of(td, report_dir=None):
    rd = report_dir or os.path.join(td, "report-out")
    import json
    with open(os.path.join(rd, "summary.json"), encoding="utf-8") as fh:
        return json.load(fh)


def statuses(summary):
    out = {}
    for m in summary["measurements"]:
        out.setdefault(m.get("status"), []).append(m)
    return out


class RatchetGate(unittest.TestCase):
    def setUp(self):
        if not ANALYZER:
            self.fail("DW_COMPLEXITY_ANALYZER is required for these tests")

    def test_new_function_boundary_7_passes_8_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK7})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            write(td, "src/b.rs", OVER8)
            git(td, "add", "src/b.rs")
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            self.assertIn("new", {m.get("status") for m in s["measurements"]})
            self.assertEqual(s["ratchet"]["counts"]["regressed"], 0)

    def test_retained_debt_stays_passes(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": DEBT24})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(len(st["retained"]), 1)

    def test_debt_decreased_passes_increased_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": DEBT24})
            base = commit(td)
            write(td, "src/a.rs", DEBT23)
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            self.assertEqual(
                statuses(summary_of(td))["improved"][0]["own"], 23
            )
            write(td, "src/a.rs", DEBT25)
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(st["regressed"][0]["own"], 25)
            self.assertEqual(st["regressed"][0]["base_own"], 24)

    def test_previously_compliant_crossing_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK7})
            base = commit(td)
            write(td, "src/a.rs", OVER8)
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(st["regressed"][0]["own"], 8)
            self.assertEqual(st["regressed"][0]["base_own"], 7)

    def test_compliant_function_may_rise_within_cap(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": fn("gate", 2)})
            base = commit(td)
            write(td, "src/a.rs", OK7)
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            self.assertNotIn("regressed", statuses(summary_of(td)))

    def test_deleted_function_is_removed_not_failure(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": DEBT24, "src/b.rs": TINY})
            base = commit(td)
            write(td, "src/a.rs", DEBT24)
            os.remove(os.path.join(td, "src", "b.rs"))
            git(td, "add", "-A")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            self.assertEqual(summary_of(td)["ratchet"]["counts"]["removed"], 1)

    def test_insertion_before_function_no_false_regression(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": DEBT24})
            base = commit(td)
            write(td, "src/a.rs", TINY + DEBT24)
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(len(st["retained"]), 1)
            self.assertEqual(st["retained"][0]["name"], "debt")
            self.assertEqual(len(st["new"]), 1)

    def test_lexical_scope_collision_tracks_each_owner(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/s.rs": SCOPE_PAIR})
            base = commit(td)
            # Worsen only impl B's shared(): add one branch there.
            worse = SCOPE_PAIR.replace(
                "impl B {\n", "impl B {\n", 1
            ).rsplit("    if true {}\n", 1)
            worse = worse[0] + "    if true {}\n    if true {}\n" + worse[1]
            write(td, "src/s.rs", worse)
            git(td, "add", "src/s.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(len(st["regressed"]), 1)
            self.assertIn("B", st["regressed"][0]["identity"])
            self.assertEqual(len(st["retained"]), 1)

    def test_rename_of_debt_is_conservative_new(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": DEBT24})
            base = commit(td)
            write(td, "src/a.rs", DEBT24.replace("debt", "renamed"))
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            self.assertEqual(s["ratchet"]["counts"]["removed"], 1)
            st = statuses(s)
            self.assertEqual(st["new"][0]["name"], "renamed")

    def test_ambiguous_closures_groupwise_not_individually_matched(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": TWO_CLOSURES})
            base = commit(td)
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(len(st["ambiguous"]), 2)
            write(td, "src/c.rs", TWO_CLOSURES_WORSE)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

    def test_new_ambiguous_closure_violation_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": ONE_CLOSURE})
            base = commit(td)
            write(td, "src/c.rs", TWO_CLOSURES)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

    def test_single_closure_under_parent_matches_by_scope(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": ONE_CLOSURE})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(len(st["retained"]), 1)
            self.assertEqual(st["retained"][0]["name"], "<anonymous>")

    def test_dispatch_run_threshold_10_passes_11_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/main.rs": RUN_SRC})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            s = summary_of(td)
            self.assertEqual(s["run_allowance"]["status"], "granted")
            self.assertEqual(
                [m for m in s["measurements"] if m["name"] == "run"][0]["cap"],
                10,
            )
            write(td, "src/main.rs", RUN11_SRC)
            git(td, "add", "src/main.rs")
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            st = statuses(summary_of(td))
            self.assertEqual(st["regressed"][0]["own"], 11)

    def test_run_in_other_file_gets_no_dispatch_cap(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/other.rs": RUN_SRC})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            run_m = [
                m for m in summary_of(td)["measurements"]
                if m["name"] == "run"
            ][0]
            self.assertEqual(run_m["cap"], 7)
            self.assertEqual(run_m["status"], "retained")

    def test_missing_base_arg_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK7})
            commit(td)
            res = run_checker(td, base=None)
            self.assertEqual(res.returncode, 2)

    def test_unresolvable_base_fails_closed(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK7})
            commit(td)
            for bad in ("no-such-ref", "HEAD~99"):
                res = run_checker(td, base=bad)
                self.assertEqual(res.returncode, 2, bad)
                self.assertIn("complexity check failed", res.stderr)

    def test_base_without_tracked_src_fails_closed(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"README.md": "x\n"})
            base = commit(td)
            write(td, "src/a.rs", OK7)
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 2, res.stdout + res.stderr)
            self.assertIn("base", res.stderr)

    def test_uncommitted_candidate_is_worktree_evidence(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK7})
            base = commit(td)
            write(td, "src/a.rs", OVER8)  # dirty worktree, not even staged
            res = run_checker(td, base=base)
            # unstaged file is still tracked -> measured; regression found.
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            self.assertEqual(s["candidate_sha"], base)
            self.assertNotEqual(s.get("candidate_source"), "commit")

    def test_staged_candidate_measured(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK7})
            base = commit(td)
            write(td, "src/b.rs", OVER8)
            git(td, "add", "src/b.rs")  # staged, uncommitted
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

    def test_base_measured_from_commit_not_user_files(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/a.rs": OK7})
            base = commit(td)
            # A stale user-authored base report must be ignored entirely.
            write(td, "src/a.rs", DEBT24)
            git(td, "add", "src/a.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            self.assertEqual(s["base_sha"], base)


if __name__ == "__main__":
    unittest.main()

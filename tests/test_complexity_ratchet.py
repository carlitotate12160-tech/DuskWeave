"""Ratchet gate tests for scripts/check_complexity.py --base behavior.

Exercises the real CLI on disposable Git histories with the pinned analyzer:
exit 0 gate pass, 1 McCabe regression, 2 measurement/input/base failure.
Statuses: NEW, REGRESSED, RETAINED, IMPROVED, RESOLVED, REMOVED, AMBIGUOUS.
"""
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"
CHECKER = SCRIPTS / "check_complexity.py"
ANALYZER = os.environ.get("DW_COMPLEXITY_ANALYZER")


def _load(name):
    spec = importlib.util.spec_from_file_location(
        name, SCRIPTS / f"{name}.py"
    )
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


_load("complexity_reports")
ratchet = _load("complexity_ratchet")


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

def _closures(*binding_ifs):
    out = ""
    for var, ifs in binding_ifs:
        out += (
            f"    let _{var} = || {{\n"
            + "        if true {}\n" * ifs
            + "    };\n"
        )
    return "fn parent() {\n" + out + "}\n"


MIXED_CLOSURES = _closures(("a", 19), ("b", 7))    # own {20, 8}
KEPT_LARGER_CLOSURE = _closures(("a", 19))         # own {20}

SWAP_BASE = _closures(("a", 19), ("b", 6))         # own {20, 7}
SWAP_HEAD = _closures(("a", 6), ("b", 19))         # own {7, 20}
TRANSFER_BASE = MIXED_CLOSURES                     # own {20, 8}
TRANSFER_HEAD = _closures(("a", 7), ("b", 18))     # own {8, 19}

DUP_OWNER = """fn wrap() {
    let _a = || {
""" + "        if true {}\n" * 7 + """    };
}
fn wrap() {
    let _b = || {
""" + "        if true {}\n" * 7 + """    };
}
"""


def _moved_nested(inner):
    """parent() with outer closures a and b; *inner* is the body that
    carries the nested debt plus its call, moved between a and b."""
    base = (
        "fn parent() {\n"
        "    let a = || {\n" + inner + "    };\n"
        "    let b = || {};\n"
        "    a();\n    b();\n}\n"
    )
    head = (
        "fn parent() {\n"
        "    let a = || {};\n"
        "    let b = || {\n" + inner + "    };\n"
        "    a();\n    b();\n}\n"
    )
    return base, head


MOVED_CLOSURE_BASE, MOVED_CLOSURE_HEAD = _moved_nested(
    "        let high = || {\n"
    + "            if true {}\n" * 19
    + "        };\n        high();\n"
)

MOVED_HELPER_BASE, MOVED_HELPER_HEAD = _moved_nested(
    "        fn helper() {\n"
    + "            if true {}\n" * 19
    + "        }\n"
    + "        helper();\n"
)

MOVED_LOW_BASE, MOVED_LOW_HEAD = _moved_nested(
    "        let high = || {\n"
    + "            if true {}\n" * 3
    + "        };\n        high();\n"
)

NEWLY_AMBIGUOUS_BASE = (
    "fn parent() {\n"
    "    let a = || {\n"
    "        let high = || {\n"
    + "            if true {}\n" * 19
    + "        };\n        high();\n"
    "    };\n"
    "    a();\n"
    "}\n"
)
NEWLY_AMBIGUOUS_HEAD = NEWLY_AMBIGUOUS_BASE.replace(
    "    a();\n", "    a();\n    let b = || {};\n    b();\n"
)

NESTED_AMBIGUITY = (
    "fn outer() {\n"
    "    let p = || {\n"
    "        let a = || {\n"
    "            let high = || {\n"
    + "                if true {}\n" * 19
    + "            };\n"
    "            high();\n"
    "        };\n"
    "        let b = || {};\n"
    "        a();\n        b();\n"
    "    };\n"
    "    let q = || {\n"
    "        let a = || {};\n"
    "        let b = || {};\n"
    "        a();\n        b();\n"
    "    };\n"
    "    p();\n    q();\n"
    "}\n"
)

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

    def test_ambiguous_closures_owner_proof_not_individually_matched(self):
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

    def test_unchanged_ambiguous_debt_passes_owner_proof(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": TWO_CLOSURES})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            grp = summary_of(td)["ratchet"]["ambiguous_groups"][0]
            self.assertEqual(grp["disposition"], "retained")
            self.assertIn("parent", grp["owner"])
            self.assertEqual(
                grp["owner_base_span"], grp["owner_head_span"]
            )

    def test_insertion_before_unchanged_owner_passes(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": TWO_CLOSURES})
            base = commit(td)
            write(td, "src/c.rs", TINY + "// shifted comment\n" + TWO_CLOSURES)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            grp = summary_of(td)["ratchet"]["ambiguous_groups"][0]
            self.assertEqual(grp["disposition"], "retained")
            self.assertNotEqual(
                grp["owner_base_span"], grp["owner_head_span"]
            )

    def test_sibling_edit_keeps_owner_proof(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": TINY + TWO_CLOSURES})
            base = commit(td)
            write(td, "src/c.rs", fn("tiny", 2) + TWO_CLOSURES)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            grp = summary_of(td)["ratchet"]["ambiguous_groups"][0]
            self.assertEqual(grp["disposition"], "retained")

    def test_changed_owner_equal_multiset_blocked(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": TWO_CLOSURES})
            base = commit(td)
            write(
                td, "src/c.rs",
                TWO_CLOSURES.replace(
                    "fn parent() {\n", "fn parent() {\n    // touched\n"
                ),
            )
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            grp = summary_of(td)["ratchet"]["ambiguous_groups"][0]
            self.assertEqual(grp["head_over"], [8, 8])
            self.assertEqual(grp["base_over"], [8, 8])
            self.assertEqual(grp["disposition"], "blocked")

    def test_changed_owner_all_members_under_cap_passes(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": TWO_CLOSURES})
            base = commit(td)
            write(td, "src/c.rs", _closures(("a", 6), ("b", 6)))
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            grp = summary_of(td)["ratchet"]["ambiguous_groups"][0]
            self.assertEqual(grp["head_over"], [])
            self.assertEqual(grp["disposition"], "compliant")

    def test_nonunique_owner_cannot_prove_group(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/d.rs": DUP_OWNER})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            blocked = [
                g for g in s["ratchet"]["ambiguous_groups"]
                if g["disposition"] == "blocked"
            ]
            self.assertEqual(len(blocked), 1)
            self.assertIsNone(blocked[0]["owner"])

    def test_nested_closure_group_uses_nearest_named_owner(self):
        nested = """fn parent() {
    let _a = || {
        let _x = || {
""" + "            if true {}\n" * 7 + """        };
        let _y = || {
""" + "            if true {}\n" * 7 + """        };
        if true {}
    };
}
"""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": nested})
            base = commit(td)
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            worse = nested.replace("        };\n        if true {}",
                                   "            if true {}\n        };\n        if true {}", 1)
            write(td, "src/c.rs", worse)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

    def test_unchanged_owner_proof_cannot_be_borrowed(self):
        two_owners = _closures(("a", 7), ("b", 7)) + _closures(
            ("c", 7), ("d", 7)
        ).replace("parent", "sibling")
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": two_owners})
            base = commit(td)
            write(
                td, "src/c.rs",
                two_owners.replace("let _d", "let _d_changed"),
            )
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            by_id = {
                g["identity"]: g for g in s["ratchet"]["ambiguous_groups"]
            }
            self.assertEqual(
                by_id["src/c.rs parent::<anonymous>"]["disposition"],
                "retained",
            )
            self.assertEqual(
                by_id["src/c.rs sibling::<anonymous>"]["disposition"],
                "blocked",
            )

    def test_invalid_owner_span_is_integrity_failure(self):
        err = ratchet.reports.CheckError
        with tempfile.TemporaryDirectory() as td:
            write(td, "src/c.rs", TINY)
            m = {"file": "src/c.rs", "start_line": 1, "end_line": 99}
            with self.assertRaises(err) as ctx:
                ratchet._span_lines(td, m)
            self.assertIn("invalid owner span", str(ctx.exception))
            m = {"file": "src/absent.rs", "start_line": 1, "end_line": 1}
            with self.assertRaises(err) as ctx:
                ratchet._span_lines(td, m)
            self.assertIn("source unavailable", str(ctx.exception))

    def test_ambiguous_overcap_moved_between_bindings_fails(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": SWAP_BASE})
            base = commit(td)
            write(td, "src/c.rs", SWAP_HEAD)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

    def test_ambiguous_improvement_cannot_pay_regression(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": TRANSFER_BASE})
            base = commit(td)
            write(td, "src/c.rs", TRANSFER_HEAD)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

    def test_ambiguous_member_removed_in_changed_owner_blocked(self):
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": MIXED_CLOSURES})
            base = commit(td)
            write(td, "src/c.rs", KEPT_LARGER_CLOSURE)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            grp = summary_of(td)["ratchet"]["ambiguous_groups"][0]
            self.assertEqual(grp["disposition"], "blocked")

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

    def test_nested_closure_moved_between_ambiguous_parents_blocked(self):
        """Repro 1: a unique-keyed over-cap closure nested under an
        ambiguous ancestor must inherit the ambiguity; moving it between
        ambiguous parents is blocked even though its own body is unchanged."""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": MOVED_CLOSURE_BASE})
            base = commit(td)
            write(td, "src/c.rs", MOVED_CLOSURE_HEAD)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            grp = [
                g for g in s["ratchet"]["ambiguous_groups"]
                if len(g["scope"]) == 2
            ][0]
            self.assertEqual(grp["disposition"], "blocked")
            self.assertEqual(grp["ambiguity"], "inherited")
            self.assertIn("::<anonymous>", grp["ancestor"])

    def test_named_helper_moved_between_ambiguous_parents_blocked(self):
        """Repro 2: a uniquely-named fn under an ambiguous ancestor cannot
        serve as a trustworthy unchanged-owner anchor merely because its own
        body is unchanged."""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": MOVED_HELPER_BASE})
            base = commit(td)
            write(td, "src/c.rs", MOVED_HELPER_HEAD)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
            s = summary_of(td)
            st = statuses(s)
            self.assertEqual(
                [m["name"] for m in st["ambiguous"] if m["name"] == "helper"],
                ["helper"],
            )
            grp = [
                g for g in s["ratchet"]["ambiguous_groups"]
                if g["name"] == "helper"
            ][0]
            self.assertEqual(grp["disposition"], "blocked")
            self.assertEqual(grp["ambiguity"], "inherited")

    def test_unchanged_parent_proves_inherited_ambiguous_debt(self):
        """When the entire ambiguous enclosing parent is unchanged, the
        nested inherited-ambiguous over-cap member is retained via the
        safe owner proof and the gate passes."""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": MOVED_CLOSURE_BASE})
            commit(td)
            res = run_checker(td)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            s = summary_of(td)
            grp = [
                g for g in s["ratchet"]["ambiguous_groups"]
                if len(g["scope"]) == 2
            ][0]
            self.assertEqual(grp["disposition"], "retained")
            self.assertEqual(grp["ambiguity"], "inherited")

    def test_inherited_ambiguity_unchanged_parent_with_sibling_edit(self):
        """Unrelated sibling edits and insertion before the safe owner do
        not invalidate the owner proof for inherited ambiguity."""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": MOVED_CLOSURE_BASE + "\nfn tiny() {}\n"})
            base = commit(td)
            write(
                td, "src/c.rs",
                "\n// comment\n" + MOVED_CLOSURE_BASE + "\nfn tiny() {}\nfn other() {}\n",
            )
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)

    def test_inherited_ambiguity_under_cap_leaf_passes(self):
        """A changed parent does not block when every inherited-ambiguous
        member is at or below its cap — no over-cap debt is inherited."""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": MOVED_LOW_BASE})
            base = commit(td)
            write(td, "src/c.rs", MOVED_LOW_HEAD)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)

    def test_newly_ambiguous_ancestry_blocks(self):
        """An ancestry that becomes ambiguous only in the candidate still
        blocks an over-cap descendant when the safe owner changed."""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": NEWLY_AMBIGUOUS_BASE})
            base = commit(td)
            write(td, "src/c.rs", NEWLY_AMBIGUOUS_HEAD)
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

    def test_nested_ambiguous_ancestry_blocked_when_owner_changes(self):
        """Ambiguity at two enclosing levels propagates; the innermost
        ambiguous ancestor is reported and the outermost safe named owner
        governs the proof."""
        with tempfile.TemporaryDirectory() as td:
            make_repo(td, {"src/c.rs": NESTED_AMBIGUITY})
            base = commit(td)
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
            s = summary_of(td)
            grp = [
                g for g in s["ratchet"]["ambiguous_groups"]
                if len(g["scope"]) == 3
            ][0]
            self.assertEqual(grp["disposition"], "retained")
            self.assertEqual(grp["ambiguity"], "inherited")
            self.assertEqual(grp["owner"], "src/c.rs outer")
            self.assertEqual(
                grp["ancestor"],
                "src/c.rs outer::<anonymous>::<anonymous>",
            )
            write(
                td, "src/c.rs",
                NESTED_AMBIGUITY.replace("    p();", "    p(); // touch"),
            )
            git(td, "add", "src/c.rs")
            res = run_checker(td, base=base)
            self.assertEqual(res.returncode, 1, res.stdout + res.stderr)

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

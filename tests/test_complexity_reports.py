"""Report-boundary controls for the McCabe gate.

Import-level tests for scripts/complexity_reports.py (schema qualification,
source binding, span and metric validation, reconciliation, projection) plus
a POSIX analyzer-stub subprocess control proving malformed report input
reaches CLI exit 2 through the real analyzer boundary.
"""
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
SRC = "fn f() {\n    if true {}\n    if true {}\n}\n"


def load_module(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


REPORTS = load_module("complexity_reports")


def fn_node(name, cyc_sum, kids=(), start=1, end=2, kind="function"):
    return {
        "kind": kind,
        "name": name,
        "start_line": start,
        "end_line": end,
        "spaces": list(kids),
        "metrics": {"cyclomatic": {"sum": cyc_sum}, "nom": {"total": 0.0}},
    }


def unit_report(nodes, nom_total, name="src/x.rs", kind="unit", start=1, end=4):
    return {
        "kind": kind,
        "name": name,
        "start_line": start,
        "end_line": end,
        "spaces": list(nodes),
        "metrics": {"nom": {"total": nom_total}},
    }


def measure(report, source=SRC, rel="src/x.rs"):
    with tempfile.TemporaryDirectory() as td:
        src = Path(td) / "x.rs"
        src.write_text(source, encoding="utf-8", newline="\n")
        return REPORTS.measure_report(report, rel, str(src))


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


class AssertBad:
    def assert_bad(self, report, source=SRC, rel="src/x.rs", needle=""):
        with self.assertRaises(REPORTS.CheckError) as ctx:
            measure(report, source, rel)
        if needle:
            self.assertIn(needle, str(ctx.exception))


class SchemaBinding(unittest.TestCase, AssertBad):
    def test_root_kind_must_be_unit(self):
        self.assert_bad(unit_report([], 0, kind="function"), needle="root kind")

    def test_root_name_must_match_source(self):
        self.assert_bad(unit_report([], 0, name="src/other.rs"), needle="name")

    def test_root_name_backslash_normalized(self):
        self.assertEqual(
            measure(unit_report([], 0, name="src\\x.rs")), []
        )

    def test_unknown_child_kind_rejected(self):
        self.assert_bad(
            unit_report([fn_node("S", 1, kind="struct")], 0),
            needle="unrecognized kind",
        )

    def test_non_object_child_rejected(self):
        self.assert_bad(unit_report(["oops"], 0), needle="not an object")

    def test_non_object_report_rejected(self):
        self.assert_bad([1, 2], needle="not an object")

    def test_non_string_node_name_rejected(self):
        node = fn_node("f", 1)
        node["name"] = 7
        self.assert_bad(unit_report([node], 1), needle="name")

    def test_non_list_spaces_rejected(self):
        node = fn_node("f", 1)
        node["spaces"] = "nope"
        self.assert_bad(unit_report([node], 1), needle="spaces")

    def test_impl_and_trait_children_accepted(self):
        impl = fn_node(
            "impl S", 0, kind="impl",
            kids=[fn_node("m", 1, start=2, end=3)], start=1, end=3,
        )
        trait = fn_node("trait T", 0, kind="trait", start=4, end=4)
        out = measure(unit_report([impl, trait], 1))
        self.assertEqual([m["name"] for m in out], ["m"])


class SpanValidation(unittest.TestCase, AssertBad):
    def test_function_end_beyond_source_rejected(self):
        self.assert_bad(
            unit_report([fn_node("f", 1, start=1, end=99)], 1),
            needle="outside source",
        )

    def test_function_start_below_one_rejected(self):
        self.assert_bad(unit_report([fn_node("f", 1, start=0, end=1)], 1))

    def test_inverted_span_rejected(self):
        self.assert_bad(unit_report([fn_node("f", 1, start=3, end=1)], 1))

    def test_child_span_escaping_parent_rejected(self):
        parent = fn_node(
            "p", 2, kids=[fn_node("c", 1, start=3, end=4)], start=1, end=2
        )
        self.assert_bad(unit_report([parent], 2), needle="escapes parent")

    def test_root_span_outside_source_rejected(self):
        self.assert_bad(
            unit_report([], 0, end=99), needle="root span"
        )

    def test_non_integral_span_rejected(self):
        self.assert_bad(
            unit_report([fn_node("f", 1, start=1.5, end=2)], 1),
            needle="non-integral",
        )

    def test_missing_span_fields_rejected(self):
        node = fn_node("f", 1)
        del node["start_line"]
        self.assert_bad(unit_report([node], 1), needle="non-integral")

    def test_empty_source_allows_parser_empty_unit(self):
        self.assertEqual(
            measure(unit_report([], 0, start=0, end=0), source=""), []
        )
        self.assertEqual(measure(unit_report([], 0), source=""), [])

    def test_empty_source_rejects_function_node(self):
        self.assert_bad(unit_report([fn_node("f", 1)], 1), source="")

    def test_deeply_nested_report_fails_closed_not_recursion_error(self):
        node = fn_node("c", 1, start=1, end=4)
        for _ in range(250):
            node = fn_node("c", 0, kids=[node], start=1, end=4, kind="impl")
        self.assert_bad(unit_report([node], 0), needle="depth")


class MetricValidation(unittest.TestCase, AssertBad):
    def test_invalid_cyclomatic_values_rejected(self):
        for value in (float("nan"), float("inf"), 1.5, "2", True, -1, None):
            node = fn_node("f", value)
            self.assert_bad(unit_report([node], 1))

    def test_missing_cyclomatic_object_rejected(self):
        node = fn_node("f", 1)
        del node["metrics"]["cyclomatic"]
        self.assert_bad(unit_report([node], 1))

    def test_missing_metrics_object_rejected(self):
        node = fn_node("f", 1)
        del node["metrics"]
        self.assert_bad(unit_report([node], 1))

    def test_huge_int_metrics_do_not_raise_overflow(self):
        node = fn_node("f", 10**400)
        out = measure(unit_report([node], 1))
        self.assertEqual(out[0]["own"], 10**400)
        self.assert_bad(unit_report([fn_node("f", 1)], 10**400))

    def test_own_below_one_rejected(self):
        parent = fn_node("p", 2, kids=[fn_node("c", 5)])
        self.assert_bad(unit_report([parent], 2), needle="own")

    def test_nom_total_mismatch_rejected(self):
        self.assert_bad(unit_report([fn_node("a", 1), fn_node("b", 1)], 3))
        self.assert_bad(unit_report([fn_node("a", 1)], 2))
        report = unit_report([fn_node("a", 1)], 1)
        report["metrics"] = {}
        self.assert_bad(report)
        report["metrics"]["nom"] = {"total": "1"}
        self.assert_bad(report)


class ValidMeasurement(unittest.TestCase):
    def test_single_function_measured(self):
        out = measure(unit_report([fn_node("f", 3, start=1, end=4)], 1))
        self.assertEqual(len(out), 1)
        self.assertEqual(
            (out[0]["own"], out[0]["depth"], out[0]["span"]), (3, 1, 4)
        )

    def test_nested_function_own_is_difference(self):
        parent = fn_node(
            "f", 4, kids=[fn_node("<anonymous>", 2, start=2, end=3)],
            start=1, end=4,
        )
        out = measure(unit_report([parent], 2))
        self.assertEqual([m["own"] for m in out], [2, 2])
        self.assertEqual([m["depth"] for m in out], [1, 2])
        self.assertTrue(out[0]["identity"].endswith("::f@0"))
        self.assertTrue(out[1]["identity"].endswith("::<anonymous>@0"))

    def test_declarations_only_report(self):
        self.assertEqual(measure(unit_report([], 0)), [])


class LoadReconcileProject(unittest.TestCase):
    def test_load_report_rejects_invalid_and_duplicate_keys(self):
        with tempfile.TemporaryDirectory() as td:
            bad = os.path.join(td, "b.json")
            with open(bad, "w") as fh:
                fh.write("{not json")
            with self.assertRaises(REPORTS.CheckError):
                REPORTS.load_report(bad)
            with open(bad, "w") as fh:
                fh.write('{"a": 1, "a": 2}')
            with self.assertRaises(REPORTS.CheckError) as ctx:
                REPORTS.load_report(bad)
            self.assertIn("duplicate", str(ctx.exception))
            with open(bad, "w") as fh:
                fh.write("[1, 2]")
            with self.assertRaises(REPORTS.CheckError) as ctx:
                REPORTS.load_report(bad)
            self.assertIn("not an object", str(ctx.exception))
            with open(bad, "w") as fh:
                fh.write('{"n1": 1, "N1": 2}')
            self.assertEqual(REPORTS.load_report(bad), {"n1": 1, "N1": 2})

    def test_reconcile_rejects_missing_and_extra_reports(self):
        with tempfile.TemporaryDirectory() as td:
            os.makedirs(os.path.join(td, "src"))
            with self.assertRaises(REPORTS.CheckError) as ctx:
                REPORTS.reconcile_reports(td, ["src/a.rs"])
            self.assertIn("missing", str(ctx.exception))
            with open(os.path.join(td, "src", "a.rs.json"), "w") as fh:
                fh.write("{}")
            with open(os.path.join(td, "src", "b.rs.json"), "w") as fh:
                fh.write("{}")
            with self.assertRaises(REPORTS.CheckError) as ctx:
                REPORTS.reconcile_reports(td, ["src/a.rs"])
            self.assertIn("unexpected", str(ctx.exception))
            paths = REPORTS.reconcile_reports(td, ["src/a.rs", "src/b.rs"])
            self.assertEqual(sorted(paths), ["src/a.rs", "src/b.rs"])

    def test_build_and_write_summaries(self):
        m = {
            "file": "src/x.rs", "name": "f",
            "identity": "src/x.rs src/x.rs@0::f@0",
            "start_line": 1, "end_line": 4, "span": 4, "own": 3,
            "depth": 1, "cap": 7, "allowance": None,
        }
        summary = REPORTS.build_summary(
            "tool", "0.0.25", "sha", ["src/x.rs"], [m], [m], [],
            {"status": "absent"},
        )
        self.assertEqual(summary["violation_count"], 1)
        with tempfile.TemporaryDirectory() as td:
            REPORTS.write_summaries(td, summary)
            with open(os.path.join(td, "summary.json")) as fh:
                self.assertEqual(json.load(fh)["functions"], 1)
            with open(os.path.join(td, "summary.txt")) as fh:
                self.assertIn("VIOLATION", fh.read())


FAKE_ANALYZER_SH = """#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "rust-code-analysis-cli 0.0.25"; exit 0
fi
out=""
while [ $# -gt 0 ]; do
    if [ "$1" = "-o" ]; then out="$2"; shift; fi
    shift
done
if [ "$FAKE_MODE" = "fail" ]; then exit 3; fi
mkdir -p "$out/src"
if [ "$FAKE_MODE" = "malformed" ]; then
    printf '{bad json' > "$out/src/a.rs.json"
fi
exit 0
"""


@unittest.skipUnless(os.name == "posix", "needs an executable shell stub")
class AnalyzerBoundary(unittest.TestCase):
    """CLI exit-2 controls through a controlled analyzer stub.

    The same production code paths are covered platform-independently by the
    import-level controls above; the stub additionally proves the CLI maps
    them to exit 2 where a POSIX stub can run.
    """

    def run_with_stub(self, mode):
        td = tempfile.mkdtemp()
        self.addCleanup(
            __import__("shutil").rmtree, td, ignore_errors=True
        )
        make_repo(td, {"src/a.rs": "pub fn a() {}\n"})
        stub = os.path.join(td, "fake-analyzer")
        with open(stub, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(FAKE_ANALYZER_SH)
        os.chmod(stub, 0o755)
        env = dict(os.environ, FAKE_MODE=mode)
        return subprocess.run(
            [
                sys.executable, str(CHECKER), "--analyzer", stub,
                "--report-dir", os.path.join(td, "out"),
            ],
            cwd=td, capture_output=True, text=True, env=env,
        )

    def test_malformed_report_exits_2(self):
        res = self.run_with_stub("malformed")
        self.assertEqual(res.returncode, 2, res.stdout + res.stderr)
        self.assertIn("complexity check failed", res.stderr)
        self.assertIn("invalid JSON", res.stderr)

    def test_missing_report_exits_2(self):
        res = self.run_with_stub("missing")
        self.assertEqual(res.returncode, 2, res.stdout + res.stderr)
        self.assertIn("inventory mismatch", res.stderr)

    def test_nonzero_analyzer_exits_2(self):
        res = self.run_with_stub("fail")
        self.assertEqual(res.returncode, 2, res.stdout + res.stderr)
        self.assertIn("exited 3", res.stderr)


if __name__ == "__main__":
    unittest.main()

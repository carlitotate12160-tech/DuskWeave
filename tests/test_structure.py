"""Regression tests for category size limits and preserved hygiene."""
import contextlib
import importlib.util
import io
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "scripts/check_structure.py"


def check_files(files):
    spec = importlib.util.spec_from_file_location("structure", SCRIPT)
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    with tempfile.TemporaryDirectory() as tmp:
        for rel, text in files.items():
            path = Path(tmp) / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        checker.ROOT = tmp
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = checker.main()
        return code, output.getvalue()


class StructureTests(unittest.TestCase):
    def assert_boundary(self, path, cap):
        for final_newline in (False, True):
            for size, expected in ((cap, 0), (cap + 1, 1)):
                with self.subTest(path=path, size=size, newline=final_newline):
                    text = "\n".join("// bounded" for _ in range(size))
                    if final_newline:
                        text += "\n"
                    code, out = check_files({path: text})
                    self.assertEqual(code, expected, out)
                    if expected:
                        self.assertIn(f"{size} lines > {cap}", out)

    def test_production_cap_stays_400(self):
        self.assert_boundary("src/domain.rs", 400)

    def test_tooling_cap_stays_400(self):
        self.assert_boundary("scripts/task.py", 400)

    def test_tests_and_benchmarks_cap_500(self):
        for path in ("tests/domain.rs", "tests/test_gate.py", "benches/domain.rs",
                     "src/domain/tests.rs"):
            self.assert_boundary(path, 500)

    def test_markdown_cap_600_independent_of_location(self):
        for path in ("docs/contract.md", "AGENTS.md", "tests/README.md"):
            self.assert_boundary(path, 600)

    def test_comments_and_blanks_still_count(self):
        code, out = check_files({"src/domain.rs": "// comment\n" * 200 + "\n" * 201})
        self.assertEqual(code, 1, out)
        self.assertIn("401 lines > 400", out)

    def test_forbidden_paths_still_fail(self):
        for path in ("src/helpers.rs", "tests/common/test_case.rs"):
            code, out = check_files({path: "// small\n"})
            self.assertEqual(code, 1, out)
            self.assertIn("forbidden:", out)

    def test_conflict_markers_still_fail(self):
        code, out = check_files({"docs/contract.md": "<<<<<<< HEAD\n"})
        self.assertEqual(code, 1, out)
        self.assertIn("Conflict markers: 1", out)

    def test_production_hygiene_still_fails(self):
        for text in ("fn bad() { value.unwrap(); }", "unsafe { action(); }"):
            code, out = check_files({"src/domain.rs": text})
            self.assertEqual(code, 1, out)
            self.assertIn("Rust production hygiene: 1", out)

    def test_child_tests_filename_does_not_waive_hygiene(self):
        code, out = check_files({"src/domain/tests.rs": "fn bad() { value.unwrap(); }"})
        self.assertEqual(code, 1, out)
        self.assertIn("Rust production hygiene: 1", out)

    def test_existing_test_hygiene_exemptions_preserved(self):
        for path, text in (
            ("tests/domain.rs", "fn test() { value.unwrap(); }"),
            ("src/domain.rs", "#[cfg(test)]\nmod tests { fn test() { value.unwrap(); } }"),
        ):
            code, out = check_files({path: text})
            self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()

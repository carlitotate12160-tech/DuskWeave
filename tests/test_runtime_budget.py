#!/usr/bin/env python3
"""Tests for scripts/check_runtime_budget.py in isolated Git fixtures."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCRIPT = REPO / "scripts" / "check_runtime_budget.py"


def run_git(repo: Path, *args: str) -> str:
    out = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True
    )
    assert out.returncode == 0, f"git {' '.join(args)}: {out.stderr}"
    return out.stdout


def run_budget(repo: Path, base: str, head: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--base", base, "--head", head, "--repo", str(repo)],
        capture_output=True,
        text=True,
    )
def load_checker():
    import importlib.util

    spec = importlib.util.spec_from_file_location("check_runtime_budget", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Fixture:
    """A fresh git repo with one base commit."""

    def __init__(self, tmp: Path):
        self.repo = tmp
        run_git(tmp, "init", "-q")
        run_git(tmp, "config", "user.email", "t@t")
        run_git(tmp, "config", "user.name", "t")
        (tmp / "README.md").write_text("x\n")
        run_git(tmp, "add", "README.md")
        run_git(tmp, "commit", "-qm", "base")
        self.base = run_git(tmp, "rev-parse", "HEAD").strip()

    def write(self, rel: str, text: str) -> None:
        p = self.repo / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def commit_all(self) -> str:
        run_git(self.repo, "add", "-A")
        run_git(self.repo, "commit", "-qm", "change")
        return run_git(self.repo, "rev-parse", "HEAD").strip()


class BudgetTests(unittest.TestCase):
    def make(self) -> tuple[tempfile.TemporaryDirectory, Fixture]:
        tmp = tempfile.TemporaryDirectory()
        return tmp, Fixture(Path(tmp.name))

    def test_oversized_diff_reports_actual_size_and_exits_zero(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("src/a.rs", "".join(f"// {i}\n" for i in range(600)))
            head = fx.commit_all()
            ok = run_budget(fx.repo, fx.base, head)
            self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)
            fx.write("src/a.rs", "".join(f"// {i}\n" for i in range(601)))
            head = fx.commit_all()
            oversize = run_budget(fx.repo, fx.base, head)
            self.assertEqual(oversize.returncode, 0, oversize.stdout)
            self.assertIn("total runtime changed lines: 601", oversize.stdout)
            self.assertIn("warning: runtime diff exceeds 600", oversize.stdout)
            self.assertIn("review_required=true threshold=400", oversize.stdout)

    def test_review_trigger_at_401_is_not_a_hard_failure(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("src/a.rs", "//\n" * 200)
            fx.write("src/b.rs", "//\n" * 200)
            head = fx.commit_all()
            normal = run_budget(fx.repo, fx.base, head)
            self.assertEqual(normal.returncode, 0, normal.stdout)
            self.assertNotIn("review_required=true", normal.stdout)
            fx.write("src/b.rs", "//\n" * 201)
            head = fx.commit_all()
            review = run_budget(fx.repo, fx.base, head)
            self.assertEqual(review.returncode, 0, review.stdout)
            self.assertIn("review_required=true threshold=400", review.stdout)
            self.assertIn("total runtime changed lines: 401", review.stdout)

    def test_binary_runtime_change_still_rejected(self) -> None:
        tmp, fx = self.make()
        with tmp:
            (fx.repo / "src").mkdir()
            (fx.repo / "src/a.rs").write_bytes(b"\x00\x01")
            head = fx.commit_all()
            res = run_budget(fx.repo, fx.base, head)
            self.assertEqual(res.returncode, 2, res.stdout)
            self.assertIn("binary production change", res.stdout)

    def test_deletions_count(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("src/a.rs", "".join(f"// {i}\n" for i in range(300)))
            mid = fx.commit_all()
            fx.write("src/a.rs", "".join(f"// {i}\n" for i in range(50)))
            head = fx.commit_all()
            res = run_budget(fx.repo, mid, head)  # 250 deletions vs mid commit
            self.assertEqual(res.returncode, 0)
            self.assertIn("250", res.stdout)

    def test_non_runtime_paths_not_counted(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("tests/big.rs", "//\n" * 900)
            fx.write("Cargo.lock", "x\n" * 900)
            fx.write("docs/big.md", "x\n" * 900)
            fx.write("src/a.rs", "// small\n" * 10)
            head = fx.commit_all()
            res = run_budget(fx.repo, fx.base, head)
            self.assertEqual(res.returncode, 0, res.stdout)
            self.assertIn("total runtime changed lines: 10", res.stdout)

    def test_sql_and_config_counted(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("migrations/0001.sql", "select 1;\n" * 200)
            fx.write("Cargo.toml", "x\n" * 150)
            fx.write("rust-toolchain.toml", "x\n" * 60)
            head = fx.commit_all()
            res = run_budget(fx.repo, fx.base, head)
            self.assertEqual(res.returncode, 0)
            self.assertIn("review_required=true", res.stdout)
            self.assertIn("410", res.stdout)

    def test_malformed_refs_rejected(self) -> None:
        tmp, fx = self.make()
        with tmp:
            res = run_budget(fx.repo, "no-such-ref", "HEAD")
            self.assertNotEqual(res.returncode, 0)
            res = run_budget(fx.repo, fx.base, "no-such-ref")
            self.assertNotEqual(res.returncode, 0)

    def test_index_includes_staged_new_files(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("src/new.rs", "//\n" * 601)
            run_git(fx.repo, "add", "src/new.rs")  # staged, uncommitted
            res = run_budget(fx.repo, fx.base, "INDEX")
            self.assertEqual(res.returncode, 0)
            self.assertIn("src/new.rs", res.stdout)
            self.assertIn("total runtime changed lines: 601", res.stdout)

    def test_mixed_runtime_and_tests_counts_only_runtime(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("src/a.rs", "//\n" * 300)
            fx.write("tests/t.rs", "//\n" * 300)
            fx.write("migrations/m.sql", "select 1;\n" * 120)
            head = fx.commit_all()
            res = run_budget(fx.repo, fx.base, head)
            self.assertEqual(res.returncode, 0)
            self.assertIn("review_required=true", res.stdout)
            self.assertIn("420", res.stdout)


if __name__ == "__main__":
    unittest.main()

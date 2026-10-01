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


def run_budget_inproc(repo: Path, base: str, head: str, pinned: str | None = None):
    """Runs the checker in-process; `pinned` substitutes M0A_BASE so a fixture
    base SHA can stand in for the real pinned EXPECTED_BASE_SHA."""
    import contextlib
    import io

    mod = load_checker()
    if pinned:
        mod.M0A_BASE = pinned
    buf = io.StringIO()
    argv = sys.argv
    sys.argv = ["check", "--base", base, "--head", head, "--repo", str(repo)]
    try:
        with contextlib.redirect_stdout(buf):
            try:
                rc = mod.main()
            except SystemExit as e:
                rc = e.code if isinstance(e.code, int) else 2
    finally:
        sys.argv = argv
    return rc, buf.getvalue()


RECORD = (
    "M0A_RUNTIME_DIFF_EXCEPTION: DW-IMPLEMENT-M0A; "
    "base=a9da047b2bf0bf4822536187ab3e1734ac56fc12; cap=1200; initial-only"
)


def stage_all(repo: Path) -> None:
    run_git(repo, "add", "-A")


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

    def test_exactly_600_passes_601_fails(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("src/a.rs", "".join(f"// {i}\n" for i in range(600)))
            head = fx.commit_all()
            ok = run_budget(fx.repo, fx.base, head)
            self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)
            fx.write("src/a.rs", "".join(f"// {i}\n" for i in range(601)))
            head = fx.commit_all()
            bad = run_budget(fx.repo, fx.base, head)
            self.assertEqual(bad.returncode, 1)

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
            self.assertEqual(res.returncode, 0)  # 410 requires review, not an exception
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
            self.assertEqual(res.returncode, 1)
            self.assertIn("src/new.rs", res.stdout)

    def test_mixed_runtime_and_tests_counts_only_runtime(self) -> None:
        tmp, fx = self.make()
        with tmp:
            fx.write("src/a.rs", "//\n" * 300)
            fx.write("tests/t.rs", "//\n" * 300)
            fx.write("migrations/m.sql", "select 1;\n" * 120)
            head = fx.commit_all()
            res = run_budget(fx.repo, fx.base, head)
            self.assertEqual(res.returncode, 0)  # 420 counted, tests excluded
            self.assertIn("review_required=true", res.stdout)
            self.assertIn("420", res.stdout)

    def write_map(self, fx: Fixture, lines: int) -> None:
        """Spread `lines` across in-map runtime files; QUALITY_BAR carries the record."""
        fx.write("QUALITY_BAR.md", RECORD + "\n")
        for name in ("src/main.rs", "src/mission.rs"):
            fx.write(name, "//\n" * (lines // 2))
        fx.write("migrations/0001_mission_registration.sql", "--\n" * (lines - 2 * (lines // 2)))

    def test_eligible_1200_passes_1201_fails(self) -> None:
        tmp, fx = self.make()
        with tmp:
            self.write_map(fx, 1200)
            head = fx.commit_all()
            rc, out = run_budget_inproc(fx.repo, fx.base, head, pinned=fx.base)
            self.assertEqual(rc, 0, out)
            self.assertIn("policy=m0a_exception cap=1200", out)
            self.write_map(fx, 1201)
            head = fx.commit_all()
            rc, out = run_budget_inproc(fx.repo, fx.base, head, pinned=fx.base)
            self.assertEqual(rc, 1)
            self.assertIn("1201", out)

    def test_wrong_base_uses_default_cap(self) -> None:
        tmp, fx = self.make()
        with tmp:
            self.write_map(fx, 100)
            mid = fx.commit_all()  # record committed here; mid is a wrong base
            fx.write("src/main.rs", "".join(f"// {i}\n" for i in range(650)))
            head = fx.commit_all()
            rc, out = run_budget_inproc(fx.repo, mid, head, pinned=fx.base)
            self.assertEqual(rc, 1)  # cumulative replacements exceed default 600
            self.assertIn("policy=default cap=600", out)

    def test_stale_previous_base_uses_default_cap(self) -> None:
        # A superseded baseline must not qualify: the record pins exactly
        # one protected-master base; evaluating against the prior base is
        # indistinguishable from any other wrong base.
        tmp, fx = self.make()
        with tmp:
            self.write_map(fx, 601)
            mid = fx.commit_all()  # mid stands in for the newer pinned base
            # Candidate contains the exact record + in-map paths, but the
            # resolved base is the superseded commit, not the pinned one.
            rc, out = run_budget_inproc(fx.repo, fx.base, mid, pinned=mid)
            self.assertEqual(rc, 1)
            self.assertIn("policy=default cap=600", out)

    def test_absent_or_malformed_record_uses_default(self) -> None:
        tmp, fx = self.make()
        with tmp:
            # No record at all.
            fx.write("src/main.rs", "//\n" * 601)
            head = fx.commit_all()
            rc, out = run_budget_inproc(fx.repo, fx.base, head, pinned=fx.base)
            self.assertEqual(rc, 1)
            self.assertIn("policy=default", out)
            # Malformed record (wrong cap field) still ineligible.
            fx.write("QUALITY_BAR.md", RECORD.replace("cap=1200", "cap=800") + "\n")
            head = fx.commit_all()
            rc, out = run_budget_inproc(fx.repo, fx.base, head, pinned=fx.base)
            self.assertEqual(rc, 1)
            self.assertIn("policy=default", out)
            # Duplicate records are invalid even if one is exact.
            fx.write("QUALITY_BAR.md", RECORD + "\n" + RECORD + "\n")
            head = fx.commit_all()
            rc, out = run_budget_inproc(fx.repo, fx.base, head, pinned=fx.base)
            self.assertEqual(rc, 1)
            self.assertIn("policy=default", out)

    def test_worktree_record_not_in_index_is_ineligible(self) -> None:
        tmp, fx = self.make()
        with tmp:
            self.write_map(fx, 601)
            run_git(fx.repo, "add", "src/main.rs", "src/mission.rs")
            run_git(fx.repo, "add", "migrations/0001_mission_registration.sql")
            # QUALITY_BAR.md intentionally left unstaged: INDEX lacks the record.
            rc, out = run_budget_inproc(fx.repo, fx.base, "INDEX", pinned=fx.base)
            self.assertEqual(rc, 1)
            self.assertIn("policy=default", out)
            stage_all(fx.repo)
            rc, out = run_budget_inproc(fx.repo, fx.base, "INDEX", pinned=fx.base)
            self.assertEqual(rc, 0, out)  # eligible -> cap 1200 -> 601 passes
            self.assertIn("policy=m0a_exception", out)

    def test_out_of_map_runtime_path_is_ineligible(self) -> None:
        tmp, fx = self.make()
        with tmp:
            self.write_map(fx, 300)
            fx.write("src/extra.rs", "//\n" * 350)
            head = fx.commit_all()
            rc, out = run_budget_inproc(fx.repo, fx.base, head, pinned=fx.base)
            self.assertEqual(rc, 1)  # 650 total vs default 600
            self.assertIn("policy=default", out)


if __name__ == "__main__":
    unittest.main()

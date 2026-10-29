#!/usr/bin/env python3
"""Tests for scripts/check_file_loc.py: real CLI in isolated Git fixtures."""

import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCRIPT = REPO / "scripts" / "check_file_loc.py"
WORKFLOW = REPO / ".github" / "workflows" / "doc-check.yml"


def git(repo, *args, check=True, stdin=None):
    out = subprocess.run(["git", "-C", str(repo), *args], input=stdin,
                         capture_output=True, text=True)
    if check:
        assert out.returncode == 0, f"git {' '.join(args)}: {out.stderr}"
    return out


def run_loc(repo, base, head=None):
    argv = [sys.executable, str(SCRIPT), "--base", base, "--repo", str(repo)]
    if head is not None:
        argv += ["--head", head]
    return subprocess.run(argv, capture_output=True, text=True)


def write_text(repo, rel, text):
    path = repo / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def write_lines(repo, rel, count, eol="\n", final=True):
    body = eol.join(f"line {i}" for i in range(count))
    write_text(repo, rel, body + (eol if final and count else ""))


class Fixture:
    def __init__(self, root: Path):
        self.repo = root
        git(root, "init", "-q", "-b", "master")
        git(root, "config", "user.email", "t@t")
        git(root, "config", "user.name", "t")
        write_text(root, "README.md", "x\n")
        git(root, "add", "README.md")
        git(root, "commit", "-qm", "base")
        self.base = git(root, "rev-parse", "HEAD").stdout.strip()

    def lines(self, rel, count, **kw):
        write_lines(self.repo, rel, count, **kw)

    def commit_all(self):
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "change")
        return git(self.repo, "rev-parse", "HEAD").stdout.strip()

    def rebase(self):
        """Measure the next change against the just-committed tree."""
        self.base = git(self.repo, "rev-parse", "HEAD").stdout.strip()

    def loc(self, head=None):
        return run_loc(self.repo, self.base, head)


class LocTest(unittest.TestCase):
    def make(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        return Fixture(Path(tmp.name))


class ThresholdBoundaries(LocTest):
    def test_new_production_file_400_passes_401_fails(self):
        fx = self.make()
        fx.lines("src/a.rs", 401)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("NEW_OVER", res.stdout)
        self.assertIn("src/a.rs", res.stdout)
        fx.lines("src/a.rs", 400)
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
        self.assertIn("WITHIN", res.stdout)

    def test_test_and_benchmark_files_get_500(self):
        for rel in ("tests/test_x.py", "tests/t.rs", "benches/b.rs",
                    "src/domain/tests.rs"):
            with self.subTest(rel=rel):
                fx = self.make()
                fx.lines(rel, 500)
                self.assertEqual(fx.loc().returncode, 0)
                fx.lines(rel, 501)
                res = fx.loc()
                self.assertEqual(res.returncode, 1, rel)
                self.assertIn("NEW_OVER", res.stdout)

    def test_compliant_file_crossing_threshold_fails(self):
        fx = self.make()
        fx.lines("src/a.rs", 400)
        fx.commit_all()
        fx.rebase()
        fx.lines("src/a.rs", 401)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("CROSSED", res.stdout)

    def test_growth_up_to_threshold_passes(self):
        fx = self.make()
        fx.lines("src/a.rs", 300)
        fx.commit_all()
        fx.lines("src/a.rs", 400)
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertIn("WITHIN", res.stdout)

    def test_tooling_paths_use_production_threshold(self):
        for rel in ("scripts/tool.py", "tools/x.sh"):
            with self.subTest(rel=rel):
                fx = self.make()
                fx.lines(rel, 401)
                self.assertEqual(fx.loc().returncode, 1, rel)


class DebtRatchet(LocTest):
    def test_retained_same_count_passes(self):
        fx = self.make()
        fx.lines("src/big.rs", 450)
        fx.commit_all()
        fx.rebase()
        write_text(fx.repo, "src/big.rs",
                   "".join(f"changed {i}\n" for i in range(450)))
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertIn("RETAINED", res.stdout)

    def test_smaller_oversized_is_improved(self):
        fx = self.make()
        for before, after in ((450, 449), (450, 400)):
            with self.subTest(after=after):
                write_text(fx.repo, "src/big.rs", "".join(
                    f"v{after} line {i}\n" for i in range(before)))
                fx.commit_all()
                fx.rebase()
                fx.lines("src/big.rs", after)
                res = fx.loc()
                self.assertEqual(res.returncode, 0, res.stdout)
                self.assertIn("IMPROVED", res.stdout)

    def test_oversized_growth_fails(self):
        fx = self.make()
        fx.lines("src/big.rs", 450)
        fx.commit_all()
        fx.rebase()
        fx.lines("src/big.rs", 451)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("REGRESSED", res.stdout)

    def test_one_growth_fails_despite_larger_shrink(self):
        fx = self.make()
        fx.lines("src/grow.rs", 500)
        fx.lines("src/shrink.rs", 500)
        fx.commit_all()
        fx.rebase()
        fx.lines("src/grow.rs", 505)
        fx.lines("src/shrink.rs", 100)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("REGRESSED", res.stdout)
        self.assertIn("src/grow.rs", res.stdout)

    def test_deletion_reported_and_passes_unstaged_and_staged(self):
        for staged in (False, True):
            fx = self.make()
            fx.lines("src/old.rs", 450)
            fx.commit_all()
            fx.rebase()
            if staged:
                git(fx.repo, "rm", "-q", "src/old.rs")
            else:
                (fx.repo / "src/old.rs").unlink()
            res = fx.loc()
            self.assertEqual(res.returncode, 0, res.stdout)
            self.assertIn("DELETED", res.stdout)


class WorktreeModes(LocTest):
    def test_staged_new_over_file_fails(self):
        fx = self.make()
        fx.lines("src/new.rs", 401)
        git(fx.repo, "add", "src/new.rs")
        self.assertEqual(fx.loc().returncode, 1)

    def test_unstaged_modification_fails(self):
        fx = self.make()
        fx.lines("src/a.rs", 400)
        fx.commit_all()
        fx.lines("src/a.rs", 401)  # not staged
        self.assertEqual(fx.loc().returncode, 1)

    def test_nonignored_untracked_fails_ignored_does_not(self):
        fx = self.make()
        write_text(fx.repo, ".gitignore", "ignored/\n")
        fx.lines("ignored/big.rs", 500)
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertNotIn("ignored/big.rs", res.stdout)
        fx.lines("src/new.rs", 401)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("src/new.rs", res.stdout)

    def test_head_mode_ignores_dirty_worktree(self):
        fx = self.make()
        fx.lines("src/ok.rs", 10)
        head = fx.commit_all()
        fx.lines("src/dirty.rs", 999)
        self.assertEqual(fx.loc(head=head).returncode, 0)
        res = fx.loc()
        self.assertEqual(res.returncode, 1)
        self.assertIn("src/dirty.rs", res.stdout)

    def test_assume_unchanged_flag_does_not_hide_growth(self):
        fx = self.make()
        fx.lines("src/big.rs", 450)
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "update-index", "--assume-unchanged", "src/big.rs")
        fx.lines("src/big.rs", 451)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("src/big.rs", res.stdout)

    def test_symlinked_parent_dir_is_measurement_error(self):
        fx = self.make()
        fx.lines("src/real.rs", 5)
        fx.commit_all()
        fx.rebase()
        (fx.repo / "src/real.rs").unlink()
        (fx.repo / "src").rmdir()
        write_lines(fx.repo, "outside/real.rs", 5)
        try:
            (fx.repo / "src").symlink_to(fx.repo / "outside",
                                         target_is_directory=True)
        except OSError:
            self.skipTest("symlink privilege unavailable")
        res = fx.loc()
        self.assertEqual(res.returncode, 2, res.stdout + res.stderr)


class BranchDepth(LocTest):
    def test_earlier_commit_regression_caught_against_base(self):
        fx = self.make()
        fx.lines("src/big.rs", 401)
        fx.commit_all()
        fx.lines("src/other.rs", 5)  # unrelated tip commit
        head = fx.commit_all()
        for mode in ({"head": head}, {}):
            res = fx.loc(**mode)
            self.assertEqual(res.returncode, 1, mode)
            self.assertIn("NEW_OVER", res.stdout)
            self.assertIn("src/big.rs", res.stdout)


class Renames(LocTest):
    def test_pure_same_category_rename_keeps_retained_debt(self):
        fx = self.make()
        fx.lines("src/old.py", 450)
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/old.py", "src/new.py")
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertIn("RETAINED", res.stdout)
        self.assertIn("src/new.py", res.stdout)

    def test_copied_debt_gets_no_allowance(self):
        fx = self.make()
        fx.lines("src/old.py", 450)
        fx.commit_all()
        fx.rebase()
        fx.lines("src/copy.py", 450)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("NEW_OVER", res.stdout)

    def test_modified_rename_is_new_file(self):
        fx = self.make()
        fx.lines("src/old.py", 450)
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/old.py", "src/new.py")
        fx.lines("src/new.py", 451)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("NEW_OVER", res.stdout)

    def test_ambiguous_rename_identity_gets_no_allowance(self):
        fx = self.make()
        fx.lines("src/old.py", 450)
        fx.commit_all()
        fx.rebase()
        (fx.repo / "src/old.py").unlink()
        for dest in ("src/b.py", "src/c.py"):
            fx.lines(dest, 450)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertEqual(res.stdout.count("NEW_OVER"), 2)

    def test_category_changing_move_gets_no_allowance(self):
        fx = self.make()
        fx.lines("tests/t.py", 450)
        fx.commit_all()
        fx.rebase()
        (fx.repo / "src").mkdir()
        git(fx.repo, "mv", "tests/t.py", "src/t.py")
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("NEW_OVER", res.stdout)
        self.assertIn("src/t.py", res.stdout)

    def test_commit_mode_rename_allowance(self):
        fx = self.make()
        fx.lines("src/old.py", 450)
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/old.py", "src/new.py")
        head = fx.commit_all()
        res = fx.loc(head=head)
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertIn("RETAINED", res.stdout)

    def test_rename_of_compliant_file_stays_within(self):
        fx = self.make()
        fx.lines("src/a.py", 300)
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/a.py", "src/b.py")
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertIn("WITHIN", res.stdout)


class InputsAndErrors(LocTest):
    def test_empty_and_no_final_newline_counted_honestly(self):
        fx = self.make()
        fx.lines("src/empty.rs", 0)
        write_text(fx.repo, "src/nolf.rs", "\n".join(f"l{i}" for i in range(400)))
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
        write_text(fx.repo, "src/nolf.rs", "\n".join(f"l{i}" for i in range(401)))
        self.assertEqual(fx.loc().returncode, 1)

    def test_crlf_lines_counted(self):
        fx = self.make()
        fx.lines("src/win.rs", 400, eol="\r\n")
        self.assertEqual(fx.loc().returncode, 0)
        fx.lines("src/win.rs", 401, eol="\r\n")
        self.assertEqual(fx.loc().returncode, 1)

    def test_whitespace_bearing_path(self):
        fx = self.make()
        fx.lines("src/dir with space/a.rs", 401)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("src/dir with space/a.rs", res.stdout)

    def test_malformed_or_missing_base_is_invalid(self):
        fx = self.make()
        for bad in ("no-such-ref", "0" * 40, "HEAD^{tree}", ""):
            with self.subTest(bad=bad):
                res = run_loc(fx.repo, bad)
                self.assertEqual(res.returncode, 2, bad)
                self.assertIn("error", res.stderr.lower())

    def test_invalid_utf8_in_scope_is_measurement_error(self):
        fx = self.make()
        (fx.repo / "src").mkdir()
        (fx.repo / "src/bad.rs").write_bytes(b"\xff\xfe\x00binary")
        res = fx.loc()
        self.assertEqual(res.returncode, 2, res.stdout)
        self.assertIn("src/bad.rs", res.stderr)
        head = fx.commit_all()
        res = fx.loc(head=head)
        self.assertEqual(res.returncode, 2)

    def test_nonregular_committed_entries_are_errors(self):
        for mode, rel in (("120000", "src/link.rs"),
                          ("160000", "src/sub.rs")):
            fx = self.make()
            oid = fx.base if mode == "160000" else git(
                fx.repo, "hash-object", "-w", "--stdin",
                stdin="payload\n").stdout.strip()
            git(fx.repo, "update-index", "--add", "--cacheinfo",
                f"{mode},{oid},{rel}")
            git(fx.repo, "commit", "-qm", "nonregular")
            res = run_loc(fx.repo, fx.base, "HEAD")
            self.assertEqual(res.returncode, 2, mode)

    def test_missing_git_object_is_measurement_error(self):
        fx = self.make()
        fx.lines("src/gone.rs", 5)
        head = fx.commit_all()
        oid = git(fx.repo, "rev-parse", "HEAD:src/gone.rs").stdout.strip()
        obj = fx.repo / ".git/objects" / oid[:2] / oid[2:]
        obj.chmod(0o644)
        obj.unlink()
        res = run_loc(fx.repo, fx.base, head)
        self.assertEqual(res.returncode, 2, res.stdout + res.stderr)

    def test_missing_or_nongit_repo_is_invalid(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        plain = Path(tmp.name) / "plain"
        plain.mkdir()
        for target in (plain, Path(tmp.name) / "absent"):
            res = run_loc(target, "HEAD")
            self.assertEqual(res.returncode, 2, str(target))


class ScopeAndAdvisory(LocTest):
    def test_markdown_and_manifest_changes_are_advisory_only(self):
        fx = self.make()
        fx.lines("docs/big.md", 700)
        fx.lines("Cargo.toml", 700)
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)

    def test_migration_sql_is_advisory_first_party_sql_is_not(self):
        fx = self.make()
        fx.lines("migrations/0099_new.sql", 500)
        self.assertEqual(fx.loc().returncode, 0)
        fx.lines("scripts/query.sql", 401)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("scripts/query.sql", res.stdout)

    def test_pinned_genai_excluded_unknown_vendor_is_not(self):
        fx = self.make()
        fx.lines("third_party/genai/src/lib.rs", 500)
        self.assertEqual(fx.loc().returncode, 0)
        fx.lines("third_party/other/lib.rs", 401)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("third_party/other/lib.rs", res.stdout)

    def test_all_enforced_extensions(self):
        fx = self.make()
        exts = (".go", ".zig", ".c", ".h", ".cc", ".cpp", ".hpp",
                ".ps1", ".sh")
        for ext in exts:
            fx.lines(f"src/x{ext}", 401)
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        for ext in exts:
            self.assertIn(f"src/x{ext}", res.stdout)


class WorkflowWiring(LocTest):
    """Metrics job runs the LOC gate on its resolved base and stays required."""

    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")
        match = re.search(r"^  metrics:\n(.*?)(?=^  \w)", cls.text,
                          re.M | re.S)
        assert match, "metrics job not found"
        cls.metrics = match.group(1)

    def test_metrics_invokes_loc_gate_on_resolved_base(self):
        body = self.metrics
        command = 'python scripts/check_file_loc.py --base "${{ steps.base.outputs.base }}"'
        self.assertIn(command, body)
        self.assertLess(body.index("Resolve diff base"), body.index(command))
        self.assertLess(body.index(command),
                        body.index("Install pinned rust-code-analysis"))
        # One shared base resolution: no second event-base resolver.
        self.assertEqual(body.count("pull_request.base.sha"), 1)

    def test_metrics_preserves_exit_and_appends_summary(self):
        body = self.metrics
        step = body[body.index("check_file_loc.py"):]
        self.assertIn("GITHUB_STEP_SUMMARY", step[: step.index("exit ")])
        self.assertIn('exit "$rc"', step)

    def test_metrics_runs_loc_unit_tests_and_stays_required(self):
        self.assertIn("'test_file_loc*.py'", self.metrics)
        ci_ok = re.search(r"^  ci-ok:\n(.*?)(?=^  \w|\Z)", self.text,
                          re.M | re.S).group(1)
        self.assertIn("metrics", ci_ok)


if __name__ == "__main__":
    unittest.main()

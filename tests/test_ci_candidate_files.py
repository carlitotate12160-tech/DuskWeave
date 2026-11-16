#!/usr/bin/env python3
"""Unit tests for candidate_files custody, ancestry, full-buffer writes, and driver pins."""

from __future__ import annotations

import errno
import io
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

QUALIFY_DIR = Path(__file__).resolve().parents[1] / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import candidate_files  # noqa: E402


def _make_tree(root: Path, mode: int = 0o755) -> Path:
    sub = root / "a" / "b"
    sub.mkdir(parents=True, mode=mode)
    for d in (root, root / "a", sub):
        os.chmod(d, mode)
    return sub


class CandidateFilesTests(unittest.TestCase):
    """Verify full-ancestor custody, O_NOFOLLOW/nlink guards, short-write loops, and driver pins."""

    def test_require_safe_ancestors_rejects_symlink_and_writable_parent(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "safe"
            sub = _make_tree(base)
            candidate_files.require_safe_ancestors(sub, (uid,), stop_at=base)

            os.chmod(base / "a", 0o777)
            with self.assertRaises(candidate_files.FileCustodyError) as ctx:
                candidate_files.require_safe_ancestors(sub, (uid,), stop_at=base)
            self.assertEqual(ctx.exception.reason.split(":")[0], "unprotected_directory")
            os.chmod(base / "a", 0o755)

            link = Path(tmp) / "link"
            link.symlink_to(base / "a")
            with self.assertRaises(candidate_files.FileCustodyError) as ctx2:
                candidate_files.require_safe_ancestors(link / "b", (uid,), stop_at=link)
            self.assertEqual(ctx2.exception.reason.split(":")[0], "unsafe_ancestor_type")

    def test_require_safe_ancestors_respects_stop_at_scope(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "outer"
            sub = _make_tree(base)
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.require_safe_ancestors(sub, (uid,), stop_at=Path("/"))
            candidate_files.require_safe_ancestors(sub, (uid,), stop_at=base)

    def test_read_protected_bytes_guards_nlink_symlink_mode_and_size(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "root"
            sub = _make_tree(base)
            target = sub / "secret.json"
            target.write_bytes(b'{"ok":true}\n')
            os.chmod(target, 0o400)
            self.assertEqual(
                candidate_files.read_protected_bytes(target, expected_uids=(uid,), allowed_modes=(0o400,), stop_at=base),
                b'{"ok":true}\n',
            )
            with self.assertRaises(candidate_files.FileCustodyError) as mode_err:
                candidate_files.read_protected_bytes(target, expected_uids=(uid,), allowed_modes=(0o444,), stop_at=base)
            self.assertEqual(mode_err.exception.reason, "unexpected_file_mode:secret.json")

            hard = sub / "hard"
            os.link(target, hard)
            with self.assertRaises(candidate_files.FileCustodyError) as nlink_err:
                candidate_files.read_protected_bytes(target, expected_uids=(uid,), stop_at=base)
            self.assertEqual(nlink_err.exception.reason, "unsafe_regular_file:secret.json")
            hard.unlink()

            sym = sub / "sym"
            sym.symlink_to(target)
            with self.assertRaises(candidate_files.FileCustodyError) as sym_err:
                candidate_files.read_protected_bytes(sym, expected_uids=(uid,), stop_at=base)
            self.assertEqual(sym_err.exception.reason, "protected_open_failed:sym")

            with self.assertRaises(candidate_files.FileCustodyError) as lim_err:
                candidate_files.read_protected_bytes(target, expected_uids=(uid,), max_bytes=4, stop_at=base)
            self.assertEqual(lim_err.exception.reason, "file_exceeds_limit:secret.json")

    def test_write_exclusive_handles_partial_writes_eintr_and_zero_progress(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "w"
            base.mkdir(mode=0o755)
            os.chmod(base, 0o755)
            calls = []

            def flaky_writer(fd: int, buf: memoryview) -> int:
                calls.append(len(buf))
                if len(calls) == 2:
                    raise InterruptedError()
                return os.write(fd, buf[:3])

            p1 = base / "partial.txt"
            digest = candidate_files.write_exclusive_verified(
                p1, b"abcdef0123", mode=0o400, expected_uids=(uid,), stop_at=base, writer=flaky_writer,
            )
            self.assertEqual(p1.read_bytes(), b"abcdef0123")
            self.assertEqual(len(digest), 64)
            self.assertGreaterEqual(len(calls), 4)

            p2 = base / "zero.txt"

            def zero_writer(fd: int, buf: memoryview) -> int:
                os.write(fd, buf[:2])
                return 0 if len(buf) < len(b"abcdef") else 2

            calls.clear()

            def second_zero(fd: int, buf: memoryview) -> int:
                calls.append(1)
                return os.write(fd, buf[:2]) if len(calls) == 1 else 0

            with self.assertRaises(candidate_files.FileCustodyError) as z_err:
                candidate_files.write_exclusive_verified(
                    p2, b"abcdef", mode=0o400, expected_uids=(uid,), stop_at=base, writer=second_zero,
                )
            self.assertEqual(z_err.exception.status, "INCOMPLETE")
            self.assertEqual(z_err.exception.reason, "custody_short_write_zero_progress")
            self.assertEqual(p2.read_bytes(), b"ab")

    def test_write_exclusive_fsync_failure_and_publish_atomic(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "p"
            base.mkdir(mode=0o755)
            os.chmod(base, 0o755)
            out = base / "state.json"

            def failing_fsync(_fd: int) -> None:
                raise OSError(errno.EIO, "disk error")

            with self.assertRaises(candidate_files.FileCustodyError) as f_err:
                candidate_files.write_exclusive_verified(
                    base / "bad.json", b"{}", mode=0o400, expected_uids=(uid,), stop_at=base, fsync_fn=failing_fsync,
                )
            self.assertEqual(f_err.exception.status, "INCOMPLETE")
            self.assertEqual(f_err.exception.reason, f"custody_fsync_or_chmod_failed:{errno.EIO}")

            candidate_files.publish_atomic_verified(out, b'{"v":1}\n', mode=0o444, expected_uids=(uid,), stop_at=base)
            d2 = candidate_files.publish_atomic_verified(out, b'{"v":2}\n', mode=0o444, expected_uids=(uid,), stop_at=base)
            self.assertEqual(out.read_bytes(), b'{"v":2}\n')
            self.assertEqual(len(d2), 64)

            sym = base / "badlink"
            sym.symlink_to(out)
            with self.assertRaises(candidate_files.FileCustodyError) as s_err:
                candidate_files.publish_atomic_verified(sym, b"x", mode=0o444, expected_uids=(uid,), stop_at=base)
            self.assertEqual(s_err.exception.reason, "symlink_destination_refused:badlink")

    def test_fsync_dir_normalizes_open_sync_and_close_oserror_preserving_cleanup(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "dir_fail"
            base.mkdir(mode=0o755)
            os.chmod(base, 0o755)
            secret_msg = "/secret/custody/path"

            missing = base / "nonexistent"
            with self.assertRaises(candidate_files.FileCustodyError) as open_err:
                candidate_files.fsync_dir(missing)
            self.assertEqual(open_err.exception.status, "INCOMPLETE")
            self.assertTrue(open_err.exception.reason.startswith("custody_dir_fsync_failed:"))
            self.assertIsInstance(open_err.exception.__cause__, OSError)

            with mock.patch("candidate_files.os.open", side_effect=OSError(errno.EACCES, secret_msg)):
                with self.assertRaises(candidate_files.FileCustodyError) as m_open_err:
                    candidate_files.fsync_dir(base)
            self.assertEqual(m_open_err.exception.status, "INCOMPLETE")
            self.assertEqual(m_open_err.exception.reason, f"custody_dir_fsync_failed:{errno.EACCES}")
            self.assertNotIn(secret_msg, m_open_err.exception.reason)
            self.assertNotIn(secret_msg, str(m_open_err.exception))
            self.assertIsInstance(m_open_err.exception.__cause__, OSError)

            closed = []
            real_close = os.close

            def track_close(fd: int) -> None:
                closed.append(fd)
                real_close(fd)

            def sync_fail(_fd: int) -> None:
                raise OSError(errno.EIO, secret_msg)

            with mock.patch("candidate_files.os.close", side_effect=track_close):
                with self.assertRaises(candidate_files.FileCustodyError) as sync_err:
                    candidate_files.fsync_dir(base, fsync_fn=sync_fail)
            self.assertEqual(len(closed), 1)
            self.assertEqual(sync_err.exception.status, "INCOMPLETE")
            self.assertEqual(sync_err.exception.reason, f"custody_dir_fsync_failed:{errno.EIO}")
            self.assertNotIn(secret_msg, sync_err.exception.reason)
            self.assertIsInstance(sync_err.exception.__cause__, OSError)

            def close_fail(fd: int) -> None:
                real_close(fd)
                raise OSError(errno.EIO, secret_msg)

            with mock.patch("candidate_files.os.close", side_effect=close_fail):
                with self.assertRaises(candidate_files.FileCustodyError) as close_err:
                    candidate_files.fsync_dir(base, fsync_fn=lambda _fd: None)
            self.assertEqual(close_err.exception.status, "INCOMPLETE")
            self.assertEqual(close_err.exception.reason, f"custody_dir_fsync_failed:{errno.EIO}")
            self.assertNotIn(secret_msg, close_err.exception.reason)

            closed.clear()
            with mock.patch("candidate_files.os.close", side_effect=track_close):
                with self.assertRaises(ValueError):
                    candidate_files.fsync_dir(base, fsync_fn=lambda _fd: (_ for _ in ()).throw(ValueError("bug")))
            self.assertEqual(len(closed), 1)

            sync_calls = 0

            def dir_sync_only_fail(_fd: int) -> None:
                nonlocal sync_calls
                sync_calls += 1
                if sync_calls == 2:
                    raise OSError(errno.EIO, secret_msg)

            with self.assertRaises(candidate_files.FileCustodyError) as prop_err:
                candidate_files.write_exclusive_verified(
                    base / "dir_sync.json", b"{}", mode=0o400, expected_uids=(uid,),
                    stop_at=base, fsync_fn=dir_sync_only_fail,
                )
            self.assertEqual(sync_calls, 2)
            self.assertEqual(prop_err.exception.status, "INCOMPLETE")
            self.assertEqual(prop_err.exception.reason, f"custody_dir_fsync_failed:{errno.EIO}")

    def test_verify_driver_custody_validates_files_git_pins_and_clean_tree(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "repo"
            qdir = repo / "boundary" / "qualify"
            sdir = repo / "tests" / "support"
            qdir.mkdir(parents=True, mode=0o755)
            sdir.mkdir(parents=True, mode=0o755)
            for d in (repo, repo / "boundary", qdir, repo / "tests", sdir):
                os.chmod(d, 0o755)
            for name in candidate_files.DRIVER_QUALIFY_FILES:
                f = qdir / name
                f.write_text("#!/bin/sh\n", encoding="utf-8")
                os.chmod(f, 0o444)
            boot = sdir / "m1_native_bootstrap.py"
            boot.write_text("# boot\n", encoding="utf-8")
            os.chmod(boot, 0o444)

            env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
            subprocess.run(["git", "-C", str(repo), "init", "-q"], check=True, env=env)
            subprocess.run(["git", "-C", str(repo), "config", "user.email", "t@e.invalid"], check=True, env=env)
            subprocess.run(["git", "-C", str(repo), "config", "user.name", "Test"], check=True, env=env)
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True, env=env)
            subprocess.run(["git", "-C", str(repo), "commit", "-q", "-m", "init"], check=True, env=env)
            head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True, env=env).strip()
            tree = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD^{tree}"], text=True, env=env).strip()

            candidate_files.verify_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)

            with self.assertRaises(candidate_files.FileCustodyError) as pin_err:
                candidate_files.verify_driver_custody(qdir, "0" * 40, tree, expected_uids=(uid,), stop_at=repo)
            self.assertEqual(pin_err.exception.reason, "driver_git_pin_mismatch")

            (repo / ".cargo").mkdir(mode=0o755)
            with self.assertRaises(candidate_files.FileCustodyError) as dirty_err:
                candidate_files.verify_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)
            self.assertEqual(dirty_err.exception.reason, "driver_worktree_unclean_or_cargo_present")
            (repo / ".cargo").rmdir()

            os.unlink(qdir / "candidate_install.py")
            with self.assertRaises(candidate_files.FileCustodyError) as miss_err:
                candidate_files.verify_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)
            self.assertEqual(miss_err.exception.reason, "protected_open_failed:candidate_install.py")


if __name__ == "__main__":
    unittest.main()

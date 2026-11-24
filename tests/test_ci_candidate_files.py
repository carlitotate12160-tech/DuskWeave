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
import candidate_install_files  # noqa: E402
import https_lab_host  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
MUTANTS_TOML_BYTES = (ROOT / ".cargo" / "mutants.toml").read_bytes()


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

            self.assertFalse(hasattr(candidate_files, "publish_atomic_verified"))
            # Relocated publisher preserves short-write handling, replace failure, and symlink refusal.
            w_calls = []

            def partial_writer(fd: int, buf: memoryview) -> int:
                w_calls.append(len(buf))
                return os.write(fd, buf[:4])

            candidate_install_files.publish_atomic_verified(
                out, b'{"v":1}\n', mode=0o444, expected_uids=(uid,), stop_at=base, writer=partial_writer,
            )
            self.assertGreaterEqual(len(w_calls), 2)
            d2 = candidate_install_files.publish_atomic_verified(out, b'{"v":2}\n', mode=0o444, expected_uids=(uid,), stop_at=base)
            self.assertEqual(out.read_bytes(), b'{"v":2}\n')
            self.assertEqual(len(d2), 64)

            def bad_replace(_s: Path, _d: Path) -> None:
                raise OSError(errno.EIO, "replace error")

            with self.assertRaises(candidate_files.FileCustodyError) as r_err:
                candidate_install_files.publish_atomic_verified(
                    out, b'{"v":3}\n', mode=0o444, expected_uids=(uid,), stop_at=base, replacer=bad_replace,
                )
            self.assertEqual(r_err.exception.status, "INCOMPLETE")
            self.assertEqual(out.read_bytes(), b'{"v":2}\n')

            sym = base / "badlink"
            sym.symlink_to(out)
            with self.assertRaises(candidate_files.FileCustodyError) as s_err:
                candidate_install_files.publish_atomic_verified(sym, b"x", mode=0o444, expected_uids=(uid,), stop_at=base)
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

    def _init_driver_repo(self, repo: Path, *, extra_files: dict[str, bytes] | None = None, exec_cargo: bool = False) -> tuple[Path, str, str]:
        qdir, sdir = repo / "boundary" / "qualify", repo / "tests" / "support"
        qdir.mkdir(parents=True, mode=0o755)
        sdir.mkdir(parents=True, mode=0o755)
        for d in (repo, repo / "boundary", qdir, repo / "tests", sdir):
            os.chmod(d, 0o755)
        for name in set(candidate_files.DRIVER_QUALIFY_FILES) | set(https_lab_host.HTTPS_LAB_DRIVER_FILES):
            f = qdir / name
            if name in ("deployment_custody.sh", "candidate_files.py"):
                f.write_text((QUALIFY_DIR / name).read_text(encoding="utf-8"), encoding="utf-8")
            else:
                f.write_text("#!/bin/sh\n", encoding="utf-8")
            os.chmod(f, 0o444)
        for sname in ("m1_native_bootstrap.py", "m1_native_migrations.py"):
            sf = sdir / sname
            sf.write_text("# support\n", encoding="utf-8")
            os.chmod(sf, 0o444)
        for rel, data in (extra_files or {}).items():
            p = repo / rel
            p.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
            os.chmod(p.parent, 0o755)
            p.write_bytes(data)
            os.chmod(p, 0o755 if (exec_cargo and rel.startswith(".cargo/")) else 0o444)
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
        subprocess.run(["git", "-C", str(repo), "init", "-q"], check=True, env=env)
        subprocess.run(["git", "-C", str(repo), "config", "user.email", "t@e.invalid"], check=True, env=env)
        subprocess.run(["git", "-C", str(repo), "config", "user.name", "Test"], check=True, env=env)
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True, env=env)
        subprocess.run(["git", "-C", str(repo), "commit", "-q", "-m", "init"], check=True, env=env)
        head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True, env=env).strip()
        tree = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD^{tree}"], text=True, env=env).strip()
        return qdir, head, tree

    def test_tracked_mutants_toml_and_cargo_custody_matrix(self) -> None:
        uid = os.geteuid()
        self.assertIn("candidate_install_files.py", candidate_files.DRIVER_QUALIFY_FILES)
        self.assertIn("candidate_install_files.py", https_lab_host.HTTPS_LAB_DRIVER_FILES)
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            q_abs, h_abs, t_abs = self._init_driver_repo(base / "absent")
            candidate_files.verify_driver_custody(q_abs, h_abs, t_abs, expected_uids=(uid,), stop_at=base / "absent")
            https_lab_host.verify_https_lab_driver_custody(q_abs, h_abs, t_abs, expected_uids=(uid,), stop_at=base / "absent")

            # Real Git repo with exact repo .cargo/mutants.toml passes all driver verifiers and rejects tampering.
            repo = base / "repo"
            qdir, head, tree = self._init_driver_repo(repo, extra_files={".cargo/mutants.toml": MUTANTS_TOML_BYTES})
            candidate_files.verify_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)
            https_lab_host.verify_https_lab_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)

            # Transitive publisher module custody is enforced.
            os.chmod(qdir / "candidate_install_files.py", 0o666)
            with self.assertRaisesRegex(candidate_files.FileCustodyError, "^unprotected_file_owner_or_mode:candidate_install_files.py$"):
                candidate_files.verify_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)
            os.chmod(qdir / "candidate_install_files.py", 0o444)

            # Negative controls on verify_cargo_dir_custody (real Git, pin mismatch, mode, hardlink, symlinks, extras).
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.verify_cargo_dir_custody(
                    repo, commit=head, expected_uids=(uid + 1,), stop_at=repo, reason="driver_worktree_unclean_or_cargo_present",
                )
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.verify_cargo_dir_custody(
                    repo, commit="0" * 40, expected_uids=(uid,), stop_at=repo, verify_head_pin=True,
                )
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.verify_cargo_dir_custody(
                    repo, commit=head, expected_uids=(uid,), stop_at=repo,
                    git_runner=lambda *_a, **_k: subprocess.CompletedProcess([], 1, b"", b""),
                )
            mut = repo / ".cargo" / "mutants.toml"
            os.chmod(mut, 0o666)
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.verify_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)
            os.chmod(mut, 0o444)

            hard = base / "hard_mutants"
            os.link(mut, hard)
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.verify_driver_custody(qdir, head, tree, expected_uids=(uid,), stop_at=repo)
            hard.unlink()

            os.unlink(mut)
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.verify_cargo_dir_custody(repo, commit=head, expected_uids=(uid,), stop_at=repo)
            (repo / ".cargo").rmdir()
            with self.assertRaises(candidate_files.FileCustodyError):
                candidate_files.verify_cargo_dir_custody(repo, commit=head, expected_uids=(uid,), stop_at=repo)

            # Committed forbidden Cargo files, executable mode, and symlinks refuse.
            for bad_name, exec_mode in (("config", False), ("config.toml", False), ("mutants.toml", True)):
                b_repo = base / f"bad_{bad_name}_{exec_mode}"
                bq, bh, bt = self._init_driver_repo(b_repo, extra_files={f".cargo/{bad_name}": MUTANTS_TOML_BYTES}, exec_cargo=exec_mode)
                with self.assertRaises(candidate_files.FileCustodyError):
                    candidate_files.verify_driver_custody(bq, bh, bt, expected_uids=(uid,), stop_at=b_repo)

            for kind in ("dir_sym", "file_sym", "hidden", "ignored", "nested"):
                s_repo = base / f"sym_{kind}"
                sq, sh, st = self._init_driver_repo(s_repo, extra_files={".gitignore": b".cargo/ign.toml\n"} if kind == "ignored" else None)
                cdir = s_repo / ".cargo"
                if kind == "dir_sym":
                    tgt = base / f"tgt_{kind}"
                    tgt.mkdir()
                    (tgt / "mutants.toml").write_bytes(MUTANTS_TOML_BYTES)
                    cdir.symlink_to(tgt)
                else:
                    cdir.mkdir(mode=0o755)
                    if kind == "file_sym":
                        tgt = base / "sym_mut.toml"
                        tgt.write_bytes(MUTANTS_TOML_BYTES)
                        (cdir / "mutants.toml").symlink_to(tgt)
                    elif kind == "hidden":
                        (cdir / ".hidden").write_bytes(b"x")
                    elif kind == "ignored":
                        (cdir / "ign.toml").write_bytes(b"x")
                    elif kind == "nested":
                        (cdir / "sub").mkdir()
                with self.assertRaises(candidate_files.FileCustodyError):
                    candidate_files.verify_cargo_dir_custody(s_repo, commit=sh, expected_uids=(uid,), stop_at=s_repo)

    def test_legacy_shell_bridge_accepts_tracked_mutants_and_refuses_evasions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            ok_repo = base / "ok"
            qdir, head, tree = self._init_driver_repo(ok_repo, extra_files={".cargo/mutants.toml": MUTANTS_TOML_BYTES})
            env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": tmp}
            script = (
                f'set -euo pipefail; refuse() {{ printf "%s\\n" "$1" >&2; exit 1; }}; '
                f'source "{qdir / "deployment_custody.sh"}"; '
                f'qualify_verify_driver_before_load "{qdir}" "{head}" "{tree}" 0'
            )
            res = subprocess.run(["bash", "-c", script], capture_output=True, text=True, env=env, timeout=15)
            self.assertEqual(res.returncode, 0, res.stderr)

            # Dirty worktree and pin mismatch both refuse before importing candidate_files.py.
            os.chmod(qdir / "candidate_files.py", 0o644)
            (qdir / "candidate_files.py").write_text("raise RuntimeError('POISONED_IMPORT')\n", encoding="utf-8")
            os.chmod(qdir / "candidate_files.py", 0o444)
            dirty_sh = subprocess.run(["bash", "-c", script], capture_output=True, text=True, env=env, timeout=15)
            self.assertEqual(dirty_sh.returncode, 1)
            self.assertEqual(dirty_sh.stderr.strip(), "driver_worktree_unclean")
            self.assertNotIn("POISONED_IMPORT", dirty_sh.stderr)
            g_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
            subprocess.run(["git", "-C", str(ok_repo), "add", "boundary/qualify/candidate_files.py"], check=True, env=g_env)
            subprocess.run(["git", "-C", str(ok_repo), "commit", "-qm", "poison"], check=True, env=g_env)
            bad_pin = subprocess.run(["bash", "-c", script], capture_output=True, text=True, env=env, timeout=15)
            self.assertEqual(bad_pin.returncode, 1)
            self.assertEqual(bad_pin.stderr.strip(), "driver_git_pin_mismatch")
            self.assertNotIn("POISONED_IMPORT", bad_pin.stderr)

            # Committed .cargo/config.toml and unsafe candidate_install_files.py refuse via legacy shell bridge.
            cfg_repo = base / "cfg"
            cq, ch, ct = self._init_driver_repo(cfg_repo, extra_files={".cargo/config.toml": b"[net]\noffline=true\n"})
            res_cfg = subprocess.run(
                ["bash", "-c", f'set -euo pipefail; refuse() {{ printf "%s\\n" "$1" >&2; exit 1; }}; source "{cq / "deployment_custody.sh"}"; qualify_verify_driver_before_load "{cq}" "{ch}" "{ct}" 0'],
                capture_output=True, text=True, env=env, timeout=15,
            )
            self.assertEqual(res_cfg.returncode, 1)
            self.assertEqual(res_cfg.stderr.strip(), "driver_cargo_config_present")

            os.unlink(cq / "candidate_install_files.py")
            res_miss = subprocess.run(
                ["bash", "-c", f'set -euo pipefail; refuse() {{ printf "%s\\n" "$1" >&2; exit 1; }}; source "{cq / "deployment_custody.sh"}"; qualify_verify_driver_before_load "{cq}" "{ch}" "{ct}" 0'],
                capture_output=True, text=True, env=env, timeout=15,
            )
            self.assertEqual(res_miss.returncode, 1)
            self.assertEqual(res_miss.stderr.strip(), "driver_custody_refused")


if __name__ == "__main__":
    unittest.main()

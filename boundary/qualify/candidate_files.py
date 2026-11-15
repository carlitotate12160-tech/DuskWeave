#!/usr/bin/env python3
"""Protected directory/file observation, full-buffer custody writes, and driver custody."""

from __future__ import annotations

import hashlib
import os
import re
import stat
import subprocess
from pathlib import Path

HEX40_RE = re.compile(r"^[0-9a-f]{40}$")
DEFAULT_MAX_READ_BYTES = 134_217_728
SENSITIVE_ENV_ALLOW = frozenset({"DW_TEST_FIXTURE_SUPERVISOR"})


class FileCustodyError(RuntimeError):
    """Fail-closed custody, ancestry, or durable-write failure."""

    def __init__(self, status: str, reason: str):
        super().__init__(reason)
        self.status = status
        self.reason = reason


def reject_sensitive_env(raw_env: dict[str, str] | None, *, error_cls=FileCustodyError) -> dict[str, str]:
    """Refuse ambient provider keys, DW_* overrides, and *_FILE overrides."""
    source = os.environ if raw_env is None else raw_env
    for key in source:
        if key in SENSITIVE_ENV_ALLOW:
            continue
        if key == "DEEPSEEK_API_KEY" or key.startswith("DW_") or key.endswith("_FILE"):
            raise error_cls("REFUSED", f"forbidden_env_variable:{key}")
    return dict(source)


def fsync_dir(directory: Path, fsync_fn=os.fsync) -> None:
    """Fsync a directory descriptor so metadata/renames are durable."""
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    fd = os.open(directory, flags)
    try:
        fsync_fn(fd)
    finally:
        os.close(fd)


def _ancestor_chain(target_dir: Path) -> list[Path]:
    parts = target_dir.parts
    chain = [Path("/")]
    cur = Path("/")
    for part in parts[1:]:
        cur = cur / part
        chain.append(cur)
    return chain


def _check_ancestor(
    cur: Path, expected_uids: tuple[int, ...], enforce_perm: bool, lstat_fn
) -> None:
    try:
        st = lstat_fn(cur)
    except OSError as exc:
        raise FileCustodyError("REFUSED", f"directory_missing:{cur}") from exc
    if stat.S_ISLNK(st.st_mode) or not stat.S_ISDIR(st.st_mode):
        raise FileCustodyError("REFUSED", f"unsafe_ancestor_type:{cur}")
    if enforce_perm and ((st.st_mode & 0o022) != 0 or st.st_uid not in expected_uids):
        raise FileCustodyError("REFUSED", f"unprotected_directory:{cur}")


def require_safe_ancestors(
    target_dir: Path,
    expected_uids: tuple[int, ...],
    *,
    stop_at: Path = Path("/"),
    lstat_fn=os.lstat,
) -> None:
    """Validate zero symlinks from / and protected UID/mode at and below stop_at."""
    if not target_dir.is_absolute() or not stop_at.is_absolute():
        raise FileCustodyError("REFUSED", "relative_custody_path_refused")
    if target_dir != stop_at and stop_at not in target_dir.parents:
        raise FileCustodyError("REFUSED", "custody_path_outside_root")
    enforce_perm = False
    for cur in _ancestor_chain(target_dir):
        enforce_perm = enforce_perm or (cur == stop_at)
        _check_ancestor(cur, expected_uids, enforce_perm, lstat_fn)


def _open_nofollow_ro(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        return os.open(path, flags)
    except OSError as exc:
        raise FileCustodyError("REFUSED", f"protected_open_failed:{path.name}") from exc


def _validate_open_stat(
    st: os.stat_result,
    name: str,
    expected_uids: tuple[int, ...],
    allowed_modes: tuple[int, ...] | None,
    max_bytes: int,
    status_on_error: str,
) -> None:
    if not stat.S_ISREG(st.st_mode) or st.st_nlink != 1:
        raise FileCustodyError(status_on_error, f"unsafe_regular_file:{name}")
    if st.st_uid not in expected_uids or (st.st_mode & 0o022) != 0:
        raise FileCustodyError(status_on_error, f"unprotected_file_owner_or_mode:{name}")
    if allowed_modes is not None and stat.S_IMODE(st.st_mode) not in allowed_modes:
        raise FileCustodyError(status_on_error, f"unexpected_file_mode:{name}")
    if st.st_size > max_bytes:
        raise FileCustodyError(status_on_error, f"file_exceeds_limit:{name}")


def _read_bounded_fd(fd: int, name: str, max_bytes: int, status_on_error: str) -> bytes:
    chunks = []
    total = 0
    while True:
        block = os.read(fd, 65536)
        if not block:
            return b"".join(chunks)
        total += len(block)
        if total > max_bytes:
            raise FileCustodyError(status_on_error, f"file_exceeds_limit:{name}")
        chunks.append(block)


def read_protected_bytes(
    path: Path,
    *,
    expected_uids: tuple[int, ...],
    allowed_modes: tuple[int, ...] | None = None,
    max_bytes: int = DEFAULT_MAX_READ_BYTES,
    stop_at: Path = Path("/"),
    status_on_error: str = "REFUSED",
) -> bytes:
    """Verify full ancestry and open regular single-link file safely with O_NOFOLLOW."""
    require_safe_ancestors(path.parent, expected_uids, stop_at=stop_at)
    fd = _open_nofollow_ro(path)
    try:
        st = os.fstat(fd)
        _validate_open_stat(st, path.name, expected_uids, allowed_modes, max_bytes, status_on_error)
        return _read_bounded_fd(fd, path.name, max_bytes, status_on_error)
    finally:
        os.close(fd)


def _write_all_fd(fd: int, data: bytes, writer=os.write) -> None:
    view = memoryview(data)
    offset = 0
    while offset < len(data):
        try:
            written = writer(fd, view[offset:])
        except InterruptedError:
            continue
        except OSError as exc:
            raise FileCustodyError("INCOMPLETE", f"custody_write_failed:{exc.errno}") from exc
        if written <= 0:
            raise FileCustodyError("INCOMPLETE", "custody_short_write_zero_progress")
        offset += written


def _verify_written_file(
    path: Path,
    expected: bytes,
    *,
    expected_uids: tuple[int, ...],
    expected_mode: int,
) -> str:
    fd = _open_nofollow_ro(path)
    try:
        st = os.fstat(fd)
        bad_meta = (
            not stat.S_ISREG(st.st_mode)
            or st.st_nlink != 1
            or st.st_uid not in expected_uids
            or stat.S_IMODE(st.st_mode) != expected_mode
        )
        if bad_meta:
            raise FileCustodyError("INCOMPLETE", f"written_metadata_mismatch:{path.name}")
        chunks = []
        while True:
            part = os.read(fd, 65536)
            if not part:
                break
            chunks.append(part)
        actual = b"".join(chunks)
    finally:
        os.close(fd)
    if actual != expected:
        raise FileCustodyError("INCOMPLETE", f"written_digest_mismatch:{path.name}")
    return hashlib.sha256(actual).hexdigest()


def _commit_fd_metadata(
    fd: int,
    *,
    mode: int,
    owner_uid: int | None,
    owner_gid: int | None,
    fsync_fn,
) -> None:
    try:
        if owner_uid is not None and os.geteuid() == 0:
            os.fchown(fd, owner_uid, -1 if owner_gid is None else owner_gid)
        os.fchmod(fd, mode)
        fsync_fn(fd)
    except OSError as exc:
        raise FileCustodyError("INCOMPLETE", f"custody_fsync_or_chmod_failed:{exc.errno}") from exc


def write_exclusive_verified(
    path: Path,
    data: bytes,
    *,
    mode: int,
    expected_uids: tuple[int, ...],
    owner_uid: int | None = None,
    owner_gid: int | None = None,
    stop_at: Path = Path("/"),
    writer=os.write,
    fsync_fn=os.fsync,
) -> str:
    """Create path exclusively with full-buffer loop, fsync, and readback verification."""
    try:
        os.lstat(path)
        raise FileCustodyError("REFUSED", f"destination_collision:{path.name}")
    except FileNotFoundError:
        pass
    require_safe_ancestors(path.parent, expected_uids, stop_at=stop_at)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags, 0o400)
    except OSError as exc:
        raise FileCustodyError("REFUSED", f"exclusive_create_failed:{path.name}") from exc
    try:
        _write_all_fd(fd, data, writer=writer)
        _commit_fd_metadata(
            fd, mode=mode, owner_uid=owner_uid, owner_gid=owner_gid, fsync_fn=fsync_fn
        )
    finally:
        os.close(fd)
    verify_uids = expected_uids if owner_uid is None else (owner_uid,)
    digest = _verify_written_file(path, data, expected_uids=verify_uids, expected_mode=mode)
    fsync_dir(path.parent, fsync_fn=fsync_fn)
    return digest


def publish_atomic_verified(
    path: Path,
    data: bytes,
    *,
    mode: int,
    expected_uids: tuple[int, ...],
    owner_uid: int | None = None,
    owner_gid: int | None = None,
    stop_at: Path = Path("/"),
    writer=os.write,
    fsync_fn=os.fsync,
    replacer=os.replace,
) -> str:
    """Stage exclusively, verify content/metadata, replace atomically, fsync dir, reverify."""
    if path.is_symlink():
        raise FileCustodyError("REFUSED", f"symlink_destination_refused:{path.name}")
    require_safe_ancestors(path.parent, expected_uids, stop_at=stop_at)
    stage = path.with_name(f".{path.name}.stage.{os.getpid()}")
    write_exclusive_verified(
        stage,
        data,
        mode=mode,
        expected_uids=expected_uids,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
        stop_at=stop_at,
        writer=writer,
        fsync_fn=fsync_fn,
    )
    try:
        replacer(stage, path)
        fsync_dir(path.parent, fsync_fn=fsync_fn)
    except OSError as exc:
        raise FileCustodyError("INCOMPLETE", f"atomic_replace_failed:{path.name}") from exc
    verify_uids = expected_uids if owner_uid is None else (owner_uid,)
    return _verify_written_file(path, data, expected_uids=verify_uids, expected_mode=mode)


def create_exclusive_dir(
    path: Path,
    *,
    mode: int,
    expected_uids: tuple[int, ...],
    stop_at: Path = Path("/"),
    fsync_fn=os.fsync,
) -> None:
    """Exclusively create one fresh protected directory beneath verified ancestors."""
    try:
        os.lstat(path)
        raise FileCustodyError("REFUSED", f"directory_collision:{path.name}")
    except FileNotFoundError:
        pass
    require_safe_ancestors(path.parent, expected_uids, stop_at=stop_at)
    try:
        os.mkdir(path, mode)
        os.chmod(path, mode)
    except OSError as exc:
        raise FileCustodyError("REFUSED", f"directory_create_failed:{path.name}") from exc
    st = os.lstat(path)
    bad = (
        stat.S_ISLNK(st.st_mode)
        or not stat.S_ISDIR(st.st_mode)
        or stat.S_IMODE(st.st_mode) != mode
        or st.st_uid not in expected_uids
    )
    if bad:
        raise FileCustodyError("INCOMPLETE", f"directory_metadata_mismatch:{path.name}")
    fsync_dir(path.parent, fsync_fn=fsync_fn)


DRIVER_QUALIFY_FILES = (
    "candidate.sh",
    "candidate_binding.py",
    "candidate_database.py",
    "candidate_files.py",
    "candidate_host.py",
    "candidate_install.py",
    "candidate_postgres.py",
    "deployment_custody.sh",
    "qualify.sh",
    "release_descriptor.py",
)


def verify_driver_custody(
    qualify_dir: Path,
    expected_commit: str,
    expected_tree: str,
    *,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
    git_runner=subprocess.run,
) -> None:
    """Validate driver files under protected custody and clean pinned Git commit/tree."""
    if not HEX40_RE.fullmatch(expected_commit) or not HEX40_RE.fullmatch(expected_tree):
        raise FileCustodyError("REFUSED", "driver_pin_malformed")
    repo_root = qualify_dir.parents[1]
    tracked = [qualify_dir / name for name in DRIVER_QUALIFY_FILES] + [
        repo_root / "tests" / "support" / "m1_native_bootstrap.py"
    ]
    for item in tracked:
        read_protected_bytes(item, expected_uids=expected_uids, stop_at=stop_at)
    clean_git_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    head = git_runner(
        ["git", "-C", str(repo_root), "rev-parse", "HEAD"],
        capture_output=True, text=True, env=clean_git_env, timeout=15,
    )
    tree = git_runner(
        ["git", "-C", str(repo_root), "rev-parse", "HEAD^{tree}"],
        capture_output=True, text=True, env=clean_git_env, timeout=15,
    )
    status = git_runner(
        ["git", "-C", str(repo_root), "status", "--porcelain=v1", "--untracked-files=all"],
        capture_output=True, text=True, env=clean_git_env, timeout=15,
    )
    if head.returncode != 0 or tree.returncode != 0 or status.returncode != 0:
        raise FileCustodyError("REFUSED", "driver_git_inspection_failed")
    if head.stdout.strip() != expected_commit or tree.stdout.strip() != expected_tree:
        raise FileCustodyError("REFUSED", "driver_git_pin_mismatch")
    if status.stdout.strip() or (repo_root / ".cargo").exists() or (repo_root / ".cargo").is_symlink():
        raise FileCustodyError("REFUSED", "driver_worktree_unclean_or_cargo_present")

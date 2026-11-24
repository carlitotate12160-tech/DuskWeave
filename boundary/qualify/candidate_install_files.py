#!/usr/bin/env python3
"""Protected file-custody, backup, staging, commit-verification, and receipt helpers."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat

from candidate_files import (
    FileCustodyError,
    create_exclusive_dir,
    fsync_dir,
    read_protected_bytes,
    require_safe_ancestors,
    verify_written_file,
    write_exclusive_verified,
)


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
    return verify_written_file(path, data, expected_uids=verify_uids, expected_mode=mode)


def observe_optional_file(
    path: Path,
    expected_uids: tuple[int, ...],
    stop_at: Path,
    *,
    error_cls,
    tag: str,
    allowed_modes: tuple[int, ...] = (0o400, 0o444),
    max_bytes: int = 1_048_576,
) -> dict | None:
    """Validate optional protected file custody and capture mode/owner/digest before replacement."""
    if not (path.exists() or path.is_symlink()):
        return None
    try:
        raw = read_protected_bytes(
            path, expected_uids=expected_uids, allowed_modes=allowed_modes,
            max_bytes=max_bytes, stop_at=stop_at, status_on_error="REFUSED",
        )
    except FileCustodyError as exc:
        raise error_cls(exc.status, f"{tag}_unqualified:{exc.reason}") from exc
    st = os.lstat(path)
    return {
        "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw),
        "mode": stat.S_IMODE(st.st_mode), "uid": st.st_uid, "gid": st.st_gid, "bytes": raw,
    }


def validate_destination_dir(
    dst_dir: Path,
    expected_uids: tuple[int, ...],
    stop_at: Path,
    *,
    error_cls,
    tag: str,
) -> tuple[int, int]:
    """Validate existing non-symlink destination directory custody and return (uid, gid)."""
    try:
        require_safe_ancestors(dst_dir, expected_uids, stop_at=stop_at)
        if not dst_dir.is_dir() or dst_dir.is_symlink():
            raise FileCustodyError("REFUSED", f"{tag}_dir_invalid")
        st = os.lstat(dst_dir)
        if st.st_uid not in expected_uids or (stat.S_IMODE(st.st_mode) & 0o022) != 0:
            raise FileCustodyError("REFUSED", f"{tag}_dir_unqualified")
    except FileCustodyError as exc:
        raise error_cls(exc.status, exc.reason) from exc
    return st.st_uid, st.st_gid


def backup_release(
    backup_dir: Path,
    binaries: tuple[str, ...],
    old_bins: dict[str, dict],
    old_manifest: dict | None,
    expected_uids: tuple[int, ...],
    stop_at: Path,
    *,
    fsync_dir_fn,
    error_cls,
    extra_files: dict[str, tuple[str, dict | None]] | None = None,
) -> None:
    """Persist complete old release backup and metadata before any protected write."""
    try:
        create_exclusive_dir(backup_dir, mode=0o700, expected_uids=expected_uids, stop_at=stop_at)
        meta: dict[str, object] = {"binaries": {}, "old_manifest_present": old_manifest is not None}
        for name in binaries:
            info = old_bins[name]
            digest = write_exclusive_verified(
                backup_dir / name, info["bytes"], mode=info["mode"], expected_uids=expected_uids,
                owner_uid=info["uid"], owner_gid=info["gid"], stop_at=stop_at,
            )
            meta["binaries"][name] = {**{k: info[k] for k in ("size", "mode", "uid", "gid")}, "sha256": digest}
        if old_manifest is not None:
            m_sha = write_exclusive_verified(
                backup_dir / "deployment.json", old_manifest["bytes"], mode=old_manifest["mode"],
                expected_uids=expected_uids, owner_uid=old_manifest["uid"], owner_gid=old_manifest["gid"],
                stop_at=stop_at,
            )
            meta["old_manifest"] = {**{k: old_manifest[k] for k in ("size", "mode", "uid", "gid")}, "sha256": m_sha}
        if extra_files:
            meta["extra_files"] = {}
            for key, (bak_name, info) in extra_files.items():
                if info is None:
                    meta["extra_files"][key] = {"present": False}
                    continue
                e_sha = write_exclusive_verified(
                    backup_dir / bak_name, info["bytes"], mode=info["mode"], expected_uids=expected_uids,
                    owner_uid=info["uid"], owner_gid=info["gid"], stop_at=stop_at,
                )
                meta["extra_files"][key] = {
                    "present": True, "backup_name": bak_name,
                    **{k: info[k] for k in ("size", "mode", "uid", "gid")}, "sha256": e_sha,
                }
        meta_bytes = (json.dumps(meta, sort_keys=True, indent=2) + "\n").encode("utf-8")
        write_exclusive_verified(
            backup_dir / "backup-manifest.json", meta_bytes, mode=0o400,
            expected_uids=expected_uids, stop_at=stop_at,
        )
        fsync_dir_fn(backup_dir)
        fsync_dir_fn(backup_dir.parent)
    except FileCustodyError as exc:
        raise error_cls("INCOMPLETE", f"backup_failed:{exc.reason}") from exc


def cleanup_staged(staged: dict[str, Path]) -> None:
    """Best-effort unlink of staged pre-commit temporary artifacts."""
    for path in staged.values():
        try:
            path.unlink(missing_ok=True)
        except OSError:
            pass


def stage_artifacts(
    package_dir: Path,
    bin_dir: Path,
    binaries: tuple[str, ...],
    old_bins: dict[str, dict],
    expected_shas: dict[str, str],
    expected_uids: tuple[int, ...],
    stop_at: Path,
    *,
    fsync_dir_fn,
    error_cls,
    extra_targets: tuple[tuple[str, str, Path, int, int, int], ...] = (),
) -> dict[str, Path]:
    """Stage all candidate artifacts alongside their target paths and fsync before commit."""
    staged: dict[str, Path] = {}
    fsynced_dirs: set[Path] = set()
    pid = os.getpid()
    try:
        for name in binaries:
            stage = bin_dir / f".{name}.stage.{pid}"
            raw = read_protected_bytes(
                package_dir / name, expected_uids=expected_uids,
                allowed_modes=(0o555,), stop_at=stop_at, status_on_error="REFUSED",
            )
            digest = write_exclusive_verified(
                stage, raw, mode=old_bins[name]["mode"], expected_uids=expected_uids,
                owner_uid=old_bins[name]["uid"], owner_gid=old_bins[name]["gid"], stop_at=stop_at,
            )
            if digest != expected_shas[name]:
                raise error_cls("REFUSED", f"staged_digest_mismatch:{name}")
            staged[name] = stage
        for key, pkg_rel, dst_path, mode, uid, gid in extra_targets:
            stage = dst_path.parent / f".{dst_path.name}.stage.{pid}"
            raw = read_protected_bytes(
                package_dir / pkg_rel, expected_uids=expected_uids,
                allowed_modes=(0o400, 0o444), stop_at=stop_at, status_on_error="REFUSED",
            )
            digest = write_exclusive_verified(
                stage, raw, mode=mode, expected_uids=expected_uids,
                owner_uid=uid, owner_gid=gid, stop_at=stop_at,
            )
            if digest != expected_shas[key]:
                raise error_cls("REFUSED", f"staged_digest_mismatch:{key}")
            staged[key] = stage
            fsynced_dirs.add(dst_path.parent)
        fsync_dir_fn(bin_dir)
        for d in sorted(fsynced_dirs):
            if d != bin_dir:
                fsync_dir_fn(d)
    except (FileCustodyError, error_cls) as exc:
        cleanup_staged(staged)
        raise error_cls("REFUSED", f"staging_aborted_zero_installed:{getattr(exc, 'reason', str(exc))}") from exc
    return staged


def verify_installed_artifacts(
    bin_dir: Path,
    binaries: tuple[str, ...],
    expected_shas: dict[str, str],
    old_bins: dict[str, dict],
    expected_uids: tuple[int, ...],
    stop_at: Path,
    *,
    error_cls,
    extra_targets: tuple[tuple[str, Path, int], ...] = (),
) -> None:
    """Re-read installed artifacts and verify ownership, mode, and SHA-256 digests."""
    for name in binaries:
        raw = read_protected_bytes(
            bin_dir / name, expected_uids=expected_uids,
            allowed_modes=(old_bins[name]["mode"],), stop_at=stop_at, status_on_error="INCOMPLETE",
        )
        if hashlib.sha256(raw).hexdigest() != expected_shas[name]:
            raise error_cls("INCOMPLETE", f"final_installed_digest_mismatch:{name}")
    for key, dst_path, mode in extra_targets:
        raw = read_protected_bytes(
            dst_path, expected_uids=expected_uids,
            allowed_modes=(mode,), stop_at=stop_at, status_on_error="INCOMPLETE",
        )
        if hashlib.sha256(raw).hexdigest() != expected_shas[key]:
            raise error_cls("INCOMPLETE", f"final_installed_digest_mismatch:{key}")


def write_receipt_safe(
    receipt_path: Path | None,
    receipt: dict,
    expected_uids: tuple[int, ...],
    stop_at: Path,
) -> dict:
    """Persist install receipt or downgrade SUCCESS to UNKNOWN if receipt acknowledgement fails."""
    if receipt_path is None:
        return receipt
    payload = (json.dumps(receipt, sort_keys=True, indent=2) + "\n").encode("utf-8")
    try:
        write_exclusive_verified(receipt_path, payload, mode=0o444, expected_uids=expected_uids, stop_at=stop_at)
        return receipt
    except FileCustodyError as exc:
        return {
            "status": "UNKNOWN" if receipt["status"] == "SUCCESS" else receipt["status"],
            "descriptor_sha256": receipt.get("descriptor_sha256", ""),
            "backup_dir": receipt.get("backup_dir", ""),
            "replaced": receipt.get("replaced", []),
            "reason": f"install_receipt_persist_failed:{exc.reason}",
        }

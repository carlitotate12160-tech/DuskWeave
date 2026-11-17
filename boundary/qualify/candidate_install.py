#!/usr/bin/env python3
"""Privileged candidate four-binary installation with full backup, pre-staging, and bounded receipts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat

from candidate_files import (
    FileCustodyError,
    create_exclusive_dir,
    fsync_dir,
    read_protected_bytes,
    reject_sensitive_env,
    require_safe_ancestors,
    verify_driver_custody,
    write_exclusive_verified,
)
from candidate_host import (
    HostObservationError,
    INSTALL_BINARIES,
    PRIVILEGED_SCOPE,
    WORKER_BINARIES,
    check_quiescence,
    extract_image_worker_digests,
    observe_installed_facts,
)
import release_descriptor

HEX40_RE = re.compile(r"^[0-9a-f]{40}$")
HEX64_RE = re.compile(r"^[0-9a-f]{64}$")


class InstallError(RuntimeError):
    """Fail-closed installation error with bounded non-secret status and reason."""

    def __init__(self, status: str, reason: str) -> None:
        super().__init__(reason)
        self.status = status
        self.reason = reason


def _member_sha_map(desc: dict) -> dict[str, str]:
    return {m["path"]: m["sha256"] for m in desc["members"]}


def _verify_image_matches_package(desc: dict, expected_shas: dict[str, str], image_extractor) -> dict:
    try:
        img = image_extractor(desc["image_digest"])
    except HostObservationError as exc:
        raise InstallError(exc.status, exc.reason) from exc
    if img.get("image_id") != desc["image_digest"] or img.get("os") != "linux" or img.get("architecture") != "arm64":
        raise InstallError("REFUSED", "image_identity_or_platform_mismatch")
    workers = img.get("workers") or {}
    for wname in WORKER_BINARIES:
        if workers.get(wname) != expected_shas.get(wname):
            raise InstallError("REFUSED", f"image_worker_bytes_mismatch:{wname}")
    return img


def _observe_old_manifest(manifest_path: Path, expected_uids: tuple[int, ...], stop_at: Path) -> dict | None:
    if not (manifest_path.exists() or manifest_path.is_symlink()):
        return None
    try:
        raw = read_protected_bytes(manifest_path, expected_uids=expected_uids,
                                   allowed_modes=(0o400, 0o444), stop_at=stop_at, status_on_error="REFUSED")
    except FileCustodyError as exc:
        raise InstallError(exc.status, f"old_manifest_unqualified:{exc.reason}") from exc
    st = os.lstat(manifest_path)
    return {
        "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw),
        "mode": stat.S_IMODE(st.st_mode), "uid": st.st_uid, "gid": st.st_gid, "bytes": raw,
    }


def _backup_old_release(backup_dir: Path, old_bins: dict[str, dict], old_manifest: dict | None,
                        expected_uids: tuple[int, ...], stop_at: Path) -> None:
    try:
        create_exclusive_dir(backup_dir, mode=0o700, expected_uids=expected_uids, stop_at=stop_at)
        meta: dict[str, object] = {"binaries": {}, "old_manifest_present": old_manifest is not None}
        for name in INSTALL_BINARIES:
            info = old_bins[name]
            digest = write_exclusive_verified(
                backup_dir / name, info["bytes"], mode=info["mode"], expected_uids=expected_uids,
                owner_uid=info["uid"], owner_gid=info["gid"], stop_at=stop_at,
            )
            meta["binaries"][name] = {k: info[k] for k in ("size", "mode", "uid", "gid")}
            meta["binaries"][name]["sha256"] = digest
        if old_manifest is not None:
            m_sha = write_exclusive_verified(
                backup_dir / "deployment.json", old_manifest["bytes"], mode=old_manifest["mode"],
                expected_uids=expected_uids, owner_uid=old_manifest["uid"], owner_gid=old_manifest["gid"], stop_at=stop_at,
            )
            meta["old_manifest"] = {k: old_manifest[k] for k in ("size", "mode", "uid", "gid")}
            meta["old_manifest"]["sha256"] = m_sha
        meta_bytes = (json.dumps(meta, sort_keys=True, indent=2) + "\n").encode("utf-8")
        write_exclusive_verified(backup_dir / "backup-manifest.json", meta_bytes, mode=0o400,
                                 expected_uids=expected_uids, stop_at=stop_at)
        fsync_dir(backup_dir)
        fsync_dir(backup_dir.parent)
    except FileCustodyError as exc:
        raise InstallError("INCOMPLETE", f"backup_failed:{exc.reason}") from exc


def _cleanup_staged(staged: dict[str, Path]) -> None:
    for path in staged.values():
        try:
            path.unlink(missing_ok=True)
        except OSError:
            pass


def _stage_all_four(package_dir: Path, install_bin_dir: Path, old_bins: dict[str, dict],
                    expected_shas: dict[str, str], expected_uids: tuple[int, ...], stop_at: Path) -> dict[str, Path]:
    staged: dict[str, Path] = {}
    pid = os.getpid()
    try:
        for name in INSTALL_BINARIES:
            stage = install_bin_dir / f".{name}.stage.{pid}"
            raw = read_protected_bytes(package_dir / name, expected_uids=expected_uids,
                                       allowed_modes=(0o555,), stop_at=stop_at, status_on_error="REFUSED")
            digest = write_exclusive_verified(
                stage, raw, mode=old_bins[name]["mode"], expected_uids=expected_uids,
                owner_uid=old_bins[name]["uid"], owner_gid=old_bins[name]["gid"], stop_at=stop_at,
            )
            if digest != expected_shas[name]:
                raise InstallError("REFUSED", f"staged_digest_mismatch:{name}")
            staged[name] = stage
        fsync_dir(install_bin_dir)
    except (FileCustodyError, InstallError) as exc:
        _cleanup_staged(staged)
        raise InstallError("REFUSED", f"staging_aborted_zero_installed:{exc.reason}") from exc
    return staged


def _verify_all_installed(install_bin_dir: Path, expected_shas: dict[str, str], old_bins: dict[str, dict],
                          expected_uids: tuple[int, ...], stop_at: Path) -> None:
    for name in INSTALL_BINARIES:
        raw = read_protected_bytes(install_bin_dir / name, expected_uids=expected_uids,
                                   allowed_modes=(old_bins[name]["mode"],), stop_at=stop_at, status_on_error="INCOMPLETE")
        if hashlib.sha256(raw).hexdigest() != expected_shas[name]:
            raise InstallError("INCOMPLETE", f"final_installed_digest_mismatch:{name}")


def _write_receipt_safe(receipt_path: Path | None, receipt: dict, expected_uids: tuple[int, ...],
                        stop_at: Path) -> dict:
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


def _verify_provenance_and_desc(
    package_dir: Path, expected_descriptor_sha256: str, expected_driver_commit: str,
    expected_driver_tree: str, backup_dir: Path, receipt_path: Path | None,
    allow_synthetic: bool, expected_uids: tuple[int, ...], stop_at: Path,
    driver_dir: Path, driver_verifier,
) -> dict:
    if not (HEX40_RE.fullmatch(expected_driver_commit) and HEX40_RE.fullmatch(expected_driver_tree)):
        raise InstallError("REFUSED", "expected_driver_pin_invalid")
    for path, err in ((backup_dir, "backup_dir_collision"), (receipt_path, "install_receipt_collision")):
        if path is not None and (path.exists() or path.is_symlink()):
            raise InstallError("REFUSED", err)
    try:
        driver_verifier(driver_dir, expected_driver_commit, expected_driver_tree, expected_uids=expected_uids, stop_at=stop_at)
        require_safe_ancestors(backup_dir.parent, expected_uids, stop_at=stop_at)
        desc = release_descriptor.verify_descriptor(
            package_dir, expected_descriptor_sha256, allow_synthetic=allow_synthetic,
            expected_uids=expected_uids, stop_at=stop_at,
        )
    except (FileCustodyError, release_descriptor.ReleaseError) as exc:
        raise InstallError("REFUSED", getattr(exc, "reason", str(exc))) from exc
    if desc["driver_commit"] != expected_driver_commit or desc["driver_tree"] != expected_driver_tree:
        raise InstallError("REFUSED", "descriptor_driver_pin_mismatch")
    return desc


def _run_preflight(package_dir: Path, expected_descriptor_sha256: str, expected_driver_commit: str,
                   expected_driver_tree: str, backup_dir: Path, manifest_path: Path,
                   receipt_path: Path | None, allow_synthetic: bool, expected_uids: tuple[int, ...],
                   stop_at: Path, driver_dir: Path, driver_verifier, host_observer,
                   image_extractor, quiescence_checker) -> tuple[dict, dict, dict, dict | None]:
    desc = _verify_provenance_and_desc(
        package_dir, expected_descriptor_sha256, expected_driver_commit, expected_driver_tree,
        backup_dir, receipt_path, allow_synthetic, expected_uids, stop_at, driver_dir, driver_verifier,
    )
    shas = _member_sha_map(desc)
    _verify_image_matches_package(desc, shas, image_extractor)
    try:
        host_facts = host_observer()
        quiescence_checker()
    except HostObservationError as exc:
        raise InstallError(exc.status, exc.reason) from exc
    if set(host_facts.get("binaries", {})) != set(INSTALL_BINARIES):
        raise InstallError("REFUSED", "existing_installed_binaries_incomplete")
    old_manifest = _observe_old_manifest(manifest_path, expected_uids, stop_at)
    return desc, shas, host_facts["binaries"], old_manifest


def run_install(
    *,
    expected_driver_commit: str,
    expected_driver_tree: str,
    expected_descriptor_sha256: str,
    package_dir: Path | None = None,
    install_bin_dir: Path | None = None,
    backup_dir: Path | None = None,
    manifest_path: Path | None = None,
    receipt_path: Path | None = None,
    driver_dir: Path | None = None,
    raw_env: dict[str, str] | None = None,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
    allow_synthetic: bool = False,
    driver_verifier=verify_driver_custody,
    host_observer=None,
    image_extractor=extract_image_worker_digests,
    quiescence_checker=None,
    replacer=os.replace,
) -> dict:
    """Install four verified binaries after complete old backup and four-way staging."""
    reject_sensitive_env(raw_env, error_cls=lambda _s, r: InstallError("REFUSED", r))
    pkg = package_dir or Path(PRIVILEGED_SCOPE["bundle_dir"])
    bin_dir = install_bin_dir or Path(PRIVILEGED_SCOPE["install_bin_dir"])
    bak = backup_dir or Path(PRIVILEGED_SCOPE["backup_dir"])
    mfst = manifest_path or Path(PRIVILEGED_SCOPE["manifest_path"])
    rcpt = receipt_path if receipt_path is not None else Path(PRIVILEGED_SCOPE["report_root"]) / "install-receipt.json"
    drv = driver_dir or Path(__file__).resolve().parent
    h_obs = host_observer or (lambda: observe_installed_facts(
        install_bin_dir=bin_dir, expected_uids=expected_uids, stop_at=stop_at, require_binaries=True,
    ))
    q_chk = quiescence_checker or (lambda: check_quiescence(pg_container=PRIVILEGED_SCOPE["pg_container"]))
    desc, shas, old_bins, old_mfst = _run_preflight(
        pkg, expected_descriptor_sha256, expected_driver_commit, expected_driver_tree,
        bak, mfst, rcpt, allow_synthetic, expected_uids, stop_at, drv,
        driver_verifier, h_obs, image_extractor, q_chk,
    )
    _backup_old_release(bak, old_bins, old_mfst, expected_uids, stop_at)
    staged: dict[str, Path] = {}
    try:
        staged = _stage_all_four(pkg, bin_dir, old_bins, shas, expected_uids, stop_at)
        q_chk()
    except (InstallError, HostObservationError) as exc:
        _cleanup_staged(staged)
        receipt = {"status": "REFUSED", "descriptor_sha256": expected_descriptor_sha256,
                   "backup_dir": str(bak), "replaced": [], "reason": exc.reason}
        return _write_receipt_safe(rcpt, receipt, expected_uids, stop_at)
    return _commit_staged_install(bin_dir, bak, staged, shas, old_bins, desc, rcpt,
                                  expected_descriptor_sha256, expected_uids, stop_at, replacer)


def _commit_staged_install(bin_dir: Path, bak: Path, staged: dict[str, Path], shas: dict[str, str],
                           old_bins: dict[str, dict], desc: dict, rcpt: Path | None,
                           expected_sha: str, expected_uids: tuple[int, ...], stop_at: Path, replacer) -> dict:
    replaced: list[str] = []
    try:
        for name in INSTALL_BINARIES:
            replacer(staged[name], bin_dir / name)
            replaced.append(name)
            fsync_dir(bin_dir)
        _verify_all_installed(bin_dir, shas, old_bins, expected_uids, stop_at)
    except Exception as exc:
        reason = getattr(exc, "reason", None) or str(exc) or type(exc).__name__
        receipt = {"status": "INCOMPLETE", "descriptor_sha256": expected_sha,
                   "backup_dir": str(bak), "replaced": replaced, "reason": reason}
        return _write_receipt_safe(rcpt, receipt, expected_uids, stop_at)
    receipt = {
        "status": "SUCCESS", "descriptor_sha256": expected_sha, "backup_dir": str(bak),
        "replaced": replaced, "image_digest": desc["image_digest"], "installed_hashes": shas,
    }
    return _write_receipt_safe(rcpt, receipt, expected_uids, stop_at)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Privileged fixed-scope candidate install owner")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_ins = sub.add_parser("install")
    for flag in ("--expected-driver-commit", "--expected-driver-tree", "--expected-descriptor-sha256"):
        p_ins.add_argument(flag, required=True)
    args = parser.parse_args(argv)
    drv_dir = Path(__file__).resolve().parent
    expected_drv = Path(PRIVILEGED_SCOPE["source_dir"]) / "boundary" / "qualify"
    try:
        if drv_dir != expected_drv:
            raise InstallError("REFUSED", "privileged_driver_outside_fixed_source")
        receipt = run_install(
            expected_driver_commit=args.expected_driver_commit,
            expected_driver_tree=args.expected_driver_tree,
            expected_descriptor_sha256=args.expected_descriptor_sha256,
        )
    except InstallError as exc:
        print(json.dumps({"status": exc.status, "reason": exc.reason}, sort_keys=True))
        return 1
    print(json.dumps(receipt, sort_keys=True))
    return 0 if receipt["status"] == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())

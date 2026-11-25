#!/usr/bin/env python3
"""Privileged candidate installation owners for schema-0036 legacy and HTTPS LAB 0038."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re

from candidate_files import (
    FileCustodyError,
    fsync_dir,
    reject_sensitive_env,
    require_safe_ancestors,
    verify_driver_custody,
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
from candidate_install_files import (
    backup_release,
    cleanup_staged,
    observe_optional_file,
    stage_artifacts,
    validate_destination_dir,
    verify_installed_artifacts,
    write_receipt_safe,
)
import https_lab_host
import https_lab_release
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


def _verify_image_matches_package(
    desc: dict, expected_shas: dict[str, str], image_extractor, *, extra_image_shas: dict[str, str] | None = None,
) -> dict:
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
    extras = img.get("extras") or {}
    for img_rel, exp_sha in (extra_image_shas or {}).items():
        if extras.get(img_rel) != exp_sha:
            raise InstallError("REFUSED", f"image_extra_bytes_mismatch:{img_rel}")
    return img


def _verify_provenance_and_desc(
    package_dir: Path, expected_descriptor_sha256: str, expected_driver_commit: str,
    expected_driver_tree: str, backup_dir: Path, receipt_path: Path | None,
    allow_synthetic: bool, expected_uids: tuple[int, ...], stop_at: Path,
    driver_dir: Path, driver_verifier, desc_verifier, *,
    expected_build_commit: str | None = None, expected_build_tree: str | None = None,
) -> dict:
    for pin_name, val in (("driver_commit", expected_driver_commit), ("driver_tree", expected_driver_tree)):
        if not HEX40_RE.fullmatch(val):
            raise InstallError("REFUSED", f"expected_{pin_name}_invalid")
    for name, val in (("build_commit", expected_build_commit), ("build_tree", expected_build_tree)):
        if val is not None and not HEX40_RE.fullmatch(val):
            raise InstallError("REFUSED", f"expected_{name}_invalid")
    for path, err in ((backup_dir, "backup_dir_collision"), (receipt_path, "install_receipt_collision")):
        if path is not None and (path.exists() or path.is_symlink()):
            raise InstallError("REFUSED", err)
    try:
        driver_verifier(driver_dir, expected_driver_commit, expected_driver_tree, expected_uids=expected_uids, stop_at=stop_at)
        require_safe_ancestors(backup_dir.parent, expected_uids, stop_at=stop_at)
        desc = desc_verifier(
            package_dir, expected_descriptor_sha256, allow_synthetic=allow_synthetic,
            expected_uids=expected_uids, stop_at=stop_at,
        )
    except (FileCustodyError, release_descriptor.ReleaseError) as exc:
        raise InstallError("REFUSED", getattr(exc, "reason", str(exc))) from exc
    if desc["driver_commit"] != expected_driver_commit or desc["driver_tree"] != expected_driver_tree:
        raise InstallError("REFUSED", "descriptor_driver_pin_mismatch")
    if expected_build_commit is not None and desc["build_source_commit"] != expected_build_commit:
        raise InstallError("REFUSED", "descriptor_build_pin_mismatch")
    if expected_build_tree is not None and desc["build_source_tree"] != expected_build_tree:
        raise InstallError("REFUSED", "descriptor_build_pin_mismatch")
    return desc


def _commit_staged_install(
    bin_dir: Path, bak: Path, staged: dict[str, Path], shas: dict[str, str],
    old_bins: dict[str, dict], desc: dict, rcpt: Path | None,
    expected_sha: str, expected_uids: tuple[int, ...], stop_at: Path, replacer,
    *, extra_commits: tuple[tuple[str, Path, int], ...] = (),
) -> dict:
    replaced: list[str] = []
    try:
        for name in INSTALL_BINARIES:
            replacer(staged[name], bin_dir / name)
            replaced.append(name)
            fsync_dir(bin_dir)
        for key, dst_path, _mode in extra_commits:
            replacer(staged[key], dst_path)
            replaced.append(key)
            fsync_dir(dst_path.parent)
        verify_installed_artifacts(
            bin_dir, INSTALL_BINARIES, shas, old_bins, expected_uids, stop_at,
            error_cls=InstallError, extra_targets=extra_commits,
        )
    except Exception as exc:
        reason = getattr(exc, "reason", None) or str(exc) or type(exc).__name__
        receipt = {"status": "INCOMPLETE", "descriptor_sha256": expected_sha,
                   "backup_dir": str(bak), "replaced": replaced, "reason": reason}
        return write_receipt_safe(rcpt, receipt, expected_uids, stop_at)
    receipt = {
        "status": "SUCCESS", "descriptor_sha256": expected_sha, "backup_dir": str(bak),
        "replaced": replaced, "image_digest": desc["image_digest"], "installed_hashes": shas,
    }
    return write_receipt_safe(rcpt, receipt, expected_uids, stop_at)


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
    desc = _verify_provenance_and_desc(
        pkg, expected_descriptor_sha256, expected_driver_commit, expected_driver_tree,
        bak, rcpt, allow_synthetic, expected_uids, stop_at, drv, driver_verifier, release_descriptor.verify_descriptor,
    )
    shas = _member_sha_map(desc)
    _verify_image_matches_package(desc, shas, image_extractor)
    try:
        host_facts = h_obs()
        q_chk()
    except HostObservationError as exc:
        raise InstallError(exc.status, exc.reason) from exc
    if set(host_facts.get("binaries", {})) != set(INSTALL_BINARIES):
        raise InstallError("REFUSED", "existing_installed_binaries_incomplete")
    old_bins = host_facts["binaries"]
    old_mfst = observe_optional_file(mfst, expected_uids, stop_at, error_cls=InstallError, tag="old_manifest")
    backup_release(bak, INSTALL_BINARIES, old_bins, old_mfst, expected_uids, stop_at,
                   fsync_dir_fn=lambda p: fsync_dir(p), error_cls=InstallError)
    staged: dict[str, Path] = {}
    try:
        staged = stage_artifacts(pkg, bin_dir, INSTALL_BINARIES, old_bins, shas, expected_uids, stop_at,
                                 fsync_dir_fn=lambda p: fsync_dir(p), error_cls=InstallError)
        q_chk()
    except (InstallError, HostObservationError) as exc:
        cleanup_staged(staged)
        return write_receipt_safe(rcpt, {"status": "REFUSED", "descriptor_sha256": expected_descriptor_sha256,
                                         "backup_dir": str(bak), "replaced": [], "reason": exc.reason},
                                  expected_uids, stop_at)
    return _commit_staged_install(bin_dir, bak, staged, shas, old_bins, desc, rcpt,
                                  expected_descriptor_sha256, expected_uids, stop_at, replacer)


def run_https_lab_install(
    *,
    expected_driver_commit: str,
    expected_driver_tree: str,
    expected_descriptor_sha256: str,
    expected_build_commit: str | None = None,
    expected_build_tree: str | None = None,
    package_dir: Path | None = None,
    install_bin_dir: Path | None = None,
    backup_dir: Path | None = None,
    manifest_path: Path | None = None,
    root_pem_path: Path | None = None,
    receipt_path: Path | None = None,
    driver_dir: Path | None = None,
    raw_env: dict[str, str] | None = None,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
    allow_synthetic: bool = False,
    driver_verifier=https_lab_host.verify_https_lab_driver_custody,
    host_observer=None,
    image_extractor=https_lab_host.extract_https_lab_image_digests,
    quiescence_checker=None,
    replacer=os.replace,
) -> dict:
    """Install verified HTTPS_LOOP_LAB binaries and root CA PEM after full preflight and backup."""
    reject_sensitive_env(raw_env, error_cls=lambda _s, r: InstallError("REFUSED", r))
    sc = https_lab_host.HTTPS_LAB_SCOPE
    pkg = package_dir or Path(sc["bundle_dir"])
    bin_dir = install_bin_dir or Path(sc["install_bin_dir"])
    bak = backup_dir or Path(sc["backup_dir"])
    mfst = manifest_path or Path(sc["manifest_path"])
    root_dst = root_pem_path or Path(sc["root_pem_path"])
    rcpt = receipt_path if receipt_path is not None else Path(sc["report_root"]) / "install-receipt.json"
    drv = driver_dir or Path(__file__).resolve().parent
    h_obs = host_observer or (lambda: observe_installed_facts(
        install_bin_dir=bin_dir, expected_uids=expected_uids, stop_at=stop_at, require_binaries=True,
    ))
    q_chk = quiescence_checker or (lambda: https_lab_host.check_https_lab_quiescence(
        pg_container=sc["pg_container"], candidate_db=sc["database"],
    ))
    desc = _verify_provenance_and_desc(
        pkg, expected_descriptor_sha256, expected_driver_commit, expected_driver_tree,
        bak, rcpt, allow_synthetic, expected_uids, stop_at, drv, driver_verifier,
        https_lab_release.verify_https_lab_descriptor,
        expected_build_commit=expected_build_commit, expected_build_tree=expected_build_tree,
    )
    shas = _member_sha_map(desc)
    _verify_image_matches_package(
        desc, shas, image_extractor, extra_image_shas={https_lab_host.ROOT_CA_IMAGE_PATH: https_lab_release.ROOT_CA_SHA256},
    )
    try:
        host_facts = h_obs()
        q_chk()
    except HostObservationError as exc:
        raise InstallError(exc.status, exc.reason) from exc
    if set(host_facts.get("binaries", {})) != set(INSTALL_BINARIES):
        raise InstallError("REFUSED", "existing_installed_binaries_incomplete")
    old_bins = host_facts["binaries"]
    validate_destination_dir(bin_dir, expected_uids, stop_at, error_cls=InstallError, tag="install_bin")
    r_uid, r_gid = validate_destination_dir(root_dst.parent, expected_uids, stop_at, error_cls=InstallError, tag="root_pem_dir")
    old_mfst = observe_optional_file(mfst, expected_uids, stop_at, error_cls=InstallError, tag="old_manifest")
    old_root = observe_optional_file(root_dst, expected_uids, stop_at, error_cls=InstallError, tag="old_root_pem")
    root_mode = old_root["mode"] if old_root is not None else 0o444
    root_owner = (old_root["uid"], old_root["gid"]) if old_root is not None else (r_uid, r_gid)
    root_member = https_lab_release.ROOT_CA_MEMBER
    backup_release(
        bak, INSTALL_BINARIES, old_bins, old_mfst, expected_uids, stop_at,
        fsync_dir_fn=lambda p: fsync_dir(p), error_cls=InstallError,
        extra_files={root_member: (root_member, old_root)},
    )
    staged: dict[str, Path] = {}
    try:
        staged = stage_artifacts(
            pkg, bin_dir, INSTALL_BINARIES, old_bins, shas, expected_uids, stop_at,
            fsync_dir_fn=lambda p: fsync_dir(p), error_cls=InstallError,
            extra_targets=((root_member, root_member, root_dst, root_mode, *root_owner),),
        )
        q_chk()
    except (InstallError, HostObservationError) as exc:
        cleanup_staged(staged)
        return write_receipt_safe(rcpt, {"status": "REFUSED", "descriptor_sha256": expected_descriptor_sha256,
                                         "backup_dir": str(bak), "replaced": [], "reason": exc.reason},
                                  expected_uids, stop_at)
    return _commit_staged_install(
        bin_dir, bak, staged, shas, old_bins, desc, rcpt, expected_descriptor_sha256,
        expected_uids, stop_at, replacer, extra_commits=((root_member, root_dst, root_mode),),
    )


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

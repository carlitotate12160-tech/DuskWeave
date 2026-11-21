#!/usr/bin/env python3
"""HTTPS LAB schema-0038 release packaging, descriptor checks, and source provenance."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path

from candidate_files import (
    FileCustodyError,
    create_exclusive_dir,
    fsync_dir,
    read_protected_bytes,
    write_exclusive_verified,
)
import release_descriptor
from release_descriptor import (
    HEX40_RE,
    HEX64_RE,
    IMAGE_DIGEST_RE,
    IMAGE_REF_RE,
    NOTICE_MEMBERS,
    PROD_BASELINE_PATHS,
    ReleaseError,
    _check_clean_git_identity,
    _no_dup_pairs_hook,
    _run_git,
    _sha256_file,
    _validate_member_entry,
    _walk_package_files,
    sanitize_release_env,
    verify_elf64_aarch64,
)

SCHEMA_REVISION = "0038"
PLATFORM = "linux/arm64"
ROOT_CA_MEMBER = "https-lab-root-v1.pem"
ROOT_CA_SOURCE_REL = "boundary/fixtures/https-lab-root-v1.pem"
ROOT_CA_VERSION = "duskweave-https-lab-root-v1"
ROOT_CA_SIZE = 473
ROOT_CA_SHA256 = "af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc"
BINARY_MEMBERS = ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter", "httpslab")
FIXED_MEMBERS = BINARY_MEMBERS + (ROOT_CA_MEMBER,) + NOTICE_MEMBERS
ALLOWED_DESCRIPTOR_KEYS = frozenset({
    "schema_version", "schema_revision", "platform", "build_source_commit", "build_source_tree",
    "driver_commit", "driver_tree", "image_ref", "image_digest",
    "https_lab_root_version", "https_lab_root_sha256", "synthetic_fixture", "members",
})


def _verify_root_ca_bytes(path: Path, expected_uids: tuple[int, ...], stop_at: Path) -> str:
    size, digest = _sha256_file(path, expected_uids=expected_uids, stop_at=stop_at)
    if size != ROOT_CA_SIZE or digest != ROOT_CA_SHA256:
        raise ReleaseError("https_lab_root_pem_mismatch")
    return digest


def verify_https_lab_source(
    source_root: Path,
    *,
    expected_commit: str | None = None,
    expected_tree: str | None = None,
    expected_driver_commit: str | None = None,
    expected_driver_tree: str | None = None,
    check_clean_git: bool = True,
    expected_uids: tuple[int, ...] | None = None,
    stop_at: Path | None = None,
) -> tuple[str, str]:
    """Verify clean HTTPS LAB source tree, absent .cargo/build.rs, and exact pinned root CA."""
    for forbidden, err in ((".cargo", "candidate_cargo_config_present"), ("build.rs", "unexpected_root_build_rs")):
        if (source_root / forbidden).exists() or (source_root / forbidden).is_symlink():
            raise ReleaseError(err)
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    anchor = stop_at or (Path("/") if os.geteuid() == 0 else source_root)
    _verify_root_ca_bytes(source_root / ROOT_CA_SOURCE_REL, uids, anchor)
    if not check_clean_git:
        return ("", "")
    head, tree = _check_clean_git_identity(
        source_root, expected_commit, expected_tree, expected_driver_commit, expected_driver_tree,
    )
    if _run_git(source_root, ["ls-files", "--others", "-i", "--exclude-standard", "--", *PROD_BASELINE_PATHS]).strip():
        raise ReleaseError("ignored_production_input_present")
    return head, tree


def parse_https_lab_descriptor_bytes(raw: bytes, *, allow_synthetic: bool = True) -> dict:
    """Strictly decode HTTPS LAB release-descriptor.json rejecting duplicates and drifted root/schema."""
    try:
        obj = json.loads(raw.decode("utf-8"), object_pairs_hook=_no_dup_pairs_hook)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseError("descriptor_json_malformed") from exc
    if not isinstance(obj, dict) or set(obj.keys()) != ALLOWED_DESCRIPTOR_KEYS:
        raise ReleaseError("descriptor_keys_refused")
    if obj["schema_version"] != 2 or obj["schema_revision"] != SCHEMA_REVISION or obj["platform"] != PLATFORM:
        raise ReleaseError("descriptor_schema_or_platform_refused")
    if obj["https_lab_root_version"] != ROOT_CA_VERSION or obj["https_lab_root_sha256"] != ROOT_CA_SHA256:
        raise ReleaseError("descriptor_root_ca_refused")
    for field, pattern in (
        ("build_source_commit", HEX40_RE), ("build_source_tree", HEX40_RE),
        ("driver_commit", HEX40_RE), ("driver_tree", HEX40_RE),
        ("image_ref", IMAGE_REF_RE), ("image_digest", IMAGE_DIGEST_RE),
    ):
        if not isinstance(obj[field], str) or not pattern.fullmatch(obj[field]):
            raise ReleaseError(f"descriptor_field_invalid:{field}")
    if type(obj["synthetic_fixture"]) is not bool or (obj["synthetic_fixture"] and not allow_synthetic):
        raise ReleaseError("descriptor_synthetic_refused")
    members = obj["members"]
    if not isinstance(members, list) or len(members) != len(FIXED_MEMBERS):
        raise ReleaseError("descriptor_member_count_invalid")
    if tuple(_validate_member_entry(m)[0] for m in members) != FIXED_MEMBERS:
        raise ReleaseError("descriptor_member_inventory_mismatch")
    return obj


def verify_https_lab_descriptor(
    package_dir: Path,
    expected_descriptor_sha256: str,
    *,
    allow_synthetic: bool = False,
    expected_uids: tuple[int, ...] | None = None,
    stop_at: Path | None = None,
) -> dict:
    """Verify closed 9-member HTTPS LAB package inventory, descriptor hash, ELFs, and root PEM."""
    if not HEX64_RE.fullmatch(expected_descriptor_sha256):
        raise ReleaseError("expected_descriptor_sha256_invalid")
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    anchor = stop_at or (Path("/") if os.geteuid() == 0 else package_dir)
    if _walk_package_files(package_dir, uids, anchor) != (set(FIXED_MEMBERS) | {"release-descriptor.json"}):
        raise ReleaseError("package_inventory_mismatch")
    try:
        desc_bytes = read_protected_bytes(
            package_dir / "release-descriptor.json", expected_uids=uids,
            allowed_modes=(0o444, 0o400), stop_at=anchor, status_on_error="REFUSED",
        )
    except FileCustodyError as exc:
        raise ReleaseError(exc.reason) from exc
    if hashlib.sha256(desc_bytes).hexdigest() != expected_descriptor_sha256:
        raise ReleaseError("descriptor_sha256_mismatch")
    desc = parse_https_lab_descriptor_bytes(desc_bytes, allow_synthetic=allow_synthetic)
    release_descriptor._verify_package_members(package_dir, desc["members"], uids, anchor, BINARY_MEMBERS)
    _verify_root_ca_bytes(package_dir / ROOT_CA_MEMBER, uids, anchor)
    return desc


def _write_lab_members(
    package_dir: Path,
    source_root: Path,
    artifact_dir: Path,
    uids: tuple[int, ...],
    pkg_anchor: Path,
) -> list[dict]:
    members = []
    art_anchor = pkg_anchor if pkg_anchor == Path("/") else artifact_dir
    src_anchor = pkg_anchor if pkg_anchor == Path("/") else source_root
    for rel in FIXED_MEMBERS:
        if rel in BINARY_MEMBERS:
            src, anchor, mode = artifact_dir / rel, art_anchor, 0o555
            verify_elf64_aarch64(src, expected_uids=uids, stop_at=anchor)
        elif rel == ROOT_CA_MEMBER:
            src, anchor, mode = source_root / ROOT_CA_SOURCE_REL, src_anchor, 0o444
            _verify_root_ca_bytes(src, uids, anchor)
        else:
            src, anchor, mode = source_root / rel, src_anchor, 0o444
        try:
            raw = read_protected_bytes(src, expected_uids=uids, stop_at=anchor, status_on_error="REFUSED")
            digest = write_exclusive_verified(package_dir / rel, raw, mode=mode, expected_uids=uids, stop_at=pkg_anchor)
        except FileCustodyError as exc:
            raise ReleaseError(exc.reason) from exc
        members.append({"path": rel, "size": len(raw), "sha256": digest})
    return members


def package_https_lab_release(
    *,
    package_dir: Path,
    source_root: Path,
    artifact_dir: Path,
    image_ref: str,
    image_digest: str,
    build_source_commit: str,
    build_source_tree: str,
    driver_commit: str,
    driver_tree: str,
    synthetic_fixture: bool = False,
    verify_git_baseline: bool = True,
    raw_env: dict[str, str] | None = None,
    expected_uids: tuple[int, ...] | None = None,
    stop_at: Path | None = None,
) -> dict:
    """Package exact 5 binaries, pinned root CA PEM, and notices with schema-0038 descriptor."""
    sanitize_release_env(raw_env)
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    pkg_anchor = stop_at or (Path("/") if os.geteuid() == 0 else package_dir.parent)
    if package_dir.exists() or package_dir.is_symlink():
        raise ReleaseError("package_dir_collision")
    verify_https_lab_source(
        source_root, expected_commit=build_source_commit, expected_tree=build_source_tree,
        expected_driver_commit=driver_commit, expected_driver_tree=driver_tree,
        check_clean_git=verify_git_baseline, expected_uids=uids,
        stop_at=pkg_anchor if pkg_anchor == Path("/") else source_root,
    )
    try:
        create_exclusive_dir(package_dir, mode=0o755, expected_uids=uids, stop_at=pkg_anchor)
        create_exclusive_dir(package_dir / "third_party", mode=0o755, expected_uids=uids, stop_at=pkg_anchor)
        create_exclusive_dir(package_dir / "third_party/genai", mode=0o755, expected_uids=uids, stop_at=pkg_anchor)
    except FileCustodyError as exc:
        raise ReleaseError(exc.reason) from exc
    members = _write_lab_members(package_dir, source_root, artifact_dir, uids, pkg_anchor)
    descriptor = {
        "schema_version": 2, "schema_revision": SCHEMA_REVISION, "platform": PLATFORM,
        "build_source_commit": build_source_commit, "build_source_tree": build_source_tree,
        "driver_commit": driver_commit, "driver_tree": driver_tree,
        "image_ref": image_ref, "image_digest": image_digest,
        "https_lab_root_version": ROOT_CA_VERSION, "https_lab_root_sha256": ROOT_CA_SHA256,
        "synthetic_fixture": synthetic_fixture, "members": members,
    }
    desc_bytes = (json.dumps(descriptor, sort_keys=True, indent=2) + "\n").encode("utf-8")
    try:
        desc_sha = write_exclusive_verified(
            package_dir / "release-descriptor.json", desc_bytes, mode=0o444, expected_uids=uids, stop_at=pkg_anchor,
        )
        fsync_dir(package_dir / "third_party/genai")
        fsync_dir(package_dir)
    except FileCustodyError as exc:
        raise ReleaseError(exc.reason) from exc
    return {"status": "SUCCESS", "package_dir": str(package_dir), "descriptor_sha256": desc_sha}


def _dispatch_cli(args) -> dict:
    if args.cmd == "package":
        return package_https_lab_release(
            package_dir=Path(args.package_dir), source_root=Path(args.source_root),
            artifact_dir=Path(args.artifact_dir), image_ref=args.image_ref, image_digest=args.image_digest,
            build_source_commit=args.build_source_commit, build_source_tree=args.build_source_tree,
            driver_commit=args.driver_commit, driver_tree=args.driver_tree,
        )
    if args.cmd == "verify-source":
        head, tree = verify_https_lab_source(
            Path(args.source_root), expected_commit=args.expected_commit, expected_tree=args.expected_tree,
            expected_driver_commit=args.expected_driver_commit, expected_driver_tree=args.expected_driver_tree,
        )
        return {"status": "SUCCESS", "head_commit": head, "head_tree": tree}
    return verify_https_lab_descriptor(args.package_dir, args.expected_descriptor_sha256)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="HTTPS LAB schema-0038 release packaging and verification")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_pkg = sub.add_parser("package")
    for flag in ("--package-dir", "--source-root", "--artifact-dir", "--image-ref", "--image-digest",
                 "--build-source-commit", "--build-source-tree", "--driver-commit", "--driver-tree"):
        p_pkg.add_argument(flag, required=True)
    p_ver = sub.add_parser("verify-descriptor")
    p_ver.add_argument("--package-dir", required=True, type=Path)
    p_ver.add_argument("--expected-descriptor-sha256", required=True)
    p_src = sub.add_parser("verify-source")
    p_src.add_argument("--source-root", required=True)
    for flag in ("--expected-commit", "--expected-tree", "--expected-driver-commit", "--expected-driver-tree"):
        p_src.add_argument(flag, required=False)
    try:
        out = _dispatch_cli(parser.parse_args(argv))
    except ReleaseError as exc:
        print(json.dumps({"status": "REFUSED", "reason": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(out, sort_keys=True))
    return 0 if out.get("status", "SUCCESS") == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())

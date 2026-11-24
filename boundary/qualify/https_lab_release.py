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
    ReleaseError,
    _no_dup_pairs_hook,
    _sha256_file,
    _validate_member_entry,
    _walk_package_files,
    sanitize_release_env,
    verify_elf64_aarch64,
    verify_pinned_source_tree,
)

SCHEMA_VERSION = 3
SCHEMA_REVISION = "0038"
PURPOSE = "HTTPS_LOOP_LAB"
PLATFORM = "linux/arm64"
RELEASE_BASELINE_COMMIT = "c2077f0e77ad99fa5f9ce1daa22b12dfb4d4a2b2"
RELEASE_BASELINE_TREE = "a421fef312e18e06ae3276db82bd6e3a38bed9ac"
ROOT_CA_MEMBER = "https-lab-root-v1.pem"
ROOT_CA_SOURCE_REL = "boundary/fixtures/https-lab-root-v1.pem"
ROOT_CA_VERSION = "duskweave-https-lab-root-v1"
ROOT_CA_SIZE = 473
ROOT_CA_SHA256 = "af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc"
BINARY_MEMBERS = ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter")
FIXED_MEMBERS = BINARY_MEMBERS + (ROOT_CA_MEMBER,) + NOTICE_MEMBERS
RESPONDER_BINARY = "httpslab-arm64"
ALLOWED_DESCRIPTOR_KEYS = frozenset({
    "schema_version", "schema_revision", "purpose", "platform",
    "release_baseline_commit", "release_baseline_tree",
    "build_source_commit", "build_source_tree", "driver_commit", "driver_tree",
    "image_ref", "image_digest", "https_lab_root_version", "https_lab_root_sha256",
    "synthetic_fixture", "members",
})
RESPONDER_PROV_KEYS = frozenset({
    "schema_version", "purpose", "binary", "size", "sha256",
    "release_baseline_commit", "release_baseline_tree",
    "build_source_commit", "build_source_tree", "driver_commit", "driver_tree",
})


def _validate_distinct_pins(obj: dict) -> None:
    if (obj.get("release_baseline_commit") != RELEASE_BASELINE_COMMIT
            or obj.get("release_baseline_tree") != RELEASE_BASELINE_TREE):
        raise ReleaseError("descriptor_baseline_refused")
    for field in ("build_source_commit", "build_source_tree", "driver_commit", "driver_tree"):
        val = obj.get(field)
        if not isinstance(val, str) or not HEX40_RE.fullmatch(val):
            raise ReleaseError(f"descriptor_field_invalid:{field}")
    bc, bt = obj["build_source_commit"], obj["build_source_tree"]
    dc, dt = obj["driver_commit"], obj["driver_tree"]
    if (bc == RELEASE_BASELINE_COMMIT or dc == RELEASE_BASELINE_COMMIT
            or bt == RELEASE_BASELINE_TREE or dt == RELEASE_BASELINE_TREE
            or bc == bt or dc == dt or bc == dt or dc == bt):
        raise ReleaseError("descriptor_identity_swap_or_baseline_reuse")


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
    """Verify clean HTTPS LAB source tree, absent root build.rs, narrow .cargo policy, and root CA."""
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    anchor = stop_at or (Path("/") if os.geteuid() == 0 else source_root)
    head, tree = verify_pinned_source_tree(
        source_root, expected_commit=expected_commit, expected_tree=expected_tree,
        expected_driver_commit=expected_driver_commit, expected_driver_tree=expected_driver_tree,
        check_clean_git=check_clean_git, expected_uids=uids, stop_at=anchor,
    )
    _verify_root_ca_bytes(source_root / ROOT_CA_SOURCE_REL, uids, anchor)
    return head, tree


def parse_https_lab_descriptor_bytes(raw: bytes, *, allow_synthetic: bool = True) -> dict:
    """Strictly decode schema-3 HTTPS_LOOP_LAB release-descriptor.json rejecting duplicates and drift."""
    try:
        obj = json.loads(raw.decode("utf-8"), object_pairs_hook=_no_dup_pairs_hook)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseError("descriptor_json_malformed") from exc
    if not isinstance(obj, dict) or set(obj.keys()) != ALLOWED_DESCRIPTOR_KEYS:
        raise ReleaseError("descriptor_keys_refused")
    if (obj["schema_version"] != SCHEMA_VERSION or obj["purpose"] != PURPOSE
            or obj["schema_revision"] != SCHEMA_REVISION or obj["platform"] != PLATFORM):
        raise ReleaseError("descriptor_schema_or_platform_refused")
    if obj["https_lab_root_version"] != ROOT_CA_VERSION or obj["https_lab_root_sha256"] != ROOT_CA_SHA256:
        raise ReleaseError("descriptor_root_ca_refused")
    _validate_distinct_pins(obj)
    for field, pattern in (("image_ref", IMAGE_REF_RE), ("image_digest", IMAGE_DIGEST_RE)):
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
    """Verify closed 8-member HTTPS LAB package inventory, descriptor hash, ELFs, and root PEM."""
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
    """Package exact 4 binaries, pinned root CA PEM, and notices with schema-3 HTTPS_LOOP_LAB descriptor."""
    sanitize_release_env(raw_env)
    descriptor = {
        "schema_version": SCHEMA_VERSION, "schema_revision": SCHEMA_REVISION,
        "purpose": PURPOSE, "platform": PLATFORM,
        "release_baseline_commit": RELEASE_BASELINE_COMMIT, "release_baseline_tree": RELEASE_BASELINE_TREE,
        "build_source_commit": build_source_commit, "build_source_tree": build_source_tree,
        "driver_commit": driver_commit, "driver_tree": driver_tree,
        "image_ref": image_ref, "image_digest": image_digest,
        "https_lab_root_version": ROOT_CA_VERSION, "https_lab_root_sha256": ROOT_CA_SHA256,
        "synthetic_fixture": synthetic_fixture,
    }
    _validate_distinct_pins(descriptor)
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
    descriptor["members"] = _write_lab_members(package_dir, source_root, artifact_dir, uids, pkg_anchor)
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


def record_responder_provenance(
    *,
    binary_path: Path,
    provenance_path: Path,
    build_source_commit: str,
    build_source_tree: str,
    driver_commit: str,
    driver_tree: str,
    expected_uids: tuple[int, ...] | None = None,
    stop_at: Path | None = None,
) -> dict:
    """Record fail-closed provenance for the standalone qualification httpslab-arm64 responder."""
    prov = {
        "schema_version": SCHEMA_VERSION, "purpose": PURPOSE, "binary": RESPONDER_BINARY,
        "release_baseline_commit": RELEASE_BASELINE_COMMIT, "release_baseline_tree": RELEASE_BASELINE_TREE,
        "build_source_commit": build_source_commit, "build_source_tree": build_source_tree,
        "driver_commit": driver_commit, "driver_tree": driver_tree,
    }
    _validate_distinct_pins(prov)
    if provenance_path.exists() or provenance_path.is_symlink():
        raise ReleaseError("responder_provenance_collision")
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    anchor = stop_at or (Path("/") if os.geteuid() == 0 else binary_path.parent)
    verify_elf64_aarch64(binary_path, expected_uids=uids, stop_at=anchor)
    try:
        raw = read_protected_bytes(binary_path, expected_uids=uids, allowed_modes=(0o555, 0o500), stop_at=anchor, status_on_error="REFUSED")
        prov.update({"size": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        p_bytes = (json.dumps(prov, sort_keys=True, indent=2) + "\n").encode("utf-8")
        p_anchor = stop_at or (Path("/") if os.geteuid() == 0 else provenance_path.parent)
        p_sha = write_exclusive_verified(provenance_path, p_bytes, mode=0o444, expected_uids=uids, stop_at=p_anchor)
        fsync_dir(provenance_path.parent)
    except FileCustodyError as exc:
        raise ReleaseError(exc.reason) from exc
    return {"status": "SUCCESS", "binary_path": str(binary_path), "provenance_path": str(provenance_path),
            "sha256": prov["sha256"], "size": prov["size"], "provenance_sha256": p_sha}


def verify_responder_provenance(
    *,
    binary_path: Path,
    provenance_path: Path,
    expected_build_commit: str | None = None,
    expected_build_tree: str | None = None,
    expected_driver_commit: str | None = None,
    expected_driver_tree: str | None = None,
    expected_uids: tuple[int, ...] | None = None,
    stop_at: Path | None = None,
) -> dict:
    """Verify standalone qualification httpslab-arm64 responder binary bytes and provenance JSON."""
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    p_anchor = stop_at or (Path("/") if os.geteuid() == 0 else provenance_path.parent)
    try:
        raw_p = read_protected_bytes(provenance_path, expected_uids=uids, allowed_modes=(0o444, 0o400), stop_at=p_anchor, status_on_error="REFUSED")
        obj = json.loads(raw_p.decode("utf-8"), object_pairs_hook=_no_dup_pairs_hook)
    except (FileCustodyError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseError(getattr(exc, "reason", "responder_provenance_malformed")) from exc
    if not isinstance(obj, dict) or set(obj.keys()) != RESPONDER_PROV_KEYS:
        raise ReleaseError("responder_provenance_keys_refused")
    if obj["schema_version"] != SCHEMA_VERSION or obj["purpose"] != PURPOSE or obj["binary"] != RESPONDER_BINARY:
        raise ReleaseError("responder_provenance_scope_refused")
    _validate_distinct_pins(obj)
    for k, exp in (("build_source_commit", expected_build_commit), ("build_source_tree", expected_build_tree),
                   ("driver_commit", expected_driver_commit), ("driver_tree", expected_driver_tree)):
        if exp is not None and obj[k] != exp:
            raise ReleaseError(f"responder_pin_mismatch:{k}")
    b_anchor = stop_at or (Path("/") if os.geteuid() == 0 else binary_path.parent)
    verify_elf64_aarch64(binary_path, expected_uids=uids, stop_at=b_anchor)
    try:
        b_raw = read_protected_bytes(binary_path, expected_uids=uids, allowed_modes=(0o555, 0o500), stop_at=b_anchor, status_on_error="REFUSED")
    except FileCustodyError as exc:
        raise ReleaseError(exc.reason) from exc
    if len(b_raw) != obj["size"] or hashlib.sha256(b_raw).hexdigest() != obj["sha256"]:
        raise ReleaseError("responder_binary_bytes_mismatch")
    return {"status": "SUCCESS", "provenance": obj}


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
    if args.cmd == "record-responder":
        return record_responder_provenance(
            binary_path=Path(args.binary_path), provenance_path=Path(args.provenance_path),
            build_source_commit=args.build_source_commit, build_source_tree=args.build_source_tree,
            driver_commit=args.driver_commit, driver_tree=args.driver_tree,
        )
    if args.cmd == "verify-responder":
        return verify_responder_provenance(
            binary_path=Path(args.binary_path), provenance_path=Path(args.provenance_path),
            expected_build_commit=args.expected_build_commit, expected_build_tree=args.expected_build_tree,
            expected_driver_commit=args.expected_driver_commit, expected_driver_tree=args.expected_driver_tree,
        )
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
    for cmd_name, req in (("record-responder", True), ("verify-responder", False)):
        p_rsp = sub.add_parser(cmd_name)
        p_rsp.add_argument("--binary-path", required=True)
        p_rsp.add_argument("--provenance-path", required=True)
        pfx = "--" if req else "--expected-"
        for flag in (f"{pfx}build-source-commit" if req else "--expected-build-commit",
                     f"{pfx}build-source-tree" if req else "--expected-build-tree",
                     f"{pfx}driver-commit", f"{pfx}driver-tree"):
            p_rsp.add_argument(flag, required=req)
    try:
        out = _dispatch_cli(parser.parse_args(argv))
    except ReleaseError as exc:
        print(json.dumps({"status": "REFUSED", "reason": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(out, sort_keys=True))
    return 0 if out.get("status", "SUCCESS") == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())

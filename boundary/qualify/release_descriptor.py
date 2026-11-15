#!/usr/bin/env python3
"""Candidate PR154-baseline release packaging, descriptor checks, and source provenance."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import struct
import subprocess

from candidate_files import (
    FileCustodyError,
    create_exclusive_dir,
    fsync_dir,
    read_protected_bytes,
    reject_sensitive_env,
    require_safe_ancestors,
    write_exclusive_verified,
)

PR154_BASELINE_COMMIT = "850b8b3b3a161ebfa051830fc5037bd5aeff04d0"
PR154_BASELINE_TREE = "0c2d228b9c303959ab9a7a22d54fb852886bf104"
SCHEMA_REVISION = "0035"
PLATFORM = "linux/arm64"
BINARY_MEMBERS = ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter")
NOTICE_MEMBERS = (
    "third_party/genai/LICENSE-APACHE",
    "third_party/genai/LICENSE-MIT",
    "third_party/genai/source-manifest.json",
)
FIXED_MEMBERS = BINARY_MEMBERS + NOTICE_MEMBERS
ALLOWED_DESCRIPTOR_KEYS = frozenset({
    "schema_version", "schema_revision", "platform", "build_source_commit", "build_source_tree",
    "driver_commit", "driver_tree", "pr154_baseline_commit", "pr154_baseline_tree",
    "image_ref", "image_digest", "synthetic_fixture", "members",
})
ALLOWED_MEMBER_KEYS = frozenset({"path", "size", "sha256"})
HEX64_RE = re.compile(r"^[0-9a-f]{64}$")
HEX40_RE = re.compile(r"^[0-9a-f]{40}$")
IMAGE_DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
IMAGE_REF_RE = re.compile(r"^[a-z0-9][a-z0-9_.-]*:[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$")
QUAL_TEST_TAIL = (
    '#[cfg(test)]\n#[path = "../../../tests/m1_live_dns/units/candidate_binding.rs"]\n'
    "mod candidate_binding_tests;\n"
)
PROD_BASELINE_PATHS = (
    "src", "migrations", "boundary/cmd", "boundary/internal", "boundary/Dockerfile",
    "boundary/go.mod", "boundary/go.sum", "Cargo.toml", "Cargo.lock",
    "go.mod", "go.sum", "third_party", "build.rs", ".cargo",
)
ALLOWED_NON_PR154_PREFIXES = ("boundary/qualify/", "tests/", "docs/")


class ReleaseError(RuntimeError):
    """Fail-closed release verification, provenance, or packaging error."""


def sanitize_release_env(raw_env: dict[str, str] | None = None) -> None:
    """Refuse secret-bearing or override env variables before release operations."""
    try:
        reject_sensitive_env(raw_env, error_cls=ReleaseError)
    except ReleaseError:
        raise


def _sha256_file(path: Path, *, expected_uids: tuple[int, ...], stop_at: Path) -> tuple[int, str]:
    try:
        data = read_protected_bytes(path, expected_uids=expected_uids, stop_at=stop_at, status_on_error="REFUSED")
    except FileCustodyError as exc:
        raise ReleaseError(exc.reason) from exc
    return len(data), hashlib.sha256(data).hexdigest()


def verify_elf64_aarch64(path: Path, *, expected_uids: tuple[int, ...] | None = None,
                         stop_at: Path | None = None) -> None:
    """Verify actual ELF64 little-endian AArch64 executable header using safe custody."""
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    anchor = stop_at or (Path("/") if os.geteuid() == 0 else path.parent)
    try:
        hdr = read_protected_bytes(path, expected_uids=uids, stop_at=anchor, status_on_error="REFUSED")[:64]
    except FileCustodyError as exc:
        raise ReleaseError(f"elf_header_refused:{path.name}:{exc.reason}") from exc
    if len(hdr) < 64 or hdr[:4] != b"\x7fELF" or hdr[4] != 2 or hdr[5] != 1:
        raise ReleaseError(f"elf_class_or_endian_refused:{path.name}")
    if struct.unpack_from("<H", hdr, 18)[0] != 183:
        raise ReleaseError(f"elf_arch_not_aarch64:{path.name}")


def _no_dup_pairs_hook(pairs: list[tuple[str, object]]) -> dict[str, object]:
    out: dict[str, object] = {}
    for key, val in pairs:
        if key in out:
            raise ReleaseError(f"duplicate_json_key:{key}")
        out[key] = val
    return out


def _validate_member_entry(entry: object) -> tuple[str, int, str]:
    if not isinstance(entry, dict) or set(entry.keys()) != ALLOWED_MEMBER_KEYS:
        raise ReleaseError("malformed_member_keys")
    path, size, digest = entry["path"], entry["size"], entry["sha256"]
    if not isinstance(path, str) or type(size) is not int or size <= 0:
        raise ReleaseError("malformed_member_fields")
    if not isinstance(digest, str) or not HEX64_RE.fullmatch(digest):
        raise ReleaseError("malformed_member_digest")
    return path, size, digest


def _validate_descriptor_metadata(obj: dict, allow_synthetic: bool) -> None:
    if obj["schema_version"] != 1 or obj["schema_revision"] != SCHEMA_REVISION or obj["platform"] != PLATFORM:
        raise ReleaseError("descriptor_schema_or_platform_refused")
    if obj["pr154_baseline_commit"] != PR154_BASELINE_COMMIT or obj["pr154_baseline_tree"] != PR154_BASELINE_TREE:
        raise ReleaseError("descriptor_baseline_refused")
    checks = (
        ("build_source_commit", HEX40_RE), ("build_source_tree", HEX40_RE),
        ("driver_commit", HEX40_RE), ("driver_tree", HEX40_RE),
        ("image_ref", IMAGE_REF_RE), ("image_digest", IMAGE_DIGEST_RE),
    )
    for field, pattern in checks:
        if not isinstance(obj[field], str) or not pattern.fullmatch(obj[field]):
            raise ReleaseError(f"descriptor_field_invalid:{field}")
    if type(obj["synthetic_fixture"]) is not bool or (obj["synthetic_fixture"] and not allow_synthetic):
        raise ReleaseError("descriptor_synthetic_refused")


def parse_descriptor_bytes(raw: bytes, *, allow_synthetic: bool = True) -> dict:
    """Strictly decode release-descriptor.json rejecting duplicates and unknown keys."""
    try:
        obj = json.loads(raw.decode("utf-8"), object_pairs_hook=_no_dup_pairs_hook)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseError("descriptor_json_malformed") from exc
    if not isinstance(obj, dict) or set(obj.keys()) != ALLOWED_DESCRIPTOR_KEYS:
        raise ReleaseError("descriptor_keys_refused")
    _validate_descriptor_metadata(obj, allow_synthetic)
    members = obj["members"]
    if not isinstance(members, list) or len(members) != len(FIXED_MEMBERS):
        raise ReleaseError("descriptor_member_count_invalid")
    if tuple(_validate_member_entry(m)[0] for m in members) != FIXED_MEMBERS:
        raise ReleaseError("descriptor_member_inventory_mismatch")
    return obj


def _run_git(source_root: Path, args: list[str]) -> str:
    clean_git_env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        proc = subprocess.run(["git", "-C", str(source_root), *args], capture_output=True, text=True, env=clean_git_env, timeout=20)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ReleaseError("git_provenance_invocation_failed") from exc
    if proc.returncode != 0:
        raise ReleaseError("git_provenance_command_failed")
    return proc.stdout


def _check_clean_git_identity(source_root: Path, expected_commit: str | None, expected_tree: str | None,
                              expected_driver_commit: str | None, expected_driver_tree: str | None) -> tuple[str, str]:
    if _run_git(source_root, ["rev-parse", "--is-inside-work-tree"]).strip() != "true":
        raise ReleaseError("source_not_git_worktree")
    head = _run_git(source_root, ["rev-parse", "HEAD"]).strip()
    tree = _run_git(source_root, ["rev-parse", "HEAD^{tree}"]).strip()
    if not HEX40_RE.fullmatch(head) or not HEX40_RE.fullmatch(tree):
        raise ReleaseError("git_head_identity_invalid")
    for expected, actual, err in (
        (expected_commit, head, "source_commit_mismatch"), (expected_tree, tree, "source_tree_mismatch"),
        (expected_driver_commit, head, "driver_commit_mismatch"), (expected_driver_tree, tree, "driver_tree_mismatch"),
    ):
        if expected is not None and expected != actual:
            raise ReleaseError(err)
    if _run_git(source_root, ["status", "--porcelain=v1", "--untracked-files=all"]).strip():
        raise ReleaseError("dirty_source_worktree")
    if _run_git(source_root, ["ls-files", "--others", "-i", "--exclude-standard", "--", *PROD_BASELINE_PATHS]).strip():
        raise ReleaseError("ignored_production_input_present")
    return head, tree


def _check_qualification_rs(source_root: Path) -> None:
    qual_path = source_root / "src/bin/dw-boundary-launcher/qualification.rs"
    head_blob = _run_git(source_root, ["cat-file", "blob", f"{PR154_BASELINE_COMMIT}:src/bin/dw-boundary-launcher/qualification.rs"])
    cur_text = qual_path.read_text(encoding="utf-8")
    if cur_text not in (head_blob, head_blob + "\n" + QUAL_TEST_TAIL):
        raise ReleaseError("qualification_rs_production_drift")


def _check_pr154_diff(source_root: Path) -> None:
    prod_changed = [ln.strip() for ln in _run_git(
        source_root, ["diff", "--name-only", PR154_BASELINE_COMMIT, "--", *PROD_BASELINE_PATHS]
    ).splitlines() if ln.strip()]
    if prod_changed and prod_changed != ["src/bin/dw-boundary-launcher/qualification.rs"]:
        raise ReleaseError(f"production_source_drift:{','.join(prod_changed)}")
    all_changed = [ln.strip() for ln in _run_git(
        source_root, ["diff", "--name-only", PR154_BASELINE_COMMIT]
    ).splitlines() if ln.strip()]
    for path in all_changed:
        if path != "src/bin/dw-boundary-launcher/qualification.rs" and not path.startswith(ALLOWED_NON_PR154_PREFIXES):
            raise ReleaseError(f"unexpected_candidate_diff:{path}")
    if prod_changed:
        _check_qualification_rs(source_root)


def verify_production_matches_pr154(
    source_root: Path, *, expected_commit: str | None = None, expected_tree: str | None = None,
    expected_driver_commit: str | None = None, expected_driver_tree: str | None = None,
    check_clean_git: bool = True,
) -> tuple[str, str]:
    """Verify runtime/SQL/build inputs match PR154 cleanly and check exact commit/tree identity."""
    for forbidden, err in ((".cargo", "candidate_cargo_config_present"), ("build.rs", "unexpected_root_build_rs")):
        if (source_root / forbidden).exists() or (source_root / forbidden).is_symlink():
            raise ReleaseError(err)
    head, tree = ("", "")
    if check_clean_git:
        head, tree = _check_clean_git_identity(
            source_root, expected_commit, expected_tree, expected_driver_commit, expected_driver_tree
        )
    _check_pr154_diff(source_root)
    return head, tree


def _check_safe_lstat(path: Path, entry: str, require_file: bool, expected_uids: tuple[int, ...]) -> None:
    if entry.startswith("."):
        raise ReleaseError(f"hidden_package_entry:{entry}")
    st = os.lstat(path)
    if stat.S_ISLNK(st.st_mode) or st.st_uid not in expected_uids or (st.st_mode & 0o022) != 0:
        raise ReleaseError(f"unsafe_package_entry:{entry}")
    if require_file and (not stat.S_ISREG(st.st_mode) or st.st_nlink != 1):
        raise ReleaseError(f"unsafe_package_file:{entry}")


def _walk_package_files(package_dir: Path, expected_uids: tuple[int, ...], stop_at: Path) -> set[str]:
    try:
        require_safe_ancestors(package_dir, expected_uids, stop_at=stop_at)
    except FileCustodyError as exc:
        raise ReleaseError(f"package_dir_invalid:{exc.reason}") from exc
    files: set[str] = set()
    allowed_dirs = {"third_party", "third_party/genai"}
    for root, dirs, names in os.walk(package_dir, followlinks=False):
        rel_dir = Path(root).relative_to(package_dir).as_posix()
        for d in dirs:
            drel = d if rel_dir == "." else f"{rel_dir}/{d}"
            _check_safe_lstat(Path(root) / d, d, False, expected_uids)
            if drel not in allowed_dirs:
                raise ReleaseError(f"extra_package_dir:{drel}")
        for n in names:
            frel = n if rel_dir == "." else f"{rel_dir}/{n}"
            _check_safe_lstat(Path(root) / n, n, True, expected_uids)
            files.add(frel)
    return files


def _verify_package_members(package_dir: Path, members: list[dict], uids: tuple[int, ...], anchor: Path) -> None:
    for member in members:
        mpath = package_dir / member["path"]
        size, digest = _sha256_file(mpath, expected_uids=uids, stop_at=anchor)
        if size != member["size"] or digest != member["sha256"]:
            raise ReleaseError(f"member_digest_mismatch:{member['path']}")
        if member["path"] in BINARY_MEMBERS:
            verify_elf64_aarch64(mpath, expected_uids=uids, stop_at=anchor)


def verify_descriptor(package_dir: Path, expected_descriptor_sha256: str, *, allow_synthetic: bool = False,
                      expected_uids: tuple[int, ...] | None = None, stop_at: Path | None = None) -> dict:
    """Verify closed package inventory, external descriptor hash, member hashes, and ELF headers."""
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
    desc = parse_descriptor_bytes(desc_bytes, allow_synthetic=allow_synthetic)
    _verify_package_members(package_dir, desc["members"], uids, anchor)
    return desc


def package_release(*, package_dir: Path, source_root: Path, artifact_dir: Path,
                    image_ref: str, image_digest: str, build_source_commit: str,
                    build_source_tree: str, driver_commit: str, driver_tree: str,
                    synthetic_fixture: bool = False, verify_git_baseline: bool = True,
                    raw_env: dict[str, str] | None = None,
                    expected_uids: tuple[int, ...] | None = None, stop_at: Path | None = None) -> dict:
    """Package exact binaries and notices with release-descriptor.json in a fresh directory."""
    sanitize_release_env(raw_env)
    uids = expected_uids if expected_uids is not None else (0, os.geteuid())
    pkg_anchor = stop_at or (Path("/") if os.geteuid() == 0 else package_dir.parent)
    if package_dir.exists() or package_dir.is_symlink():
        raise ReleaseError("package_dir_collision")
    if verify_git_baseline:
        verify_production_matches_pr154(
            source_root, expected_commit=build_source_commit, expected_tree=build_source_tree,
            expected_driver_commit=driver_commit, expected_driver_tree=driver_tree, check_clean_git=True,
        )
    try:
        create_exclusive_dir(package_dir, mode=0o755, expected_uids=uids, stop_at=pkg_anchor)
        create_exclusive_dir(package_dir / "third_party", mode=0o755, expected_uids=uids, stop_at=pkg_anchor)
        create_exclusive_dir(package_dir / "third_party/genai", mode=0o755, expected_uids=uids, stop_at=pkg_anchor)
    except FileCustodyError as exc:
        raise ReleaseError(exc.reason) from exc
    members = _write_package_members(package_dir, source_root, artifact_dir, uids, pkg_anchor)
    descriptor = {
        "schema_version": 1, "schema_revision": SCHEMA_REVISION, "platform": PLATFORM,
        "build_source_commit": build_source_commit, "build_source_tree": build_source_tree,
        "driver_commit": driver_commit, "driver_tree": driver_tree,
        "pr154_baseline_commit": PR154_BASELINE_COMMIT, "pr154_baseline_tree": PR154_BASELINE_TREE,
        "image_ref": image_ref, "image_digest": image_digest,
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


def _write_package_members(package_dir: Path, source_root: Path, artifact_dir: Path,
                           uids: tuple[int, ...], pkg_anchor: Path) -> list[dict]:
    members = []
    art_anchor = pkg_anchor if pkg_anchor == Path("/") else artifact_dir
    src_anchor = pkg_anchor if pkg_anchor == Path("/") else source_root
    for rel in BINARY_MEMBERS + NOTICE_MEMBERS:
        src = (artifact_dir / rel) if rel in BINARY_MEMBERS else (source_root / rel)
        anchor = art_anchor if rel in BINARY_MEMBERS else src_anchor
        if rel in BINARY_MEMBERS:
            verify_elf64_aarch64(src, expected_uids=uids, stop_at=anchor)
        try:
            raw = read_protected_bytes(src, expected_uids=uids, stop_at=anchor, status_on_error="REFUSED")
            mode = 0o555 if rel in BINARY_MEMBERS else 0o444
            digest = write_exclusive_verified(package_dir / rel, raw, mode=mode, expected_uids=uids, stop_at=pkg_anchor)
        except FileCustodyError as exc:
            raise ReleaseError(exc.reason) from exc
        members.append({"path": rel, "size": len(raw), "sha256": digest})
    return members


def _dispatch_cli(args) -> dict:
    if args.cmd == "package":
        return package_release(
            package_dir=Path(args.package_dir), source_root=Path(args.source_root),
            artifact_dir=Path(args.artifact_dir), image_ref=args.image_ref, image_digest=args.image_digest,
            build_source_commit=args.build_source_commit, build_source_tree=args.build_source_tree,
            driver_commit=args.driver_commit, driver_tree=args.driver_tree,
        )
    if args.cmd == "verify-pr154":
        head, tree = verify_production_matches_pr154(
            Path(args.source_root), expected_commit=args.expected_commit, expected_tree=args.expected_tree,
            expected_driver_commit=args.expected_driver_commit, expected_driver_tree=args.expected_driver_tree,
        )
        return {"status": "SUCCESS", "head_commit": head, "head_tree": tree}
    return verify_descriptor(args.package_dir, args.expected_descriptor_sha256)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Candidate release packaging, verification, and provenance")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_pkg = sub.add_parser("package")
    for flag in ("--package-dir", "--source-root", "--artifact-dir", "--image-ref", "--image-digest",
                 "--build-source-commit", "--build-source-tree", "--driver-commit", "--driver-tree"):
        p_pkg.add_argument(flag, required=True)
    p_ver = sub.add_parser("verify-descriptor")
    p_ver.add_argument("--package-dir", required=True, type=Path)
    p_ver.add_argument("--expected-descriptor-sha256", required=True)
    p_p = sub.add_parser("verify-pr154")
    p_p.add_argument("--source-root", required=True)
    for flag in ("--expected-commit", "--expected-tree", "--expected-driver-commit", "--expected-driver-tree"):
        p_p.add_argument(flag, required=False)
    try:
        out = _dispatch_cli(parser.parse_args(argv))
    except ReleaseError as exc:
        print(json.dumps({"status": "REFUSED", "reason": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(out, sort_keys=True))
    return 0 if out.get("status", "SUCCESS") == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())

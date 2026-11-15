#!/usr/bin/env python3
"""Candidate PR154-baseline release packaging, ELF/descriptor checks, and install."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import struct
import subprocess
from pathlib import Path

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
    "schema_version",
    "schema_revision",
    "platform",
    "build_source_commit",
    "build_source_tree",
    "driver_commit",
    "driver_tree",
    "pr154_baseline_commit",
    "pr154_baseline_tree",
    "image_ref",
    "image_digest",
    "synthetic_fixture",
    "members",
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
    "src", "migrations", "boundary/cmd", "boundary/internal",
    "boundary/Dockerfile", "Cargo.toml", "Cargo.lock", "go.mod", "go.sum", "third_party",
)


class ReleaseError(RuntimeError):
    """Fail-closed release verification or packaging error."""


def sanitize_release_env(raw_env: dict[str, str] | None = None) -> None:
    """Refuse secret-bearing or override env variables before release operations."""
    env = os.environ if raw_env is None else raw_env
    for key in env:
        if key == "DW_TEST_FIXTURE_SUPERVISOR":
            continue
        if key == "DEEPSEEK_API_KEY" or key.startswith("DW_") or key.endswith("_FILE"):
            raise ReleaseError(f"forbidden_release_env:{key}")


def _sha256_file(path: Path) -> tuple[int, str]:
    st = os.lstat(path)
    if stat.S_ISLNK(st.st_mode) or not stat.S_ISREG(st.st_mode) or st.st_nlink != 1:
        raise ReleaseError(f"unsafe_regular_file:{path}")
    data = path.read_bytes()
    return len(data), hashlib.sha256(data).hexdigest()


def verify_elf64_aarch64(path: Path) -> None:
    """Verify actual ELF64 little-endian AArch64 executable header."""
    st = os.lstat(path)
    if stat.S_ISLNK(st.st_mode) or not stat.S_ISREG(st.st_mode) or st.st_size < 64:
        raise ReleaseError(f"elf_header_refused:{path.name}")
    with path.open("rb") as handle:
        hdr = handle.read(64)
    if hdr[:4] != b"\x7fELF" or hdr[4] != 2 or hdr[5] != 1:
        raise ReleaseError(f"elf_class_or_endian_refused:{path.name}")
    machine = struct.unpack_from("<H", hdr, 18)[0]
    if machine != 183:
        raise ReleaseError(f"elf_arch_not_aarch64:{path.name}:{machine}")


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
    parsed_paths = [_validate_member_entry(m)[0] for m in members]
    if tuple(parsed_paths) != FIXED_MEMBERS:
        raise ReleaseError("descriptor_member_inventory_mismatch")
    return obj


def _fsync_dir(directory: Path) -> None:
    fd = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _write_exclusive(path: Path, data: bytes, mode: int) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, flags, mode)
    try:
        os.write(fd, data)
        os.fchmod(fd, mode)
        os.fsync(fd)
    finally:
        os.close(fd)


def verify_production_matches_pr154(source_root: Path) -> None:
    """Verify runtime/SQL/worker sources match PR154 baseline except qualification cfg(test) tail."""
    proc = subprocess.run(
        ["git", "-C", str(source_root), "diff", "--name-only", PR154_BASELINE_COMMIT, "--", *PROD_BASELINE_PATHS],
        capture_output=True, text=True, timeout=20,
    )
    if proc.returncode != 0:
        raise ReleaseError("git_baseline_diff_failed")
    changed = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
    if changed and changed != ["src/bin/dw-boundary-launcher/qualification.rs"]:
        raise ReleaseError(f"production_source_drift:{','.join(changed)}")
    if changed:
        qual_path = source_root / "src/bin/dw-boundary-launcher/qualification.rs"
        head_blob = subprocess.check_output(
            ["git", "-C", str(source_root), "cat-file", "blob",
             f"{PR154_BASELINE_COMMIT}:src/bin/dw-boundary-launcher/qualification.rs"]
        ).decode("utf-8")
        cur_text = qual_path.read_text(encoding="utf-8")
        if cur_text.replace(QUAL_TEST_TAIL, "") != head_blob and cur_text != head_blob:
            raise ReleaseError("qualification_rs_production_drift")


def _check_safe_lstat(path: Path, entry: str, require_file: bool) -> None:
    if entry.startswith("."):
        raise ReleaseError(f"hidden_package_entry:{entry}")
    st = os.lstat(path)
    if stat.S_ISLNK(st.st_mode):
        raise ReleaseError(f"symlink_package_entry:{entry}")
    if require_file and (not stat.S_ISREG(st.st_mode) or st.st_nlink != 1):
        raise ReleaseError(f"unsafe_package_file:{entry}")


def _walk_package_files(package_dir: Path) -> set[str]:
    if not package_dir.is_dir() or package_dir.is_symlink():
        raise ReleaseError("package_dir_invalid")
    files: set[str] = set()
    allowed_dirs = {"third_party", "third_party/genai"}
    for root, dirs, names in os.walk(package_dir, followlinks=False):
        rel_dir = Path(root).relative_to(package_dir).as_posix()
        for d in dirs:
            drel = d if rel_dir == "." else f"{rel_dir}/{d}"
            _check_safe_lstat(Path(root) / d, d, False)
            if drel not in allowed_dirs:
                raise ReleaseError(f"extra_package_dir:{drel}")
        for n in names:
            frel = n if rel_dir == "." else f"{rel_dir}/{n}"
            _check_safe_lstat(Path(root) / n, n, True)
            files.add(frel)
    return files


def verify_descriptor(package_dir: Path, expected_descriptor_sha256: str, *,
                      allow_synthetic: bool = False) -> dict:
    """Verify closed package inventory, external descriptor hash, member hashes, and ELF headers."""
    if not HEX64_RE.fullmatch(expected_descriptor_sha256):
        raise ReleaseError("expected_descriptor_sha256_invalid")
    files = _walk_package_files(package_dir)
    expected_files = set(FIXED_MEMBERS) | {"release-descriptor.json"}
    if files != expected_files:
        raise ReleaseError("package_inventory_mismatch")
    desc_bytes = (package_dir / "release-descriptor.json").read_bytes()
    actual_desc_sha = hashlib.sha256(desc_bytes).hexdigest()
    if actual_desc_sha != expected_descriptor_sha256:
        raise ReleaseError("descriptor_sha256_mismatch")
    desc = parse_descriptor_bytes(desc_bytes, allow_synthetic=allow_synthetic)
    for member in desc["members"]:
        mpath = package_dir / member["path"]
        size, digest = _sha256_file(mpath)
        if size != member["size"] or digest != member["sha256"]:
            raise ReleaseError(f"member_digest_mismatch:{member['path']}")
        if member["path"] in BINARY_MEMBERS:
            verify_elf64_aarch64(mpath)
    return desc


def package_release(*, package_dir: Path, source_root: Path, artifact_dir: Path,
                    image_ref: str, image_digest: str, build_source_commit: str,
                    build_source_tree: str, driver_commit: str, driver_tree: str,
                    synthetic_fixture: bool = False, verify_git_baseline: bool = True,
                    raw_env: dict[str, str] | None = None) -> dict:
    """Package exact binaries and notices with release-descriptor.json in a fresh directory."""
    sanitize_release_env(raw_env)
    if package_dir.exists() or package_dir.is_symlink():
        raise ReleaseError("package_dir_collision")
    if verify_git_baseline:
        verify_production_matches_pr154(source_root)
    (package_dir / "third_party" / "genai").mkdir(parents=True, mode=0o755)
    members = []
    for rel in FIXED_MEMBERS:
        src = (artifact_dir / rel) if rel in BINARY_MEMBERS else (source_root / rel)
        size, digest = _sha256_file(src)
        if rel in BINARY_MEMBERS:
            verify_elf64_aarch64(src)
        mode = 0o555 if rel in BINARY_MEMBERS else 0o444
        _write_exclusive(package_dir / rel, src.read_bytes(), mode)
        members.append({"path": rel, "size": size, "sha256": digest})
    descriptor = {
        "schema_version": 1,
        "schema_revision": SCHEMA_REVISION,
        "platform": PLATFORM,
        "build_source_commit": build_source_commit,
        "build_source_tree": build_source_tree,
        "driver_commit": driver_commit,
        "driver_tree": driver_tree,
        "pr154_baseline_commit": PR154_BASELINE_COMMIT,
        "pr154_baseline_tree": PR154_BASELINE_TREE,
        "image_ref": image_ref,
        "image_digest": image_digest,
        "synthetic_fixture": synthetic_fixture,
        "members": members,
    }
    desc_bytes = (json.dumps(descriptor, sort_keys=True, indent=2) + "\n").encode("utf-8")
    _write_exclusive(package_dir / "release-descriptor.json", desc_bytes, 0o444)
    _fsync_dir(package_dir / "third_party" / "genai")
    _fsync_dir(package_dir)
    desc_sha = hashlib.sha256(desc_bytes).hexdigest()
    return {"status": "SUCCESS", "package_dir": str(package_dir), "descriptor_sha256": desc_sha}


def _backup_existing_binaries(install_bin_dir: Path, backup_dir: Path) -> None:
    if backup_dir.exists() or backup_dir.is_symlink():
        raise ReleaseError("backup_dir_collision")
    backup_dir.mkdir(parents=True, mode=0o755)
    for name in BINARY_MEMBERS:
        existing = install_bin_dir / name
        if existing.exists():
            _write_exclusive(backup_dir / name, existing.read_bytes(), 0o444)
    _fsync_dir(backup_dir)


def install_release(*, package_dir: Path, expected_descriptor_sha256: str,
                    install_bin_dir: Path, backup_dir: Path,
                    quiescence_check, sudoers_check, image_worker_check,
                    env: dict[str, str] | None = None,
                    replacer=None, allow_synthetic: bool = False) -> dict:
    """Verify descriptor, check quiescence/sudoers/image, backup old state, stage and replace."""
    sanitize_release_env(env)
    desc = verify_descriptor(package_dir, expected_descriptor_sha256, allow_synthetic=allow_synthetic)
    image_worker_check(desc)
    quiescence_check()
    sudoers_check()
    _backup_existing_binaries(install_bin_dir, backup_dir)
    do_replace = replacer or os.replace
    replaced = []
    try:
        for name in BINARY_MEMBERS:
            stage = install_bin_dir / f".{name}.stage.{os.getpid()}"
            _write_exclusive(stage, (package_dir / name).read_bytes(), 0o755)
            do_replace(stage, install_bin_dir / name)
            _fsync_dir(install_bin_dir)
            replaced.append(name)
            expected_sha = next(m["sha256"] for m in desc["members"] if m["path"] == name)
            if _sha256_file(install_bin_dir / name)[1] != expected_sha:
                raise ReleaseError(f"installed_digest_mismatch:{name}")
    except Exception as exc:
        return {
            "status": "INCOMPLETE",
            "backup_dir": str(backup_dir),
            "replaced": replaced,
            "reason": str(exc) or type(exc).__name__,
        }
    return {"status": "SUCCESS", "descriptor_sha256": expected_descriptor_sha256, "replaced": replaced}


def _default_quiescence_check() -> None:
    if subprocess.run(["pgrep", "-af", "/usr/local/libexec/duskweave/(duskweave|dw-boundary-launcher)"],
                      capture_output=True, text=True).returncode == 0:
        raise ReleaseError("active_runtime_processes_present")


def _default_sudoers_check() -> None:
    policy = Path("/etc/sudoers.d/duskweave-m1-launcher")
    if not policy.is_file() or policy.is_symlink():
        raise ReleaseError("sudoers_policy_missing")
    if subprocess.run(["visudo", "-c", "-f", str(policy)], capture_output=True, text=True).returncode != 0:
        raise ReleaseError("sudoers_policy_invalid")


def _default_image_worker_check(desc: dict) -> None:
    arch = subprocess.run(["docker", "image", "inspect", "--format", "{{.Architecture}}", desc["image_digest"]],
                          capture_output=True, text=True)
    if arch.returncode != 0 or arch.stdout.strip() != "arm64":
        raise ReleaseError("image_architecture_mismatch")


def _dispatch_cli(args) -> dict:
    if args.cmd == "package":
        return package_release(
            package_dir=Path(args.package_dir), source_root=Path(args.source_root),
            artifact_dir=Path(args.artifact_dir), image_ref=args.image_ref, image_digest=args.image_digest,
            build_source_commit=args.build_source_commit, build_source_tree=args.build_source_tree,
            driver_commit=args.driver_commit, driver_tree=args.driver_tree,
        )
    if args.cmd == "install":
        return install_release(
            package_dir=args.package_dir, expected_descriptor_sha256=args.expected_descriptor_sha256,
            install_bin_dir=args.install_bin_dir, backup_dir=args.backup_dir,
            quiescence_check=_default_quiescence_check, sudoers_check=_default_sudoers_check,
            image_worker_check=_default_image_worker_check,
        )
    return verify_descriptor(args.package_dir, args.expected_descriptor_sha256)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Candidate release packaging, verification, and install")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_pkg = sub.add_parser("package")
    for flag in ("--package-dir", "--source-root", "--artifact-dir", "--image-ref", "--image-digest",
                 "--build-source-commit", "--build-source-tree", "--driver-commit", "--driver-tree"):
        p_pkg.add_argument(flag, required=True)
    for name in ("verify-descriptor", "install"):
        p = sub.add_parser(name)
        p.add_argument("--package-dir", required=True, type=Path)
        p.add_argument("--expected-descriptor-sha256", required=True)
        if name == "install":
            p.add_argument("--install-bin-dir", required=True, type=Path)
            p.add_argument("--backup-dir", required=True, type=Path)
    try:
        out = _dispatch_cli(parser.parse_args(argv))
    except ReleaseError as exc:
        print(json.dumps({"status": "REFUSED", "reason": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(out, sort_keys=True))
    return 0 if out.get("status", "SUCCESS") == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())

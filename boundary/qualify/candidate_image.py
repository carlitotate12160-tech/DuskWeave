#!/usr/bin/env python3
"""Fixed-scope OCI image inspection and regular-layer member digest extraction."""

from __future__ import annotations

import hashlib
import io
import json
import stat
import subprocess
import tarfile

DEFAULT_WORKER_BINARIES = ("dw-dns-capture", "dw-dns-exporter")
MAX_WORKER_BYTES = 64 * 1024 * 1024
MAX_IMAGE_STREAM_BYTES = 96 * 1024 * 1024
ALLOWED_EXTRA_DIR_MODES = (0o555, 0o755)
EXPECTED_EXTRA_FILE_MODE = 0o444


class HostObservationError(RuntimeError):
    """Fail-closed error carrying bounded non-secret status and reason."""

    def __init__(self, status: str, reason: str) -> None:
        super().__init__(reason)
        self.status, self.reason = status, reason


def _inspect_image_meta(image_ref: str, docker_runner) -> tuple[str, int]:
    try:
        proc = docker_runner(
            ["docker", "image", "inspect", "--format", "{{json .}}", image_ref],
            capture_output=True, text=True, timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", "docker_image_inspect_failed") from exc
    if proc.returncode != 0:
        raise HostObservationError("REFUSED", "image_inspect_nonzero")
    return _parse_image_meta(proc.stdout)


def _parse_image_meta(raw: str) -> tuple[str, int]:
    try:
        doc = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise HostObservationError("UNKNOWN", "image_inspect_malformed") from exc
    if not isinstance(doc, dict) or doc.get("Os") != "linux" or doc.get("Architecture") != "arm64":
        raise HostObservationError("REFUSED", "image_platform_mismatch")
    image_id, rootfs = doc.get("Id"), doc.get("RootFS") or {}
    layers = rootfs.get("Layers") if isinstance(rootfs, dict) else None
    if not isinstance(image_id, str) or rootfs.get("Type") != "layers" or not layers:
        raise HostObservationError("REFUSED", "image_rootfs_invalid")
    return image_id, len(layers)


def _normalize_tar_member(name: str) -> str:
    clean = name.removeprefix("./")
    parts = clean.split("/")
    if not clean or "\\" in clean or any(p in ("", ".", "..") or p.startswith(".wh.") for p in parts):
        raise HostObservationError("REFUSED", "image_layer_unsafe_member")
    return clean


def _normalize_dir_member(name: str) -> str | None:
    if name in (".", "./"):
        return None
    clean = name.removeprefix("./")
    if clean.endswith("/"):
        clean = clean[:-1]
    return _normalize_tar_member(clean)


def _extract_layer_regular_member(
    ltf: tarfile.TarFile,
    member: tarfile.TarInfo,
    found: dict[str, str],
    sizes: dict[str, int],
    total_ref: list[int],
    allowed: set[str],
    meta: dict[str, tuple[str, int, int, int]] | None = None,
) -> None:
    norm = _normalize_tar_member(member.name)
    if norm not in allowed or norm in found:
        raise HostObservationError("REFUSED", f"image_unexpected_worker_member:{norm}")
    if member.size <= 0 or member.size > MAX_WORKER_BYTES:
        raise HostObservationError("REFUSED", f"image_worker_size_invalid:{norm}")
    total_ref[0] += member.size
    if total_ref[0] > MAX_IMAGE_STREAM_BYTES:
        raise HostObservationError("REFUSED", "image_extracted_size_exceeded")
    extracted = ltf.extractfile(member)
    raw = extracted.read() if extracted is not None else b""
    if len(raw) != member.size:
        raise HostObservationError("REFUSED", f"image_worker_truncated:{norm}")
    found[norm] = hashlib.sha256(raw).hexdigest()
    sizes[norm] = len(raw)
    if meta is not None:
        meta[norm] = ("reg", member.uid, member.gid, stat.S_IMODE(member.mode))


def _validate_layer_member(
    ltf: tarfile.TarFile,
    member: tarfile.TarInfo,
    found: dict[str, str],
    sizes: dict[str, int],
    total_ref: list[int],
    allowed: set[str],
    meta: dict[str, tuple[str, int, int, int]] | None = None,
) -> None:
    if member.issym() or member.islnk() or member.isdev() or member.isfifo():
        raise HostObservationError("REFUSED", "image_layer_link_or_device_refused")
    if member.isdir():
        norm = _normalize_dir_member(member.name)
        if norm is not None and meta is not None:
            meta[norm] = ("dir", member.uid, member.gid, stat.S_IMODE(member.mode))
        return
    if not member.isreg():
        raise HostObservationError("REFUSED", "image_layer_nonregular_member")
    _extract_layer_regular_member(ltf, member, found, sizes, total_ref, allowed, meta)


def _scan_layer_tar(
    layer_bytes: bytes,
    found: dict[str, str],
    sizes: dict[str, int],
    total_ref: list[int],
    allowed: set[str],
    meta: dict[str, tuple[str, int, int, int]] | None = None,
) -> None:
    try:
        with tarfile.open(fileobj=io.BytesIO(layer_bytes), mode="r:*") as ltf:
            for member in ltf.getmembers():
                _validate_layer_member(ltf, member, found, sizes, total_ref, allowed, meta)
    except tarfile.TarError as exc:
        raise HostObservationError("UNKNOWN", "image_layer_tar_corrupt") from exc


def _extract_workers_from_save(
    raw_tar: bytes,
    expected_layers: int,
    allowed: set[str] | None = None,
    meta: dict[str, tuple[str, int, int, int]] | None = None,
) -> tuple[dict[str, str], dict[str, int]]:
    allowed_set = set(DEFAULT_WORKER_BINARIES) if allowed is None else allowed
    found: dict[str, str] = {}
    sizes: dict[str, int] = {}
    total_ref = [0]
    try:
        with tarfile.open(fileobj=io.BytesIO(raw_tar), mode="r:*") as tf:
            mfile = tf.extractfile("manifest.json")
            manifest = json.loads(mfile.read().decode("utf-8")) if mfile is not None else None
            if not isinstance(manifest, list) or len(manifest) != 1:
                raise HostObservationError("REFUSED", "image_manifest_invalid")
            layers = manifest[0].get("Layers")
            if not isinstance(layers, list) or len(layers) != expected_layers:
                raise HostObservationError("REFUSED", "image_layer_count_mismatch")
            for layer_path in layers:
                lfile = tf.extractfile(layer_path)
                if lfile is None:
                    raise HostObservationError("REFUSED", "image_layer_missing")
                _scan_layer_tar(lfile.read(), found, sizes, total_ref, allowed_set, meta)
    except (tarfile.TarError, UnicodeDecodeError, json.JSONDecodeError, KeyError) as exc:
        raise HostObservationError("UNKNOWN", "image_archive_parse_failed") from exc
    return found, sizes


def _validate_extra_file(
    path: str,
    exp_size: int,
    exp_sha: str,
    found: dict[str, str],
    sizes: dict[str, int],
    meta: dict[str, tuple[str, int, int, int]],
) -> str:
    parts = path.split("/")
    for idx in range(1, len(parts)):
        parent = "/".join(parts[:idx])
        entry = meta.get(parent)
        if entry is None or entry[0] != "dir":
            raise HostObservationError("REFUSED", f"image_extra_parent_missing:{parent}")
        if entry[1] != 0 or entry[2] != 0 or entry[3] not in ALLOWED_EXTRA_DIR_MODES:
            raise HostObservationError("REFUSED", f"image_extra_parent_unsafe:{parent}")
    fentry = meta.get(path)
    if fentry is None or fentry[0] != "reg" or fentry[1] != 0 or fentry[2] != 0 or fentry[3] != EXPECTED_EXTRA_FILE_MODE:
        raise HostObservationError("REFUSED", f"image_extra_member_unsafe:{path}")
    if sizes[path] != exp_size or found[path] != exp_sha:
        raise HostObservationError("REFUSED", f"image_extra_member_mismatch:{path}")
    return found[path]


def extract_image_worker_digests(
    image_ref: str,
    *,
    docker_runner=subprocess.run,
    expected_files: tuple[str, ...] = DEFAULT_WORKER_BINARIES,
    expected_extra_files: dict[str, tuple[int, str]] | None = None,
) -> dict:
    """Inspect OCI image metadata and extract exact layer worker and optional asset digests."""
    allowed = set(expected_files) | set(expected_extra_files or {})
    image_id, expected_layers = _inspect_image_meta(image_ref, docker_runner)
    try:
        save_proc = docker_runner(
            ["docker", "image", "save", image_ref],
            capture_output=True, text=False, timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", "docker_image_save_failed") from exc
    if save_proc.returncode != 0 or len(save_proc.stdout) > MAX_IMAGE_STREAM_BYTES:
        raise HostObservationError("REFUSED", "docker_image_save_rejected")
    meta: dict[str, tuple[str, int, int, int]] = {}
    found, sizes = _extract_workers_from_save(save_proc.stdout, expected_layers, allowed, meta)
    if set(found) != allowed:
        raise HostObservationError("REFUSED", "image_workers_incomplete")
    workers = {name: found[name] for name in expected_files}
    result: dict = {"image_id": image_id, "os": "linux", "architecture": "arm64", "workers": workers}
    if expected_extra_files is not None:
        result["extras"] = {
            path: _validate_extra_file(path, exp_size, exp_sha, found, sizes, meta)
            for path, (exp_size, exp_sha) in expected_extra_files.items()
        }
    return result

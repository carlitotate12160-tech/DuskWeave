#!/usr/bin/env python3
"""Candidate LIVE qualification manifest rendering, ordered publication, and reconcile."""

from __future__ import annotations

import argparse
import hashlib
import ipaddress
import json
import os
import re
import time
import uuid
from pathlib import Path

PURPOSE_LIVE = "LIVE"
LIVE_RUN_PROFILE = "autonomous-dns-genai-live-v1"
PRO_V2_INFERENCE_PROFILE = "deepseek-genai-v4-pro-low-v2"
MAX_MANIFEST_BYTES = 16384
HEX64_RE = re.compile(r"^[0-9a-f]{64}$")
IMAGE_DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
CANONICAL_IPV4_RE = re.compile(r"^(?:0|[1-9][0-9]{0,2})(?:\.(?:0|[1-9][0-9]{0,2})){3}$")
SPECIAL_IPV4_NETS = tuple(ipaddress.IPv4Network(c) for c in (
    "0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "127.0.0.0/8", "169.254.0.0/16",
    "172.16.0.0/12", "192.0.0.0/24", "192.0.2.0/24", "192.168.0.0/16", "198.18.0.0/15",
    "198.51.100.0/24", "203.0.113.0/24", "224.0.0.0/4", "240.0.0.0/4",
))
IDENTITY_HASH_FIELDS = ("release_sha256", "launcher_sha256", "capture_sha256", "exporter_sha256")
SQL_COLUMNS = (
    "qualification_ref", "run_profile", "purpose", "release_sha256",
    "launcher_sha256", "capture_sha256", "exporter_sha256", "image_digest",
    "manifest_sha256", "boot_id", "resolver_ipv4", "valid_from_ms", "valid_until_ms",
)


class BindingError(RuntimeError):
    """Fail-closed LIVE qualification binding refusal."""


def _sanitize_binding_env(raw_env: dict[str, str] | None) -> None:
    env = os.environ if raw_env is None else raw_env
    for key in env:
        if key == "DW_TEST_FIXTURE_SUPERVISOR":
            continue
        if key == "DEEPSEEK_API_KEY" or key.startswith("DW_") or key.endswith("_FILE"):
            raise BindingError(f"forbidden_binding_env:{key}")


def validate_global_ipv4(value: str) -> str:
    """Require canonical global IPv4; refuse RFC1918, loopback, TEST-NET, CGNAT, etc."""
    if not isinstance(value, str) or not CANONICAL_IPV4_RE.fullmatch(value):
        raise BindingError("resolver_ipv4_malformed")
    try:
        addr = ipaddress.IPv4Address(value)
    except ValueError as exc:
        raise BindingError("resolver_ipv4_invalid") from exc
    if str(addr) != value or any(addr in net for net in SPECIAL_IPV4_NETS):
        raise BindingError("resolver_ipv4_not_global")
    return value


def _validate_uuid_non_nil(raw: str, field: str) -> str:
    try:
        parsed = uuid.UUID(str(raw))
    except ValueError as exc:
        raise BindingError(f"uuid_invalid:{field}") from exc
    if parsed.int == 0 or str(parsed) != str(raw).lower():
        raise BindingError(f"uuid_non_canonical_or_nil:{field}")
    return str(parsed)


def _validate_identities(identities: dict) -> dict:
    if not isinstance(identities, dict):
        raise BindingError("identities_malformed")
    for field in IDENTITY_HASH_FIELDS:
        val = identities.get(field)
        if not isinstance(val, str) or not HEX64_RE.fullmatch(val):
            raise BindingError(f"identity_hash_invalid:{field}")
    digest = identities.get("image_digest")
    if not isinstance(digest, str) or not IMAGE_DIGEST_RE.fullmatch(digest):
        raise BindingError("identity_image_digest_invalid")
    boot_id = _validate_uuid_non_nil(identities.get("boot_id", ""), "boot_id")
    core_uid = identities.get("core_uid")
    if type(core_uid) is not int or core_uid <= 0 or core_uid > 0xFFFFFFFF:
        raise BindingError("identity_core_uid_invalid")
    return {**identities, "boot_id": boot_id}


def _validate_window(valid_from_ms: int, valid_until_ms: int, now_ms: int | None) -> tuple[int, int]:
    if type(valid_from_ms) is not int or type(valid_until_ms) is not int:
        raise BindingError("validity_window_not_integer")
    if valid_from_ms <= 0 or valid_until_ms <= valid_from_ms:
        raise BindingError("validity_window_bounds_invalid")
    current = int(time.time() * 1000) if now_ms is None else now_ms
    if type(current) is not int or not (valid_from_ms <= current < valid_until_ms):
        raise BindingError("validity_window_expired_or_future")
    return valid_from_ms, valid_until_ms


def build_manifest_bundle(*, identities: dict, resolver_ipv4: str,
                          valid_from_ms: int, valid_until_ms: int,
                          now_ms: int | None = None,
                          qualification_ref: str | None = None,
                          purpose: str = PURPOSE_LIVE,
                          run_profile: str = LIVE_RUN_PROFILE) -> tuple[dict, bytes, str, dict]:
    """Build exact manifest JSON, serialized bytes, SHA-256, and derived SQL row."""
    if purpose != PURPOSE_LIVE or run_profile != LIVE_RUN_PROFILE:
        raise BindingError("live_purpose_or_profile_required")
    checked_ids = _validate_identities(identities)
    ip_str = validate_global_ipv4(resolver_ipv4)
    v_from, v_until = _validate_window(valid_from_ms, valid_until_ms, now_ms)
    qref = str(uuid.uuid4()) if qualification_ref is None else _validate_uuid_non_nil(qualification_ref, "qualification_ref")
    manifest = {
        "qualification_ref": qref, "purpose": PURPOSE_LIVE, "run_profile": LIVE_RUN_PROFILE,
        "resolver_ipv4": ip_str, "valid_from_ms": v_from, "valid_until_ms": v_until,
        **{k: checked_ids[k] for k in (*IDENTITY_HASH_FIELDS, "image_digest", "boot_id", "core_uid")},
    }
    encoded = (json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(encoded) > MAX_MANIFEST_BYTES:
        raise BindingError("manifest_exceeds_byte_limit")
    manifest_sha = hashlib.sha256(encoded).hexdigest()
    sql_row = {col: manifest[col] for col in SQL_COLUMNS if col != "manifest_sha256"}
    sql_row["manifest_sha256"] = manifest_sha
    return manifest, encoded, manifest_sha, sql_row


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


def render_binding(*, identities: dict, resolver_ipv4: str, valid_from_ms: int,
                   valid_until_ms: int, now_ms: int | None = None,
                   qualification_ref: str | None = None, purpose: str = PURPOSE_LIVE,
                   run_profile: str = LIVE_RUN_PROFILE,
                   output_file: Path | None = None) -> dict:
    """Render UNAPPLIED LIVE manifest bytes and derived SQL columns offline."""
    manifest, encoded, manifest_sha, sql_row = build_manifest_bundle(
        identities=identities, resolver_ipv4=resolver_ipv4,
        valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms,
        now_ms=now_ms, qualification_ref=qualification_ref,
        purpose=purpose, run_profile=run_profile,
    )
    if output_file is not None:
        _write_exclusive(Path(output_file), encoded, 0o444)
        _fsync_dir(Path(output_file).parent)
    return {
        "status": "UNAPPLIED",
        "applied": False,
        "qualification_ref": manifest["qualification_ref"],
        "manifest_sha256": manifest_sha,
        "inference_profile": PRO_V2_INFERENCE_PROFILE,
        "manifest": manifest,
        "sql_row": sql_row,
    }


def _identities_match(left: dict, right: dict) -> bool:
    keys = (*IDENTITY_HASH_FIELDS, "image_digest", "boot_id", "core_uid")
    return all(left.get(k) == right.get(k) for k in keys)


def _sql_row_matches(row: dict | None, manifest: dict, manifest_sha: str) -> bool:
    if not isinstance(row, dict):
        return False
    expected = {col: manifest[col] for col in SQL_COLUMNS if col != "manifest_sha256"}
    expected["manifest_sha256"] = manifest_sha
    return all(row.get(k) == expected[k] for k in SQL_COLUMNS)


def _preserve_intent_and_backup(manifest_path: Path, intent_dir: Path, qref: str,
                                manifest_sha: str) -> Path:
    if manifest_path.exists():
        backup = intent_dir / f"prior-deployment-{qref}.json"
        _write_exclusive(backup, manifest_path.read_bytes(), 0o400)
    intent_path = intent_dir / f"intent-{qref}.json"
    intent = json.dumps({"qualification_ref": qref, "manifest_sha256": manifest_sha}, sort_keys=True).encode("utf-8")
    _write_exclusive(intent_path, intent + b"\n", 0o400)
    _fsync_dir(intent_dir)
    return intent_path


def _publish_manifest_atomically(manifest_path: Path, encoded: bytes, replacer) -> None:
    stage = manifest_path.with_name(f".{manifest_path.name}.stage.{os.getpid()}")
    _write_exclusive(stage, encoded, 0o444)
    replacer(stage, manifest_path)
    _fsync_dir(manifest_path.parent)


def _insert_with_status(sql_executor, sql_row: dict, manifest_sha: str) -> dict | None:
    try:
        sql_executor("insert", sql_row)
    except TimeoutError as exc:
        return {"status": "UNKNOWN", "qualification_ref": sql_row["qualification_ref"],
                "manifest_sha256": manifest_sha, "reason": str(exc) or "sql_commit_ack_uncertain"}
    except Exception as exc:
        return {"status": "INCOMPLETE", "qualification_ref": sql_row["qualification_ref"],
                "manifest_sha256": manifest_sha, "reason": str(exc) or type(exc).__name__}
    return None


def publish_binding(*, identities: dict, installed_identities: dict,
                    resolver_ipv4: str, valid_from_ms: int, valid_until_ms: int,
                    manifest_path: Path, intent_dir: Path, sql_executor,
                    now_ms: int | None = None, qualification_ref: str | None = None,
                    env: dict[str, str] | None = None, replacer=None) -> dict:
    """Publish LIVE manifest then insert ONE SQL row and verify via fresh readback."""
    _sanitize_binding_env(env)
    manifest, encoded, manifest_sha, sql_row = build_manifest_bundle(
        identities=identities, resolver_ipv4=resolver_ipv4,
        valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms,
        now_ms=now_ms, qualification_ref=qualification_ref,
    )
    if not _identities_match(manifest, installed_identities):
        raise BindingError("installed_identity_mismatch")
    qref = manifest["qualification_ref"]
    if sql_executor("exists", qref):
        raise BindingError("qualification_ref_collision")
    _preserve_intent_and_backup(manifest_path, intent_dir, qref, manifest_sha)
    try:
        _publish_manifest_atomically(manifest_path, encoded, replacer or os.replace)
    except Exception as exc:
        return {"status": "INCOMPLETE", "qualification_ref": qref, "reason": str(exc)}
    sql_err = _insert_with_status(sql_executor, sql_row, manifest_sha)
    if sql_err is not None:
        return sql_err
    return reconcile_binding(
        qualification_ref=qref,
        manifest_path=manifest_path,
        installed_identities=installed_identities,
        sql_reader=lambda ref: sql_executor("read", ref),
        readback_reason="sql_readback_mismatch",
    )


def reconcile_binding(*, qualification_ref: str, manifest_path: Path,
                      installed_identities: dict, sql_reader,
                      readback_reason: str = "reconcile_mismatch") -> dict:
    """Read-only reconciliation comparing published manifest, installed facts, and SQL row."""
    qref = _validate_uuid_non_nil(qualification_ref, "qualification_ref")
    if not manifest_path.is_file() or manifest_path.is_symlink():
        return {"status": "INCOMPLETE", "qualification_ref": qref, "reason": "manifest_missing"}
    raw = manifest_path.read_bytes()
    manifest_sha = hashlib.sha256(raw).hexdigest()
    manifest = json.loads(raw.decode("utf-8"))
    if manifest.get("qualification_ref") != qref or not _identities_match(manifest, installed_identities):
        return {"status": "INCOMPLETE", "qualification_ref": qref, "reason": "manifest_identity_mismatch"}
    row = sql_reader(qref)
    if row is None:
        return {"status": "INCOMPLETE", "qualification_ref": qref, "reason": "sql_binding_unissued"}
    if not _sql_row_matches(row, manifest, manifest_sha):
        return {"status": "INCOMPLETE", "qualification_ref": qref, "reason": readback_reason}
    return {
        "status": "SUCCESS",
        "qualification_ref": qref,
        "manifest_sha256": manifest_sha,
        "inference_profile": PRO_V2_INFERENCE_PROFILE,
    }


def _run_pg_sql(container: str, database: str, statement: str, custody_file: Path | None) -> str:
    import subprocess
    try:
        if custody_file is not None:
            admin_dsn = json.loads(custody_file.read_text(encoding="utf-8"))["dsns"]["admin"]
            cmd = ["docker", "exec", "-i", container, "sh", "-c",
                   "IFS= read -r dsn && exec psql \"$dsn\" -X -v ON_ERROR_STOP=1 -Atc \"$1\"", "sh", statement]
            proc = subprocess.run(cmd, input=admin_dsn + "\n", text=True, capture_output=True, timeout=30)
        else:
            cmd = ["docker", "exec", container, "psql", "-X", "-U", "postgres", "-d", database,
                   "-v", "ON_ERROR_STOP=1", "-Atc", statement]
            proc = subprocess.run(cmd, text=True, capture_output=True, timeout=30)
    except subprocess.TimeoutExpired as exc:
        raise TimeoutError("sql_ack_timeout") from exc
    if proc.returncode != 0:
        raise RuntimeError("pg_sql_statement_failed")
    return proc.stdout.strip()


def _format_insert_sql(arg: dict) -> str:
    vals = (
        f"'{arg['qualification_ref']}'::uuid", f"'{arg['run_profile']}'", f"'{arg['purpose']}'",
        f"'{arg['release_sha256']}'", f"'{arg['launcher_sha256']}'", f"'{arg['capture_sha256']}'",
        f"'{arg['exporter_sha256']}'", f"'{arg['image_digest']}'", f"'{arg['manifest_sha256']}'",
        f"'{arg['boot_id']}'::uuid", f"'{arg['resolver_ipv4']}'::inet",
        str(arg["valid_from_ms"]), str(arg["valid_until_ms"]),
    )
    return f"BEGIN; INSERT INTO execution.live_qualifications ({', '.join(SQL_COLUMNS)}) VALUES ({', '.join(vals)}); COMMIT;"


def _read_row_sql(qref: str) -> str:
    cols = ", ".join(
        f"'{c}', {'host(resolver_ipv4)' if c == 'resolver_ipv4' else (f'{c}::text' if c in ('qualification_ref', 'boot_id') else c)}"
        for c in SQL_COLUMNS
    )
    return f"SELECT json_build_object({cols})::text FROM execution.live_qualifications WHERE qualification_ref='{qref}'"


def pg_sql_executor(container: str, database: str, custody_file: Path | None = None):
    """Build PostgreSQL executor over docker exec psql using stdin DSN custody."""
    def _exec(op: str, arg: dict | str | None = None):
        if op == "exists":
            qref = _validate_uuid_non_nil(str(arg), "qualification_ref")
            return _run_pg_sql(container, database, f"SELECT 1 FROM execution.live_qualifications WHERE qualification_ref='{qref}'", None) == "1"
        if op == "insert":
            assert isinstance(arg, dict)
            _run_pg_sql(container, database, _format_insert_sql(arg), custody_file)
            return None
        if op == "read":
            out = _run_pg_sql(container, database, _read_row_sql(_validate_uuid_non_nil(str(arg), "qualification_ref")), None)
            return json.loads(out) if out else None
        raise BindingError(f"unknown_sql_op:{op}")
    return _exec


def _ids_from_args(args) -> dict:
    return {k: getattr(args, k) for k in (*IDENTITY_HASH_FIELDS, "image_digest", "boot_id", "core_uid")}


def _add_common_flags(parser: argparse.ArgumentParser, with_window: bool = True) -> None:
    for flag in ("--release-sha256", "--launcher-sha256", "--capture-sha256", "--exporter-sha256", "--image-digest", "--boot-id"):
        parser.add_argument(flag, required=True)
    parser.add_argument("--core-uid", required=True, type=int)
    if with_window:
        parser.add_argument("--resolver-ipv4", required=True)
        parser.add_argument("--valid-from-ms", required=True, type=int)
        parser.add_argument("--valid-until-ms", required=True, type=int)
        parser.add_argument("--now-ms", type=int, default=None)
        parser.add_argument("--purpose", default=PURPOSE_LIVE)
        parser.add_argument("--run-profile", default=LIVE_RUN_PROFILE)


def _dispatch_cli(args) -> tuple[dict, int]:
    ids = _ids_from_args(args)
    if args.cmd == "reconcile":
        out = reconcile_binding(
            qualification_ref=args.qualification_ref, manifest_path=args.manifest_path,
            installed_identities=ids, sql_reader=lambda r: pg_sql_executor(args.container, args.database)("read", r),
        )
        return out, (0 if out["status"] == "SUCCESS" else 2)
    if args.cmd == "bind":
        out = publish_binding(
            identities=ids, installed_identities=ids, resolver_ipv4=args.resolver_ipv4,
            valid_from_ms=args.valid_from_ms, valid_until_ms=args.valid_until_ms, now_ms=args.now_ms,
            manifest_path=args.manifest_path, intent_dir=args.intent_dir,
            sql_executor=pg_sql_executor(args.container, args.database, args.custody_file),
        )
        return out, (0 if out["status"] == "SUCCESS" else 2)
    return render_binding(
        identities=ids, resolver_ipv4=args.resolver_ipv4, valid_from_ms=args.valid_from_ms,
        valid_until_ms=args.valid_until_ms, now_ms=args.now_ms, purpose=args.purpose,
        run_profile=args.run_profile, output_file=args.output_file,
    ), 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Candidate LIVE binding renderer and publisher")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_render = sub.add_parser("render-binding")
    _add_common_flags(p_render)
    p_render.add_argument("--output-file", type=Path, default=None)
    p_bind = sub.add_parser("bind")
    _add_common_flags(p_bind)
    for flag in ("--container", "--database"):
        p_bind.add_argument(flag, required=True)
    for flag in ("--custody-file", "--manifest-path", "--intent-dir"):
        p_bind.add_argument(flag, required=True, type=Path)
    p_rec = sub.add_parser("reconcile")
    _add_common_flags(p_rec, with_window=False)
    for flag in ("--qualification-ref", "--container", "--database"):
        p_rec.add_argument(flag, required=True)
    p_rec.add_argument("--manifest-path", required=True, type=Path)
    try:
        out, code = _dispatch_cli(parser.parse_args(argv))
    except BindingError as exc:
        print(json.dumps({"status": "REFUSED", "reason": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(out, sort_keys=True))
    return code


if __name__ == "__main__":
    raise SystemExit(main())

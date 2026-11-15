#!/usr/bin/env python3
"""Candidate PostgreSQL schema-0035 provisioning and protected credential custody."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

ALLOWED_ENV_KEYS = ("PATH", "HOME", "LANG", "LC_ALL", "TERM", "TZ", "PYTHONDONTWRITEBYTECODE", "DW_TEST_FIXTURE_SUPERVISOR")
RUNTIME_OWNERS = ("core", "broker", "cognition", "history", "terrain", "stop")
ENV_FILE_MAP = {
    "core": "DW_DATABASE_URL_FILE",
    "broker": "DW_INFERENCE_BROKER_DATABASE_URL_FILE",
    "cognition": "DW_COGNITION_DATABASE_URL_FILE",
    "history": "DW_TRAJECTORY_DATABASE_URL_FILE",
    "terrain": "DW_TERRAIN_DATABASE_URL_FILE",
    "stop": "DW_STOP_WRITER_DATABASE_URL_FILE",
}
SCHEMA_0035_FUNCTIONS = (
    "execution.inference_profile_matches",
    "execution.inference_reservation",
    "execution.inference_window_ms",
    "execution.inference_may_send",
    "execution.genai_expected_intent",
    "execution.inference_call_state",
    "execution.run_start",
    "execution.run_current",
    "execution.assert_autonomous_pack",
    "execution.run_phase_diagnostic",
)
CONTROL_ROOT = "/run/duskweave/m1"


class CandidateDatabaseError(RuntimeError):
    """Sanitized failure carrying a closed operational status."""

    def __init__(self, status: str, reason: str):
        super().__init__(reason)
        self.status = status
        self.reason = reason


def sanitize_env(raw_env: dict[str, str] | None = None) -> dict[str, str]:
    """Refuse ambient provider or DB override variables and return allowlisted env."""
    source = os.environ if raw_env is None else raw_env
    for key in source:
        if key == "DW_TEST_FIXTURE_SUPERVISOR":
            continue
        if key == "DEEPSEEK_API_KEY" or key.startswith("DW_") or key.endswith("_FILE"):
            raise CandidateDatabaseError("REFUSED", f"forbidden_env_variable:{key}")
    cleaned = {key: source[key] for key in ALLOWED_ENV_KEYS if key in source}
    cleaned["PYTHONDONTWRITEBYTECODE"] = "1"
    return cleaned


def _fsync_dir(directory: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    fd = os.open(directory, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _require_safe_ancestors(target_dir: Path, expected_uids: tuple[int, ...]) -> None:
    resolved = Path("/")
    for part in target_dir.parts[1:]:
        resolved = resolved / part
        if resolved.is_symlink():
            raise CandidateDatabaseError("REFUSED", f"symlink_ancestor_refused:{resolved}")
    if not target_dir.is_dir():
        raise CandidateDatabaseError("REFUSED", f"directory_missing:{target_dir}")
    st = os.lstat(target_dir)
    if stat.S_ISLNK(st.st_mode) or (st.st_mode & 0o022) != 0 or st.st_uid not in expected_uids:
        raise CandidateDatabaseError("REFUSED", f"unprotected_directory:{target_dir}")


def write_exclusive_custody(custody_file: Path, plan: dict, expected_uids: tuple[int, ...]) -> None:
    """Create a fresh mode-0400 admin/owner custody file before any database writes."""
    if custody_file.exists() or custody_file.is_symlink():
        raise CandidateDatabaseError("REFUSED", "custody_file_collision")
    _require_safe_ancestors(custody_file.parent, expected_uids)
    payload = (json.dumps(plan, sort_keys=True, indent=2) + "\n").encode("utf-8")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(custody_file, flags, 0o400)
    try:
        os.write(fd, payload)
        os.fsync(fd)
    finally:
        os.close(fd)
    _fsync_dir(custody_file.parent)
    st = os.lstat(custody_file)
    bad = stat.S_ISLNK(st.st_mode) or st.st_nlink != 1 or stat.S_IMODE(st.st_mode) != 0o400
    if bad or st.st_uid not in expected_uids:
        raise CandidateDatabaseError("INCOMPLETE", "custody_metadata_verification_failed")


def _atomic_write_text(path: Path, text: str, mode: int, gid: int | None = None) -> None:
    stage = path.with_name(f".{path.name}.stage.{os.getpid()}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(stage, flags, 0o400)
    try:
        os.write(fd, text.encode("utf-8"))
        os.fchmod(fd, mode)
        if gid is not None and os.geteuid() == 0:
            os.fchown(fd, 0, gid)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.replace(stage, path)
    _fsync_dir(path.parent)


def write_runtime_config(config_root: Path, dsns: dict[str, str],
                         expected_uids: tuple[int, ...], runtime_gid: int | None = None) -> dict[str, str]:
    """Emit the six runtime .dsn files and file-mode environment.json without admin DSNs."""
    _require_safe_ancestors(config_root, expected_uids)
    dsn_mode = 0o440 if runtime_gid is not None else 0o400
    env_payload = {
        "DW_DATABASE_CONFIG_MODE": "file",
        "DW_M1_RUN_CONTROL_ROOT": CONTROL_ROOT,
    }
    for owner in RUNTIME_OWNERS:
        if owner not in dsns:
            raise CandidateDatabaseError("INCOMPLETE", f"missing_runtime_dsn:{owner}")
        target = config_root / f"{owner}.dsn"
        _atomic_write_text(target, dsns[owner] + "\n", dsn_mode, gid=runtime_gid)
        env_payload[ENV_FILE_MAP[owner]] = str(target)
    env_mode = 0o440 if runtime_gid is not None else 0o400
    _atomic_write_text(
        config_root / "environment.json",
        json.dumps(env_payload, sort_keys=True, indent=2) + "\n",
        env_mode,
        gid=runtime_gid,
    )
    return env_payload


def _load_bootstrap():
    support = Path(__file__).resolve().parents[2] / "tests" / "support"
    if str(support) not in sys.path:
        sys.path.insert(0, str(support))
    import m1_native_bootstrap as bootstrap
    return bootstrap


def verify_schema_0035_and_identities(container: str, database: str, roles: dict[str, str],
                                      dsns: dict[str, str], env: dict[str, str]) -> dict:
    """Query role OIDs, memberships, schema-0035 function fingerprints, and direct session_user."""
    bootstrap = _load_bootstrap()
    role_sql = (
        "SELECT r.rolname || '|' || r.oid || '|' || "
        "COALESCE(string_agg(g.rolname, ',' ORDER BY g.rolname), '') "
        "FROM pg_roles r LEFT JOIN pg_auth_members m ON m.member = r.oid "
        "LEFT JOIN pg_roles g ON g.oid = m.roleid "
        "WHERE r.rolname = ANY(ARRAY[" + ",".join(f"'{v}'" for v in roles.values()) + "]) "
        "GROUP BY r.rolname, r.oid ORDER BY r.rolname"
    )
    role_rows = bootstrap.private_run(
        ["docker", "exec", container, "psql", "-U", "postgres", "-d", database, "-Atqc", role_sql]
    ).strip().splitlines()
    role_oids = {}
    for line in role_rows:
        rname, roid, _ = line.split("|", 2)
        role_oids[rname] = int(roid)
    fn_sql = (
        "SELECT n.nspname || '.' || p.proname || '|' || "
        "encode(sha256(convert_to(pg_get_functiondef(p.oid), 'UTF8')), 'hex') "
        "FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace "
        "WHERE n.nspname || '.' || p.proname = ANY(ARRAY["
        + ",".join(f"'{name}'" for name in SCHEMA_0035_FUNCTIONS)
        + "]) ORDER BY 1"
    )
    fn_rows = bootstrap.private_run(
        ["docker", "exec", container, "psql", "-U", "postgres", "-d", database, "-Atqc", fn_sql]
    ).strip().splitlines()
    fingerprints = dict(line.split("|", 1) for line in fn_rows if "|" in line)
    if set(fingerprints) != set(SCHEMA_0035_FUNCTIONS):
        raise CandidateDatabaseError("INCOMPLETE", "schema_0035_functions_incomplete")
    session_users = _probe_session_users(container, dsns, env)
    if any(session_users.get(owner) != role for owner, role in roles.items()):
        raise CandidateDatabaseError("INCOMPLETE", "session_user_identity_mismatch")
    return {"role_oids": role_oids, "schema_0035_functions": fingerprints, "session_users": session_users}


def _probe_session_users(container: str, dsns: dict[str, str], env: dict[str, str]) -> dict[str, str]:
    users = {}
    for owner, dsn in dsns.items():
        proc = subprocess.run(
            ["docker", "exec", "-i", container, "sh", "-c",
             "IFS= read -r dsn && exec psql \"$dsn\" -Atqc \"SELECT session_user\""],
            input=dsn + "\n",
            text=True,
            capture_output=True,
            env=env,
            timeout=15,
        )
        if proc.returncode != 0:
            raise CandidateDatabaseError("INCOMPLETE", f"session_user_probe_failed:{owner}")
        users[owner] = proc.stdout.strip()
    return users


def _default_factory(**kwargs):
    return _load_bootstrap().Fixture(**kwargs)


def _custody_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def provision_database(*, container: str, database: str, role_prefix: str,
                       config_root: Path, custody_file: Path,
                       env: dict[str, str] | None = None,
                       runtime_gid: int | None = None,
                       fixture_factory=None, verifier=None) -> dict:
    """Provision fresh schema-0035 DB with pre-create custody and fail-closed state."""
    clean_env = sanitize_env(env)
    expected_uids = (0, os.geteuid())
    _require_safe_ancestors(Path(config_root), expected_uids)
    _require_safe_ancestors(Path(custody_file).parent, expected_uids)
    if Path(custody_file).exists() or Path(custody_file).is_symlink():
        raise CandidateDatabaseError("REFUSED", "custody_file_collision")
    captured_plan: dict = {}
    custody_written = False

    def custody_cb(plan: dict) -> None:
        nonlocal custody_written
        write_exclusive_custody(Path(custody_file), plan, expected_uids)
        custody_written = True
        captured_plan["roles"] = dict(plan["roles"])
        captured_plan["dsns"] = dict(plan["dsns"])

    factory = fixture_factory or _default_factory
    orig_environ = os.environ.copy()
    try:
        os.environ.clear()
        os.environ.update(clean_env)
        fx = factory(
            container=container,
            kind="fixture",
            ci=False,
            database=database,
            schema_revision="0035",
            role_prefix=role_prefix,
            custody_callback=custody_cb,
            negative_controls=False,
        )
        write_runtime_config(Path(config_root), captured_plan["dsns"], expected_uids, runtime_gid)
        facts = (verifier or (lambda inst: verify_schema_0035_and_identities(
            container, inst.database, captured_plan["roles"], captured_plan["dsns"], clean_env
        )))(fx)
    except CandidateDatabaseError as exc:
        if exc.status == "REFUSED" and not custody_written:
            raise
        return {
            "status": exc.status if custody_written else "REFUSED",
            "database": database,
            "role_prefix": role_prefix,
            "schema_revision": "0035",
            "custody_preserved": custody_written,
            "reason": exc.reason,
        }
    except Exception as exc:
        status = "INCOMPLETE" if custody_written else "REFUSED"
        if not custody_written:
            raise CandidateDatabaseError("REFUSED", type(exc).__name__) from None
        return {
            "status": status,
            "database": database,
            "role_prefix": role_prefix,
            "schema_revision": "0035",
            "custody_preserved": True,
            "custody_sha256": _custody_sha256(Path(custody_file)),
            "reason": type(exc).__name__,
        }
    finally:
        os.environ.clear()
        os.environ.update(orig_environ)

    return {
        "status": "SUCCESS",
        "database": fx.database,
        "role_prefix": role_prefix,
        "schema_revision": "0035",
        "custody_preserved": True,
        "custody_sha256": _custody_sha256(Path(custody_file)),
        "roles": dict(captured_plan["roles"]),
        "verification": facts,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Candidate schema-0035 database provisioner")
    parser.add_argument("--container", required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--role-prefix", required=True)
    parser.add_argument("--config-root", required=True, type=Path)
    parser.add_argument("--custody-file", required=True, type=Path)
    parser.add_argument("--runtime-gid", type=int, default=None)
    args = parser.parse_args(argv)
    try:
        receipt = provision_database(
            container=args.container,
            database=args.database,
            role_prefix=args.role_prefix,
            config_root=args.config_root,
            custody_file=args.custody_file,
            runtime_gid=args.runtime_gid,
        )
    except CandidateDatabaseError as exc:
        print(json.dumps({"status": exc.status, "reason": exc.reason}, sort_keys=True))
        return 1
    print(json.dumps(receipt, sort_keys=True))
    return 0 if receipt["status"] == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())

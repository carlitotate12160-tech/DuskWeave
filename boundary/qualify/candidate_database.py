#!/usr/bin/env python3
"""Candidate PostgreSQL schema-0036 provisioning and protected credential custody."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sys

from candidate_files import (
    FileCustodyError,
    create_exclusive_dir,
    reject_sensitive_env,
    require_safe_ancestors,
    verify_driver_custody,
    write_exclusive_verified,
)
from candidate_host import HostObservationError, PRIVILEGED_SCOPE, resolve_core_account
from candidate_postgres import PostgresError, verify_release_schema_and_identities

ALLOWED_ENV_KEYS = (
    "PATH", "HOME", "LANG", "LC_ALL", "TERM", "TZ",
    "PYTHONDONTWRITEBYTECODE", "DW_TEST_FIXTURE_SUPERVISOR",
)
RUNTIME_OWNERS = ("core", "broker", "cognition", "history", "terrain", "stop")
ENV_FILE_MAP = {
    "core": "DW_DATABASE_URL_FILE",
    "broker": "DW_INFERENCE_BROKER_DATABASE_URL_FILE",
    "cognition": "DW_COGNITION_DATABASE_URL_FILE",
    "history": "DW_TRAJECTORY_DATABASE_URL_FILE",
    "terrain": "DW_TERRAIN_DATABASE_URL_FILE",
    "stop": "DW_STOP_WRITER_DATABASE_URL_FILE",
}
HEX40_RE = re.compile(r"^[0-9a-f]{40}$")
SCHEMA_REVISIONS = ("0036", "0038")
DEFAULT_MIGRATION = Path(__file__).resolve().parents[2] / "migrations" / "0035_pro_budget_c_v2.sql"


class CandidateDatabaseError(RuntimeError):
    """Sanitized failure carrying a closed operational status."""

    def __init__(self, status: str, reason: str) -> None:
        super().__init__(reason)
        self.status = status
        self.reason = reason


def sanitize_env(raw_env: dict[str, str] | None = None) -> dict[str, str]:
    """Refuse ambient provider, DB, or path override variables and return allowlisted env."""
    source = reject_sensitive_env(
        raw_env,
        error_cls=lambda s, r: CandidateDatabaseError(s, r.replace("forbidden_release_env:", "forbidden_env_variable:")),
    )
    cleaned = {key: source[key] for key in ALLOWED_ENV_KEYS if key in source}
    cleaned["PYTHONDONTWRITEBYTECODE"] = "1"
    return cleaned


def _load_bootstrap():
    support = Path(__file__).resolve().parents[2] / "tests" / "support"
    if str(support) not in sys.path:
        sys.path.insert(0, str(support))
    import m1_native_bootstrap as bootstrap
    return bootstrap


def _default_factory(**kwargs):
    return _load_bootstrap().Fixture(**kwargs)


def write_exclusive_custody(custody_file: Path, plan: dict, expected_uids: tuple[int, ...],
                            *, stop_at: Path = Path("/"), writer=os.write, fsync_fn=os.fsync) -> None:
    """Create mode-0400 root-owned admin/owner custody file before any database writes."""
    payload = (json.dumps(plan, sort_keys=True, indent=2) + "\n").encode("utf-8")
    try:
        write_exclusive_verified(
            custody_file, payload, mode=0o400, expected_uids=expected_uids,
            stop_at=stop_at, writer=writer, fsync_fn=fsync_fn,
        )
    except FileCustodyError as exc:
        raise CandidateDatabaseError(exc.status, exc.reason) from exc


def _write_core_owned_file(path: Path, data: bytes, expected_uids: tuple[int, ...],
                           core_uid: int | None, core_gid: int | None, stop_at: Path,
                           writer, fsync_fn) -> None:
    try:
        write_exclusive_verified(
            path, data, mode=0o400, expected_uids=expected_uids,
            owner_uid=core_uid, owner_gid=core_gid, stop_at=stop_at, writer=writer, fsync_fn=fsync_fn,
        )
    except FileCustodyError as exc:
        raise CandidateDatabaseError("INCOMPLETE", exc.reason) from exc


def write_runtime_config(config_root: Path, dsns: dict[str, str], expected_uids: tuple[int, ...],
                         *, core_uid: int | None = None, core_gid: int | None = None,
                         stop_at: Path = Path("/"), writer=os.write, fsync_fn=os.fsync) -> dict[str, str]:
    """Emit mode-0400 core-owned runtime .dsn files and file-mode environment.json without admin DSN."""
    try:
        require_safe_ancestors(config_root, expected_uids, stop_at=stop_at)
    except FileCustodyError as exc:
        raise CandidateDatabaseError(exc.status, exc.reason) from exc
    env_payload = {
        "DW_DATABASE_CONFIG_MODE": "file",
        "DW_M1_RUN_CONTROL_ROOT": PRIVILEGED_SCOPE["control_root"],
    }
    for owner in RUNTIME_OWNERS:
        if owner not in dsns:
            raise CandidateDatabaseError("INCOMPLETE", f"missing_runtime_dsn:{owner}")
        target = config_root / f"{owner}.dsn"
        _write_core_owned_file(target, (dsns[owner] + "\n").encode("utf-8"), expected_uids,
                               core_uid, core_gid, stop_at, writer, fsync_fn)
        env_payload[ENV_FILE_MAP[owner]] = str(target)
    env_bytes = (json.dumps(env_payload, sort_keys=True, indent=2) + "\n").encode("utf-8")
    _write_core_owned_file(config_root / "environment.json", env_bytes, expected_uids,
                           core_uid, core_gid, stop_at, writer, fsync_fn)
    return env_payload


def _default_verifier(container: str, database: str, roles: dict[str, str], dsns: dict[str, str],
                      role_prefix: str, migration_path: Path) -> dict:
    try:
        return verify_release_schema_and_identities(
            container, database, roles, dsns, migration_path=migration_path, role_prefix=role_prefix,
        )
    except PostgresError as exc:
        raise CandidateDatabaseError(exc.status, exc.reason) from exc


def _prepare_fresh_config_root(cfg_root: Path, custody_path: Path, expected_uids: tuple[int, ...],
                               stop_at: Path, fsync_fn) -> None:
    if cfg_root.exists() or cfg_root.is_symlink():
        raise CandidateDatabaseError("REFUSED", "config_root_collision")
    if custody_path.exists() or custody_path.is_symlink():
        raise CandidateDatabaseError("REFUSED", "custody_file_collision")
    try:
        require_safe_ancestors(cfg_root.parent, expected_uids, stop_at=stop_at)
        create_exclusive_dir(cfg_root, mode=0o755, expected_uids=expected_uids, stop_at=stop_at, fsync_fn=fsync_fn)
    except FileCustodyError as exc:
        raise CandidateDatabaseError(exc.status, exc.reason) from exc


def _resolve_account_for_provision(core_account: dict[str, int | str] | None, core_resolver) -> dict[str, int | str]:
    if core_account is not None:
        return core_account
    resolver = core_resolver or resolve_core_account
    try:
        return resolver()
    except HostObservationError as exc:
        raise CandidateDatabaseError(exc.status, exc.reason) from exc


def _run_provision_sequence(*, container: str, database: str, role_prefix: str, cfg_root: Path,
                            custody_path: Path, expected_uids: tuple[int, ...], stop_at: Path,
                            core_uid: int, core_gid: int, clean_env: dict[str, str],
                            schema_revision: str, migration_path: Path, fixture_factory,
                            verifier, writer, fsync_fn) -> dict:
    captured_plan: dict = {}
    custody_written = False

    def custody_cb(plan: dict) -> None:
        nonlocal custody_written
        write_exclusive_custody(custody_path, plan, expected_uids, stop_at=stop_at, writer=writer, fsync_fn=fsync_fn)
        custody_written = True
        captured_plan["roles"] = dict(plan["roles"])
        captured_plan["dsns"] = dict(plan["dsns"])

    factory = fixture_factory or _default_factory
    orig_environ = os.environ.copy()
    try:
        os.environ.clear()
        os.environ.update(clean_env)
        fx = factory(
            container=container, kind="fixture", ci=False, database=database, schema_revision=schema_revision,
            role_prefix=role_prefix, custody_callback=custody_cb, negative_controls=False,
        )
        write_runtime_config(
            cfg_root, captured_plan["dsns"], expected_uids, core_uid=core_uid,
            core_gid=core_gid, stop_at=stop_at, writer=writer, fsync_fn=fsync_fn,
        )
        facts = (verifier or (lambda inst: _default_verifier(
            container, inst.database, captured_plan["roles"], captured_plan["dsns"], role_prefix, migration_path,
        )))(fx)
    except CandidateDatabaseError as exc:
        if exc.status == "REFUSED" and not custody_written:
            raise
        return {
            "status": exc.status if (custody_written or exc.status == "INCOMPLETE") else "REFUSED",
            "database": database, "role_prefix": role_prefix, "schema_revision": schema_revision,
            "custody_preserved": custody_written, "reason": exc.reason,
        }
    except Exception as exc:
        if not custody_written:
            raise CandidateDatabaseError("REFUSED", type(exc).__name__) from exc
        return {
            "status": "INCOMPLETE", "database": database, "role_prefix": role_prefix,
            "schema_revision": schema_revision, "custody_preserved": True, "reason": type(exc).__name__,
        }
    finally:
        os.environ.clear()
        os.environ.update(orig_environ)

    return {
        "status": "SUCCESS", "database": fx.database, "role_prefix": role_prefix,
        "schema_revision": schema_revision, "custody_preserved": True, "config_root": str(cfg_root),
        "custody_file": str(custody_path), "core_uid": core_uid, "core_gid": core_gid,
        "roles": dict(captured_plan["roles"]), "verification": facts,
    }


def provision_database(
    *,
    container: str,
    database: str,
    role_prefix: str,
    config_root: Path,
    custody_file: Path,
    schema_revision: str = "0036",
    env: dict[str, str] | None = None,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
    core_account: dict[str, int | str] | None = None,
    core_resolver=None,
    migration_path: Path = DEFAULT_MIGRATION,
    fixture_factory=None,
    verifier=None,
    writer=os.write,
    fsync_fn=os.fsync,
) -> dict:
    """Provision fresh schema DB with pre-create custody, core-0400 configs, and exact catalog check."""
    if schema_revision not in SCHEMA_REVISIONS:
        raise CandidateDatabaseError("REFUSED", "schema_revision_refused")
    if schema_revision != "0036" and verifier is None:
        raise CandidateDatabaseError("REFUSED", "schema_verifier_required")
    clean_env = sanitize_env(env)
    account = _resolve_account_for_provision(core_account, core_resolver)
    cfg_root = Path(config_root)
    custody_path = Path(custody_file)
    if custody_path.parent != cfg_root:
        raise CandidateDatabaseError("REFUSED", "custody_outside_config_root")
    _prepare_fresh_config_root(cfg_root, custody_path, expected_uids, stop_at, fsync_fn)
    return _run_provision_sequence(
        container=container, database=database, role_prefix=role_prefix,
        cfg_root=cfg_root, custody_path=custody_path, expected_uids=expected_uids,
        stop_at=stop_at, core_uid=int(account["uid"]), core_gid=int(account["gid"]),
        clean_env=clean_env, schema_revision=schema_revision, migration_path=migration_path,
        fixture_factory=fixture_factory, verifier=verifier, writer=writer, fsync_fn=fsync_fn,
    )


def main(argv: list[str] | None = None, *, driver_verifier=verify_driver_custody) -> int:
    parser = argparse.ArgumentParser(description="Privileged fixed-scope schema-0036 database provisioner")
    parser.add_argument("--expected-driver-commit", required=True)
    parser.add_argument("--expected-driver-tree", required=True)
    args = parser.parse_args(argv)
    drv_dir = Path(__file__).resolve().parent
    expected_drv = Path(PRIVILEGED_SCOPE["source_dir"]) / "boundary" / "qualify"
    try:
        if not (HEX40_RE.fullmatch(args.expected_driver_commit) and HEX40_RE.fullmatch(args.expected_driver_tree)):
            raise CandidateDatabaseError("REFUSED", "expected_driver_pin_invalid")
        if drv_dir != expected_drv:
            raise CandidateDatabaseError("REFUSED", "privileged_driver_outside_fixed_source")
        try:
            driver_verifier(drv_dir, args.expected_driver_commit, args.expected_driver_tree, expected_uids=(0,))
        except FileCustodyError as exc:
            raise CandidateDatabaseError(exc.status, exc.reason) from exc
        receipt = provision_database(
            container=PRIVILEGED_SCOPE["pg_container"], database=PRIVILEGED_SCOPE["database"],
            role_prefix=PRIVILEGED_SCOPE["role_prefix"], config_root=Path(PRIVILEGED_SCOPE["config_dir"]),
            custody_file=Path(PRIVILEGED_SCOPE["owner_custody_path"]), expected_uids=(0,), stop_at=Path("/"),
        )
    except CandidateDatabaseError as exc:
        print(json.dumps({"status": exc.status, "reason": exc.reason}, sort_keys=True))
        return 1
    print(json.dumps(receipt, sort_keys=True))
    return 0 if receipt["status"] == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())

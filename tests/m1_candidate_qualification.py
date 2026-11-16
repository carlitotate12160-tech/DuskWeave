#!/usr/bin/env python3
"""Explicit real PostgreSQL integration qualification runner for pinned-release candidate tooling."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "boundary" / "qualify"))
sys.path.insert(0, str(ROOT / "tests" / "support"))

import candidate_binding  # noqa: E402
import candidate_database  # noqa: E402
import m1_native_bootstrap as bootstrap  # noqa: E402

SAMPLE_IDS = {
    "release_sha256": "1" * 64,
    "launcher_sha256": "2" * 64,
    "capture_sha256": "3" * 64,
    "exporter_sha256": "4" * 64,
    "image_digest": "sha256:" + ("5" * 64),
    "boot_id": "22222222-2222-4222-8222-222222222222",
    "core_uid": os.geteuid(),
}


def _psql(container: str, database: str, sql: str) -> str:
    proc = subprocess.run(
        ["docker", "exec", container, "psql", "-X", "-U", "postgres", "-d", database,
         "-v", "ON_ERROR_STOP=1", "-Atc", sql],
        capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=30,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"psql_failed:{proc.returncode}")
    return proc.stdout.strip()


def _clean_env() -> dict[str, str]:
    return {
        k: os.environ[k]
        for k in ("PATH", "HOME", "LANG", "LC_ALL", "TERM", "TZ", "DW_TEST_FIXTURE_SUPERVISOR")
        if k in os.environ
    }


def _local_core_account() -> dict[str, int | str]:
    return {"name": "duskweave-core", "uid": os.geteuid(), "gid": os.getegid()}


def _verify_custody_and_env(cfg_dir: Path, custody_file: Path, roles: dict[str, str]) -> str:
    st = os.lstat(custody_file)
    assert not stat.S_ISLNK(st.st_mode) and stat.S_IMODE(st.st_mode) == 0o400 and st.st_nlink == 1
    assert set(roles.keys()) == {"admin", *candidate_database.RUNTIME_OWNERS}
    assert "otherbroker" not in roles
    env_data = json.loads((cfg_dir / "environment.json").read_text(encoding="utf-8"))
    assert env_data["DW_DATABASE_CONFIG_MODE"] == "file"
    assert env_data["DW_M1_RUN_CONTROL_ROOT"] == "/run/duskweave/m1"
    assert "DW_TEST_ADMIN_DATABASE_URL" not in env_data
    for owner in candidate_database.RUNTIME_OWNERS:
        dsn_p = cfg_dir / f"{owner}.dsn"
        assert dsn_p.is_file() and stat.S_IMODE(dsn_p.stat().st_mode) == 0o400
    custody_obj = json.loads(custody_file.read_text(encoding="utf-8"))
    return custody_obj["dsns"]["admin"]


def _verify_core_inspect(duskweave_bin: Path, cfg_dir: Path) -> str:
    env_data = json.loads((cfg_dir / "environment.json").read_text(encoding="utf-8"))
    core_env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LC_ALL": "C", **env_data}
    if os.environ.get("LLVM_PROFILE_FILE"):
        core_env["LLVM_PROFILE_FILE"] = os.environ["LLVM_PROFILE_FILE"]
    proc = subprocess.run(
        [
            str(duskweave_bin), "inspect",
            "--engagement", "11111111-1111-4111-8111-111111111111",
            "--campaign", "22222222-2222-4222-8222-222222222222",
        ],
        capture_output=True, text=True, env=core_env, stdin=subprocess.DEVNULL, timeout=30,
    )
    assert proc.returncode == 0, f"duskweave_inspect_failed:{proc.returncode}"
    assert "inspect result=empty" in proc.stdout, proc.stdout
    return proc.stdout.strip()


def _case_custody_failure(container: str, database: str, prefix: str, env: dict[str, str]) -> dict:
    writes = {"callback_ran": False}

    def fail_cb(_plan):
        writes["callback_ran"] = True
        raise OSError("simulated_custody_fsync_failure")

    orig = os.environ.copy()
    try:
        os.environ.clear()
        os.environ.update(env)
        try:
            bootstrap.Fixture(
                container, "fixture", database=database, schema_revision="0036",
                role_prefix=prefix, custody_callback=fail_cb, negative_controls=False,
            )
            raise AssertionError("expected OSError from failing custody callback")
        except OSError:
            pass
    finally:
        os.environ.clear()
        os.environ.update(orig)

    db_count = _psql(container, "postgres", f"SELECT count(*) FROM pg_database WHERE datname='{database}'")
    role_count = _psql(container, "postgres", f"SELECT count(*) FROM pg_roles WHERE rolname LIKE '{prefix}%'")
    assert writes["callback_ran"] and db_count == "0" and role_count == "0"
    return {"case": "custody_failure_zero_pg_writes", "status": "PASS", "db_count": 0, "role_count": 0}


def _verify_refusals(container: str, database: str, admin_dsn: str, manifest_path: Path, qref: str, stop_at: Path) -> tuple[list[str], list[str]]:
    uid = os.geteuid()
    app_precheck = []
    dup_rec = candidate_binding.publish_binding(
        identities=SAMPLE_IDS, installed_identities=SAMPLE_IDS, resolver_ipv4="1.1.1.1",
        valid_from_ms=1000, valid_until_ms=4_000_000_000_000, now_ms=2000,
        qualification_ref=qref, manifest_path=manifest_path, intent_dir=stop_at / "dup-intent",
        sql_executor=candidate_binding.pg_sql_executor(container, database, admin_dsn),
        env=_clean_env(), expected_uids=(uid,), stop_at=stop_at,
    )
    assert dup_rec["status"] == "REFUSED" and dup_rec["reason"] == "qualification_ref_collision", dup_rec
    app_precheck.append("duplicate_qualification_ref_precheck_refused")

    sql_constraints = []
    for label, bad_sql in (
        ("sql_lab_profile_under_live_refused",
         "INSERT INTO execution.live_qualifications VALUES "
         f"(gen_random_uuid(),'autonomous-dns-genai-lab-v1','LIVE','{'a'*64}','{'a'*64}','{'a'*64}','{'a'*64}',"
         f"'sha256:{'a'*64}','{'a'*64}',gen_random_uuid(),'1.1.1.1'::inet,1000,2000)"),
        ("sql_expired_order_refused",
         "INSERT INTO execution.live_qualifications VALUES "
         f"(gen_random_uuid(),'autonomous-dns-genai-live-v1','LIVE','{'a'*64}','{'a'*64}','{'a'*64}','{'a'*64}',"
         f"'sha256:{'a'*64}','{'a'*64}',gen_random_uuid(),'1.1.1.1'::inet,2000,1000)"),
    ):
        proc = subprocess.run(
            ["docker", "exec", container, "psql", "-X", "-U", "postgres", "-d", database, "-Atc", bad_sql],
            capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=15,
        )
        assert proc.returncode != 0, f"expected SQL constraint failure for {label}"
        sql_constraints.append(label)
    return app_precheck, sql_constraints


def _case_positive_provision_and_bind(container: str, database: str, prefix: str,
                                      report_dir: Path, env: dict[str, str], duskweave_bin: Path | None) -> dict:
    uid = os.geteuid()
    cfg = report_dir / "live-cfg"
    custody = cfg / "owner-custody.json"
    receipt = candidate_database.provision_database(
        container=container, database=database, role_prefix=prefix,
        config_root=cfg, custody_file=custody, env=env,
        expected_uids=(uid,), stop_at=report_dir, core_account=_local_core_account(),
    )
    assert receipt["status"] == "SUCCESS", receipt
    admin_dsn = _verify_custody_and_env(cfg, custody, receipt["roles"])
    role_count = _psql(container, database, f"SELECT count(*) FROM pg_roles WHERE rolname LIKE '{prefix}%'")
    assert role_count == "7", f"expected 7 owners, got {role_count}"

    inspect_out = _verify_core_inspect(duskweave_bin, cfg) if duskweave_bin else "skipped"
    manifest_path = cfg / "deployment.json"
    bind_res = candidate_binding.publish_binding(
        identities=SAMPLE_IDS, installed_identities=SAMPLE_IDS,
        resolver_ipv4="1.1.1.1", valid_from_ms=1000, valid_until_ms=4_000_000_000_000,
        now_ms=2000, manifest_path=manifest_path, intent_dir=report_dir / "live-intent",
        sql_executor=candidate_binding.pg_sql_executor(container, database, admin_dsn),
        env=env, expected_uids=(uid,), stop_at=report_dir,
    )
    assert bind_res["status"] == "SUCCESS", bind_res
    manifest_sha = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    assert manifest_sha == bind_res["manifest_sha256"]
    app_precheck, sql_constraints = _verify_refusals(
        container, database, admin_dsn, manifest_path, bind_res["qualification_ref"], report_dir,
    )
    return {
        "case": "positive_schema_0036_and_live_binding",
        "status": "PASS",
        "database": database,
        "role_count": 7,
        "schema_0036_function_count": len(receipt["verification"]["schema_0036_functions"]),
        "duskweave_inspect": inspect_out,
        "qualification_ref": bind_res["qualification_ref"],
        "manifest_sha256": manifest_sha,
        "application_precheck_refusals": app_precheck,
        "direct_sql_constraint_refusals": sql_constraints,
        "role_oids": receipt["verification"]["role_oids"],
    }


def _case_ack_loss_and_reconcile(container: str, database: str, prefix: str,
                                 report_dir: Path, env: dict[str, str]) -> dict:
    uid = os.geteuid()
    cfg = report_dir / "fault-cfg"
    custody = cfg / "owner-custody.json"
    prov = candidate_database.provision_database(
        container=container, database=database, role_prefix=prefix,
        config_root=cfg, custody_file=custody, env=env,
        expected_uids=(uid,), stop_at=report_dir, core_account=_local_core_account(),
    )
    assert prov["status"] == "SUCCESS", prov
    admin_dsn = json.loads(custody.read_text(encoding="utf-8"))["dsns"]["admin"]
    real_exec = candidate_binding.pg_sql_executor(container, database, admin_dsn)
    writes = {"count": 0}

    def ack_loss_exec(op: str, arg=None):
        res = real_exec(op, arg)
        if op == "insert":
            writes["count"] += 1
            committed = real_exec("read", arg["qualification_ref"])
            assert committed is not None and committed["manifest_sha256"] == arg["manifest_sha256"]
            raise TimeoutError("simulated_ack_loss_after_commit")
        return res

    manifest_path = cfg / "deployment.json"
    pub = candidate_binding.publish_binding(
        identities=SAMPLE_IDS, installed_identities=SAMPLE_IDS,
        resolver_ipv4="9.9.9.9", valid_from_ms=1000, valid_until_ms=4_000_000_000_000,
        now_ms=2000, manifest_path=manifest_path, intent_dir=report_dir / "fault-intent",
        sql_executor=ack_loss_exec, env=env, expected_uids=(uid,), stop_at=report_dir,
    )
    assert pub["status"] == "UNKNOWN" and writes["count"] == 1
    qref = pub["qualification_ref"]
    rec = candidate_binding.reconcile_binding(
        qualification_ref=qref, manifest_path=manifest_path,
        installed_identities=SAMPLE_IDS, sql_reader=lambda ref: real_exec("read", ref),
        expected_uids=(uid,), stop_at=report_dir, clock_fn=lambda: 2000,
    )
    assert rec["status"] == "SUCCESS" and rec["qualification_ref"] == qref and writes["count"] == 1
    return {
        "case": "commit_ack_loss_unknown_and_readonly_reconcile",
        "status": "PASS",
        "database": database,
        "qualification_ref": qref,
        "insert_count": writes["count"],
    }


def _assert_sanitized(payload: dict) -> None:
    dumped = json.dumps(payload)
    for forbidden in ("postgresql://", "postgres://", "DEEPSEEK_API_KEY", "custody_sha256"):
        if forbidden in dumped:
            raise AssertionError(f"forbidden secret pattern leaked: {forbidden}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Real PostgreSQL candidate qualification suite")
    parser.add_argument("--container", required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--role-prefix", required=True)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--duskweave-bin", type=Path, default=None)
    parser.add_argument("--fault-database", default="dw_native_lpqfix1_fault_20261115r1")
    parser.add_argument("--fault-prefix", default="dw_lpqfix1fault15")
    parser.add_argument("--custody-database", default="dw_native_lpqfix1_custody_20261115r1")
    parser.add_argument("--custody-prefix", default="dw_lpqfix1cust15")
    args = parser.parse_args(argv)
    if args.report.exists() and any(args.report.iterdir()):
        raise RuntimeError("integration_report_dir_collision")
    args.report.mkdir(parents=True, mode=0o755, exist_ok=True)
    os.chmod(args.report, 0o755)
    env = _clean_env()
    bootstrap.service_qualified(args.container, version_prefix="17.", schema_revision="0036")
    results = [
        _case_custody_failure(args.container, args.custody_database, args.custody_prefix, env),
        _case_positive_provision_and_bind(args.container, args.database, args.role_prefix, args.report, env, args.duskweave_bin),
        _case_ack_loss_and_reconcile(args.container, args.fault_database, args.fault_prefix, args.report, env),
    ]
    summary = {"status": "PASS", "container": args.container, "cases": results}
    _assert_sanitized(summary)
    (args.report / "integration-report.json").write_text(
        json.dumps(summary, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

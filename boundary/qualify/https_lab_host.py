#!/usr/bin/env python3
"""HTTPS LAB host custody, OCI image + root PEM extraction, schema-0038 verification, and quiescence."""

from __future__ import annotations

import hashlib
import os
import re
import subprocess
from pathlib import Path

from candidate_files import (
    FileCustodyError,
    read_protected_bytes,
    verify_file_list_custody,
)
from candidate_host import (
    HostObservationError,
    RETAINED_QUIESCENCE_DBS,
    check_quiescence,
    observe_installed_facts,
)
from candidate_image import extract_image_worker_digests
from candidate_postgres import (
    PostgresError,
    _norm_args,
    _probe_all_session_identities,
    _verify_db_and_logins,
    _verify_group_roles,
    run_psql_stdin,
)
import https_lab_release

_RPT = "/srv/duskweave/reports/https-lab-r1"
_CFG = "/etc/duskweave/m1/https-lab-r1"
HTTPS_LAB_SCOPE = {
    "source_dir": "/srv/duskweave/src/https-lab-r1",
    "build_root": "/srv/duskweave-build/https-lab-r1",
    "report_root": _RPT,
    "bundle_dir": f"{_RPT}/bundle",
    "backup_dir": f"{_RPT}/install-backup",
    "binding_intent_dir": f"{_RPT}/binding",
    "config_dir": _CFG,
    "owner_custody_path": f"{_CFG}/owner-custody.json",
    "manifest_path": "/etc/duskweave/m1/deployment.json",
    "root_pem_path": "/etc/duskweave/https-lab-root-v1.pem",
    "install_bin_dir": "/usr/local/libexec/duskweave",
    "database": "dw_native_https_lab_r1",
    "pg_container": "dw-live-dns-20261030-pg",
    "role_prefix": "dw_hlr1",
    "core_uid_path": "/srv/duskweave/.core-account-uid",
    "sudoers_path": "/etc/sudoers.d/duskweave-m1-launcher",
    "control_root": "/run/duskweave/m1",
    "launcher_root": "/run/duskweave/launcher",
    "netns_dir": "/run/netns",
    "live_dns_owner_path": "/etc/duskweave/m1/.live-dns-owner",
}
HTTPS_LAB_DRIVER_FILES = (
    "candidate.sh",
    "candidate_binding.py",
    "candidate_database.py",
    "candidate_files.py",
    "candidate_host.py",
    "candidate_image.py",
    "candidate_install.py",
    "candidate_postgres.py",
    "deployment_custody.sh",
    "https_lab.sh",
    "https_lab_binding.py",
    "https_lab_host.py",
    "https_lab_release.py",
    "qualify.sh",
    "release_descriptor.py",
)
ROOT_CA_IMAGE_PATH = "etc/duskweave/https-lab-root-v1.pem"
ROOT_CA_SIZE = https_lab_release.ROOT_CA_SIZE
ROOT_CA_SHA256 = https_lab_release.ROOT_CA_SHA256
REQUIRED_0038_FUNCTIONS = (
    "execution.inference_profile_matches",
    "execution.inference_reservation",
    "execution.inference_window_ms",
    "execution.genai_expected_intent",
    "execution.genai_binding_valid",
    "execution.assert_successor_pack",
    "execution.assert_autonomous_pack",
    "execution.run_start",
    "execution.run_current",
    "execution.live_https_loop_dns_reserve",
    "execution.live_https_loop_head_reserve",
    "execution.live_attempt_current",
    "execution.live_attempt_may_start",
    "execution.live_attempt_head_phase",
    "execution.live_attempt_dns_result",
    "execution.live_attempt_head_result",
    "execution.live_attempt_stop",
    "execution.read_https_loop_head_result",
    "terrain.accept_https_observation",
)
LAB_FN_DEF_RE = re.compile(
    r"CREATE OR REPLACE FUNCTION\s+((?:execution|terrain)\.\w+)\(([^)]*)\)\s+RETURNS\s+(.*?)\s+"
    r"(LANGUAGE\s+(?:sql|plpgsql)\b[^\n]*?)\s+AS\s+(\$[A-Za-z0-9_]*\$)(.*?)\5;",
    re.DOTALL,
)
LANG_RE = re.compile(r"\bLANGUAGE\s+(sql|plpgsql)\b")
SEARCH_PATH_RE = re.compile(r"\bSET\s+search_path\s*=\s*([^\s].*)$")


def verify_https_lab_driver_custody(
    qualify_dir: Path,
    expected_commit: str,
    expected_tree: str,
    *,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
    git_runner=subprocess.run,
) -> None:
    """Validate HTTPS LAB driver files and native bootstrap modules under Git custody."""
    repo_root = qualify_dir.parents[1]
    tracked = [qualify_dir / name for name in HTTPS_LAB_DRIVER_FILES] + [
        repo_root / "tests" / "support" / "m1_native_bootstrap.py",
        repo_root / "tests" / "support" / "m1_native_migrations.py",
    ]
    verify_file_list_custody(
        repo_root, tracked, expected_commit, expected_tree,
        expected_uids=expected_uids, stop_at=stop_at, git_runner=git_runner,
    )


def extract_https_lab_image_digests(image_ref: str, *, docker_runner=subprocess.run) -> dict:
    """Inspect OCI image for exact worker binaries and embedded synthetic root CA PEM."""
    return extract_image_worker_digests(
        image_ref,
        docker_runner=docker_runner,
        expected_files=("dw-dns-capture", "dw-dns-exporter"),
        expected_extra_files={ROOT_CA_IMAGE_PATH: (ROOT_CA_SIZE, ROOT_CA_SHA256)},
    )


def observe_installed_root_pem(
    root_pem_path: Path | None = None,
    *,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
) -> str:
    """Verify installed HTTPS LAB synthetic root CA PEM ownership, mode, size, and SHA-256."""
    target = root_pem_path or Path(HTTPS_LAB_SCOPE["root_pem_path"])
    try:
        raw = read_protected_bytes(
            target, expected_uids=expected_uids, allowed_modes=(0o400, 0o444),
            max_bytes=4096, stop_at=stop_at, status_on_error="REFUSED",
        )
    except FileCustodyError as exc:
        raise HostObservationError(exc.status, f"https_lab_root_pem_unqualified:{exc.reason}") from exc
    digest = hashlib.sha256(raw).hexdigest()
    if len(raw) != ROOT_CA_SIZE or digest != ROOT_CA_SHA256:
        raise HostObservationError("REFUSED", "https_lab_root_pem_mismatch")
    return digest


def observe_https_lab_facts(*, root_pem_path: Path | None = None, **kw) -> dict:
    """Observe host account, sinks, sudoers, installed binaries, and installed root CA PEM."""
    facts = observe_installed_facts(**kw)
    facts["root_pem_sha256"] = observe_installed_root_pem(
        root_pem_path, expected_uids=kw.get("expected_uids", (0,)), stop_at=kw.get("stop_at", Path("/")),
    )
    return facts


def extract_0038_function_specs(migration_0038_path: Path) -> dict[str, dict]:
    """Extract canonical schema-0038 execution and terrain function signatures and hashes."""
    sql = migration_0038_path.read_text(encoding="utf-8").replace("\r\n", "\n")
    wanted = set(REQUIRED_0038_FUNCTIONS)
    specs: dict[str, dict] = {}
    for match in LAB_FN_DEF_RE.finditer(sql):
        name, args, _ret, attrs, _tag, body = match.groups()
        if name not in wanted:
            continue
        lang_m = LANG_RE.search(attrs)
        sp_m = SEARCH_PATH_RE.search(attrs)
        if not lang_m or not sp_m or name in specs:
            raise PostgresError("INCOMPLETE", f"migration_0038_parse_ambiguous:{name}")
        specs[name] = {
            "args": _norm_args(args),
            "language": lang_m.group(1),
            "security_definer": "SECURITY DEFINER" in attrs,
            "search_path": f"search_path={sp_m.group(1).strip()}",
            "prosrc_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
        }
    if set(specs.keys()) != wanted:
        raise PostgresError("INCOMPLETE", "migration_0038_incomplete_specs")
    return specs


def _verify_0038_functions(container: str, database: str, specs: dict[str, dict], run_sql) -> dict[str, str]:
    exec_names = ",".join(f"'{n.split('.', 1)[1]}'" for n in specs if n.startswith("execution."))
    fn_rows = run_sql(
        container, None, database,
        "SELECT n.nspname || '.' || p.proname || '|' || pg_get_function_identity_arguments(p.oid) "
        "|| '|' || l.lanname || '|' || p.prosecdef::text || '|' || "
        "COALESCE(array_to_string(p.proconfig, ','), '') || '|' || "
        "encode(sha256(convert_to(replace(p.prosrc, E'\\r\\n', E'\\n'), 'UTF8')), 'hex') "
        "FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace "
        "JOIN pg_language l ON l.oid = p.prolang "
        f"WHERE (n.nspname = 'execution' AND p.proname = ANY(ARRAY[{exec_names}])) "
        "OR (n.nspname = 'terrain' AND p.proname = 'accept_https_observation') ORDER BY 1",
    ).splitlines()
    if len(fn_rows) != len(specs):
        raise PostgresError("INCOMPLETE", "schema_0038_function_count_or_overload_mismatch")
    fingerprints: dict[str, str] = {}
    for line in fn_rows:
        fname, fargs, lang, secdef, proconfig, sha = line.split("|", 5)
        exp = specs.get(fname)
        if exp is None or fname in fingerprints:
            raise PostgresError("INCOMPLETE", f"schema_0038_unexpected_or_duplicate:{fname}")
        if not (
            _norm_args(fargs) == exp["args"] and lang == exp["language"]
            and (secdef == "true") == exp["security_definer"]
            and proconfig == exp["search_path"] and sha == exp["prosrc_sha256"]
        ):
            raise PostgresError("INCOMPLETE", f"schema_0038_body_or_attr_mismatch:{fname}")
        fingerprints[fname] = sha
    return fingerprints


def verify_https_lab_schema_and_identities(
    container: str,
    database: str,
    roles: dict[str, str],
    dsns: dict[str, str],
    *,
    migration_0038_path: Path,
    role_prefix: str | None = None,
    sql_runner=None,
) -> dict:
    """Verify DB ownership, 7 logins, 6 group roles, exact 0038 functions, and direct sessions."""
    if set(roles.keys()) != {"admin", "core", "broker", "cognition", "history", "terrain", "stop"}:
        raise PostgresError("INCOMPLETE", "unexpected_role_owner_set")
    run_sql = sql_runner or run_psql_stdin
    specs = extract_0038_function_specs(migration_0038_path)
    role_oids = _verify_db_and_logins(container, database, roles, run_sql)
    _verify_group_roles(container, database, run_sql)
    if role_prefix is not None:
        all_pref = run_sql(
            container, None, "postgres", f"SELECT count(*) FROM pg_roles WHERE rolname LIKE '{role_prefix}%'",
        )
        if all_pref != str(len(roles)):
            raise PostgresError("INCOMPLETE", "unexpected_prefixed_roles_present")
    fingerprints = _verify_0038_functions(container, database, specs, run_sql)
    sessions = _probe_all_session_identities(container, roles, dsns, run_sql)
    return {
        "database": database, "role_oids": role_oids,
        "schema_0038_functions": fingerprints, "session_users": sessions,
    }


def check_https_lab_quiescence(*, extra_dbs: tuple[str, ...] = (), **kw) -> dict:
    """Verify host, container, netns, launcher, and 0038 live_effect_control quiescence."""
    _ = RETAINED_QUIESCENCE_DBS
    return check_quiescence(extra_dbs=extra_dbs, **kw)

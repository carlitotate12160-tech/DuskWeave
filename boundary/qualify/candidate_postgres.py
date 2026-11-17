#!/usr/bin/env python3
"""Key-free PostgreSQL stdin transport and exact schema-0036 / role verification."""

from __future__ import annotations

import hashlib
import re
import shutil
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit

RELEASE_FUNCTIONS = (
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
    "execution.assert_successor_pack",
)
EXPECTED_ROLE_GROUPS = {
    "admin": (),
    "core": ("dw_runtime",),
    "broker": ("dw_m1_broker",),
    "cognition": ("dw_m1_cognition",),
    "history": ("dw_m1_trajectory",),
    "terrain": ("dw_m1_terrain",),
    "stop": ("dw_m1_stop_writer",),
}
CANONICAL_GROUPS = (
    "dw_runtime",
    "dw_m1_broker",
    "dw_m1_cognition",
    "dw_m1_trajectory",
    "dw_m1_terrain",
    "dw_m1_stop_writer",
)
INNER_PSQL_SH = (
    "IFS= read -r PGHOST && IFS= read -r PGPORT && IFS= read -r PGDATABASE && "
    "IFS= read -r PGUSER && IFS= read -r PGPASSWORD && "
    "export PGHOST PGPORT PGDATABASE PGUSER PGPASSWORD && "
    'exec "$1" -X -v ON_ERROR_STOP=1 -Atc "$2"'
)
FN_DEF_RE = re.compile(
    r"CREATE OR REPLACE FUNCTION\s+(execution\.\w+)\(([^)]*)\)\s+RETURNS\s+(.*?)\s+"
    r"(LANGUAGE\s+(?:sql|plpgsql)\b[^\n]*?)\s+AS\s+(\$[A-Za-z0-9_]*\$)(.*?)\5;",
    re.DOTALL,
)
LANG_RE = re.compile(r"\bLANGUAGE\s+(sql|plpgsql)\b")
SEARCH_PATH_RE = re.compile(r"\bSET\s+search_path\s*=\s*([^\s].*)$")


class PostgresError(RuntimeError):
    """Bounded, credential-free PostgreSQL transport or verification error."""

    def __init__(self, status: str, reason: str):
        super().__init__(reason)
        self.status = status
        self.reason = reason


def _norm_args(raw_args: str) -> str:
    return re.sub(r"\s*,\s*", ", ", " ".join(raw_args.split())).strip()


def _has_ctrl(text: str) -> bool:
    return any(ch in text for ch in ("\r", "\n", "\x00"))


def parse_loopback_dsn(dsn: str) -> tuple[str, str, str, str, str]:
    """Parse generated loopback postgresql:// DSN into 5 clean connection fields."""
    if not isinstance(dsn, str) or not dsn or _has_ctrl(dsn):
        raise PostgresError("REFUSED", "dsn_malformed_or_control_chars")
    parts = urlsplit(dsn)
    if parts.scheme not in ("postgresql", "postgres") or parts.hostname not in ("127.0.0.1", "::1"):
        raise PostgresError("REFUSED", "dsn_non_loopback_or_scheme")
    port = str(parts.port or 5432)
    dbname = unquote(parts.path.lstrip("/"))
    user = unquote(parts.username or "")
    password = unquote(parts.password or "")
    fields = (parts.hostname, port, dbname, user, password)
    if any((not f or _has_ctrl(f)) for f in fields):
        raise PostgresError("REFUSED", "dsn_missing_or_unsafe_field")
    return fields


def run_psql_stdin(
    container: str,
    dsn: str | None,
    database: str,
    sql: str,
    *,
    timeout: int = 30,
    runner=subprocess.run,
    exec_prefix: list[str] | None = None,
    psql_bin: str = "psql",
) -> str:
    """Run psql keeping credentials exclusively in stdin and inner process memory."""
    clean_env = {"PATH": "/usr/bin:/bin:/usr/local/bin", "LC_ALL": "C"}
    docker_bin = shutil.which("docker") or "docker"
    if dsn is not None:
        host, port, dbname, user, password = parse_loopback_dsn(dsn)
        prefix = [docker_bin, "exec", "-i", container] if exec_prefix is None else list(exec_prefix)
        cmd = [*prefix, "sh", "-c", INNER_PSQL_SH, "sh", psql_bin, sql]
        payload = f"{host}\n{port}\n{dbname}\n{user}\n{password}\n"
    else:
        prefix = [docker_bin, "exec", container] if exec_prefix is None else list(exec_prefix)
        cmd = [*prefix, psql_bin, "-X", "-U", "postgres", "-d", database, "-v", "ON_ERROR_STOP=1", "-Atc", sql]
        payload = None
    try:
        proc = runner(cmd, input=payload, text=True, capture_output=True, env=clean_env, timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        raise PostgresError("UNKNOWN", "pg_ack_timeout") from exc
    except OSError as exc:
        raise PostgresError("UNKNOWN", "pg_client_spawn_failed") from exc
    if proc.returncode != 0:
        raise PostgresError("INCOMPLETE", f"pg_sql_failed:{proc.returncode}")
    return proc.stdout.strip()


def extract_release_function_specs(migration_path: Path) -> dict[str, dict]:
    """Extract canonical schema-0036 function signatures, attributes, and body hashes."""
    successor = migration_path.with_name("0036_live_context_purpose.sql")
    sql = "\n".join(p.read_text(encoding="utf-8").replace("\r\n", "\n") for p in (migration_path, successor))
    specs: dict[str, dict] = {}
    for match in FN_DEF_RE.finditer(sql):
        name, args, _ret, attrs, _tag, body = match.groups()
        lang_m = LANG_RE.search(attrs)
        sp_m = SEARCH_PATH_RE.search(attrs)
        if not lang_m or not sp_m or name in specs:
            raise PostgresError("INCOMPLETE", f"migration_0036_parse_ambiguous:{name}")
        specs[name] = {
            "args": _norm_args(args),
            "language": lang_m.group(1),
            "security_definer": "SECURITY DEFINER" in attrs,
            "search_path": f"search_path={sp_m.group(1).strip()}",
            "prosrc_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
        }
    if set(specs.keys()) != set(RELEASE_FUNCTIONS):
        raise PostgresError("INCOMPLETE", "migration_0036_incomplete_specs")
    return specs


def _verify_db_and_logins(container: str, database: str, roles: dict[str, str], run_sql) -> dict[str, int]:
    db_row = run_sql(
        container, None, "postgres",
        "SELECT d.datistemplate::text || '|' || pg_get_userbyid(d.datdba) "
        f"FROM pg_database d WHERE d.datname = '{database}'",
    )
    if db_row != f"false|{roles['admin']}":
        raise PostgresError("INCOMPLETE", "database_identity_or_owner_mismatch")
    quoted = ",".join(f"'{v}'" for v in roles.values())
    role_rows = run_sql(
        container, None, database,
        "SELECT r.rolname || '|' || r.oid || '|' || r.rolcanlogin || '|' || r.rolsuper || '|' || "
        "r.rolcreatedb || '|' || r.rolcreaterole || '|' || r.rolreplication || '|' || r.rolbypassrls || '|' || "
        "COALESCE(string_agg(g.rolname, ',' ORDER BY g.rolname), '') "
        "FROM pg_roles r LEFT JOIN pg_auth_members m ON m.member = r.oid "
        "LEFT JOIN pg_roles g ON g.oid = m.roleid "
        f"WHERE r.rolname = ANY(ARRAY[{quoted}]) GROUP BY r.rolname, r.oid, r.rolcanlogin, "
        "r.rolsuper, r.rolcreatedb, r.rolcreaterole, r.rolreplication, r.rolbypassrls ORDER BY r.rolname",
    ).splitlines()
    if len(role_rows) != len(EXPECTED_ROLE_GROUPS):
        raise PostgresError("INCOMPLETE", "login_role_count_mismatch")
    name_to_owner = {v: k for k, v in roles.items()}
    oids: dict[str, int] = {}
    for line in role_rows:
        parts = line.split("|", 8)
        rname, roid, flags, members_str = parts[0], int(parts[1]), parts[2:8], parts[8]
        if flags != ["true", "false", "false", "false", "false", "false"]:
            raise PostgresError("INCOMPLETE", f"role_privilege_flags_refused:{rname}")
        members = tuple(m for m in members_str.split(",") if m)
        expected_members = EXPECTED_ROLE_GROUPS[name_to_owner[rname]]
        if members != expected_members:
            raise PostgresError("INCOMPLETE", f"role_membership_mismatch:{rname}")
        oids[rname] = roid
    return oids


def _verify_group_roles(container: str, database: str, run_sql) -> None:
    quoted = ",".join(f"'{g}'" for g in CANONICAL_GROUPS)
    rows = run_sql(
        container, None, database,
        "SELECT rolname || '|' || rolcanlogin || '|' || rolsuper || '|' || rolbypassrls "
        f"FROM pg_roles WHERE rolname = ANY(ARRAY[{quoted}]) ORDER BY rolname",
    ).splitlines()
    if len(rows) != len(CANONICAL_GROUPS):
        raise PostgresError("INCOMPLETE", "group_roles_missing")
    for line in rows:
        gname, canlogin, is_super, bypassrls = line.split("|", 3)
        if (canlogin, is_super, bypassrls) != ("false", "false", "false"):
            raise PostgresError("INCOMPLETE", f"group_role_flags_refused:{gname}")


def _verify_release_functions(container: str, database: str, specs: dict[str, dict], run_sql) -> dict[str, str]:
    quoted = ",".join(f"'{n.split('.', 1)[1]}'" for n in specs)
    fn_rows = run_sql(
        container, None, database,
        "SELECT n.nspname || '.' || p.proname || '|' || pg_get_function_identity_arguments(p.oid) "
        "|| '|' || l.lanname || '|' || p.prosecdef::text || '|' || "
        "COALESCE(array_to_string(p.proconfig, ','), '') || '|' || "
        "encode(sha256(convert_to(replace(p.prosrc, E'\\r\\n', E'\\n'), 'UTF8')), 'hex') "
        "FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace "
        "JOIN pg_language l ON l.oid = p.prolang "
        f"WHERE n.nspname = 'execution' AND p.proname = ANY(ARRAY[{quoted}]) ORDER BY 1",
    ).splitlines()
    if len(fn_rows) != len(specs):
        raise PostgresError("INCOMPLETE", "schema_0036_function_count_or_overload_mismatch")
    fingerprints: dict[str, str] = {}
    for line in fn_rows:
        fname, fargs, lang, secdef, proconfig, sha = line.split("|", 5)
        exp = specs.get(fname)
        if exp is None or fname in fingerprints:
            raise PostgresError("INCOMPLETE", f"schema_0036_unexpected_or_duplicate:{fname}")
        matches = (
            _norm_args(fargs) == exp["args"]
            and lang == exp["language"]
            and (secdef == "true") == exp["security_definer"]
            and proconfig == exp["search_path"]
            and sha == exp["prosrc_sha256"]
        )
        if not matches:
            raise PostgresError("INCOMPLETE", f"schema_0036_body_or_attr_mismatch:{fname}")
        fingerprints[fname] = sha
    return fingerprints


def _probe_all_session_identities(container: str, roles: dict[str, str], dsns: dict[str, str], run_sql) -> dict[str, str]:
    sessions: dict[str, str] = {}
    for owner, expected_role in roles.items():
        if owner not in dsns:
            raise PostgresError("INCOMPLETE", f"missing_probe_dsn:{owner}")
        got = run_sql(container, dsns[owner], "", "SELECT session_user || '|' || current_user")
        if got != f"{expected_role}|{expected_role}":
            raise PostgresError("INCOMPLETE", f"session_user_identity_mismatch:{owner}")
        sessions[owner] = expected_role
    return sessions


def verify_release_schema_and_identities(
    container: str,
    database: str,
    roles: dict[str, str],
    dsns: dict[str, str],
    *,
    migration_path: Path,
    role_prefix: str | None = None,
    sql_runner=None,
) -> dict:
    """Verify DB ownership, 7 logins, 6 group roles, exact 0036 functions, and direct sessions."""
    if set(roles.keys()) != set(EXPECTED_ROLE_GROUPS.keys()):
        raise PostgresError("INCOMPLETE", "unexpected_role_owner_set")
    run_sql = sql_runner or run_psql_stdin
    specs = extract_release_function_specs(migration_path)
    role_oids = _verify_db_and_logins(container, database, roles, run_sql)
    _verify_group_roles(container, database, run_sql)
    if role_prefix is not None:
        all_pref = run_sql(
            container, None, "postgres",
            f"SELECT count(*) FROM pg_roles WHERE rolname LIKE '{role_prefix}%'",
        )
        if all_pref != str(len(roles)):
            raise PostgresError("INCOMPLETE", "unexpected_prefixed_roles_present")
    fingerprints = _verify_release_functions(container, database, specs, run_sql)
    sessions = _probe_all_session_identities(container, roles, dsns, run_sql)
    return {
        "database": database,
        "role_oids": role_oids,
        "schema_0036_functions": fingerprints,
        "session_users": sessions,
    }

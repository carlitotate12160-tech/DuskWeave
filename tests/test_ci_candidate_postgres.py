#!/usr/bin/env python3
"""Unit tests for key-free candidate_postgres stdin transport and schema-0035 verifier."""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = REPO_ROOT / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import candidate_postgres  # noqa: E402


MIGRATION_0035 = REPO_ROOT / "migrations" / "0035_pro_budget_c_v2.sql"


class CandidatePostgresTransportTests(unittest.TestCase):
    """Verify loopback DSN parsing and real INNER_PSQL_SH key-free stdin delivery."""

    def test_parse_loopback_dsn_accepts_loopback_and_rejects_nonloopback_or_ctrl(self) -> None:
        self.assertEqual(
            candidate_postgres.parse_loopback_dsn("postgresql://dw_user:s%40creT@127.0.0.1:5432/dw_db"),
            ("127.0.0.1", "5432", "dw_db", "dw_user", "s@creT"),
        )
        self.assertEqual(
            candidate_postgres.parse_loopback_dsn("postgresql://dw_user:pw@[::1]:5433/dw_db"),
            ("::1", "5433", "dw_db", "dw_user", "pw"),
        )
        for bad in (
            "postgresql://dw:pw@10.0.0.5:5432/db",
            "http://dw:pw@127.0.0.1:5432/db",
            "postgresql://dw@127.0.0.1:5432/db",
            "postgresql://dw:pw\n@127.0.0.1:5432/db",
            "postgresql://dw:pw\r@127.0.0.1:5432/db",
        ):
            with self.assertRaises(candidate_postgres.PostgresError) as err:
                candidate_postgres.parse_loopback_dsn(bad)
            self.assertEqual(err.exception.status, "REFUSED")

    def test_real_inner_psql_sh_keeps_secret_out_of_argv_and_into_child_env(self) -> None:
        sentinel = "TOP_SECRET_POSTGRES_TOKEN_98765"
        dsn = f"postgresql://dw_core_spy:{sentinel}@127.0.0.1:55432/dw_test_db"
        with tempfile.TemporaryDirectory() as tmp:
            tdir = Path(tmp)
            spy = tdir / "fake-psql"
            spy.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "print(json.dumps({\n"
                "  'argv': sys.argv,\n"
                "  'host': os.environ.get('PGHOST'),\n"
                "  'port': os.environ.get('PGPORT'),\n"
                "  'db': os.environ.get('PGDATABASE'),\n"
                "  'user': os.environ.get('PGUSER'),\n"
                "  'pw': os.environ.get('PGPASSWORD'),\n"
                "}))\n",
                encoding="utf-8",
            )
            os.chmod(spy, stat.S_IRWXU | stat.S_IRGRP | stat.S_IXGRP)

            seen_cmds: list[list[str]] = []

            def spy_runner(cmd, **kw):
                seen_cmds.append(list(cmd))
                return subprocess.run(cmd, **kw)

            out = candidate_postgres.run_psql_stdin(
                "unused", dsn, "dw_test_db", "SELECT 1",
                runner=spy_runner, exec_prefix=[], psql_bin=str(spy),
            )
            self.assertEqual(len(seen_cmds), 1)
            for arg in seen_cmds[0]:
                self.assertNotIn(sentinel, arg)
            observed = json.loads(out)
            self.assertEqual(observed["host"], "127.0.0.1")
            self.assertEqual(observed["port"], "55432")
            self.assertEqual(observed["db"], "dw_test_db")
            self.assertEqual(observed["user"], "dw_core_spy")
            self.assertEqual(observed["pw"], sentinel)
            for arg in observed["argv"]:
                self.assertNotIn(sentinel, arg)


class CandidatePostgresVerifierTests(unittest.TestCase):
    """Verify exact 0035 migration parser and failure detection on mock catalog answers."""

    def _clean_fixtures(self):
        specs = candidate_postgres.extract_0035_function_specs(MIGRATION_0035)
        prefix = "dw_t1"
        roles = {owner: f"{prefix}_{owner}" for owner in candidate_postgres.EXPECTED_ROLE_GROUPS}
        dsns = {owner: f"postgresql://{role}:pw_{owner}@127.0.0.1:5432/dw_db" for owner, role in roles.items()}
        login_rows = "\n".join(
            f"{roles[o]}|{100 + i}|true|false|false|false|false|false|{','.join(candidate_postgres.EXPECTED_ROLE_GROUPS[o])}"
            for i, o in enumerate(sorted(roles))
        )
        group_rows = "\n".join(f"{g}|false|false|false" for g in candidate_postgres.CANONICAL_GROUPS)
        fn_rows = "\n".join(
            f"{name}|{sp['args']}|{sp['language']}|{'true' if sp['security_definer'] else 'false'}|{sp['search_path']}|{sp['prosrc_sha256']}"
            for name, sp in sorted(specs.items())
        )
        return specs, roles, dsns, login_rows, group_rows, fn_rows

    def _run_verifier(self, roles, dsns, **overrides):
        _s, _r, _d, logins, groups, fns = self._clean_fixtures()
        table = (
            ("FROM pg_database", overrides.get("db_row", f"false|{roles['admin']}")),
            ("pg_auth_members", overrides.get("login_rows", logins)),
            ("FROM pg_roles WHERE rolname = ANY", overrides.get("group_rows", groups)),
            ("SELECT count(*) FROM pg_roles", overrides.get("pref_count", str(len(roles)))),
            ("FROM pg_proc", overrides.get("fn_rows", fns)),
        )
        bad_session_owner = overrides.get("bad_session_owner")

        def mock_sql(_cnt, dsn, _db, sql):
            if dsn is not None:
                owner = next(o for o, u in dsns.items() if u == dsn)
                return f"wrong|{roles[owner]}" if owner == bad_session_owner else f"{roles[owner]}|{roles[owner]}"
            for needle, reply in table:
                if needle in sql:
                    return reply
            raise AssertionError(sql)

        return candidate_postgres.verify_schema_0035_and_identities(
            "pg", "dw_db", roles, dsns, migration_path=MIGRATION_0035, role_prefix="dw_t1", sql_runner=mock_sql,
        )

    def test_extract_0035_matches_all_ten_functions_and_verify_passes_clean(self) -> None:
        specs, roles, dsns, *_ = self._clean_fixtures()
        self.assertEqual(set(specs.keys()), set(candidate_postgres.SCHEMA_0035_FUNCTIONS))
        out = self._run_verifier(roles, dsns)
        self.assertEqual(set(out["schema_0035_functions"].keys()), set(candidate_postgres.SCHEMA_0035_FUNCTIONS))
        self.assertEqual(out["session_users"]["core"], roles["core"])

    def test_verify_schema_0035_refuses_owner_flag_body_overload_and_session_tamper(self) -> None:
        specs, roles, dsns, logins, _groups, fns = self._clean_fixtures()
        wrong_body = fns.replace(specs["execution.run_start"]["prosrc_sha256"], "f" * 64)
        wrong_sp = re.sub(
            r"(execution\.run_start\|[^\n]+)" + re.escape(specs["execution.run_start"]["search_path"]),
            r"\1search_path=public",
            fns,
        )
        dup_overload = fns + "\n" + fns.splitlines()[0]
        bad_flag = re.sub(rf"({re.escape(roles['core'])}\|\d+\|true\|)false\|", r"\1true|", logins)

        cases = (
            ("owner", {"db_row": "false|postgres"}, "database_identity_or_owner_mismatch"),
            ("flags", {"login_rows": bad_flag}, f"role_privilege_flags_refused:{roles['core']}"),
            ("prefixed", {"pref_count": "8"}, "unexpected_prefixed_roles_present"),
            ("body", {"fn_rows": wrong_body}, "schema_0035_body_or_attr_mismatch:execution.run_start"),
            ("search_path", {"fn_rows": wrong_sp}, "schema_0035_body_or_attr_mismatch:execution.run_start"),
            ("overload", {"fn_rows": dup_overload}, "schema_0035_function_count_or_overload_mismatch"),
            ("session", {"bad_session_owner": "core"}, "session_user_identity_mismatch:core"),
        )
        for label, kw, reason in cases:
            with self.subTest(case=label):
                with self.assertRaises(candidate_postgres.PostgresError) as err:
                    self._run_verifier(roles, dsns, **kw)
                self.assertEqual(err.exception.reason, reason)


if __name__ == "__main__":
    unittest.main()

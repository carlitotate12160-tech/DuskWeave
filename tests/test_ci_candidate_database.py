"""Contract tests for candidate database provisioning and custody callbacks."""

import contextlib
import io
import json
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests" / "support"))
sys.path.insert(0, str(ROOT / "boundary" / "qualify"))

import m1_native_bootstrap as bootstrap  # noqa: E402
import candidate_database  # noqa: E402

RUNTIME_OWNERS = ("core", "broker", "cognition", "history", "terrain", "stop")
ENV_FILE_KEYS = {
    "DW_DATABASE_URL_FILE",
    "DW_INFERENCE_BROKER_DATABASE_URL_FILE",
    "DW_COGNITION_DATABASE_URL_FILE",
    "DW_TRAJECTORY_DATABASE_URL_FILE",
    "DW_TERRAIN_DATABASE_URL_FILE",
    "DW_STOP_WRITER_DATABASE_URL_FILE",
}


class FixtureCustodyExtensionTests(unittest.TestCase):
    def test_invalid_custody_options_refused_before_service_or_sql(self):
        bad_cases = (
            {"negative_controls": "false"},
            {"negative_controls": 0},
            {"custody_callback": "not-callable"},
            {"custody_callback": lambda plan: None, "negative_controls": True},
        )
        for kwargs in bad_cases:
            with self.subTest(kwargs=kwargs), \
                 mock.patch.object(bootstrap, "service_qualified") as svc, \
                 mock.patch.object(bootstrap, "private_run") as run:
                with self.assertRaises(RuntimeError):
                    bootstrap.Fixture(
                        "dwpg-r3", "fixture", database="dw_native_test_01",
                        schema_revision="0036", **kwargs,
                    )
                svc.assert_not_called()
                run.assert_not_called()

    def test_custody_callback_runs_before_first_write_and_omits_negative_controls(self):
        events = []
        captured = {}

        def fake_private(args, data=None):
            text = data or ""
            if "pg_database" in text or "pg_roles" in text:
                events.append("probe")
                return ""
            if "createuser" in args:
                events.append(f"createuser:{args[-1]}")
                return ""
            events.append("sql:" + text[:32])
            return ""

        def custody_cb(plan):
            events.append("custody_callback")
            captured["plan"] = plan

        with mock.patch.object(bootstrap, "service_qualified"), \
             mock.patch.object(bootstrap, "private_run", side_effect=fake_private), \
             mock.patch.object(bootstrap.Fixture, "migrate", lambda self, initial=True: events.append("migrate")), \
             mock.patch.object(bootstrap.Fixture, "negative_owners", side_effect=AssertionError("must_not_run")):
            fx = bootstrap.Fixture(
                "dwpg-r3", "fixture", database="dw_native_lpq_unit_01",
                schema_revision="0036", role_prefix="dw_lpqu01",
                custody_callback=custody_cb, negative_controls=False,
            )

        self.assertIn("custody_callback", events)
        first_write = next(i for i, e in enumerate(events) if e.startswith("createuser:"))
        self.assertLess(events.index("custody_callback"), first_write)
        plan = captured["plan"]
        self.assertEqual(set(plan["roles"].keys()), {"admin", *RUNTIME_OWNERS})
        self.assertEqual(set(fx.roles.keys()), {"admin", *RUNTIME_OWNERS})
        self.assertNotIn("otherbroker", fx.roles)
        self.assertEqual(len([e for e in events if e.startswith("createuser:")]), 7)

    def test_callback_failure_leaves_zero_role_or_database_writes(self):
        writes = []

        def fake_private(args, data=None):
            text = data or ""
            if "pg_database" in text or "pg_roles" in text:
                return ""
            writes.append((args, data))
            return ""

        def failing_cb(_plan):
            raise OSError("fsync_refused")

        with mock.patch.object(bootstrap, "service_qualified"), \
             mock.patch.object(bootstrap, "private_run", side_effect=fake_private):
            with self.assertRaises(OSError):
                bootstrap.Fixture(
                    "dwpg-r3", "fixture", database="dw_native_lpq_unit_02",
                    schema_revision="0036", role_prefix="dw_lpqu02",
                    custody_callback=failing_cb, negative_controls=False,
                )
        self.assertEqual(writes, [])


class CandidateDatabaseProvisioningTests(unittest.TestCase):
    def _base_env(self):
        return {"PATH": "/usr/bin:/bin", "HOME": "/root", "LANG": "C.UTF-8"}

    def test_refuses_ambient_provider_and_db_override_env_before_effects(self):
        forbidden = (
            {"DEEPSEEK_API_KEY": "secret-key"},
            {"DW_DATABASE_URL": "postgresql://u:p@h/db"},
            {"DW_DATABASE_CONFIG_MODE": "env-local"},
            {"DW_TEST_ADMIN_DATABASE_URL": "postgresql://a:p@h/db"},
            {"DW_DATABASE_URL_FILE": "/tmp/override.dsn"},
            {"CUSTOM_FOO_FILE": "/tmp/foo"},
        )
        for extra in forbidden:
            env = {**self._base_env(), **extra}
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as tmp:
                base = Path(tmp)
                os.chmod(base, 0o755)
                cfg = base / "cfg"
                with self.assertRaises(candidate_database.CandidateDatabaseError) as ctx:
                    candidate_database.provision_database(
                        container="dwpg-r3",
                        database="dw_native_lpq_test_01",
                        role_prefix="dw_lpqt01",
                        config_root=cfg,
                        custody_file=cfg / "owner-custody.json",
                        env=env,
                        expected_uids=(os.geteuid(),),
                        stop_at=base,
                        core_account={"name": "duskweave-core", "uid": os.geteuid(), "gid": os.getegid()},
                        fixture_factory=mock.Mock(side_effect=AssertionError("must_not_run")),
                    )
                self.assertEqual(ctx.exception.status, "REFUSED")

    def test_writes_exclusive_custody_and_core_0400_config_without_leaks(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            cfg = base / "cfg"
            custody = cfg / "owner-custody.json"
            secret_sentinel = "UltraSecretTok_9911"

            def fake_factory(**kwargs):
                plan = {
                    "database": kwargs["database"],
                    "schema_revision": "0036",
                    "role_prefix": kwargs["role_prefix"],
                    "role_suffix": "abc123",
                    "roles": {o: f"{kwargs['role_prefix']}_abc123_{o}" for o in ("admin", *RUNTIME_OWNERS)},
                    "passwords": {o: f"{secret_sentinel}_{o}" for o in ("admin", *RUNTIME_OWNERS)},
                    "dsns": {
                        o: f"postgresql://{kwargs['role_prefix']}_abc123_{o}:{secret_sentinel}_{o}@127.0.0.1:5432/{kwargs['database']}?sslmode=disable"
                        for o in ("admin", *RUNTIME_OWNERS)
                    },
                }
                kwargs["custody_callback"](plan)
                fx = mock.Mock()
                fx.database, fx.roles = kwargs["database"], plan["roles"]
                fx.secrets = list(plan["passwords"].values()) + list(plan["dsns"].values())
                return fx

            out = io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
                receipt = candidate_database.provision_database(
                    container="dwpg-r3",
                    database="dw_native_lpq_test_02",
                    role_prefix="dw_lpqt02",
                    config_root=cfg,
                    custody_file=custody,
                    env=self._base_env(),
                    expected_uids=(os.geteuid(),),
                    stop_at=base,
                    core_account={"name": "duskweave-core", "uid": os.geteuid(), "gid": os.getegid()},
                    fixture_factory=fake_factory,
                    verifier=lambda *_a, **_kw: {
                        "role_oids": {o: i + 100 for i, o in enumerate(("admin", *RUNTIME_OWNERS))},
                        "session_users": {o: f"dw_lpqt02_abc123_{o}" for o in ("admin", *RUNTIME_OWNERS)},
                        "schema_0036_functions": {f"fn_{i}": "a" * 64 for i in range(10)},
                    },
                )

            self.assertEqual(receipt["status"], "SUCCESS")
            self.assertNotIn("custody_sha256", receipt)
            self.assertEqual(stat.S_IMODE(cfg.stat().st_mode), 0o755)
            self.assertEqual(stat.S_IMODE(custody.stat().st_mode), 0o400)
            self.assertEqual(stat.S_IMODE((cfg / "environment.json").stat().st_mode), 0o400)
            custody_data = json.loads(custody.read_text(encoding="utf-8"))
            self.assertIn("admin", custody_data["dsns"])
            env_json = json.loads((cfg / "environment.json").read_text(encoding="utf-8"))
            self.assertEqual(env_json["DW_DATABASE_CONFIG_MODE"], "file")
            self.assertEqual(env_json["DW_M1_RUN_CONTROL_ROOT"], "/run/duskweave/m1")
            self.assertEqual(set(env_json.keys()) - {"DW_DATABASE_CONFIG_MODE", "DW_M1_RUN_CONTROL_ROOT"}, ENV_FILE_KEYS)
            self.assertNotIn("DW_TEST_ADMIN_DATABASE_URL", env_json)
            for owner in RUNTIME_OWNERS:
                self.assertEqual(stat.S_IMODE((cfg / f"{owner}.dsn").stat().st_mode), 0o400)
            serialized = json.dumps(receipt) + out.getvalue()
            self.assertNotIn(secret_sentinel, serialized)
            self.assertNotIn("postgresql://", serialized)

    def test_existing_config_root_and_custody_short_write_leave_zero_pg_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            cfg = base / "cfg"
            cfg.mkdir(mode=0o750)
            must_not_call = mock.Mock(side_effect=AssertionError("pg_write_forbidden"))
            with self.assertRaises(candidate_database.CandidateDatabaseError) as col:
                candidate_database.provision_database(
                    container="dwpg-r3", database="dw_native_lpq_test_03", role_prefix="dw_lpqt03",
                    config_root=cfg, custody_file=cfg / "owner-custody.json", env=self._base_env(),
                    expected_uids=(os.geteuid(),), stop_at=base,
                    core_account={"name": "duskweave-core", "uid": os.geteuid(), "gid": os.getegid()},
                    fixture_factory=must_not_call,
                )
            self.assertEqual(col.exception.status, "REFUSED")
            must_not_call.assert_not_called()

            cfg2 = base / "cfg2"
            zero_writes = []
            def zero_writer(fd, buf):
                zero_writes.append(len(buf))
                return 0
            def factory_with_cb(**kw):
                kw["custody_callback"]({
                    "database": kw["database"], "schema_revision": "0036",
                    "role_prefix": kw["role_prefix"], "role_suffix": "s1",
                    "roles": {"admin": "r_a"}, "passwords": {"admin": "p_a"},
                    "dsns": {"admin": "postgresql://r_a:p_a@127.0.0.1:5432/d"},
                })
                raise AssertionError("factory_continued_after_short_write")
            receipt_sw = candidate_database.provision_database(
                container="dwpg-r3", database="dw_native_lpq_test_03b", role_prefix="dw_lpqt03b",
                config_root=cfg2, custody_file=cfg2 / "owner-custody.json", env=self._base_env(),
                expected_uids=(os.geteuid(),), stop_at=base,
                core_account={"name": "duskweave-core", "uid": os.geteuid(), "gid": os.getegid()},
                fixture_factory=factory_with_cb, writer=zero_writer,
            )
            self.assertEqual(receipt_sw["status"], "INCOMPLETE")
            self.assertEqual(receipt_sw["reason"], "custody_short_write_zero_progress")
            self.assertGreaterEqual(len(zero_writes), 1)

    def test_partial_provision_failure_returns_incomplete_and_cli_refuses_outside_fixed_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            cfg = base / "cfg"
            custody = cfg / "owner-custody.json"
            calls = []

            def partial_fail(**kwargs):
                calls.append(1)
                kwargs["custody_callback"]({
                    "database": kwargs["database"], "schema_revision": "0036",
                    "role_prefix": kwargs["role_prefix"], "role_suffix": "p1",
                    "roles": {o: f"{kwargs['role_prefix']}_p1_{o}" for o in ("admin", *RUNTIME_OWNERS)},
                    "passwords": {o: f"SecretFail_{o}" for o in ("admin", *RUNTIME_OWNERS)},
                    "dsns": {o: f"postgresql://u:SecretFail_{o}@127.0.0.1:5432/d" for o in ("admin", *RUNTIME_OWNERS)},
                })
                raise RuntimeError("native_migration_0035_failed")

            receipt = candidate_database.provision_database(
                container="dwpg-r3", database="dw_native_lpq_test_04", role_prefix="dw_lpqt04",
                config_root=cfg, custody_file=custody, env=self._base_env(),
                expected_uids=(os.geteuid(),), stop_at=base,
                core_account={"name": "duskweave-core", "uid": os.geteuid(), "gid": os.getegid()},
                fixture_factory=partial_fail,
            )
            self.assertEqual(receipt["status"], "INCOMPLETE")
            self.assertEqual(len(calls), 1)
            self.assertTrue(custody.exists())
            self.assertNotIn("SecretFail", json.dumps(receipt))
            self.assertNotIn("postgresql://", json.dumps(receipt))
            rc = candidate_database.main(["--expected-driver-commit", "a" * 40, "--expected-driver-tree", "b" * 40])
            self.assertEqual(rc, 1)

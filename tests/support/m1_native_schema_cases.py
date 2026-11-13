#!/usr/bin/env python3
"""SchemaRevisionCases: maintained bootstrap branches under schema 0032.

Synthetic probes only; no real credentials, containers or databases. Covers
new-revision acceptance/refusal, live-service routing, stop-owner/group
provisioning and migration selection/order. The existing helper module owns
its other scenario families.
"""

import re
import sys
import unittest
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "tests" / "support"))

import m1_native_bootstrap as bootstrap  # noqa: E402


def migrate_calls(revision, initial):
    """Record the ordered migration statements the maintained path selects."""
    fixture = object.__new__(bootstrap.Fixture)
    fixture.migration_actor = "dw_probe_admin"
    fixture.schema_revision = revision
    calls = []
    fixture.sql = calls.append
    fixture.migrate(initial=initial)
    return calls


def provisioning_probe(recorded):
    """Answer the provisioning calls with synthetic absence/acceptance values."""

    def fake_run(args, data=None):
        statement = data or ""
        recorded.append((list(args), statement))
        return ""

    return fake_run


class SchemaRevisionCases(unittest.TestCase):
    def test_revision_0032_accepted_and_out_of_range_refused(self):
        self.assertIn("0032", bootstrap.SCHEMA_REVISIONS)
        self.assertEqual(bootstrap.SCHEMA_REVISIONS[-1], "0034")

        def bomb(args, data=None):
            raise AssertionError("provisioning must not start for an invalid revision")

        for revision, kind in (("0034", "live"), ("0035", "fixture"),
                               ("0033", "live"), ("0032", "live"), ("0031", "live"), ("32", "fixture")):
            with mock.patch.object(bootstrap, "private_run", bomb):
                with self.assertRaisesRegex(RuntimeError, "^native_schema_revision_refused$"):
                    bootstrap.Fixture("dwpg-r3", kind, schema_revision=revision)

    def test_live_container_0032_uses_the_live_service_lane(self):
        with mock.patch.object(bootstrap, "live_service_qualified") as live_check, \
             mock.patch.object(bootstrap, "supervisor_request") as helper:
            for revision in ("0030", "0031", "0032", "0033", "0034"):
                bootstrap.service_qualified(bootstrap.LIVE_CONTAINER, schema_revision=revision)
                live_check.assert_called_once_with(bootstrap.LIVE_CONTAINER)
                live_check.reset_mock()
                helper.assert_not_called()
            # CI keeps the helper lane even on the live container; the local
            # non-live container also stays on the helper lane.
            bootstrap.service_qualified(bootstrap.LIVE_CONTAINER, ci=True,
                                        version_prefix="17.", schema_revision="0032")
            bootstrap.service_qualified("dwpg-r3", version_prefix="17.", schema_revision="0032")
            self.assertEqual(helper.call_count, 2)
            live_check.assert_not_called()

    def test_constructor_0032_provisions_stop_owner_group_and_dsn(self):
        recorded = []
        with mock.patch.object(bootstrap, "service_qualified"), \
             mock.patch.object(bootstrap, "private_run", provisioning_probe(recorded)):
            fixture = bootstrap.Fixture("dwpg-r3", "fixture", database="dw_native_s32_probe_a",
                                        schema_revision="0032", role_prefix="dw_s32p")
        statements = [data for _, data in recorded if data]
        argv_text = [" ".join(args) for args, _ in recorded]
        self.assertTrue(
            any("createuser" in argv and re.search(r"dw_s32p_[0-9a-f]{12}_stop\b", argv)
                for argv in argv_text),
            "the stop owner login must be provisioned at schema 0032")
        self.assertTrue(
            any("CREATE ROLE dw_m1_stop_writer" in statement for statement in statements),
            "the dw_m1_stop_writer group must be provisioned at schema 0032")
        self.assertTrue(
            any(re.search(r"GRANT dw_m1_stop_writer TO dw_s32p_[0-9a-f]{12}_stop\b", s)
                for s in statements),
            "the stop owner must join dw_m1_stop_writer at schema 0032")
        self.assertIn("DW_STOP_WRITER_DATABASE_URL", fixture.env)
        migrations = [s for s in statements if s.startswith("SET ROLE ")]
        self.assertEqual(len(migrations), 30)
        self.assertIn("ALTER FUNCTION", migrations[-1])

    def test_migrate_0032_selects_the_full_chain_in_order(self):
        initial = migrate_calls("0032", initial=True)
        refresh = migrate_calls("0032", initial=False)
        self.assertEqual(len(initial), 30)
        self.assertEqual(len(refresh), 21)
        for calls in (initial, refresh):
            self.assertIn("genai_expected_intent", calls[-4])
            self.assertIn("m1_autonomous_run_request_v4", calls[-3])
            self.assertIn("complete_inference_proposal", calls[-2])
            tail = calls[-1]
            self.assertEqual(tail.count("ALTER FUNCTION"), 25)
            self.assertIn("search_path = pg_catalog, pg_temp", tail)
            self.assertIn("execution.private_live_actor(text)", tail)
            self.assertIn("terrain.admit_dns_request(jsonb)", tail)
            self.assertIn("BEGIN;", tail)
            self.assertIn("COMMIT;", tail)

    def test_revision_tail_counts_stay_distinct(self):
        for revision, initial, refresh in (
            ("0020", 18, 9), ("0021", 19, 10), ("0022", 20, 11),
            ("0030", 28, 19), ("0031", 29, 20), ("0032", 30, 21), ("0033", 31, 22), ("0034", 32, 23),
        ):
            self.assertEqual(len(migrate_calls(revision, initial=True)), initial, revision)
            self.assertEqual(len(migrate_calls(revision, initial=False)), refresh, revision)

    def test_refresh_passes_static_alter_statements_through_verbatim(self):
        tail = migrate_calls("0032", initial=False)[-1]
        alters = [line for line in tail.splitlines() if line.startswith("ALTER FUNCTION ")]
        self.assertEqual(len(alters), 25)
        for line in alters:
            self.assertTrue(line.endswith("SET search_path = pg_catalog, pg_temp;"), line)
            self.assertNotIn("IF NOT EXISTS", line)
            self.assertNotIn("OR REPLACE", line)

    def test_refresh_keeps_0024_dynamic_trigger_replay_valid(self):
        fence = migrate_calls("0032", initial=False)[12]
        self.assertIn("private_live_mutation_guard", fence)  # sanity: the 0024 slot
        self.assertIn("format('CREATE OR REPLACE TRIGGER live_authority_fence", fence)
        self.assertNotIn("format('DO $refresh$", fence)
        self.assertNotIn("format('CREATE TRIGGER live_authority_fence", fence)
        # Statement-start triggers elsewhere still gain the pg_trigger guard.
        terrain = migrate_calls("0032", initial=False)[3]
        self.assertIn("DO $refresh$", terrain)
        self.assertIn("pg_trigger", terrain)

    def test_revision_0033_provisions_and_replaces_only_the_three_owners(self):
        recorded = []
        with mock.patch.object(bootstrap, "service_qualified"), \
             mock.patch.object(bootstrap, "private_run", provisioning_probe(recorded)):
            fixture = bootstrap.Fixture(bootstrap.LIVE_CONTAINER, "fixture",
                                        database="dw_native_s33_probe", schema_revision="0033")
        self.assertIn("stop", fixture.roles)
        self.assertIn("DW_STOP_WRITER_DATABASE_URL", fixture.env)
        self.assertTrue(any("GRANT dw_m1_stop_writer TO" in data for _, data in recorded))
        for initial in (True, False):
            calls = migrate_calls("0033", initial)
            tail = calls[-1]
            functions = re.findall(r"CREATE OR REPLACE FUNCTION execution\.(\w+)", tail)
            self.assertEqual(functions, ["inference_profile_matches", "genai_expected_intent", "inference_call_state"])
            self.assertEqual(tail.count("SET search_path=pg_catalog,pg_temp"), 3)
            self.assertEqual(tail.count("SECURITY DEFINER"), 1)
            self.assertIn("FROM pg_catalog.pg_roles", tail)
            self.assertEqual(calls[-2].count("ALTER FUNCTION"), 25)

    def test_revision_0034_provisions_and_replaces_only_the_two_owners(self):
        recorded = []
        with mock.patch.object(bootstrap, "service_qualified"), \
             mock.patch.object(bootstrap, "private_run", provisioning_probe(recorded)):
            fixture = bootstrap.Fixture(bootstrap.LIVE_CONTAINER, "fixture",
                                        database="dw_native_s34_probe", schema_revision="0034")
        self.assertIn("stop", fixture.roles)
        self.assertIn("DW_STOP_WRITER_DATABASE_URL", fixture.env)
        for initial in (True, False):
            calls = migrate_calls("0034", initial)
            tail = calls[-1]
            functions = re.findall(r"CREATE OR REPLACE FUNCTION execution\.(\w+)", tail)
            self.assertEqual(functions, ["private_live_mutation_guard", "run_renew"])
            self.assertEqual(tail.count("SET search_path=pg_catalog, pg_temp"), 2)
            self.assertEqual(tail.count("LANGUAGE plpgsql SECURITY DEFINER"), 2)
            self.assertIn("FROM pg_catalog.pg_roles", tail)

    def test_container_refusal_runs_before_any_provisioning(self):
        def bomb(args, data=None):
            raise AssertionError("provisioning must not start for an unqualified service")

        with mock.patch.object(bootstrap, "private_run", bomb):
            with self.assertRaisesRegex(RuntimeError, "^native_service_unqualified$"):
                bootstrap.Fixture("not-a-service", "fixture", schema_revision="0032")


if __name__ == "__main__":
    unittest.main()

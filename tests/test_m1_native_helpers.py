#!/usr/bin/env python3
"""Focused tests for native helper database-name and env-file key handling.

Synthetic values only; no real credentials, containers or databases.
"""

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "tests" / "support"))

import m1_native_bootstrap as bootstrap  # noqa: E402
import m1_native_live as live  # noqa: E402


class DatabaseNameSelection(unittest.TestCase):
    def test_generated_names_are_unique_namespaced_and_bounded(self):
        names = {bootstrap.database_name(kind) for kind in bootstrap.KINDS for _ in range(8)}
        self.assertEqual(len(names), 16)
        for name in names:
            self.assertRegex(name, r"^dw_native_[a-z0-9_]+$")
            self.assertLessEqual(len(name.encode()), 63)

    def test_generated_names_carry_the_provisioning_kind(self):
        self.assertTrue(bootstrap.database_name("fixture").startswith("dw_native_fixture_"))
        self.assertTrue(bootstrap.database_name("live").startswith("dw_native_live_"))

    def test_explicit_name_in_namespace_accepted(self):
        self.assertEqual(
            bootstrap.database_name("fixture", "dw_native_manual_check01"),
            "dw_native_manual_check01",
        )

    def test_explicit_name_validation_rejects(self):
        for bad in (
            "",
            "dw_native_",
            "dw_native_" + "a" * 60,
            "Dw_native_upper01",
            "dw_native_bad-name",
            "dw_native_inj'; DROP TABLE x",
            "other_namespace_x",
            "9dw_native_leading",
        ):
            with self.assertRaises(RuntimeError, msg=bad):
                bootstrap.database_name("fixture", bad)


def qualified_service_probe(existing=None):
    """Simulated docker/psql responses for the owned-service qualification."""

    def fake_run(args, data=None):
        if args[:2] == ["docker", "inspect"]:
            return json.dumps([{
                "Config": {"Image": bootstrap.IMAGE},
                "State": {"Running": True},
                "NetworkSettings": {"Ports": {"5432/tcp": [{"HostIp": "127.0.0.1", "HostPort": "5432"}]}},
            }])
        statement = data or ""
        if "SELECT version()" in statement:
            return "PostgreSQL 17.11"
        if "pg_database WHERE datname" in statement:
            name = statement.split("datname='")[1].split("'")[0]
            return "1" if name == existing else ""
        raise AssertionError(f"unexpected provisioning call: {args}")

    return fake_run


class ExistingDatabaseRefusal(unittest.TestCase):
    def test_existing_database_name_is_refused(self):
        name = "dw_native_fixture_historical01"
        with mock.patch.object(bootstrap, "private_run", qualified_service_probe(existing=name)):
            with self.assertRaises(RuntimeError) as ctx:
                bootstrap.Fixture("dwpg-r3", "fixture", database=name)
        self.assertEqual(str(ctx.exception), "native_database_collision")

    def test_invalid_name_rejected_before_any_provisioning(self):
        def bomb(args, data=None):
            raise AssertionError("provisioning must not start for an invalid name")

        with mock.patch.object(bootstrap, "private_run", bomb):
            with self.assertRaises(RuntimeError) as ctx:
                bootstrap.Fixture("dwpg-r3", "fixture", database="x'; DROP DATABASE y")
        self.assertEqual(str(ctx.exception), "native_identifier_refused")


class EnvFileKeySelection(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "env"

    def tearDown(self):
        self.dir.cleanup()

    def write(self, text):
        self.path.write_text(text, encoding="utf-8")
        return self.path

    def test_reads_only_the_selected_key(self):
        self.assertEqual(
            live.selected_key(self.write("OTHER=x\nDEEPSEEK_API_KEY=sk-synthetic-a\n")),
            "sk-synthetic-a",
        )

    def test_export_prefix_and_quotes(self):
        self.assertEqual(
            live.selected_key(self.write("export DEEPSEEK_API_KEY='sk-synthetic-b'\n")),
            "sk-synthetic-b",
        )
        self.assertEqual(
            live.selected_key(self.write('DEEPSEEK_API_KEY="sk-synthetic-c"')),
            "sk-synthetic-c",
        )

    def test_shell_syntax_is_literal_never_executed(self):
        self.assertEqual(
            live.selected_key(self.write("DEEPSEEK_API_KEY=$(echo sk-synthetic-d)\n")),
            "$(echo sk-synthetic-d)",
        )

    def test_missing_file_and_missing_key_rejected(self):
        with self.assertRaises(OSError):
            live.selected_key(self.path)
        with self.assertRaises(RuntimeError):
            live.selected_key(self.write("OTHER=x\n"))
        with self.assertRaises(RuntimeError):
            live.selected_key(self.write("DEEPSEEK_API_KEY=\n"))
        with self.assertRaises(RuntimeError):
            live.selected_key(self.write("DEEPSEEK_API_KEY_OTHER=sk-synthetic-e\n"))

    def test_duplicate_and_invalid_key_rejected(self):
        with self.assertRaises(RuntimeError):
            live.selected_key(
                self.write("DEEPSEEK_API_KEY=sk-synthetic-f\nDEEPSEEK_API_KEY=sk-synthetic-g\n")
            )
        with self.assertRaises(RuntimeError):
            live.selected_key(self.write("DEEPSEEK_API_KEY=sk-bad\th\n"))


if __name__ == "__main__":
    unittest.main()

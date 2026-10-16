#!/usr/bin/env python3
"""Focused tests for native helper database-name and env-file key handling.

Synthetic values only; no real credentials, containers or databases.
"""

import contextlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
import uuid
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


DECISION = "11111111-2222-3333-4444-555555555555"
OTHER = "99999999-8888-7777-6666-555555555555"
EVENT = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
CALL_IDS = ["00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002"]


def identity(decision=DECISION):
    return {"kind": "m1_inference_identity_v1", "decision": decision, "dispatch_granted": False}


def receipt(category, decision=DECISION, event=EVENT):
    return {"kind": "m1_inference_receipt_v1", "call_ids": CALL_IDS,
            "recovery_category": category, "decision": decision, "event": event,
            "proposal_disposition": "validated_proposal", "publication": "committed",
            "history": "completed", "consumption": "completed_charged_unused_repair_reserved",
            "dispatch_granted": False}


def lines(records):
    return "".join(json.dumps(r) + "\n" for r in records)


class FakeFixture:
    """Provisioned fixture stand-in: scripted call counts and durable counts."""

    COUNTS = {"calls": 1, "generation": 1, "repair": 0, "reserved_calls": 2,
              "decisions": 1, "outbox": 1, "history": 1, "closures": 1}

    def __init__(self, calls=(), counts=None, count_error=False):
        self.env = {"PYTHONDONTWRITEBYTECODE": "1"}
        self.database = "dw_native_live_synth01"
        self.calls = iter(calls)
        self.counts = self.COUNTS if counts is None else counts
        self.count_error = count_error

    def sql(self, statement, database=None):
        if "clock_timestamp" in statement:
            return "1760000000\n"
        if "json_build_object" in statement:
            if self.counts is None:
                raise RuntimeError("native_counts_unavailable")
            return json.dumps(self.counts)
        if "count(*) FROM execution.inference_calls" in statement:
            if self.count_error:
                raise RuntimeError("native_count_unavailable")
            return f"{next(self.calls)}\n"
        return ""


class LiveEpisodeOutcomes(unittest.TestCase):
    """run() outcome regressions on mocked provisioning and process boundaries."""

    def routes(self, infer, inspect=None, recover=None):
        seen = inspect if inspect is not None else (0, lines([receipt("no_provider_replay")]))
        return {"prepare-operation": (0, f"operation={uuid.uuid4()}\n"),
                "register": (0, "result=accepted history=completed\n"),
                "m1-infer": infer,
                "m1-infer-inspect": seen,
                "m1-infer-recover": recover if recover is not None else seen}

    def run_episode(self, fixture, routes):
        def fake_command(binary, fx, args, key=None):
            outcome = routes[args[0]]
            if isinstance(outcome, BaseException):
                raise outcome
            return outcome

        with tempfile.TemporaryDirectory() as tmp:
            report = Path(tmp) / "report"
            report.mkdir()
            binary = Path(tmp) / "duskweave.exe"
            binary.write_bytes(b"synthetic-binary")
            env = Path(tmp) / ".env"
            env.write_text("DEEPSEEK_API_KEY=sk-synthetic\n", encoding="utf-8")
            out = io.StringIO()
            with mock.patch.object(live, "Fixture", lambda *a, **k: fixture), \
                 mock.patch.object(live, "command", fake_command), \
                 contextlib.redirect_stdout(out):
                code = live.run(binary, report, env, None)
            result = json.loads((report / "live_qualification.json").read_text(encoding="utf-8"))
            stdout = json.loads(out.getvalue().strip().splitlines()[-1])
            return code, result, stdout

    def test_success_reports_measured_zero_extra_calls(self):
        text = lines([identity(), receipt("inspect_original_identity")])
        fixture = FakeFixture(calls=[1, 1, 1, 1])
        code, result, stdout = self.run_episode(fixture, self.routes((0, text)))
        self.assertEqual((code, result["status"]), (0, "LIVE_PASS"))
        self.assertNotIn("failure", result)
        self.assertEqual(result["extra_call_records"], 0)
        self.assertEqual(stdout["extra_calls"], 0)

    def test_malformed_receipt_after_identity_keeps_decision_and_fails(self):
        text = lines([identity()]) + '{"kind":"m1_inference_receipt_v1","decis'
        fixture = FakeFixture(calls=[1, 1, 1, 1])
        code, result, _ = self.run_episode(fixture, self.routes((0, text)))
        self.assertEqual((code, result["status"]), (1, "LIVE_FAIL"))
        self.assertEqual(result["failure"], "native_live_receipt_malformed")
        self.assertEqual(result["decision"], DECISION)
        self.assertEqual(result["counts"]["calls"], 1)
        self.assertEqual(result["extra_call_records"], 0)

    def test_timeout_with_identity_keeps_category_and_adopts_event(self):
        for payload in (lines([identity()]).encode(), lines([identity()])):
            with self.subTest(payload=type(payload).__name__):
                timeout = subprocess.TimeoutExpired("m1-infer", 40, output=payload)
                fixture = FakeFixture(calls=[1, 1, 1, 1])
                code, result, _ = self.run_episode(fixture, self.routes(timeout))
                self.assertEqual((code, result["status"]), (1, "LIVE_FAIL"))
                self.assertEqual(result["failure"], "native_live_inference_timeout")
                self.assertEqual(result["decision"], DECISION)
                self.assertEqual(len(result["fresh_inspect_and_two_recoveries"]), 3)
                self.assertEqual(result["extra_call_records"], 0)

    def test_timeout_drops_truncated_trailing_receipt(self):
        payload = (lines([identity()]) + '{"kind":"m1_inference_recei').encode()
        timeout = subprocess.TimeoutExpired("m1-infer", 40, output=payload)
        fixture = FakeFixture(calls=[1, 1, 1, 1])
        code, result, _ = self.run_episode(fixture, self.routes(timeout))
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertEqual(result["failure"], "native_live_inference_timeout")
        self.assertEqual(result["decision"], DECISION)

    def test_changed_known_decision_or_event_still_rejected(self):
        text = lines([identity(), receipt("inspect_original_identity")])
        bad_decision = (0, lines([receipt("no_provider_replay", decision=OTHER)]))
        fixture = FakeFixture(calls=[1, 1, 1, 1])
        _, result, _ = self.run_episode(fixture, self.routes((0, text), inspect=bad_decision))
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertEqual(result["failure"], "native_live_identity_changed")
        bad_event = (0, lines([receipt("no_provider_replay", event=OTHER)]))
        fixture = FakeFixture(calls=[1, 1, 1, 1])
        _, result, _ = self.run_episode(fixture, self.routes((0, text), inspect=bad_event))
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertEqual(result["failure"], "native_live_identity_changed")

    def test_recovery_call_growth_reports_measured_replay(self):
        counts = dict(FakeFixture.COUNTS, calls=2)
        fixture = FakeFixture(calls=[1, 2], counts=counts)
        text = lines([identity(), receipt("inspect_original_identity")])
        _, result, stdout = self.run_episode(fixture, self.routes((0, text)))
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertEqual(result["failure"], "native_live_replay_detected")
        self.assertEqual(result["decision"], DECISION)
        self.assertEqual(result["extra_call_records"], 1)
        self.assertEqual(stdout["extra_calls"], 1)
        self.assertFalse(result["automatic_retry"])

    def test_unavailable_count_measurement_reports_null(self):
        fixture = FakeFixture(count_error=True)
        text = lines([identity(), receipt("inspect_original_identity")])
        _, result, stdout = self.run_episode(fixture, self.routes((0, text)))
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertEqual(result["decision"], DECISION)
        self.assertIsNone(result["extra_call_records"])
        self.assertIsNone(stdout["extra_calls"])

    def test_missing_receipt_never_passes(self):
        fixture = FakeFixture(calls=[1, 1, 1, 1])
        code, result, _ = self.run_episode(fixture, self.routes((0, lines([identity()]))))
        self.assertEqual((code, result["status"]), (1, "LIVE_FAIL"))
        self.assertEqual(result["decision"], DECISION)


if __name__ == "__main__":
    unittest.main()

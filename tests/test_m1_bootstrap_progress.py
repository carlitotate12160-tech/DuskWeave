"""Bridge refusal and presentation behavior for the fixture supervisor helper."""
import contextlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent / "support"))
import m1_native_bootstrap as bootstrap


class BridgeSelection(unittest.TestCase):
    """The helper is selected only through the process environment."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.report = Path(self.tmp.name)
        self.fixture = object.__new__(bootstrap.Fixture)
        self.fixture.env = {"PYTHONDONTWRITEBYTECODE": "1"}
        self.fixture.secrets = []

    def test_missing_or_relative_supervisor_is_refused(self):
        for environment in ({}, {bootstrap.SUPERVISOR_VARIABLE: "relative/helper.exe"}):
            with self.subTest(environment=environment):
                self.fixture.env.update(environment)
                with self.assertRaisesRegex(RuntimeError, "^native_supervisor_unavailable$"):
                    self.fixture.run("qualification", [sys.executable, "-c", "pass"], self.report)

    def test_existing_report_is_not_overwritten_and_no_helper_runs(self):
        for name in ("qualification.log", "qualification.result.json"):
            with self.subTest(target=name):
                path = self.report / name
                path.write_text("original", encoding="utf-8")
                with mock.patch.object(bootstrap.subprocess, "Popen") as child:
                    with self.assertRaises(RuntimeError):
                        self.fixture.run("qualification", [sys.executable, "-c", "pass"], self.report)
                child.assert_not_called()
                self.assertEqual(path.read_text(), "original")

    def test_invalid_labels_and_arguments_are_refused_before_any_helper(self):
        for label, args in (("Bad Label", ["ok"]), ("ok", "not-a-list"), ("ok", []),
                            ("ok", ["a" * 8193])):
            with self.subTest(label=label, args=args):
                with mock.patch.object(bootstrap.subprocess, "Popen") as child:
                    with self.assertRaises(RuntimeError):
                        self.fixture.run(label, args, self.report)
                child.assert_not_called()


class BridgeRefusalMapping(unittest.TestCase):
    def test_closed_helper_categories_pass_through(self):
        for category in ("native_report_collision", "native_action_spawn_failed",
                         "native_report_write_failed", "native_supervisor_request_refused"):
            with self.subTest(category=category):
                line = '{"ok": false, "category": "%s"}' % category
                with self.assertRaisesRegex(RuntimeError, f"^{category}$"):
                    bootstrap.validate_publication(line, Path("unused"), "label")

    def test_unknown_or_malformed_protocols_are_refused_without_reflection(self):
        for line in ('{"ok": false, "category": "synthetic-detail"}', "not-json", "",
                     '{"ok": true, "status": "completed"}'):
            with self.subTest(line=line):
                with self.assertRaisesRegex(RuntimeError, "^native_supervisor_protocol_refused$"):
                    bootstrap.validate_publication(line, Path("unused"), "label")

    def test_inconsistent_publication_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            report = Path(tmp)
            (report / "label.result.json").write_text('{"status": "completed", "exit": 0}')
            line = '{"ok": true, "status": "action_failed", "exit": 7}'
            with self.assertRaisesRegex(RuntimeError, "^native_supervisor_result_inconsistent$"):
                bootstrap.validate_publication(line, report, "label")


class ServiceCheckDelegation(unittest.TestCase):
    """Service qualification is delegated to the development supervisor."""

    def test_refusals_surface_closed_categories(self):
        for category in ("native_service_unqualified", "native_supervisor_unavailable"):
            with self.subTest(category=category):
                with mock.patch.object(bootstrap, "supervisor_request",
                                       side_effect=RuntimeError(category)):
                    with self.assertRaisesRegex(RuntimeError, f"^{category}$"):
                        bootstrap.Fixture("dwpg-r3", "fixture", database="dw_native_fixture_ports01")

    def test_delegated_request_carries_the_owned_container(self):
        captured = mock.patch.object(bootstrap, "supervisor_request", return_value='{"ok": true}')
        bomb = mock.patch.object(bootstrap, "private_run",
                                 side_effect=AssertionError("no docker parsing in Python"))
        with captured as request, bomb:
            # The SQL phase proves no docker inspection leaked back into Python.
            with self.assertRaises(AssertionError):
                bootstrap.Fixture("dwpg-r3", "fixture", database="dw_native_fixture_ports01")
        arguments = request.call_args[0][0]
        self.assertEqual(arguments[:3], ["service-check", "--container", "dwpg-r3"])
        self.assertIn("--version-prefix", arguments)


class ProgressRelay(unittest.TestCase):
    def test_only_closed_progress_lines_are_relayed(self):
        stream = io.StringIO(
            '{"action": "a", "status": "running", "elapsed_seconds": 15}\n'
            'synthetic-secret or any other text\n'
            '{"action": "a", "status": "running", "elapsed_seconds": 15, "extra": 1}\n'
            '{"action": "a", "status": "finished"}\n'
        )
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            bootstrap.relay_progress(stream)
        self.assertEqual(out.getvalue().count('"status": "running"'), 1)
        self.assertNotIn("synthetic-secret", out.getvalue())


class BridgePresentation(unittest.TestCase):
    """Presentation through the real helper; requires DW_TEST_FIXTURE_SUPERVISOR."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.report = Path(self.tmp.name)
        self.fixture = object.__new__(bootstrap.Fixture)
        self.fixture.env = dict(os.environ)
        self.fixture.secrets = ["synthetic-secret"]

    def test_real_child_failure_and_redaction_are_preserved(self):
        stream = io.StringIO()
        command = [sys.executable, "-c", "import sys; print('synthetic-secret'); sys.exit(7)"]
        with contextlib.redirect_stdout(stream):
            result = self.fixture.run("qualification", command, self.report)
        self.assertEqual(result, 7)
        self.assertNotIn("synthetic-secret", stream.getvalue())
        self.assertNotIn("synthetic-secret", (self.report / "qualification.log").read_text())
        self.assertIn("[REDACTED]", stream.getvalue())

    def test_completed_child_prints_action_exit_and_redacted_tail(self):
        stream = io.StringIO()
        command = [sys.executable, "-c",
                   "import sys; print('visible-line-1'); print('synthetic-secret'); print('visible-line-2')"]
        with contextlib.redirect_stdout(stream):
            result = self.fixture.run("qualification", command, self.report)
        self.assertEqual(result, 0)
        self.assertIn('{"action": "qualification", "exit": 0}', stream.getvalue())
        self.assertIn("visible-line-2", stream.getvalue())
        self.assertNotIn("synthetic-secret", stream.getvalue())

    def test_legacy_call_uses_the_finite_policy_ceiling(self):
        stream = io.StringIO()

        def publish_and_return(*args):
            (self.report / "qualification.log").write_text("synthetic redacted tail\n", encoding="utf-8")
            (self.report / "qualification.result.json").write_text(
                '{"status": "completed", "exit": 0}', encoding="utf-8")
            return '{"ok": true, "status": "completed", "exit": 0}'

        with mock.patch.object(bootstrap.Fixture, "invoke_supervisor",
                               side_effect=publish_and_return) as run:
            with contextlib.redirect_stdout(stream):
                self.fixture.run("qualification", [sys.executable, "-c", "pass"], self.report)
        request = json.loads(run.call_args[0][0])
        self.assertEqual(request["action_budget_ms"], bootstrap.LEGACY_CEILING_MS)
        self.assertEqual(request["redactions"], ["synthetic-secret"])


if __name__ == "__main__":
    unittest.main()

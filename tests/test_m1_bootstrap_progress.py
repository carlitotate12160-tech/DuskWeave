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


    def test_contradictory_results_and_metadata_cannot_be_success(self):
        valid = {"ok": True, "status": "completed", "exit": 0, "child_exit": 0,
                 "capture": "complete", "cleanup": "completed", "omissions": [],
                 "timing_origin": "request_admission"}
        cases = [{"child_exit": 7}, {"capture": "incomplete", "omissions": ["stdout"]},
                 {"cleanup": "unknown"}, {"exit": False}, {"status": "action_failed"}, {"status": "action_failed", "exit": 7},
                 {"status": "harness_timeout", "exit": 1}, {"omissions": ["raw-detail"]},
                 {"omissions": [["stdout"]]}, {"exit": 7, "child_exit": 8, "status": "action_failed"}]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for delta in cases:
                result = {**valid, **delta}
                (root / "label.result.json").write_text(json.dumps(result))
                with self.subTest(delta=delta), self.assertRaises(RuntimeError):
                    bootstrap.validate_publication(json.dumps(result), root, "label")
            for key, value in (("exit", False), ("child_exit", False), ("capture", "wrong"), ("cleanup", "wrong")):
                (root / "label.result.json").write_text(json.dumps({**valid, key: value}))
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    bootstrap.validate_publication(json.dumps(valid), root, "label")


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

    def test_real_overflow_cannot_return_success(self):
        command = [sys.executable, "-c", "print('x' * (17 * 1024 * 1024))"]
        with contextlib.redirect_stdout(io.StringIO()):
            code = self.fixture.run("overflow", command, self.report, max_duration_seconds=10)
        result = json.loads((self.report / "overflow.result.json").read_text())
        self.assertEqual(result["status"], "action_failed")
        self.assertEqual(result["child_exit"], 0)
        self.assertEqual(result["capture"], "incomplete")
        self.assertNotEqual(code, 0)
        self.assertEqual(result["exit"], code)
        self.assertEqual(result["omissions"], ["stdout"])
        self.assertIn("[stdout omitted: capture_overflow]",
                      (self.report / "overflow.log").read_text())

    def test_one_shot_observes_real_supervised_overflow(self):
        self.fixture.database = "dw_native_synthetic_oneshot"
        command = [sys.executable, "-c", "print('x' * (17 * 1024 * 1024))"]
        argv = ["bootstrap", "--report", str(self.report), "--", *command]
        with mock.patch.object(sys, "argv", argv), \
             mock.patch.object(bootstrap, "Fixture", return_value=self.fixture), \
             contextlib.redirect_stdout(io.StringIO()):
            self.assertNotEqual(bootstrap.main(), 0)

    def test_legacy_call_uses_the_finite_policy_ceiling(self):
        stream = io.StringIO()

        def publish_and_return(*args):
            (self.report / "qualification.log").write_text("synthetic redacted tail\n", encoding="utf-8")
            result = {"ok": True, "status": "completed", "exit": 0, "child_exit": 0,
                      "capture": "complete", "cleanup": "completed", "omissions": [],
                      "timing_origin": "request_admission"}
            (self.report / "qualification.result.json").write_text(json.dumps(result), encoding="utf-8")
            return json.dumps(result)

        with mock.patch.object(bootstrap.Fixture, "invoke_supervisor",
                               side_effect=publish_and_return) as run:
            with contextlib.redirect_stdout(stream):
                self.fixture.run("qualification", [sys.executable, "-c", "pass"], self.report)
        request = json.loads(run.call_args[0][0])
        self.assertEqual(request["action_budget_ms"], bootstrap.LEGACY_CEILING_MS)
        self.assertEqual(request["redactions"], ["synthetic-secret"])


class SessionContinuity(unittest.TestCase):
    def test_collision_and_invalid_input_preserve_fixture_and_next_action(self):
        with tempfile.TemporaryDirectory() as tmp:
            report = Path(tmp)
            retained = report / "used.log"
            retained.write_text("retained", encoding="utf-8")
            lines = [
                json.dumps({"label": "used", "args": ["unused"]}),
                "not-json",
                json.dumps(["SYNTHETIC_PRIVATE_INPUT"]),
                json.dumps({"label": "next", "args": ["unused"]}),
                json.dumps({"exit": True}),
            ]
            output = io.StringIO()

            def run_action(label, *_):
                if label == "used":
                    raise RuntimeError("native_report_collision")
                return 0

            with mock.patch.object(sys, "argv", ["bootstrap", "--session", "--report", tmp]), \
                 mock.patch.object(sys, "stdin", io.StringIO("\n".join(lines))), \
                 mock.patch.object(bootstrap, "Fixture") as factory, \
                 contextlib.redirect_stdout(output):
                factory.return_value.database = "dw_native_session_synthetic"
                factory.return_value.run.side_effect = run_action
                self.assertEqual(bootstrap.main(), 0)
            factory.assert_called_once()
            factory.return_value.run.assert_called_once_with("next", ["unused"], report)
            self.assertEqual(retained.read_text(encoding="utf-8"), "retained")
            records = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual(records[1]["category"], "native_report_collision")
            self.assertEqual(records[1]["exit"], 1)
            self.assertEqual(records[2]["category"], "native_action_refused")
            self.assertEqual(records[3]["category"], "native_action_refused")
            self.assertNotIn("SYNTHETIC_PRIVATE_INPUT", output.getvalue())

    def test_uncertain_supervisor_failure_still_terminates_session(self):
        with tempfile.TemporaryDirectory() as tmp:
            lines = "\n".join(json.dumps({"label": label, "args": ["unused"]})
                              for label in ("first", "must_not_run"))
            with mock.patch.object(sys, "argv", ["bootstrap", "--session", "--report", tmp]), \
                 mock.patch.object(sys, "stdin", io.StringIO(lines)), \
                 mock.patch.object(bootstrap, "Fixture") as factory, \
                 contextlib.redirect_stdout(io.StringIO()):
                factory.return_value.database = "dw_native_session_synthetic"
                factory.return_value.run.side_effect = RuntimeError("native_supervisor_unresponsive")
                with self.assertRaisesRegex(RuntimeError, "^native_supervisor_unresponsive$"):
                    bootstrap.main()
            self.assertEqual(factory.return_value.run.call_count, 1)


class RolePrefixSelection(unittest.TestCase):
    def test_cli_forwards_role_prefix_without_rewriting_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            args = ["bootstrap", "--role-prefix", "dw_cli", "--report", tmp, "--", "unused"]
            with mock.patch.object(sys, "argv", args), \
                 mock.patch.object(bootstrap, "Fixture") as factory, \
                 contextlib.redirect_stdout(io.StringIO()):
                factory.return_value.database = "dw_native_prefix_synthetic"
                factory.return_value.run.return_value = 0
                self.assertEqual(bootstrap.main(), 0)
            self.assertEqual(factory.call_args.kwargs["role_prefix"], "dw_cli")

    def test_prefix_reaches_every_principal_including_negative_controls(self):
        prefix = "dw_tm_20261026b"
        with mock.patch.object(bootstrap, "service_qualified"), \
             mock.patch.object(bootstrap, "private_run", return_value="") as run, \
             mock.patch.object(bootstrap.Fixture, "migrate"):
            bootstrap.Fixture("dwpg-r3", "fixture", role_prefix=prefix)
        created = [call.args[0][-1] for call in run.call_args_list
                   if "createuser" in call.args[0]]
        self.assertEqual(len(created), 15)
        self.assertEqual(len(set(created)), 15)
        self.assertEqual(len({name.rsplit("_", 1)[0] for name in created}), 1)
        self.assertTrue(all(name.startswith(prefix + "_") for name in created))
        self.assertTrue(all(len(name) <= 63 for name in created))

    def test_invalid_prefix_is_refused_before_service_or_sql(self):
        for prefix in ("postgres", "dw_BAD", "dw_a'; DROP ROLE x", "dw_" + "a" * 50):
            with self.subTest(prefix=prefix), \
                 mock.patch.object(bootstrap, "service_qualified") as service, \
                 mock.patch.object(bootstrap, "private_run") as run:
                with self.assertRaises(RuntimeError):
                    bootstrap.Fixture("dwpg-r3", "fixture", role_prefix=prefix)
                service.assert_not_called()
                run.assert_not_called()


if __name__ == "__main__":
    unittest.main()

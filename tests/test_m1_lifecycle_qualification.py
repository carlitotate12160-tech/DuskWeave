"""Controller behavior and owned synthetic process supervision; no live provider."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent / "support"))
import m1_native_bootstrap as bootstrap
import m1_lifecycle_qualification as qualification


class QualificationController(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.report = Path(self.tmp.name) / "report"
        self.calls = []
        self.exit_codes = [0, 0, 0, 0]
        self.summary_counts = [57, 18, 57, 18]
        self.statuses = ["completed"] * 4
        self.fixture = mock.Mock()
        self.fixture.env = {"DW_TEST_RUN_BINARY": "stale", "DEEPSEEK_API_KEY": "sentinel",
                            "DW_TEST_READINESS_OBSERVE": "1"}
        self.fixture.sql.return_value = "[]"
        self.fixture.run.side_effect = self.action

    def action(self, label, args, report, max_duration_seconds=None):
        index = len(self.calls)
        self.calls.append((label, args, dict(self.fixture.env), max_duration_seconds))
        count = self.summary_counts[index]
        (report / f"{label}.log").write_text(
            f"test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n",
            encoding="utf-8")
        (report / f"{label}.result.json").write_text(json.dumps({
            "status": self.statuses[index], "exit": self.exit_codes[index]}), encoding="utf-8")
        return self.exit_codes[index]

    def run_plan(self):
        with mock.patch.object(qualification, "preflight", return_value={"head": "synthetic"}, create=True), \
             mock.patch.object(bootstrap, "Fixture", return_value=self.fixture) as factory, \
             contextlib.redirect_stdout(io.StringIO()):
            result = qualification.qualify("dw_native_synthetic_qualification", self.report)
        return result, factory

    def result(self):
        return json.loads((self.report / "qualification.json").read_text(encoding="utf-8"))

    def test_nonzero_second_action_stops_and_snapshots_without_replay(self):
        self.exit_codes[1] = 7
        self.statuses[1] = "action_failed"
        result, factory = self.run_plan()
        self.assertEqual(result, 7)
        self.assertEqual(len(self.calls), 2)
        self.assertEqual([x["status"] for x in self.result()["actions"]],
                         ["PASS", "action_failed", "NOT_RUN", "NOT_RUN"])
        factory.assert_called_once_with("dwpg-r3", "fixture", database="dw_native_synthetic_qualification",
                                        schema_revision="0021")
        self.fixture.sql.assert_called_once()

    def test_one_fixture_four_serial_actions_and_target_bound_binaries(self):
        result, factory = self.run_plan()
        self.assertEqual(result, 0)
        factory.assert_called_once()
        self.assertEqual(len(self.calls), 4)
        targets = [x[2]["CARGO_TARGET_DIR"] for x in self.calls]
        self.assertEqual(targets[:2], [qualification.NORMAL] * 2)
        self.assertEqual(targets[2:], [qualification.COVERAGE] * 2)
        for index, (_, args, env, ceiling) in enumerate(self.calls):
            self.assertEqual(ceiling, 900)
            self.assertEqual(args[-3:], ["--", "--test-threads=1", "--nocapture"])
            self.assertEqual(Path(env["DW_TEST_RUN_BINARY"]), Path(targets[index]) / "debug/duskweave.exe")
            self.assertNotIn("DEEPSEEK_API_KEY", env)
            self.assertNotIn("DW_TEST_READINESS_OBSERVE", env)
            if index < 2:
                self.assertNotIn("CARGO_LLVM_COV_TARGET_DIR", env)
            else:
                self.assertIn("--no-report", args)
                self.assertNotIn("--no-clean", args)
                self.assertEqual(env["CARGO_LLVM_COV_TARGET_DIR"], qualification.COVERAGE)
        self.assertEqual(self.result()["status"], "PASS")
        self.fixture.sql.assert_not_called()

    def test_collision_refused_before_provisioning_and_preserved(self):
        self.report.mkdir()
        retained = self.report / "retained"
        retained.write_text("original")
        result, factory = self.run_plan()
        self.assertNotEqual(result, 0)
        factory.assert_not_called()
        self.assertEqual(retained.read_text(), "original")

    def test_zero_tests_is_failure_even_with_zero_exit(self):
        self.summary_counts[0] = 0
        result, _ = self.run_plan()
        self.assertNotEqual(result, 0)
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(self.result()["actions"][0]["status"], "summary_refused")

    def test_timeout_metadata_differs_from_unsupervised_exit_124(self):
        for status in ("harness_timeout", "action_failed"):
            with self.subTest(status=status):
                self.report = Path(self.tmp.name) / status
                self.calls.clear()
                self.exit_codes[0] = 124
                self.statuses[0] = status
                result, _ = self.run_plan()
                self.assertEqual(result, 124)
                self.assertEqual(self.result()["actions"][0]["status"], status)
                self.assertEqual(len(self.calls), 1)

    def test_supervision_timeout_exception_retains_harness_timeout_attribution(self):
        self.fixture.run.side_effect = subprocess.TimeoutExpired("ordinary_binary", 900)
        result, _ = self.run_plan()
        self.assertEqual(result, 124)
        actions = self.result()["actions"]
        self.assertEqual(actions[0]["status"], "harness_timeout")
        self.assertEqual([a["status"] for a in actions[1:]], ["NOT_RUN"] * 3)

    def test_actions_inherit_the_exact_supervisor_path(self):
        helper = "X:/dw-tools/dw-fixture-supervisor.exe"
        with mock.patch.dict(os.environ, {bootstrap.SUPERVISOR_VARIABLE: helper}):
            self.run_plan()
        inherited = {call[2].get(bootstrap.SUPERVISOR_VARIABLE) for call in self.calls}
        self.assertEqual(inherited, {helper})

    def test_supervisor_prerequisite_is_a_closed_tool_availability_check(self):
        original = dict(os.environ)
        try:
            os.environ.pop(bootstrap.SUPERVISOR_VARIABLE, None)
            with self.assertRaisesRegex(RuntimeError, "^qualification_tool_unavailable$"):
                qualification.supervisor_prerequisite()
            os.environ[bootstrap.SUPERVISOR_VARIABLE] = "relative/tool"
            with self.assertRaisesRegex(RuntimeError, "^qualification_tool_unavailable$"):
                qualification.supervisor_prerequisite()
            helper = "X:/dw-tools/dw-fixture-supervisor.exe"
            os.environ[bootstrap.SUPERVISOR_VARIABLE] = helper
            self.assertEqual(qualification.supervisor_prerequisite(), helper)
        finally:
            os.environ.clear()
            os.environ.update(original)

    def test_snapshot_failure_preserves_original_result_and_not_run(self):
        self.exit_codes[0] = 9
        self.statuses[0] = "action_failed"
        self.fixture.sql.side_effect = RuntimeError("synthetic-sensitive-detail")
        result, _ = self.run_plan()
        self.assertEqual(result, 9)
        self.assertEqual(self.result()["snapshot"], "unavailable")
        self.assertNotIn("synthetic-sensitive-detail", (self.report / "qualification.json").read_text())

    def test_wrong_platform_refused_before_fixture(self):
        with mock.patch.object(qualification.sys, "platform", "linux"), \
             mock.patch.object(bootstrap, "Fixture") as factory, \
             contextlib.redirect_stdout(io.StringIO()):
            result = qualification.qualify("dw_native_synthetic", self.report)
        self.assertNotEqual(result, 0)
        factory.assert_not_called()

    def test_relative_report_and_symlink_refused_before_provisioning(self):
        for report in (Path("relative-report"), Path.cwd() / "new-report"):
            with self.subTest(report=report), self.assertRaises(RuntimeError):
                qualification.validate_report(report)
        with mock.patch.object(Path, "is_symlink", return_value=True):
            with self.assertRaises(RuntimeError):
                qualification.validate_report(self.report)

    def test_prerequisite_failure_has_closed_category_and_no_fixture(self):
        for category in ("qualification_toolchain_refused", "qualification_service_refused",
                         "qualification_source_dirty", "synthetic-sensitive-detail"):
            with self.subTest(category=category), \
                 mock.patch.object(qualification, "preflight", side_effect=RuntimeError(category)), \
                 mock.patch.object(bootstrap, "Fixture") as factory, \
                 contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(qualification.qualify("dw_native_synthetic", self.report), 1)
                factory.assert_not_called()
                self.assertNotIn("synthetic-sensitive-detail", output.getvalue())
                self.assertIn('"status": "setup_failed"', output.getvalue())

    def test_missing_full_summary_and_failed_ignored_filtered_summaries_refuse(self):
        for summary in ("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out;",
                        "test result: ok. 57 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;",
                        "test result: FAILED. 57 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;"):
            with self.subTest(summary=summary):
                self.report.mkdir(exist_ok=True)
                (self.report / "full.log").write_text(summary)
                (self.report / "full.result.json").write_text('{"status":"completed","exit":0}')
                self.assertEqual(qualification.action_result(self.report, "full", 0, 57),
                                 ("summary_refused", 1))


class OwnedActionSupervision(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.report = Path(self.tmp.name)
        self.fixture = object.__new__(bootstrap.Fixture)
        self.fixture.env = dict(os.environ)
        self.fixture.secrets = ["synthetic-secret"]

    def run_child(self, source, limit):
        with contextlib.redirect_stdout(io.StringIO()) as output:
            result = self.fixture.run("synthetic", [sys.executable, "-c", source], self.report,
                                      max_duration_seconds=limit)
        metadata = json.loads((self.report / "synthetic.result.json").read_text())
        return result, metadata, output.getvalue()

    def test_owned_tree_timeout_drains_and_redacts_both_pipes(self):
        pid_file = self.report / "owned.pid"
        source = ("import subprocess, sys, time; from pathlib import Path; "
                  "child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); "
                  f"Path({str(pid_file)!r}).write_text(str(child.pid)); "
                  "print('synthetic-secret',flush=True); "
                  "print('synthetic-secret',file=sys.stderr,flush=True); time.sleep(60)")
        started = time.monotonic()
        result, metadata, output = self.run_child(source, 2)
        self.assertLess(time.monotonic() - started, 15)
        self.assertEqual(result, 124)
        self.assertEqual(metadata["status"], "harness_timeout")
        self.assertNotIn("synthetic-secret", output)
        log = (self.report / "synthetic.log").read_text()
        self.assertNotIn("synthetic-secret", log)
        self.assertEqual(log.count("[REDACTED]"), 2)
        pid = int(pid_file.read_text())
        if sys.platform == "win32":
            probe = subprocess.run(["tasklist", "/FI", f"PID eq {pid}", "/FO", "CSV", "/NH"],
                                   capture_output=True, text=True, timeout=5)
            self.assertNotIn(f'"{pid}"', probe.stdout)
        else:
            # A killed child may be a reparented zombie until the host reaps it.
            stat = Path(f"/proc/{pid}/stat")
            self.assertTrue(not stat.exists() or stat.read_text().split()[2] == "Z")

    def test_child_exit_124_is_action_failure_and_redaction_survives(self):
        result, metadata, output = self.run_child(
            "import sys; print('synthetic-secret'); sys.exit(124)", 5)
        self.assertEqual(result, 124)
        self.assertEqual(metadata["status"], "action_failed")
        self.assertNotIn("synthetic-secret", output)

    def test_completed_child_drains_more_than_pipe_capacity_and_keeps_integer_zero(self):
        result, metadata, output = self.run_child(
            "import sys; sys.stdout.write('x'*100000+'synthetic-secret\\n'); "
            "sys.stderr.write('y'*100000+'synthetic-secret\\n')", 5)
        self.assertEqual(result, 0)
        self.assertEqual(metadata["status"], "completed")
        self.assertNotIn("synthetic-secret", output)
        self.assertGreater((self.report / "synthetic.log").stat().st_size, 200000)

    def test_exited_parent_with_descendant_held_pipes_is_bounded_and_terminated(self):
        pid_file = self.report / "descendant.pid"
        source = ("import subprocess, sys; from pathlib import Path; "
                  "child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(6)']); "
                  f"Path({str(pid_file)!r}).write_text(str(child.pid))")
        outcome = {}

        def supervise():
            with contextlib.redirect_stdout(io.StringIO()):
                outcome["value"] = self.fixture.run(
                    "exited_parent", [sys.executable, "-c", source], self.report,
                    max_duration_seconds=0.5)

        # The descendant self-expires and this guard bounds the assertion run.
        worker = threading.Thread(target=supervise, daemon=True)
        started = time.monotonic()
        worker.start()
        worker.join(7)
        elapsed = time.monotonic() - started
        self.assertFalse(worker.is_alive(), "supervision must return within the guard")
        self.assertLess(elapsed, 4, "an exited parent must not delay cleanup until pipe holders exit")
        self.assertEqual(outcome["value"], 0)
        metadata = json.loads((self.report / "exited_parent.result.json").read_text())
        self.assertEqual(metadata["status"], "completed")
        pid = int(pid_file.read_text())
        if sys.platform == "win32":
            probe = subprocess.run(["tasklist", "/FI", f"PID eq {pid}", "/FO", "CSV", "/NH"],
                                   capture_output=True, text=True, timeout=5)
            self.assertNotIn(f'"{pid}"', probe.stdout)
        else:
            stat = Path(f"/proc/{pid}/stat")
            self.assertTrue(not stat.exists() or stat.read_text().split()[2] == "Z")


if __name__ == "__main__":
    unittest.main()

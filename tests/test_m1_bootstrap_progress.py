"""Safe progress and original child-result preservation; synthetic child only."""
import contextlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent / "support"))
import m1_native_bootstrap as bootstrap


class PendingChild:
    returncode = 0
    args = ["synthetic"]

    def __enter__(self):
        self.pending = True
        return self

    def __exit__(self, *args):
        return False

    def poll(self):
        return self.returncode

    def communicate(self, input=None, timeout=None):
        if self.pending and timeout is not None:
            self.pending = False
            raise subprocess.TimeoutExpired("synthetic", timeout,
                                             output="synthetic-secret")
        return "safe result\n", ""


class SafeProgress(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.report = Path(self.tmp.name)
        self.fixture = object.__new__(bootstrap.Fixture)
        self.fixture.env = dict(os.environ)
        self.fixture.secrets = ["synthetic-secret"]

    def test_pending_child_exposes_progress_without_partial_raw_output(self):
        stream = io.StringIO()
        with mock.patch.object(bootstrap.subprocess, "Popen", return_value=PendingChild()):
            with contextlib.redirect_stdout(stream):
                result = self.fixture.run("qualification", ["synthetic"], self.report)
        self.assertEqual(result, 0)
        self.assertIn('"status": "running"', stream.getvalue())
        self.assertNotIn("synthetic-secret", stream.getvalue())
        self.assertIn("safe result", (self.report / "qualification.log").read_text())

    def test_real_child_failure_and_redaction_are_preserved(self):
        stream = io.StringIO()
        command = [sys.executable, "-c",
                   "import sys; print('synthetic-secret'); sys.exit(7)"]
        with contextlib.redirect_stdout(stream):
            result = self.fixture.run("qualification", command, self.report)
        self.assertEqual(result, 7)
        self.assertNotIn("synthetic-secret", stream.getvalue())
        self.assertNotIn("synthetic-secret", (self.report / "qualification.log").read_text())
        self.assertIn("[REDACTED]", stream.getvalue())

    def test_existing_report_is_not_overwritten(self):
        path = self.report / "qualification.log"
        path.write_text("original", encoding="utf-8")
        with mock.patch.object(bootstrap.subprocess, "Popen") as child:
            with self.assertRaises(RuntimeError):
                self.fixture.run("qualification", ["synthetic"], self.report)
        child.assert_not_called()
        self.assertEqual(path.read_text(), "original")


if __name__ == "__main__":
    unittest.main()

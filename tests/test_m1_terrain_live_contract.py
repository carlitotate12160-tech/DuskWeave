"""Focused contract and armed-lifecycle tests for m1_terrain_live."""
import contextlib
import copy
import io
import json
from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent / "support"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import m1_terrain_live as live  # noqa: E402
import test_m1_terrain_live as base  # noqa: E402
from test_m1_terrain_live import (  # noqa: E402
    CALL, D, E, NOW, REPAIR, SENTINEL, lines, pack, uid,
)


class TerrainLiveContractTests(unittest.TestCase):
    def setUp(self):
        self.h = base.TerrainRunner()
        self.h.setUp()
        self.addCleanup(self.h.tmp.cleanup)

    def reset_harness(self):
        self.h.tmp.cleanup()
        self.h = base.TerrainRunner()
        self.h.setUp()
        self.addCleanup(self.h.tmp.cleanup)

    def run_armed(self, stdin_text="continue\n", write_obs=True, precreate_obs=False,
                  mutate_static_on_ready=False, mutate_obs_after_record=False):
        h = self.h
        for name, value in (("registration", h.registration), ("context-request", h.request)):
            (h.root / f"{name}.json").write_text(json.dumps(value), encoding="utf-8")
        obs_path = h.root / "observation.json"
        if precreate_obs:
            obs_path.write_text(json.dumps(h.observation), encoding="utf-8")
        events = []
        fixture_creations = []

        def make_fixture(*args, **kwargs):
            events.append("fixture")
            fx = mock.Mock(sql=h.sql, env={"DEEPSEEK_API_KEY": SENTINEL},
                           database="dw_native_live_armed")
            fixture_creations.append(fx)
            return fx

        orig_command = h.command

        def tracked_command(binary, fixture, args, key=None):
            events.append(args[0])
            if args[0] == "m1-terrain-record":
                self.assertTrue(obs_path.is_file(), "observation must exist before intake")
                if mutate_obs_after_record:
                    code, out = orig_command(binary, fixture, args, key)
                    obs_path.write_text("{}", encoding="utf-8")
                    return code, out
            return orig_command(binary, fixture, args, key)

        class StdinHook:
            def __init__(self, inner):
                self.inner = io.StringIO(inner)

            def readline(self_inner, limit=-1):
                current_out = stdout.getvalue()
                self.assertIn('"status": "READY"', current_out)
                self.assertEqual(events, ["fixture", "prepare-operation", "register"])
                self.assertFalse(obs_path.exists())
                events.append("stdin_wait")
                if mutate_static_on_ready:
                    (h.root / "registration.json").write_text("{}", encoding="utf-8")
                if write_obs:
                    obs_path.write_text(json.dumps(h.observation), encoding="utf-8")
                    events.append("obs_written")
                return self_inner.inner.readline(limit)

        args = ["--binary", str(h.binary), "--report", str(h.root / "report"),
                "--evidence-kind", "synthetic_input_provider_qualification",
                "--await-reviewed-observation",
                "--registration", str(h.root / "registration.json"),
                "--observation", str(obs_path),
                "--context-request", str(h.root / "context-request.json")]
        stdout = io.StringIO()
        with contextlib.ExitStack() as stack:
            stack.enter_context(mock.patch.object(live, "Fixture", side_effect=make_fixture))
            stack.enter_context(mock.patch.object(live, "selected_key", return_value=SENTINEL))
            stack.enter_context(mock.patch.object(live, "command", side_effect=tracked_command))
            stack.enter_context(mock.patch.object(live, "durable_counts", side_effect=h.counts))
            stack.enter_context(mock.patch.object(live.sys, "stdin", StdinHook(stdin_text)))
            stack.enter_context(contextlib.redirect_stdout(stdout))
            stack.enter_context(contextlib.redirect_stderr(stdout))
            code = live.main(args)
        report = json.loads((h.root / "report" / "terrain_live_qualification.json").read_text(encoding="utf-8"))
        self.assertNotIn(SENTINEL, json.dumps(report) + stdout.getvalue())
        return code, report, events, len(fixture_creations), stdout.getvalue()

    def test_reservation_pair_and_actual_call_negative_controls(self):
        self.h.reservation_rows = []
        _, res = self.h.run_episode()
        self.assertEqual((res["status"], res["failures"][0]), ("LIVE_FAIL", "reservation_pair_invalid"))

        self.reset_harness()
        self.h.reservation_rows = [dict(generation=CALL, repair=CALL)]
        _, res = self.h.run_episode()
        self.assertEqual((res["status"], res["failures"][0]), ("LIVE_FAIL", "reservation_pair_invalid"))

        self.reset_harness()
        self.h.initial["call_ids"] = [REPAIR, CALL]
        _, res = self.h.run_episode()
        self.assertEqual(res["status"], "LIVE_FAIL")
        self.assertIn("inference_calls_changed", res["failures"])

        self.reset_harness()
        self.h.initial["call_ids"] = [CALL]
        _, res = self.h.run_episode()
        self.assertEqual(res["status"], "LIVE_FAIL")
        self.assertIn("inference_calls_changed", res["failures"])

        self.reset_harness()
        self.h.original = [dict(call=REPAIR, p=pack())]
        _, res = self.h.run_episode()
        self.assertEqual((res["status"], res["failures"][0]), ("LIVE_FAIL", "actual_calls_invalid"))

        self.reset_harness()
        self.h.original = [dict(call=CALL, p=pack()), dict(call=uid(99), p=pack())]
        _, res = self.h.run_episode()
        self.assertEqual((res["status"], res["failures"][0]), ("LIVE_FAIL", "actual_calls_invalid"))

        self.reset_harness()
        self.h.original = [dict(call=CALL, p=pack()), dict(call=CALL, p=pack())]
        _, res = self.h.run_episode()
        self.assertEqual((res["status"], res["failures"][0]), ("LIVE_FAIL", "actual_calls_invalid"))

    def test_armed_sequence_orders_setup_before_observation_and_preserves_timestamps(self):
        code, report, events, fixtures, out = self.run_armed(stdin_text="continue\r\n")
        self.assertEqual((code, report["status"], fixtures), (0, "LIVE_PASS", 1))
        self.assertEqual(events, [
            "fixture", "prepare-operation", "register", "stdin_wait", "obs_written",
            "m1-terrain-record", "m1-infer", "m1-infer-inspect",
            "m1-infer-recover", "m1-infer-recover",
        ])
        self.assertEqual(report["terrain"]["perceived_at"], NOW)
        self.assertEqual(report["terrain"]["remaining_lifetime_seconds"], 60)
        self.assertEqual(report["reserved_call_ids"], [CALL, REPAIR])
        self.assertEqual(report["actual_call_ids"], [CALL])
        ready_line = next(json.loads(line) for line in out.splitlines() if '"READY"' in line)
        self.assertEqual(ready_line["engagement_id"], E)

    def test_armed_rejects_preexisting_or_missing_observation_and_invalid_tokens(self):
        code, report, events, fixtures, _ = self.run_armed(precreate_obs=True)
        self.assertEqual((code, report["status"], fixtures), (1, "LIVE_BLOCKED", 0))
        self.assertIn("observation_already_present", report["failures"])
        self.assertEqual(events, [])

        self.reset_harness()
        code, report, events, fixtures, _ = self.run_armed(stdin_text="", write_obs=False)
        self.assertEqual((code, report["status"], fixtures), (1, "LIVE_BLOCKED", 1))
        self.assertIn("operator_continue_eof", report["failures"])
        self.assertNotIn("m1-terrain-record", events)
        self.assertNotIn("m1-infer", events)

        for bad_line in ("abort\n", "continue extra\n", "x" * 20 + "\n", "continue"):
            self.reset_harness()
            code, report, events, _, _ = self.run_armed(stdin_text=bad_line)
            self.assertEqual((code, report["status"]), (1, "LIVE_BLOCKED"))
            self.assertIn("operator_continue_invalid", report["failures"])
            self.assertNotIn("m1-terrain-record", events)

        self.reset_harness()
        code, report, events, _, _ = self.run_armed(stdin_text="continue\n", write_obs=False)
        self.assertEqual((code, report["status"]), (1, "LIVE_BLOCKED"))
        self.assertIn("observation_missing", report["failures"])
        self.assertNotIn("m1-terrain-record", events)

    def test_armed_detects_static_or_observation_mutation_and_insufficient_remaining_lifetime(self):
        code, report, events, _, _ = self.run_armed(mutate_static_on_ready=True)
        self.assertEqual((code, report["status"]), (1, "LIVE_BLOCKED"))
        self.assertIn("operator_input_changed", report["failures"])
        self.assertNotIn("m1-terrain-record", events)

        self.reset_harness()
        code, report, _, _, _ = self.run_armed(mutate_obs_after_record=True)
        self.assertEqual((code, report["status"]), (1, "LIVE_BLOCKED"))
        self.assertIn("operator_input_changed", report["failures"])

        self.reset_harness()
        self.h.clock = NOW + 40  # remaining = 20s, not > 20s
        code, report, events, _, _ = self.run_armed()
        self.assertEqual((code, report["status"]), (1, "LIVE_BLOCKED"))
        self.assertIn("terrain_freshness_insufficient", report["failures"])
        self.assertEqual(report["terrain"]["remaining_lifetime_seconds"], 20)
        self.assertNotIn("m1-infer", events)

    def test_context_bytes_insufficient_retains_identity_and_distinguishes_zero_release_from_possible_send(self):
        zero_counts = dict(calls=0, generation=0, repair=0, reserved_calls=0,
                           decisions=0, outbox=0, history=0, closures=0)
        self.h.counts = [dict(zero_counts) for _ in range(4)]
        self.h.reservations = [0] * 4
        self.h.inference_code = 1
        absent_receipt = dict(kind="m1_inference_receipt_v1", decision=D, event=None, call_ids=[],
                              proposal_disposition="no_decision", publication="absent_not_proof_of_no_send",
                              history="not_recorded", pack_state="not_bound", pack_utf8=None,
                              proposal=None, recovery_category="no_provider_replay", dispatch_granted=False)
        self.h.historical = [copy.deepcopy(absent_receipt) for _ in range(3)]
        orig_cmd = self.h.command

        def floor_command(binary, fixture, args, key=None):
            if args[0] == "m1-infer":
                self.h.seen.append((args, key))
                return 1, lines(self.h.identity) + "error=context_bytes_insufficient\n"
            return orig_cmd(binary, fixture, args, key)

        with mock.patch.object(self.h, "command", side_effect=floor_command):
            code, result = self.h.run_episode()
        self.assertEqual((code, result["status"], result["decision"]), (1, "LIVE_BLOCKED", D))
        self.assertEqual(result["failures"], ["context_bytes_insufficient"])
        self.assertEqual(result["extra_call_records"], 0)
        self.assertEqual(result["extra_reservation_records"], 0)
        self.assertTrue(all(c["verified"] for c in result["historical_checks"]))

        self.reset_harness()
        self.h.reservations = [1] * 4
        self.h.historical = [copy.deepcopy(absent_receipt) for _ in range(3)]
        orig_cmd2 = self.h.command

        def floor_command2(binary, fixture, args, key=None):
            if args[0] == "m1-infer":
                self.h.seen.append((args, key))
                return 1, lines(self.h.identity) + "error=context_bytes_insufficient\n"
            return orig_cmd2(binary, fixture, args, key)

        with mock.patch.object(self.h, "command", side_effect=floor_command2):
            code, result = self.h.run_episode()
        self.assertEqual((code, result["status"], result["decision"]), (1, "LIVE_FAIL", D))
        self.assertIn("context_bytes_insufficient", result["failures"])
        self.assertIn("unexpected_inference_effect", result["failures"])


if __name__ == "__main__":
    unittest.main()

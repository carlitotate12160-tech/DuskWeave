"""Synthetic entrypoint tests; no provider, Docker, credentials or target contact."""
import contextlib
import copy
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent / "support"))
import m1_terrain_live as live  # noqa: E402
from m1_native_live import command as native_command  # noqa: E402


def uid(n):
    return str(uuid.UUID(int=n))


E, C, OP, D, EVENT, CLAIM, OBS, MAT, EVAL, ASSET, VANTAGE, CALL = map(uid, range(1, 13))
NOW = 1760000000
SENTINEL = "SYNTHETIC-RAW-SECRET-DO-NOT-RETAIN"
COUNTS = dict(calls=1, generation=1, repair=0, reserved_calls=2,
              decisions=1, outbox=1, history=1, closures=1)


def terrain_receipt():
    claim = dict(claim_id=CLAIM, claim_revision=1, evaluation_id=EVAL,
                 observation_id=OBS, material_id=MAT, asset_ref=ASSET,
                 vantage_ref=VANTAGE, perceived_at=NOW, temporal_frontier=NOW,
                 valid_until=NOW + 60, claim_status="observed", trust_tier="tier1",
                 corroborated=False, conflicted=False, freshness="fresh",
                 routing_usable=True, authority_active=True, evaluated_at=NOW,
                 subject_name="synthetic.example", resolver_ipv4="192.0.2.53",
                 answers=[dict(ttl_seconds=120)])
    return dict(kind="m1_terrain_receipt_v1", result="accepted", publication="committed",
                history="completed", dispatch_granted=False, engagement_id=E, campaign_id=C,
                operation_id=OP, event_id=uid(30), observation_id=OBS, material_id=MAT,
                evaluation_id=EVAL, claim_id=CLAIM, source_kind="operator_reviewed_dns",
                evaluation=dict(lowest_ttl_seconds=120),
                claim=claim, current_claim=copy.deepcopy(claim))


def pack():
    ref = dict(id="terrain_0", owner="cyber_terrain", source="operator_reviewed_dns",
               entity=CLAIM, asset_ref=ASSET, vantage_ref=VANTAGE, revision=1,
               lineage=[uid(30), OP, OBS, MAT, EVAL], perceived_at=NOW,
               time_uncertainty=0, ttl_seconds=120, valid_until=NOW + 60,
               temporal_frontier=NOW, status="observed", tier="tier1", freshness="fresh",
               corroborated=False, conflicted=False, integrity_hold=False)
    return json.dumps(dict(version="m1_context_pack_v1", engagement_id=E, campaign_id=C,
                           decision_id=D, references=[ref],
                           candidates=[dict(reference="asset_0", terrain_references=["terrain_0"])]))


def proposal():
    return dict(version="m1_inference_proposal_v1", hypothesis="Routing supports a bounded hypothesis.",
                considered_alternative="Defer pending a fresh independent observation.",
                references=["terrain_0"], expected_observation="A permitted response.",
                falsifying_observation="A changed routing premise.", uncertainty="DNS is not access.",
                rationale="Use the observed routing premise, not inferred ownership.",
                next_intent=dict(category="validate", references=["terrain_0"]))


def inference_receipt():
    return dict(kind="m1_inference_receipt_v1", decision=D, event=EVENT, call_ids=[CALL],
                proposal_disposition="validated_proposal", publication="committed", history="completed",
                pack_state="available", pack_utf8=pack(), proposal=proposal(),
                recovery_category="no_provider_replay", dispatch_granted=False)


def lines(*items):
    return "".join(json.dumps(item) + "\n" for item in items)


class TerrainRunner(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.binary = self.root / "duskweave.exe"
        self.binary.write_bytes(b"synthetic-binary")
        self.registration = dict(engagement_id=E, campaign_id=C,
                                 m1_inference_permission=dict(version="m1_inference_permission_v2",
                                     profile="deepseek-responses-envelope32-v1",
                                     field_classes=["owner_qualified_summaries"]))
        self.observation = dict(engagement_id=E, campaign_id=C, asset_ref=ASSET,
                                vantage_ref=VANTAGE, perceived_at=NOW, uncertainty_seconds=0,
                                source_kind="operator_reviewed_dns", answers=[dict(ttl_seconds=120)])
        self.request = dict(mission=dict(engagement_id=E, campaign_id=C),
                            candidates=[dict(asset_ref=ASSET, priority="high")])
        self.terrain = terrain_receipt()
        self.initial = inference_receipt()
        self.initial["recovery_category"] = "inspect_original_identity"
        self.historical = [inference_receipt() for _ in range(3)]
        self.original = [dict(call=CALL, p=pack())]
        self.counts = [dict(COUNTS) for _ in range(4)]
        self.reservations = [1] * 4
        self.foreign_call_growth = [0] * 4
        self.foreign_reservation_growth = [0] * 4
        self.clock = NOW
        self.hold = False
        self.seen = []
        self.timeout = None
        self.inspect_error = False
        self.sql_error = False
        self.registration_code = 0
        self.inference_code = 0
        self.identity = dict(kind="m1_inference_identity_v1", decision=D, dispatch_granted=False)
        self.mutate_request = False

    def sql(self, statement):
        if self.sql_error:
            raise RuntimeError(SENTINEL)
        if "clock_timestamp" in statement:
            return str(self.clock)
        if "claim_integrity_holds" in statement:
            return "t" if self.hold else "f"
        if "json_agg" in statement and "inference_calls" in statement:
            return json.dumps(self.original)
        if "native_input_bindings" in statement:
            return json.dumps([D])
        if "'campaign_calls'" in statement:
            index = 4 - len(self.reservations)
            rows = self.reservations.pop(0)
            return json.dumps(dict(reservation_records=rows,
                                   campaign_calls=self.counts[index]["calls"] + self.foreign_call_growth[index],
                                   campaign_reservation_records=rows + self.foreign_reservation_growth[index]))
        raise AssertionError(statement)

    def command(self, binary, fixture, args, key=None):
        self.seen.append((args, key))
        name = args[0]
        if name == "prepare-operation":
            return 0, f"operation={uid(31)}\n"
        if name == "register":
            return self.registration_code, "result=accepted history=completed\n"
        if name == "m1-terrain-record":
            return 0, lines(self.terrain)
        if name == "m1-infer":
            if self.timeout:
                raise self.timeout
            if self.mutate_request:
                (self.root / "context-request.json").write_text("{}", encoding="utf-8")
            return self.inference_code, lines(self.identity, self.initial)
        if self.inspect_error:
            raise RuntimeError(SENTINEL)
        self.assertIn(name, ("m1-infer-inspect", "m1-infer-recover"))
        self.assertEqual(args[-1], D)
        return 0, lines(self.historical.pop(0))

    def run_episode(self, real_command=False):
        for name, value in (("registration", self.registration), ("observation", self.observation),
                            ("context-request", self.request)):
            (self.root / f"{name}.json").write_text(json.dumps(value), encoding="utf-8")
        fixture = mock.Mock(sql=self.sql, env={"DEEPSEEK_API_KEY": SENTINEL},
                            database="dw_native_live_synthetic")
        args = ["--binary", str(self.binary), "--report", str(self.root / "report"),
                "--evidence-kind", "synthetic_input_provider_qualification"]
        for name in ("registration", "observation", "context-request"):
            args += [f"--{name}", str(self.root / f"{name}.json")]
        stdout = io.StringIO()
        def subprocess_run(args, **kwargs):
            if args[:2] == ["git", "rev-parse"]:
                return subprocess.CompletedProcess(args, 0, "a" * 40 + "\n")
            self.assertEqual(kwargs["timeout"], 40)
            self.assertTrue(kwargs["capture_output"])
            self.assertTrue(kwargs["text"])
            key = kwargs["env"].get("DEEPSEEK_API_KEY")
            self.assertEqual(key is not None, args[1] == "m1-infer")
            code, text = self.command(args[0], fixture, args[1:], key)
            return subprocess.CompletedProcess(args, code, text, SENTINEL)

        with contextlib.ExitStack() as stack:
            if real_command:
                stack.enter_context(mock.patch.object(live.subprocess, "run", side_effect=subprocess_run))
            stack.enter_context(mock.patch.object(live, "Fixture", return_value=fixture))
            stack.enter_context(mock.patch.object(live, "selected_key", return_value=SENTINEL))
            stack.enter_context(mock.patch.object(live, "command",
                                                  side_effect=native_command if real_command else self.command))
            stack.enter_context(mock.patch.object(live, "durable_counts", side_effect=self.counts))
            stack.enter_context(contextlib.redirect_stdout(stdout))
            stack.enter_context(contextlib.redirect_stderr(stdout))
            code = live.main(args)
        text = (self.root / "report" / "terrain_live_qualification.json").read_text(encoding="utf-8")
        self.assertNotIn(SENTINEL, text + stdout.getvalue())
        self.assertNotIn("synthetic.example", text)
        self.assertNotIn("resolver_ipv4", text)
        return code, json.loads(text)

    def test_original_bytes_lineage_and_only_one_provider_child(self):
        code, result = self.run_episode()
        self.assertEqual((code, result["status"]), (0, "LIVE_PASS"))
        self.assertEqual(result["extra_call_records"], 0)
        self.assertEqual(result["extra_reservation_records"], 0)
        self.assertTrue(result["proposal_cites_terrain"])
        self.assertEqual(result["proposal"], proposal())
        names = [args[0] for args, key in self.seen]
        self.assertEqual(names, ["prepare-operation", "register", "m1-terrain-record", "m1-infer",
                                 "m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"])
        self.assertEqual([args[0] for args, key in self.seen if key], ["m1-infer"])
        self.assertEqual(result["terrain_reference_ids"], ["terrain_0"])

    def test_missing_disclosure_blocks_without_inference(self):
        self.registration["m1_inference_permission"]["field_classes"] = []
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_BLOCKED")
        self.assertFalse(any(args[0] == "m1-infer" for args, _ in self.seen))

    def test_expired_or_held_observation_blocks_before_send(self):
        self.clock = NOW + 60
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_BLOCKED")
        self.assertFalse(any(args[0] == "m1-infer" for args, _ in self.seen))

    def test_unrelated_sent_reference_fails(self):
        value = json.loads(pack())
        value["references"][0]["entity"] = uid(99)
        altered = json.dumps(value)
        self.original[0]["p"] = altered
        self.initial["pack_utf8"] = altered
        for receipt in self.historical:
            receipt["pack_utf8"] = altered
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertNotIn("proposal", result)

    def test_timeout_partial_identity_remains_fail_after_inspection_failure(self):
        identity = dict(kind="m1_inference_identity_v1", decision=D, dispatch_granted=False)
        self.timeout = subprocess.TimeoutExpired("m1-infer", 40, output=(lines(identity) + '{"bad"').encode())
        self.inspect_error = True
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertEqual(result["decision"], D)
        self.assertEqual(result["counts"]["calls"], 1)
        self.assertEqual(len([a for a, _ in self.seen if a[0].startswith("m1-infer")]), 4)

    def test_unavailable_historical_pack_never_passes(self):
        self.historical[0].update(pack_state="unavailable", pack_utf8=None)
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")

    def test_unknown_latest_counts_not_stale_zero(self):
        self.counts[-1] = RuntimeError(SENTINEL)
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertIsNone(result["counts"])
        self.assertIsNone(result["extra_call_records"])

    def test_invalid_latest_counts_not_stale_zero(self):
        self.counts[-1]["calls"] = None
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertIsNone(result["counts"])
        self.assertIsNone(result["extra_call_records"])

    def test_current_profile_required_before_send(self):
        self.registration["m1_inference_permission"].update(version="m1_inference_permission_v1",
                                                          profile="deepseek-responses-v1")
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_BLOCKED")
        self.assertFalse(any(args[0] == "m1-infer" for args, _ in self.seen))

    def test_identity_receipt_cannot_grant_dispatch(self):
        self.identity["dispatch_granted"] = True
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")

    def test_changed_decision_fails_on_original_identity(self):
        self.historical[0]["decision"] = uid(99)
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["decision"]), ("LIVE_FAIL", D))

    def test_changed_event_fails_on_original_event(self):
        self.historical[1]["event"] = uid(99)
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["event"]), ("LIVE_FAIL", EVENT))

    def test_tampered_receipt_pack_fails(self):
        self.historical[0]["pack_utf8"] = pack() + " "
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")

    def test_generation_repair_bytes_differ_fails(self):
        self.original.append(dict(call=uid(70), p=pack() + " "))
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")

    def test_matching_generation_repair_bytes_pass(self):
        self.original.append(dict(call=uid(70), p=pack()))
        for counts in self.counts:
            counts.update(calls=2, repair=1)
        self.initial["call_ids"].append(uid(70))
        for receipt in self.historical:
            receipt["call_ids"].append(uid(70))
        code, result = self.run_episode()
        self.assertEqual((code, result["status"]), (0, "LIVE_PASS"))
        self.assertTrue(result["generation_repair_pack_equal"])

    def test_missing_original_pack_fails(self):
        self.original = []
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertNotIn("proposal", result)

    def test_recovery_call_growth_is_measured_failure(self):
        self.counts[-1].update(calls=2, repair=1)
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["extra_call_records"]), ("LIVE_FAIL", 1))

    def test_recovery_reservation_growth_is_measured_failure(self):
        self.reservations[-1] = 2
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["extra_reservation_records"]), ("LIVE_FAIL", 1))

    def test_recovery_calls_on_another_decision_cannot_escape_accounting(self):
        self.foreign_call_growth[-1] = 1
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["extra_call_records"]), ("LIVE_FAIL", 1))

    def test_recovery_reservations_on_another_decision_cannot_escape_accounting(self):
        self.foreign_reservation_growth[-1] = 1
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["extra_reservation_records"]), ("LIVE_FAIL", 1))

    def test_hidden_second_decision_before_first_snapshot_cannot_pass(self):
        self.foreign_call_growth = [1] * 4
        self.foreign_reservation_growth = [1] * 4
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertEqual(result["extra_call_records"], 0)

    def test_no_terrain_in_pack_fails(self):
        value = json.loads(pack())
        value["references"] = []
        altered = json.dumps(value)
        self.original[0]["p"] = altered
        self.initial["pack_utf8"] = altered
        for receipt in self.historical:
            receipt["pack_utf8"] = altered
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")

    def test_model_ignoring_terrain_is_reported_without_another_send(self):
        self.initial["proposal"]["references"] = []
        self.initial["proposal"]["next_intent"].update(category="defer", references=[])
        for receipt in self.historical:
            receipt["proposal"] = copy.deepcopy(self.initial["proposal"])
        code, result = self.run_episode()
        self.assertEqual((code, result["status"]), (0, "LIVE_PASS"))
        self.assertFalse(result["proposal_cites_terrain"])
        self.assertEqual(sum(a[0] == "m1-infer" for a, _ in self.seen), 1)

    def test_registration_refusal_blocks_before_send(self):
        self.registration_code = 1
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_BLOCKED")
        self.assertFalse(any(args[0] == "m1-infer" for args, _ in self.seen))

    def test_integrity_hold_blocks_before_send(self):
        self.hold = True
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_BLOCKED")

    def test_post_start_unknown_sql_never_becomes_blocked(self):
        self.timeout = subprocess.TimeoutExpired("m1-infer", 40, output=lines(self.identity).encode())
        def failed_counts(*args):
            self.sql_error = True
            raise RuntimeError(SENTINEL)
        self.counts = failed_counts
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["decision"]), ("LIVE_FAIL", D))
        self.assertIsNone(result["counts"])

    def test_timeout_without_receipt_recovers_only_scoped_durable_identity(self):
        self.timeout = subprocess.TimeoutExpired("m1-infer", 40, output=b"")
        _, result = self.run_episode()
        self.assertEqual((result["status"], result["decision"]), ("LIVE_FAIL", D))
        self.assertEqual(len(result["historical_checks"]), 3)

    def test_changed_context_request_is_not_used_for_recovery(self):
        self.mutate_request = True
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertFalse(any(a[0] == "m1-infer-recover" for a, _ in self.seen))

    def test_forged_uuid_never_reaches_inspection_sql(self):
        self.identity["decision"] = "';SELECT secret;"
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertNotIn("';SELECT", json.dumps(result))

    def test_changed_call_identity_fails(self):
        self.historical[0]["call_ids"] = [uid(99)]
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")

    def test_initial_proposal_cannot_change_in_history(self):
        self.initial["proposal"]["hypothesis"] = "A different admitted hypothesis."
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")

    def test_rejected_output_is_not_retained_even_with_safe_pack(self):
        self.inference_code = 1
        self.initial.update(proposal_disposition="reference_ineligible",
                            proposal=dict(hypothesis=SENTINEL))
        self.historical[0].update(proposal_disposition="reference_ineligible", proposal=None)
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_FAIL")
        self.assertNotIn("proposal", result)

    def test_report_collision_preserves_existing_report_and_does_not_run_again(self):
        self.run_episode()
        path = self.root / "report" / "terrain_live_qualification.json"
        original = path.read_bytes()
        self.seen.clear()
        code, _ = self.run_episode()
        self.assertEqual(code, 1)
        self.assertEqual(path.read_bytes(), original)
        self.assertFalse(self.seen)

    def test_input_bound_is_enforced_before_provisioning(self):
        self.registration["ignored_synthetic_padding"] = "x" * 16384
        _, result = self.run_episode()
        self.assertEqual(result["status"], "LIVE_BLOCKED")
        self.assertIn("input_bound", result["failures"])
        self.assertFalse(self.seen)

    def test_reused_command_isolates_key_and_discards_stderr_through_entrypoint(self):
        code, result = self.run_episode(real_command=True)
        self.assertEqual((code, result["status"]), (0, "LIVE_PASS"))

    def test_real_python_entrypoint_blocks_before_key_or_docker_for_oversized_input(self):
        paths = [self.root / f"{name}.json" for name in ("registration", "observation", "context-request")]
        for path in paths:
            path.write_text("{}", encoding="utf-8")
        paths[0].write_bytes(b" " * 16385)
        report = self.root / "real-process-report"
        args = [sys.executable, str(Path(live.__file__)), "--binary", str(self.binary),
                "--report", str(report), "--evidence-kind", "synthetic_input_provider_qualification",
                "--env-file", str(self.root / "missing-env")]
        for name, path in zip(("registration", "observation", "context-request"), paths):
            args += [f"--{name}", str(path)]
        result = subprocess.run(args, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertNotIn("Traceback", result.stderr)
        evidence = json.loads((report / "terrain_live_qualification.json").read_text(encoding="utf-8"))
        self.assertEqual(evidence["status"], "LIVE_BLOCKED")
        self.assertEqual(evidence["failures"], ["input_bound"])

    def test_report_directory_failure_is_category_only(self):
        report = self.root / "blocked-report"
        report.write_text(SENTINEL, encoding="utf-8")
        args = ["--binary", str(self.binary), "--report", str(report),
                "--evidence-kind", "synthetic_input_provider_qualification"]
        for name in ("registration", "observation", "context-request"):
            args += [f"--{name}", str(self.root / name)]
        output = io.StringIO()
        with contextlib.redirect_stderr(output):
            code = live.main(args)
        self.assertEqual(code, 1)
        self.assertEqual(output.getvalue().strip(), "terrain_live_report_unavailable")
        self.assertEqual(report.read_text(encoding="utf-8"), SENTINEL)

    def test_unexpected_post_start_exception_retains_known_identity_without_raw_error(self):
        self.counts[-1] = AttributeError(SENTINEL)
        code, result = self.run_episode()
        self.assertEqual((code, result["status"], result["decision"], result["event"]),
                         (1, "LIVE_FAIL", D, EVENT))
        self.assertIn("qualification_interrupted", result["failures"])
        self.assertIsNone(result["counts"])

    def test_operator_interrupt_retains_known_identity_without_stale_counts(self):
        self.counts[-1] = KeyboardInterrupt()
        try:
            code, result = self.run_episode()
        except KeyboardInterrupt:
            self.fail("The interrupted qualification must retain its safe report")
        self.assertEqual((code, result["status"], result["decision"]), (1, "LIVE_FAIL", D))
        self.assertIn("qualification_interrupted", result["failures"])
        self.assertIsNone(result["counts"])


if __name__ == "__main__":
    unittest.main()

"""One operator-input Terrain/inference qualification; never acquire, retry or dispatch."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import uuid

from m1_native_bootstrap import Fixture, ROOT
from m1_native_live import command, durable_counts, selected_key

INGRESS_BYTES = 16384
EVIDENCE_KINDS = ("real_operator_dns", "synthetic_input_provider_qualification")
ERRORS = (RuntimeError, OSError, ValueError, KeyError, TypeError, IndexError,
          RecursionError, subprocess.SubprocessError)
ZERO_COUNTS = ("calls", "generation", "repair", "reserved_calls", "decisions", "outbox",
               "history", "closures", "reservation_records", "campaign_calls",
               "campaign_reservation_records")


class QualificationFailure(RuntimeError):
    """Only fixed local codes may cross the report boundary."""


def require(condition, code):
    if not condition:
        raise QualificationFailure(code)


def canonical_uuid(value):
    require(isinstance(value, str), "identity_invalid")
    parsed = uuid.UUID(value)
    require(parsed.int != 0 and str(parsed) == value, "identity_invalid")
    return value


def read_input(path):
    with path.open("rb") as source:
        raw = source.read(INGRESS_BYTES + 1)
    require(len(raw) <= INGRESS_BYTES, "input_bound")
    value = json.loads(raw)
    require(isinstance(value, dict), "input_shape")
    return raw, value


def unchanged(inputs):
    for path, original in inputs:
        require(read_input(path)[0] == original, "operator_input_changed")


def records(text):
    if isinstance(text, bytes):
        text = text.decode("utf-8", errors="replace")
    parsed, malformed = [], False
    for line in (text or "").splitlines():
        if not line.startswith("{"):
            continue
        try:
            require(len(line.encode("utf-8")) <= 32768, "receipt_bound")
            value = json.loads(line)
            require(isinstance(value, dict), "receipt_shape")
            parsed.append(value)
        except ERRORS:
            malformed = True
    return parsed, malformed


def snapshot(fixture, e, c, decision):
    for value in (e, c, decision):
        canonical_uuid(value)
    counts = durable_counts(fixture, decision)
    counts.update(json.loads(fixture.sql(
        "SELECT json_build_object("
        "'reservation_records',(SELECT count(*) FROM execution.inference_reservations"
        f" WHERE operation_id='{decision}'),"
        "'campaign_calls',(SELECT count(*) FROM execution.inference_calls"
        f" WHERE (e,c)=('{e}','{c}')),"
        "'campaign_reservation_records',(SELECT count(*) FROM execution.inference_reservations"
        f" WHERE (engagement_id,campaign_id)=('{e}','{c}')))")))
    require(all(type(v) is int and v >= 0 for v in counts.values()), "counts_invalid")
    return counts


def reserved_calls(fixture, e, c, decision):
    for value in (e, c, decision):
        canonical_uuid(value)
    rows = json.loads(fixture.sql(
        "SELECT coalesce(json_agg(json_build_object('generation',generation_call_id,'repair',repair_call_id)),'[]'::json)"
        f" FROM execution.inference_reservations WHERE (engagement_id,campaign_id,operation_id)=('{e}','{c}','{decision}')"
        " AND record_revision=1 AND kind='reserved_not_sent'"))
    require(isinstance(rows, list) and len(rows) == 1 and isinstance(rows[0], dict), "reservation_pair_invalid")
    pair = [canonical_uuid(rows[0].get("generation")), canonical_uuid(rows[0].get("repair"))]
    require(pair[0] != pair[1], "reservation_pair_invalid")
    return pair


def retained_packs(fixture, e, c, decision, reserved):
    for value in (e, c, decision, *(reserved if isinstance(reserved, list) else ())):
        canonical_uuid(value)
    require(isinstance(reserved, list) and len(reserved) == 2 and reserved[0] != reserved[1], "reservation_pair_invalid")
    rows = json.loads(fixture.sql(
        "SELECT coalesce(json_agg(json_build_object('call',call,'p',p)),'[]'::json)"
        f" FROM execution.inference_calls WHERE (e,c,d)=('{e}','{c}','{decision}')"))
    require(isinstance(rows, list) and 1 <= len(rows) <= 2, "original_pack_missing")
    calls = []
    for row in rows:
        require(isinstance(row, dict), "original_pack_invalid")
        calls.append(canonical_uuid(row.get("call")))
        require(isinstance(row.get("p"), str) and len(row["p"].encode()) <= 4096, "original_pack_invalid")
    require(len(set(calls)) == len(calls) and set(calls).issubset(reserved) and reserved[0] in calls, "actual_calls_invalid")
    original = rows[0]["p"]
    require(all(row["p"].encode() == original.encode() for row in rows), "generation_repair_pack_mismatch")
    return original, [call for call in reserved if call in calls]


def accepted_terrain(receipt, observation, e, c):
    require(receipt["kind"] == "m1_terrain_receipt_v1"
            and receipt["result"] == "accepted" and receipt["publication"] == "committed"
            and receipt["history"] == "completed" and receipt["dispatch_granted"] is False,
            "terrain_not_accepted")
    require((receipt["engagement_id"], receipt["campaign_id"]) == (e, c), "terrain_scope_mismatch")
    safe = {k: canonical_uuid(receipt[k]) for k in
            ("operation_id", "event_id", "observation_id", "material_id", "evaluation_id", "claim_id")}
    claim = receipt["current_claim"]
    require(claim == receipt["claim"], "terrain_claim_changed")
    require(receipt["source_kind"] == observation["source_kind"] == "operator_reviewed_dns",
            "terrain_source_mismatch")
    require(claim["claim_status"] == "observed" and claim["freshness"] == "fresh"
            and claim["routing_usable"] is True and claim["conflicted"] is False
            and claim["corroborated"] is False, "terrain_not_usable")
    for key in ("claim_id", "observation_id", "material_id", "evaluation_id"):
        require(claim[key] == safe[key], "terrain_lineage_mismatch")
    for key in ("asset_ref", "vantage_ref", "perceived_at"):
        require(claim[key] == observation[key], "terrain_input_mismatch")
    safe.update({k: claim[k] for k in ("asset_ref", "vantage_ref", "claim_revision", "perceived_at",
                                      "valid_until", "temporal_frontier", "evaluated_at")})
    safe.update(source_kind="operator_reviewed_dns", status="observed", freshness="fresh",
                uncertainty_seconds=observation.get("uncertainty_seconds", 0),
                ttl_seconds=receipt["evaluation"]["lowest_ttl_seconds"])
    return safe


def terrain_references(original, terrain, e, c, decision):
    pack = json.loads(original)
    require((pack["engagement_id"], pack["campaign_id"], pack["decision_id"]) == (e, c, decision),
            "pack_scope_mismatch")
    lineage = [terrain[k] for k in ("event_id", "operation_id", "observation_id",
                                   "material_id", "evaluation_id")]
    matched = []
    for ref in pack["references"]:
        if ref.get("owner") != "cyber_terrain":
            continue
        require(ref["entity"] == terrain["claim_id"] and ref["lineage"] == lineage,
                "pack_terrain_lineage_mismatch")
        require(ref["asset_ref"] == terrain["asset_ref"] and ref["vantage_ref"] == terrain["vantage_ref"]
                and ref["revision"] == terrain["claim_revision"], "pack_terrain_lineage_mismatch")
        require(ref["source"] == "operator_reviewed_dns" and ref["status"] == "observed"
                and ref["freshness"] == "fresh" and ref["tier"] == "tier1"
                and ref["integrity_hold"] is False and ref["conflicted"] is False
                and ref["corroborated"] is False, "pack_terrain_ineligible")
        expected = dict(perceived_at=terrain["perceived_at"], valid_until=terrain["valid_until"],
                        ttl_seconds=terrain["ttl_seconds"], time_uncertainty=terrain["uncertainty_seconds"],
                        temporal_frontier=terrain["temporal_frontier"])
        require(all(ref[k] == v for k, v in expected.items()), "pack_terrain_time_mismatch")
        require(re.fullmatch(r"terrain_[0-7]", ref["id"]), "pack_reference_invalid")
        matched.append(ref["id"])
    require(matched and any(set(matched).intersection(c.get("terrain_references", []))
                            for c in pack["candidates"]), "pack_terrain_absent")
    return matched


def verify_receipt(receipt, decision, event, original, reserved, recovery):
    require(receipt["kind"] == "m1_inference_receipt_v1"
            and receipt["dispatch_granted"] is False, "inference_receipt_invalid")
    require(receipt["decision"] == decision, "inference_identity_changed")
    received_event = canonical_uuid(receipt["event"])
    require(event is None or received_event == event, "inference_event_changed")
    require(isinstance(receipt.get("call_ids"), list) and len(receipt["call_ids"]) == 2
            and [canonical_uuid(x) for x in receipt["call_ids"]] == reserved,
            "inference_calls_changed")
    require(receipt["pack_state"] == "available" and receipt["pack_utf8"] == original,
            "historical_pack_mismatch")
    require(receipt["proposal_disposition"] == "validated_proposal"
            and receipt["publication"] == "committed" and receipt["history"] == "completed",
            "proposal_not_admitted")
    require(not recovery or receipt["recovery_category"] == "no_provider_replay", "recovery_invalid")
    return received_event


def await_operator_continue(stream=None):
    source = stream if stream is not None else sys.stdin
    line = source.readline(17)
    require(isinstance(line, str) and line != "", "operator_continue_eof")
    require(len(line.encode("utf-8")) <= 16 and line in ("continue\n", "continue\r\n"),
            "operator_continue_invalid")


def validate_static_inputs(options):
    reg_raw, registration = read_input(options.registration)
    req_raw, request = read_input(options.context_request)
    inputs = [(options.registration, reg_raw), (options.context_request, req_raw)]
    e, c = (canonical_uuid(registration[k]) for k in ("engagement_id", "campaign_id"))
    require((request["mission"]["engagement_id"], request["mission"]["campaign_id"]) == (e, c),
            "input_scope_mismatch")
    consent = registration["m1_inference_permission"]
    require("owner_qualified_summaries" in consent["field_classes"], "terrain_disclosure_missing")
    require(consent["version"] == "m1_inference_permission_v2"
            and consent["profile"] == "deepseek-responses-envelope32-v1",
            "inference_profile_ineligible")
    wall_ms = consent.get("operation_limits", {}).get("decision_wall_time_ms", 20000)
    require(type(wall_ms) is int and 1 <= wall_ms <= 20000, "inference_profile_ineligible")
    candidates = request.get("candidates")
    require(isinstance(candidates, list) and bool(candidates), "input_shape")
    assets = [canonical_uuid(x["asset_ref"]) for x in candidates
              if isinstance(x, dict) and "asset_ref" in x]
    require(len(assets) == len(candidates), "input_shape")
    return inputs, e, c, assets, wall_ms


def admit_observation(path, e, c, assets):
    require(path.is_file(), "observation_missing")
    raw, observation = read_input(path)
    require((observation.get("engagement_id"), observation.get("campaign_id")) == (e, c),
            "input_scope_mismatch")
    require(observation.get("asset_ref") in assets, "terrain_candidate_missing")
    return (path, raw), observation


def preflight(options, result):
    armed = getattr(options, "await_reviewed_observation", False)
    if armed:
        require(not options.observation.exists(), "observation_already_present")
    inputs, e, c, assets, wall_ms = validate_static_inputs(options)
    if not armed:
        obs_entry, observation = admit_observation(options.observation, e, c, assets)
        inputs.append(obs_entry)
    key = selected_key(options.env_file)
    fixture = Fixture("dwpg-r3", "live", database=options.database_name)
    result.update(engagement_id=e, campaign_id=c, fixture_database=fixture.database)
    unchanged(inputs)
    code, text = command(options.binary, fixture, ["prepare-operation", "--engagement", e, "--campaign", c])
    found = re.search(r"\boperation=([0-9a-f-]{36})\b", text)
    require(code == 0 and found is not None, "registration_prepare_failed")
    op = canonical_uuid(found[1])
    result["registration_operation_id"] = op
    code, text = command(options.binary, fixture, ["register", "--input", str(options.registration),
                                                  "--operation", op])
    require(code == 0 and "result=accepted" in text and "history=completed" in text, "registration_failed")
    unchanged(inputs)
    if armed:
        print(json.dumps({"status": "READY", "engagement_id": e, "campaign_id": c,
                          "fixture_database": fixture.database}), flush=True)
        await_operator_continue()
        unchanged(inputs)
        obs_entry, observation = admit_observation(options.observation, e, c, assets)
        inputs.append(obs_entry)
    code, text = command(options.binary, fixture, ["m1-terrain-record", "--input", str(options.observation),
                                                  "--operation", "new"])
    items, malformed = records(text)
    require(code == 0 and not malformed and len(items) == 1, "terrain_record_failed")
    terrain = accepted_terrain(items[0], observation, e, c)
    result["terrain"] = terrain
    hold = fixture.sql("SELECT EXISTS(SELECT 1 FROM terrain.claim_integrity_holds"
                       f" WHERE engagement_id='{e}' AND campaign_id='{c}')").strip()
    require(hold == "f", "terrain_integrity_hold")
    now = int(fixture.sql("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint").strip())
    remaining = terrain["valid_until"] - now
    terrain["remaining_lifetime_seconds"] = remaining
    require(terrain["perceived_at"] <= now < terrain["valid_until"], "terrain_expired_before_send")
    require(remaining * 1000 > wall_ms, "terrain_freshness_insufficient")
    unchanged(inputs)
    return fixture, key, inputs, e, c


def failure_code(exc, fallback):
    return exc.args[0] if isinstance(exc, QualificationFailure) else fallback


def run_historical_checks(options, fixture, inputs, e, c, decision, before, check_fn, result):
    failures = result["failures"]
    result["historical_checks"] = []
    all_verified = True
    for name in ("m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"):
        check = dict(command=name, verified=False, pack_equal=False)
        try:
            unchanged(inputs)
            code, output = command(options.binary, fixture, [
                name, "--input", str(options.context_request), "--operation", decision])
            receipts, bad = records(output)
            require(code == 0 and not bad and len(receipts) == 1, "historical_inspection_failed")
            check_fn(receipts[0], check)
            check["verified"] = True
        except ERRORS as exc:
            all_verified = False
            failures.append(failure_code(exc, "historical_inspection_failed"))
        result["historical_checks"].append(check)
        try:
            latest = snapshot(fixture, e, c, decision)
        except ERRORS as exc:
            all_verified = False
            result["counts"] = None
            failures.append(failure_code(exc, "counts_unavailable"))
            continue
        result["counts"] = latest
        try:
            require(before is not None, "initial_counts_unknown")
            for key in ("calls", "reserved_calls", "reservation_records",
                        "campaign_calls", "campaign_reservation_records"):
                require(latest[key] == before[key], "recovery_accounting_growth")
        except ERRORS as exc:
            all_verified = False
            failures.append(failure_code(exc, "counts_unavailable"))
    counts = result["counts"]
    if before is not None and counts is not None:
        result["extra_call_records"] = counts["campaign_calls"] - before["campaign_calls"]
        result["extra_reservation_records"] = (
            counts["campaign_reservation_records"] - before["campaign_reservation_records"])
    return all_verified


def reconcile_insufficient_floor(options, fixture, inputs, e, c, decision, initial, exit_code, result):
    failures = result["failures"]
    failures.append("context_bytes_insufficient")
    before = None
    try:
        before = snapshot(fixture, e, c, decision)
    except ERRORS as exc:
        failures.append(failure_code(exc, "counts_unavailable"))

    def check_absent(receipt, _check):
        require(receipt.get("kind") == "m1_inference_receipt_v1"
                and receipt.get("dispatch_granted") is False
                and receipt.get("decision") == decision
                and receipt.get("call_ids") == []
                and receipt.get("pack_state") == "not_bound"
                and receipt.get("pack_utf8") is None
                and receipt.get("publication") == "absent_not_proof_of_no_send"
                and receipt.get("recovery_category") == "no_provider_replay",
                "historical_inspection_failed")

    verified = run_historical_checks(options, fixture, inputs, e, c, decision, before, check_absent, result)
    counts = result["counts"]
    zero_effect = (before is not None and counts is not None and verified and not initial
                   and exit_code != 124
                   and all(before.get(k) == 0 and counts.get(k) == 0 for k in ZERO_COUNTS))
    if zero_effect:
        result["status"] = "LIVE_BLOCKED"
    elif "unexpected_inference_effect" not in failures:
        failures.append("unexpected_inference_effect")


def reconcile(options, fixture, inputs, e, c, text, exit_code, result):
    failures = result["failures"]
    items, malformed = records(text)
    if malformed:
        failures.append("inference_receipt_malformed")
    decision = None
    for item in items:
        try:
            candidate = canonical_uuid(item["decision"])
            require(decision is None or candidate == decision, "inference_identity_changed")
            decision = candidate
            result["decision"] = decision
            require(item.get("kind") in ("m1_inference_identity_v1", "m1_inference_receipt_v1")
                    and item.get("dispatch_granted") is False, "inference_receipt_invalid")
        except ERRORS as exc:
            failures.append(failure_code(exc, "inference_identity_invalid"))
    if decision is None:
        try:
            ids = json.loads(fixture.sql(
                "SELECT coalesce(json_agg(d),'[]'::json) FROM execution.native_input_bindings"
                f" WHERE e='{e}' AND c='{c}'"))
            require(len(ids) == 1, "inference_identity_unavailable")
            decision = canonical_uuid(ids[0])
            result["decision"] = decision
        except ERRORS:
            failures.append("inference_identity_unavailable")
            return
    initial = [r for r in items if r.get("kind") == "m1_inference_receipt_v1"]
    if isinstance(text, str) and "error=context_bytes_insufficient" in text.splitlines():
        reconcile_insufficient_floor(options, fixture, inputs, e, c, decision, initial, exit_code, result)
        return
    before, original, references, reserved, actual = None, None, [], [], []
    try:
        before = snapshot(fixture, e, c, decision)
        reserved = reserved_calls(fixture, e, c, decision)
        original, actual = retained_packs(fixture, e, c, decision, reserved)
        references = terrain_references(original, result["terrain"], e, c, decision)
        result.update(pack_sha256=hashlib.sha256(original.encode()).hexdigest(),
                      terrain_reference_ids=references, reserved_call_ids=reserved,
                      actual_call_ids=actual, generation_repair_pack_equal=True)
    except ERRORS as exc:
        failures.append(failure_code(exc, "original_state_unavailable"))
    state = dict(event=None, admitted=None)
    if exit_code != 0:
        failures.append("inference_exit")
    try:
        require(len(initial) == 1, "inference_receipt_absent")
        state["event"] = canonical_uuid(initial[0]["event"])
        result["event"] = state["event"]
        verify_receipt(initial[0], decision, state["event"], original, reserved, False)
        state["admitted"] = initial[0]["proposal"]
    except ERRORS as exc:
        failures.append(failure_code(exc, "inference_receipt_invalid"))

    def check_admitted(receipt, check):
        check["pack_equal"] = original is not None and receipt.get("pack_utf8") == original
        state["event"] = verify_receipt(receipt, decision, state["event"], original, reserved, True)
        result["event"] = state["event"]
        require(state["admitted"] is None or state["admitted"] == receipt["proposal"],
                "historical_proposal_changed")
        state["admitted"] = receipt["proposal"]

    run_historical_checks(options, fixture, inputs, e, c, decision, before, check_admitted, result)
    counts = result["counts"]
    admitted = state["admitted"]
    try:
        require(counts is not None, "counts_unavailable")
        require(counts["generation"] == 1 and counts["repair"] in (0, 1)
                and counts["calls"] == counts["generation"] + counts["repair"] == len(actual)
                and counts["campaign_calls"] == counts["calls"]
                and counts["reserved_calls"] == 2 and counts["reservation_records"] == 1
                and counts["campaign_reservation_records"] == 1,
                "call_accounting_invalid")
        require(all(counts[k] == 1 for k in ("decisions", "outbox", "history", "closures")),
                "durable_completion_absent")
        require(admitted is not None and references and original is not None, "terrain_proof_absent")
    except ERRORS as exc:
        failures.append(failure_code(exc, "qualification_incomplete"))
    if not failures:
        citations = set(admitted["references"] + admitted["next_intent"]["references"])
        selectors = admitted["next_intent"].get("view_request", {}).get("selectors", [])
        citations.update(selectors)
        result.update(status="LIVE_PASS", proposal=admitted,
                      proposal_cites_terrain=bool(citations.intersection(references)),
                      terrain_reasoning_assessment="operator_review_required")


def qualify(options, result):
    for name in ("binary", "registration", "context_request"):
        setattr(options, name, getattr(options, name).resolve(strict=True))
    options.observation = options.observation.resolve(
        strict=not getattr(options, "await_reviewed_observation", False))
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True,
                          text=True, check=True, timeout=10).stdout.strip()
    require(re.fullmatch(r"[0-9a-f]{40}", head), "tested_commit_unavailable")
    with options.binary.open("rb") as binary:
        result.update(tested_commit=head, binary_sha256=hashlib.file_digest(binary, "sha256").hexdigest())
    fixture, key, inputs, e, c = preflight(options, result)
    result["status"] = "LIVE_FAIL"
    try:
        code, text = command(options.binary, fixture, [
            "m1-infer", "--input", str(options.context_request), "--operation", "new"], key)
    except subprocess.TimeoutExpired as exc:
        code, text = 124, exc.stdout or ""
        result["failures"].append("inference_timeout")
    finally:
        key = None
    reconcile(options, fixture, inputs, e, c, text, code, result)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("binary", "report", "registration", "observation", "context-request"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--evidence-kind", choices=EVIDENCE_KINDS, required=True)
    parser.add_argument("--env-file", type=Path, default=ROOT / ".env")
    parser.add_argument("--database-name", default=None)
    parser.add_argument("--await-reviewed-observation", action="store_true")
    options = parser.parse_args(argv)
    path = options.report / "terrain_live_qualification.json"
    try:
        options.report.mkdir(parents=True, exist_ok=True)
        destination = path.open("x", encoding="utf-8")
    except OSError:
        print("terrain_live_report_unavailable", file=sys.stderr)
        return 1
    result = dict(status="LIVE_BLOCKED", evidence_kind=options.evidence_kind,
                  dispatch_granted=False, target_activity=False, automatic_retry=False,
                  counts=None, extra_call_records=None, extra_reservation_records=None, failures=[])
    with destination:
        try:
            qualify(options, result)
        except ERRORS as exc:
            result["failures"].append(failure_code(exc, "qualification_unavailable"))
        except (Exception, KeyboardInterrupt):
            result["status"] = "LIVE_BLOCKED" if result["status"] == "LIVE_BLOCKED" else "LIVE_FAIL"
            result.update(counts=None, extra_call_records=None, extra_reservation_records=None)
            result["failures"].append("qualification_interrupted")
        finally:
            destination.write(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ("status", "extra_call_records", "extra_reservation_records")}))
    return 0 if result["status"] == "LIVE_PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())

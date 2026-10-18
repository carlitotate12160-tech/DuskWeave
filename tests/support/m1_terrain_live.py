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


def snapshot(fixture, decision):
    canonical_uuid(decision)
    counts = durable_counts(fixture, decision)
    counts["reservation_records"] = int(fixture.sql(
        f"SELECT count(*) FROM execution.inference_reservations WHERE operation_id='{decision}'").strip())
    require(all(type(v) is int and v >= 0 for v in counts.values()), "counts_invalid")
    return counts


def retained_packs(fixture, e, c, decision):
    for value in (e, c, decision):
        canonical_uuid(value)
    rows = json.loads(fixture.sql(
        "SELECT coalesce(json_agg(json_build_object('call',call,'p',p)),'[]'::json)"
        f" FROM execution.inference_calls WHERE (e,c,d)=('{e}','{c}','{decision}')"))
    require(1 <= len(rows) <= 2, "original_pack_missing")
    for row in rows:
        canonical_uuid(row["call"])
        require(isinstance(row["p"], str) and len(row["p"].encode()) <= 4096, "original_pack_invalid")
    original = rows[0]["p"]
    require(all(row["p"].encode() == original.encode() for row in rows), "generation_repair_pack_mismatch")
    return original, sorted(row["call"] for row in rows)


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


def verify_receipt(receipt, decision, event, original, calls, recovery):
    require(receipt["kind"] == "m1_inference_receipt_v1"
            and receipt["dispatch_granted"] is False, "inference_receipt_invalid")
    require(receipt["decision"] == decision, "inference_identity_changed")
    received_event = canonical_uuid(receipt["event"])
    require(event is None or received_event == event, "inference_event_changed")
    require(sorted(receipt["call_ids"]) == calls, "inference_calls_changed")
    require(receipt["pack_state"] == "available" and receipt["pack_utf8"] == original,
            "historical_pack_mismatch")
    require(receipt["proposal_disposition"] == "validated_proposal"
            and receipt["publication"] == "committed" and receipt["history"] == "completed",
            "proposal_not_admitted")
    require(not recovery or receipt["recovery_category"] == "no_provider_replay", "recovery_invalid")
    return received_event


def preflight(options, result):
    inputs, values = [], []
    for path in (options.registration, options.observation, options.context_request):
        raw, value = read_input(path)
        inputs.append((path, raw))
        values.append(value)
    registration, observation, request = values
    e, c = (canonical_uuid(registration[k]) for k in ("engagement_id", "campaign_id"))
    for item in (observation, request["mission"]):
        require((item["engagement_id"], item["campaign_id"]) == (e, c), "input_scope_mismatch")
    require("owner_qualified_summaries" in registration["m1_inference_permission"]["field_classes"],
            "terrain_disclosure_missing")
    consent = registration["m1_inference_permission"]
    require(consent["version"] == "m1_inference_permission_v2"
            and consent["profile"] == "deepseek-responses-envelope32-v1", "inference_profile_ineligible")
    require(observation["asset_ref"] in [x["asset_ref"] for x in request["candidates"]],
            "terrain_candidate_missing")
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
    require(terrain["perceived_at"] <= now < terrain["valid_until"], "terrain_expired_before_send")
    unchanged(inputs)
    return fixture, key, inputs, e, c


def failure_code(exc, fallback):
    return exc.args[0] if isinstance(exc, QualificationFailure) else fallback


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
        # The fresh scoped fixture can have only this invocation's binding.
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
    before, original, references, calls = None, None, [], []
    try:
        before = snapshot(fixture, decision)
        original, calls = retained_packs(fixture, e, c, decision)
        references = terrain_references(original, result["terrain"], e, c, decision)
        result.update(pack_sha256=hashlib.sha256(original.encode()).hexdigest(),
                      terrain_reference_ids=references, call_ids=calls, generation_repair_pack_equal=True)
    except ERRORS as exc:
        failures.append(failure_code(exc, "original_state_unavailable"))
    event, admitted = None, None
    if exit_code != 0:
        failures.append("inference_exit")
    initial = [r for r in items if r.get("kind") == "m1_inference_receipt_v1"]
    try:
        require(len(initial) == 1, "inference_receipt_absent")
        # Preserve the allocated event even when the proposal/pack is unavailable.
        event = canonical_uuid(initial[0]["event"])
        result["event"] = event
        verify_receipt(initial[0], decision, event, original, calls, False)
        admitted = initial[0]["proposal"]
    except ERRORS as exc:
        failures.append(failure_code(exc, "inference_receipt_invalid"))
    result["historical_checks"] = []
    for name in ("m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"):
        check = dict(command=name, verified=False, pack_equal=False)
        try:
            unchanged(inputs)
            code, output = command(options.binary, fixture, [
                name, "--input", str(options.context_request), "--operation", decision])
            receipts, bad = records(output)
            require(code == 0 and not bad and len(receipts) == 1, "historical_inspection_failed")
            receipt = receipts[0]
            check["pack_equal"] = original is not None and receipt.get("pack_utf8") == original
            event = verify_receipt(receipt, decision, event, original, calls, True)
            result["event"] = event
            require(admitted is None or admitted == receipt["proposal"], "historical_proposal_changed")
            admitted = receipt["proposal"]
            check["verified"] = True
        except ERRORS as exc:
            failures.append(failure_code(exc, "historical_inspection_failed"))
        result["historical_checks"].append(check)
        try:
            latest = snapshot(fixture, decision)
        except ERRORS as exc:
            result["counts"] = None
            failures.append(failure_code(exc, "counts_unavailable"))
            continue
        result["counts"] = latest
        try:
            require(before is not None, "initial_counts_unknown")
            for key in ("calls", "reserved_calls", "reservation_records"):
                require(latest[key] == before[key], "recovery_accounting_growth")
        except ERRORS as exc:
            failures.append(failure_code(exc, "counts_unavailable"))
    counts = result["counts"]
    if before is not None and counts is not None:
        result["extra_call_records"] = counts["calls"] - before["calls"]
        result["extra_reservation_records"] = counts["reservation_records"] - before["reservation_records"]
    try:
        require(counts is not None, "counts_unavailable")
        require(counts["generation"] == 1 and counts["repair"] in (0, 1)
                and counts["calls"] == counts["generation"] + counts["repair"]
                and counts["reserved_calls"] == 2 and counts["reservation_records"] == 1,
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
    for name in ("binary", "registration", "observation", "context_request"):
        setattr(options, name, getattr(options, name).resolve(strict=True))
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True,
                          text=True, check=True, timeout=10).stdout.strip()
    require(re.fullmatch(r"[0-9a-f]{40}", head), "tested_commit_unavailable")
    with options.binary.open("rb") as binary:
        result.update(tested_commit=head, binary_sha256=hashlib.file_digest(binary, "sha256").hexdigest())
    fixture, key, inputs, e, c = preflight(options, result)
    # From this point, failures cannot be relabeled as a pre-send block.
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
    options = parser.parse_args(argv)
    options.report.mkdir(parents=True, exist_ok=True)
    path = options.report / "terrain_live_qualification.json"
    try:
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
        destination.write(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ("status", "extra_call_records", "extra_reservation_records")}))
    return 0 if result["status"] == "LIVE_PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())

"""One normal-binary native episode; no retry, endpoint override or raw response log."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import uuid
from m1_native_bootstrap import Fixture, ROOT


def selected_key(path):
    selected = None
    with path.open(encoding="utf-8-sig") as source:
        for line in source:
            name, separator, value = line.strip().removeprefix("export ").partition("=")
            if not separator or name.strip() != "DEEPSEEK_API_KEY":
                continue
            if selected is not None:
                raise RuntimeError("native_live_key_configuration_invalid")
            value = value.strip()
            if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
                value = value[1:-1]
            selected = value
    if not selected or any(ord(c) < 32 or ord(c) == 127 for c in selected):
        raise RuntimeError("native_live_key_configuration_invalid")
    return selected


def command(binary, fixture, args, key=None):
    env = dict(fixture.env)
    env.pop("DEEPSEEK_API_KEY", None)
    if key is not None:
        env["DEEPSEEK_API_KEY"] = key
    result = subprocess.run([str(binary), *args], cwd=ROOT, env=env,
                            capture_output=True, text=True, timeout=40)
    return result.returncode, result.stdout


def receipts(text):
    """Parse receipt lines incrementally; (validated items, saw malformed line)."""
    allowed = {"kind", "decision", "event", "call_ids", "proposal_disposition",
               "publication", "history", "consumption", "recovery_category", "dispatch_granted"}
    items = []
    malformed = False
    for line in text.splitlines():
        if not line.startswith("{"):
            continue
        try:
            item = json.loads(line)
        except ValueError:
            malformed = True
            continue
        items.append({k: v for k, v in item.items() if k in allowed})
    return items, malformed


def durable_counts(fixture, decision):
    """Safe durable state: actual call records versus reserved call capacity."""
    return json.loads(fixture.sql(
        "SELECT json_build_object("
        f"'calls',(SELECT count(*) FROM execution.inference_calls WHERE d='{decision}'),"
        f"'generation',(SELECT count(*) FROM execution.inference_calls i WHERE i.d='{decision}' AND i.call IN"
        f" (SELECT generation_call_id FROM execution.inference_reservations WHERE operation_id='{decision}' AND record_revision=1)),"
        f"'repair',(SELECT count(*) FROM execution.inference_calls i WHERE i.d='{decision}' AND i.call IN"
        f" (SELECT repair_call_id FROM execution.inference_reservations WHERE operation_id='{decision}' AND record_revision=1)),"
        f"'reserved_calls',(SELECT coalesce(max((capacity->>'calls')::int),0) FROM execution.inference_reservations"
        f" WHERE operation_id='{decision}' AND record_revision=1),"
        f"'decisions',(SELECT count(*) FROM cognition.reasoning_decisions WHERE d='{decision}'),"
        f"'outbox',(SELECT count(*) FROM cognition.reasoning_outbox o JOIN cognition.reasoning_decisions q USING(event) WHERE q.d='{decision}'),"
        f"'history',(SELECT count(*) FROM trajectory.reasoning_history WHERE operation_id='{decision}' AND status='accepted'),"
        f"'closures',(SELECT count(*) FROM execution.inference_closures WHERE d='{decision}'))"))


def run(binary, report, env_file, database_name):
    # The provider key is validated before any provisioning; it is passed only
    # to the inference child process and is never printed, logged or persisted.
    key = selected_key(env_file)
    fixture = Fixture("dwpg-r3", "live", database=database_name)
    e, c, operator, authority, goal, asset, permission, terms = [str(uuid.uuid4()) for _ in range(8)]
    now = int(fixture.sql("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint").strip())
    limits = dict(context_bytes=4096, candidates=8, request_bytes=16384, configuration_bytes=16384,
                  response_bytes=65536, calls_per_decision=2, output_tokens_per_call=4096,
                  decision_wall_time_ms=20000, connect_time_ms=5000, proposal_bytes=8192,
                  repair_bytes=2048, text_field_bytes=1024, repair_error_codes=8,
                  json_depth=32, collection_entries=64, concurrency=1)
    consent = dict(version="m1_inference_permission_v2", permission_id=permission,
                   engagement_id=e, campaign_id=c, operator_ref=operator, authority_ref=authority,
                   authority_revision=1, mission_revision=1, goal_ref=goal, exercise_mode="blind",
                   recipient="https://api.deepseek.com/responses", profile="deepseek-responses-envelope32-v1",
                   field_classes=["mission_question_constraints", "scoped_opaque_references", "omissions_unknown",
                                  "capacity", "admitted_operator_premises", "admitted_repair_fields_codes"],
                   starts_at=now-30, ends_at=now+1800, provider_terms_ref=terms, provider_terms_approved=True,
                   campaign_limits=dict(calls=2, request_bytes=32768, response_bytes=131072,
                                        output_token_capacity=8192, wall_time_ms=20000), operation_limits=limits)
    registration = dict(engagement_id=e, campaign_id=c, operator_ref=operator, authority_ref=authority,
                        authority_revision=1, goal_ref=goal, included_assets=[asset], excluded_assets=[],
                        exercise_mode="blind", starts_at=now-60, ends_at=now+3600, m1_inference_permission=consent)
    registration_path = report / "live-registration.json"
    registration_path.write_text(json.dumps(registration), encoding="utf-8")
    status, text = command(binary, fixture, ["prepare-operation", "--engagement", e, "--campaign", c])
    found = re.search(r"operation=([0-9a-f-]{36})", text)
    if status or not found:
        raise RuntimeError("native_live_registration_failed")
    operation = str(uuid.UUID(found[1]))
    status, text = command(binary, fixture, ["register", "--input", str(registration_path), "--operation", operation])
    if status or "result=accepted" not in text or "history=completed" not in text:
        raise RuntimeError("native_live_registration_failed")
    request = dict(version="m1_context_prepare_request_v1", mission=dict(
        version="m1_inference_reservation_request_v1", engagement_id=e, campaign_id=c,
        operator_ref=operator, expected_mission_revision=1, permission_id=permission,
        goal_ref=goal, exercise_mode="blind"), candidates=[dict(asset_ref=asset, priority="high")])
    path = report / "live-request.json"
    path.write_text(json.dumps(request), encoding="utf-8")
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    category = None
    # This is the sole invocation allowed to construct/call the production provider.
    try:
        status, text = command(binary, fixture, ["m1-infer", "--input", str(path), "--operation", "new"], key)
    except subprocess.TimeoutExpired as exc:
        # The provider-bound process started: keep complete receipt lines from
        # its partial output (captured output may be bytes despite text mode)
        # and let the guarded section record LIVE_FAIL on the original identity.
        out = exc.stdout if isinstance(exc.stdout, str) else (exc.stdout or b"").decode("utf-8", errors="replace")
        status, text = 124, out[: out.rfind("\n") + 1]
        category = "native_live_inference_timeout"
    key = None
    initial = []
    recovered = []
    counts = None
    passed = False
    decision = None
    before = None
    latest = None
    # Inference has started: any failure here keeps the original decision
    # identity and durable state; it is never a preflight block and is never
    # retried automatically.
    try:
        initial, malformed = receipts(text)
        if malformed:
            category = category or "native_live_receipt_malformed"
        found = next((r["decision"] for r in initial if "decision" in r), None)
        try:
            decision = str(uuid.UUID(found)) if isinstance(found, str) else None
        except ValueError:
            decision = None
        if decision is None:
            raise RuntimeError("native_live_identity_unavailable")
        if status != 0:
            category = category or "native_live_inference_exit"
        event = initial[-1].get("event")
        if status == 0 and event is None and category is None:
            raise RuntimeError("native_live_event_absent")
        before = int(fixture.sql(f"SELECT count(*) FROM execution.inference_calls WHERE d='{decision}'").strip())
        for name in ("m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"):
            code, safe = command(binary, fixture, [name, "--input", str(path), "--operation", decision])
            records, malformed = receipts(safe)
            if malformed:
                category = category or "native_live_receipt_malformed"
            if code or not records or records[-1].get("recovery_category") != "no_provider_replay":
                raise RuntimeError("native_live_recovery_failed")
            if records[-1].get("decision") != decision:
                raise RuntimeError("native_live_identity_changed")
            if event is None:
                event = records[-1].get("event")
            elif records[-1].get("event") != event:
                raise RuntimeError("native_live_identity_changed")
            recovered.append(records[-1])
            latest = int(fixture.sql(f"SELECT count(*) FROM execution.inference_calls WHERE d='{decision}'").strip())
            if latest != before:
                raise RuntimeError("native_live_replay_detected")
        counts = durable_counts(fixture, decision)
        final = initial[-1]
        passed = status == 0 and category is None \
            and final.get("proposal_disposition") == "validated_proposal" \
            and final.get("publication") == "committed" and final.get("history") == "completed"
        passed = passed and counts["generation"] == 1 and counts["repair"] <= 1 \
            and counts["calls"] == counts["generation"] + counts["repair"]
        passed = passed and all(counts[k] == 1 for k in ("decisions", "outbox", "history", "closures"))
        passed = passed and all(r.get("dispatch_granted") is False for r in [final, *recovered])
    except (RuntimeError, subprocess.SubprocessError, OSError, ValueError, KeyError) as exc:
        passed = False
        detail = str(exc.args[0]) if isinstance(exc, RuntimeError) and exc.args else type(exc).__name__
        category = category or detail
        if counts is None and decision is not None:
            try:
                counts = durable_counts(fixture, decision)
            except (RuntimeError, subprocess.SubprocessError, OSError, ValueError, KeyError):
                counts = None
    # Finalize only after reconciling the last available durable snapshot:
    # zero means measured zero, never a stale measurement or a default.
    latest = counts["calls"] if counts is not None else None
    if not passed and category is None and initial:
        category = initial[-1].get("proposal_disposition")
    extra = latest - before if before is not None and latest is not None else None
    if passed and extra != 0:
        category = "native_live_replay_detected"
        passed = False
    result = dict(status="LIVE_PASS" if passed else "LIVE_FAIL", head=head,
                  binary_sha256=binary_sha, decision=decision,
                  counts=counts, initial=initial, fresh_inspect_and_two_recoveries=recovered,
                  extra_call_records=extra, target_activity=False, automatic_retry=False)
    if category is not None:
        result["failure"] = category
    (report / "live_qualification.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps({"status": result["status"], "calls": (counts or {}).get("calls"), "extra_calls": extra}))
    return 0 if passed else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--env-file", type=Path, default=ROOT / ".env")
    parser.add_argument("--database-name", default=None)
    options = parser.parse_args()
    options.report.mkdir(parents=True, exist_ok=True)
    if (options.report / "live_qualification.json").exists():
        raise SystemExit("native_live_report_collision")
    try:
        raise SystemExit(run(options.binary.resolve(strict=True), options.report,
                             options.env_file, options.database_name))
    except (RuntimeError, subprocess.SubprocessError, OSError, ValueError, KeyError):
        (options.report / "live_qualification.json").write_text('{"status":"LIVE_BLOCKED","automatic_retry":false}', encoding="utf-8")
        print("native_live_qualification_failed", file=sys.stderr)
        raise SystemExit(1)

#![allow(dead_code)]
use crate::{fixture, native, run_support};
use serde_json::{Value, json};
use std::process::Command;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub const PROFILE: &str = "fixture-genai-v4-pro-low-v2";
pub const RUN_PROFILE: &str = "m1_https_loop_v1";

pub fn configure_permission(raw: &mut Value) {
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!(RUN_PROFILE);
    p["profile"] = json!(PROFILE);
    p["recipient"] = json!("http://127.0.0.1:18083/responses");
    p["run_profile"] = json!(RUN_PROFILE);
    p["execution_mode"] = json!("controlled_fixture");
    p["field_classes"] = json!([
        "mission_question_constraints",
        "scoped_opaque_references",
        "owner_qualified_summaries",
        "admitted_operator_premises",
        "manifest_availability",
        "omissions_unknown",
        "capacity",
        "admitted_repair_fields_codes",
        "admitted_campaign_decision_feedback"
    ]);
    p["campaign_limits"] = json!({
        "calls": 8,
        "request_bytes": 131072,
        "response_bytes": 524288,
        "output_token_capacity": 65536,
        "wall_time_ms": 996000
    });
    for (name, n) in [
        ("json_depth", 32),
        ("collection_entries", 64),
        ("context_bytes", 8192),
        ("configuration_bytes", 16384),
        ("request_bytes", 16384),
        ("response_bytes", 65536),
        ("proposal_bytes", 8192),
        ("text_field_bytes", 1024),
        ("repair_bytes", 2048),
        ("repair_error_codes", 8),
        ("calls_per_decision", 2),
        ("output_tokens_per_call", 8192),
        ("concurrency", 1),
        ("connect_time_ms", 5000),
        ("soft_completion_ms", 120000),
        ("per_call_wall_time_ms", 122000),
        ("shared_wall_time_ms", 244000),
        ("decision_wall_time_ms", 249000),
        ("phase_wall_time_ms", 254000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        p["operation_limits"][name] = json!(n);
    }
    raw["m1_permission"]["campaign_limits"] = json!({
        "episodes": 3,
        "provider_calls": 8,
        "dns_questions": 3,
        "dns_followups": 3,
        "tcp_connections": 2,
        "head_requests": 2
    });
}

pub fn registered_with(slot: u128, change: impl FnOnce(&mut Value)) -> Value {
    let mut q = run_support::registered_with(slot, |raw| {
        configure_permission(raw);
        change(raw);
    });
    let now_s: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    q["version"] = json!(RUN_PROFILE);
    q["run_profile"] = json!(RUN_PROFILE);
    q["execution_mode"] = json!("controlled_fixture");
    q["inference_profile"] = json!(PROFILE);
    q["deadline_at_ms"] = json!(now_s * 1000 + 635_000);
    q
}

pub fn command() -> Command {
    let mut c = run_support::command();
    c.env("DW_M1_RUN_CONTROL_ROOT", run_support::control_root());
    c
}

pub fn run_cli(q: &Value) -> (bool, Value) {
    let path = run_support::path(q);
    let mut c = command();
    let out = c
        .args([
            "m1-run",
            "--input",
            path.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-pro-v2",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    parse_receipt(&out)
}

pub fn cli(args: &[&str]) -> (bool, Value) {
    let mut c = command();
    let out = c.args(args).output().unwrap();
    parse_receipt(&out)
}

fn parse_receipt(out: &std::process::Output) -> (bool, Value) {
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    let receipt = text
        .lines()
        .find(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .unwrap_or_else(|| json!({"stdout": text.trim(), "stderr": err.trim()}));
    (out.status.success(), receipt)
}

pub fn pack_in(value: &Value) -> Option<Value> {
    match value {
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .filter(|p| p["version"] == RUN_PROFILE),
        Value::Array(a) => a.iter().find_map(pack_in),
        Value::Object(o) => o.values().find_map(pack_in),
        _ => None,
    }
}

pub fn envelope(proposal: &Value) -> Value {
    json!({
        "id": "synthetic-response",
        "model": "deepseek-v4-pro",
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": proposal.to_string()}]
        }],
        "usage": {
            "input_tokens": 120,
            "output_tokens": 90,
            "output_tokens_details": {"reasoning_tokens": 20}
        }
    })
}

pub fn proposal_json(
    hypothesis: &str,
    refs: Value,
    category: &str,
    intent_refs: Value,
    manifest: Option<&str>,
    refresh: &str,
) -> Value {
    let mut next_intent = json!({
        "category": category,
        "references": intent_refs,
        "manifest_reference": manifest,
        "dependency_refresh": refresh,
    });
    if category == "observe_more" {
        next_intent["view_request"] = Value::Null;
    }
    json!({
        "version": RUN_PROFILE,
        "hypothesis": hypothesis,
        "considered_alternative": "Alternative endpoint or stopping remains viable.",
        "references": refs,
        "expected_observation": "Qualified DNS or single-endpoint HEAD outcome.",
        "falsifying_observation": "Blocked authority, stale premise, or conflicting outcome.",
        "uncertainty": "Fixture simulation only; whole-host reachability is unproven.",
        "rationale": "Select the bounded next step from issued references only.",
        "next_intent": next_intent
    })
}

pub fn principal_witness_proposal(pack: &Value) -> Value {
    let refs = pack["references"].as_array().unwrap();
    let https_count = refs
        .iter()
        .filter(|r| r["id"].as_str().is_some_and(|id| id.starts_with("https_")))
        .count();
    let has_ep1 = refs.iter().any(|r| r["id"] == "endpoint_1");
    let feedback = pack["predecessor"]["availability"] == "committed";
    if !feedback && https_count == 0 && !refs.iter().any(|r| r["id"] == "endpoint_0") {
        return proposal_json(
            "D1: resolve admitted hostname premise via dns_udp.",
            json!(["asset_0"]),
            "observe_more",
            json!(["mission", "asset_0"]),
            Some("dns_udp"),
            "none",
        );
    }
    if https_count == 0 {
        return proposal_json(
            "D2: validate first issued endpoint_0 over https-head-v1.",
            json!(["asset_0", "endpoint_0"]),
            "validate",
            json!(["asset_0", "endpoint_0"]),
            Some("https-head-v1"),
            "dns_once_if_stale",
        );
    }
    if https_count == 1 && has_ep1 {
        return proposal_json(
            "D3: endpoint_0 failed connection; pivot explicitly to endpoint_1.",
            json!(["asset_0", "previous_decision", "https_0", "endpoint_1"]),
            "validate",
            json!(["asset_0", "endpoint_1"]),
            Some("https-head-v1"),
            "dns_once_if_stale",
        );
    }
    proposal_json(
        "D4: endpoint_1 returned 200; record further bounded follow-up proposal without dispatch.",
        json!(["asset_0", "previous_decision", "https_1", "endpoint_1"]),
        "validate",
        json!(["asset_0", "endpoint_1"]),
        Some("https-head-v1"),
        "none",
    )
}

pub fn start_server(
    choose: impl FnMut(&Value) -> Value + Send + 'static,
) -> (fixture::Server, Arc<Mutex<Vec<Value>>>) {
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let captured = bodies.clone();
    let mut choose = choose;
    let server = fixture::Server::start("127.0.0.1:18083", move |inc| {
        let req: Value = serde_json::from_slice(&inc.body).unwrap();
        assert_eq!(req["text"]["format"]["name"], RUN_PROFILE);
        let pack = pack_in(&req).expect("HTTPS-loop ContextPack in SDK request");
        let reply = envelope(&choose(&pack));
        captured.lock().unwrap().push(req);
        fixture::Reply::Length(200, serde_json::to_vec(&reply).unwrap())
    });
    (server, bodies)
}

pub fn seed_dns_claim(slot_seed: u128, q: &Value, answers: Vec<(&str, u32)>) {
    seed_dns_claim_age(slot_seed, q, answers, 5);
}

pub fn seed_dns_claim_age(slot_seed: u128, q: &Value, answers: Vec<(&str, u32)>, age_s: i64) {
    use duskweave::m1_terrain_issued::IssuedDnsReference;
    use duskweave::mission::{CampaignId, EngagementId};
    use duskweave::postgres_m1_terrain::PgTerrainStore;
    use duskweave::postgres_mission::PgMissionStore;
    use duskweave::postgres_terrain_history::record_terrain_history;
    use duskweave::trajectory::HistoryStatus;

    let _ = slot_seed;
    let tr = native::client("DW_TERRAIN_DATABASE_URL");
    let mut ms = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut admin = native::admin();
    let m = &q["context"]["mission"];
    let e = Uuid::parse_str(m["engagement_id"].as_str().unwrap()).unwrap();
    let c = Uuid::parse_str(m["campaign_id"].as_str().unwrap()).unwrap();
    let row = admin
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint, \
             contract#>'{fields,m1_permission}' FROM mission.registration_outbox \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .unwrap();
    let now_s: i64 = row.get(0);
    let pol: Value = row.get(1);
    let seed = &q["seed_names"][0];
    let host = seed["name"].as_str().unwrap();
    let asset = Uuid::parse_str(seed["asset_ref"].as_str().unwrap()).unwrap();
    let vantage = Uuid::parse_str(pol["vantage_ref"].as_str().unwrap()).unwrap();
    let resolver = pol["resolver_ipv4"].as_str().unwrap();
    let cur_rev: i32 = admin
        .query_one(
            "SELECT coalesce(max(claim_revision), 0)::int FROM terrain.current_claims \
             WHERE engagement_id=$1 AND campaign_id=$2 AND subject_name=$3 \
               AND vantage_ref=$4 AND resolver_ipv4=$5",
            &[&e, &c, &host, &vantage, &resolver],
        )
        .unwrap()
        .get(0);
    let broker_oid: u32 = broker
        .query_one(
            "SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user",
            &[],
        )
        .unwrap()
        .get(0);
    let prior_run_id = Uuid::new_v4();
    let dec1 = Uuid::new_v4();
    let dec2 = Uuid::new_v4();
    let att_id = Uuid::new_v4();
    let res_id = Uuid::new_v4();
    let op_id = Uuid::new_v4();
    let rel = "a".repeat(64);
    let mut prior_req = q.clone();
    prior_req["run_id"] = json!(prior_run_id);
    let perceived_ms = (now_s - age_s) * 1000;
    let dns_json = json!({
        "version": "m1_terrain_dns_issued_observation_v1",
        "engagement_id": m["engagement_id"],
        "campaign_id": m["campaign_id"],
        "operator_ref": m["operator_ref"],
        "expected_mission_revision": 1,
        "goal_ref": m["goal_ref"],
        "exercise_mode": m["exercise_mode"],
        "correlation_id": res_id,
        "source_kind": "fixture_dns_acquisition",
        "vantage_ref": pol["vantage_ref"],
        "resolver_ipv4": pol["resolver_ipv4"],
        "subject_name": host,
        "asset_ref": seed["asset_ref"],
        "expected_claim_revision": cur_rev,
        "perceived_at": now_s - age_s,
        "uncertainty_seconds": 0,
        "transport": "udp",
        "qtype": "A",
        "qclass": "IN",
        "rcode": "noerror",
        "truncated": false,
        "completeness": "complete",
        "limitations": ["single_vantage_only"],
        "answers": answers.into_iter().map(|(ip, ttl)| json!({"ipv4": ip, "ttl_seconds": ttl})).collect::<Vec<_>>()
    });
    admin
        .execute(
            "INSERT INTO execution.autonomous_runs \
             (run_id, e, c, writer_oid, request, decision_one, decision_two, attempt, result, \
              terrain_operation, release_sha256, qualification_ref, deadline_at_ms) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
            &[
                &prior_run_id,
                &e,
                &c,
                &broker_oid,
                &prior_req,
                &dec1,
                &dec2,
                &att_id,
                &res_id,
                &op_id,
                &rel,
                &Uuid::new_v4(),
                &((now_s + 300) * 1000),
            ],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO execution.autonomous_control (run_id, lease_until_ms, status, phase, claimed) \
             VALUES ($1,0,'completed','decision_four',false)",
            &[&prior_run_id],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO execution.https_loop_steps \
             (run_id, e, c, step_index, decision_id, decision_phase, acquisition_phase, status, \
              protection_state, next_intent_kind, resolved_asset_ref, resolved_hostname, episode_index, \
              dns_attempt_id, dns_result_id, terrain_operation_id, updated_at_ms) \
             VALUES ($1,$2,$3,1,$4,'decision_one','acquisition_one','completed','none','dns_udp',$5,$6,1,$7,$8,$9,$10)",
            &[
                &prior_run_id, &e, &c, &dec1, &asset, &host, &att_id, &res_id, &op_id,
                &(now_s * 1000),
            ],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO execution.fixture_dns_attempts \
             (attempt, run_id, decision, seed_index, profile, mode, release_sha256, session_generation, \
              reserved_at_ms, episode_count, dns_questions, run_kind, step_index, attempt_kind) \
             VALUES ($1,$2,$3,0,'dns-routing-fixture-v1','controlled_fixture',$4,1,$5,1,1,'m1_https_loop_v1',1,'initial')",
            &[&att_id, &prior_run_id, &dec1, &rel, &perceived_ms],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO execution.fixture_dns_results \
             (result, run_id, attempt, profile, mode, release_sha256, received_at_ms, publication_obligation, \
              request, run_kind, step_index, attempt_kind) \
             VALUES ($1,$2,$3,'dns-routing-fixture-v1','controlled_fixture',$4,$5,'terrain.issued_dns.v1',$6,'m1_https_loop_v1',1,'initial')",
            &[&res_id, &prior_run_id, &att_id, &rel, &perceived_ms, &dns_json],
        )
        .unwrap();
    let reference =
        IssuedDnsReference::controlled_fixture(EngagementId(e), CampaignId(c), res_id, RUN_PROFILE)
            .unwrap();
    let mut store = PgTerrainStore::new(tr);
    let (event, _, _, _) = store.record_issued(&mut ms, &reference).unwrap().unwrap();
    let mut tj = native::client("DW_TRAJECTORY_DATABASE_URL");
    assert_eq!(
        record_terrain_history(&mut tj, &event).unwrap(),
        HistoryStatus::Completed
    );
}

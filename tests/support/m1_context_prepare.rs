//! Shared fixtures and helpers for m1-context-prepare test suites.
#![allow(dead_code)]

use duskweave::Res;
use duskweave::m1_context::{ContextBasis, ContextReader};
use duskweave::m1_context_capacity::ContextCapacity;
use duskweave::m1_context_input::ContextRequest;
use duskweave::m1_inference_budget::ReservedCapacity;
use duskweave::mission::{CONTRACT_KIND, MissionRegistered, OperationId, PRODUCER};
use serde_json::{Value, json};
use std::process::Output;
use std::time::Duration;
use uuid::Uuid;

pub struct MockReader {
    pub basis: Res<ContextBasis>,
}

impl ContextReader for MockReader {
    fn read(&mut self, _q: &ContextRequest, _op: OperationId) -> Res<ContextBasis> {
        match &self.basis {
            Ok(b) => Ok(clone_basis(b)),
            Err(e) => Err(*e),
        }
    }
}

pub fn candidate(asset: Uuid, priority: &str) -> Value {
    json!({
        "asset_ref": asset,
        "priority": priority
    })
}

pub fn request(mission: &Value, candidates: &[Value]) -> Value {
    json!({
        "version": "m1_context_prepare_request_v1",
        "mission": mission,
        "candidates": candidates
    })
}

pub fn run(args: &[String], dsn: &str) -> Output {
    crate::wait_db::run_bounded(&mut crate::wait_db::cli(args, dsn), Duration::from_secs(15)).0
}

pub fn receipt(out: &Output) -> Value {
    let line = out
        .stdout
        .split(|b| *b == b'\n')
        .find(|line| !line.is_empty())
        .expect("receipt line present");
    serde_json::from_slice(line).expect("receipt is valid JSON")
}

pub fn assert_false_flags(r: &Value) {
    assert_eq!(r["kind"], "m1_context_preparation_receipt_v1");
    assert_eq!(r["current_permission"], false);
    assert_eq!(r["dispatch_granted"], false);
    assert_eq!(r["provider_qualified"], false);
    assert_eq!(r["acquisition_qualified"], false);
    assert_eq!(r["send_qualified"], false);
    assert_eq!(r["reservation_granted"], false);
    assert_eq!(r["decision_published"], false);
    assert_eq!(r["history_completed"], false);
    assert_eq!(r["request_binding_qualified"], false);
}

pub fn sample_basis(now: i64, e: Uuid, c: Uuid, with_premises: bool) -> (ContextBasis, Value) {
    let mut raw = crate::budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    raw["included_assets"] = json!([Uuid::from_u128(0x701), Uuid::from_u128(0x702)]);
    raw["excluded_assets"] = json!([Uuid::from_u128(0x703)]);
    if with_premises {
        let classes = raw["m1_inference_permission"]["field_classes"]
            .as_array_mut()
            .unwrap();
        classes.push(json!("admitted_operator_premises"));
    }
    let op = Uuid::from_u128(0xa002);
    let mission: MissionRegistered = serde_json::from_value(json!({
        "event_id": Uuid::from_u128(0xa001),
        "operation_id": op,
        "engagement_id": e,
        "campaign_id": c,
        "producer": PRODUCER,
        "affected_entity": c,
        "owner_revision": 1,
        "kind": CONTRACT_KIND,
        "version": 3,
        "causation_id": op,
        "correlation_id": op,
        "occurred_at": now,
        "recorded_at": now,
        "fields": raw
    }))
    .unwrap();
    let capacity = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 1,
        session_claimed: false,
        active_holds: 0,
        charged: ReservedCapacity {
            calls: 0,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    };
    (
        ContextBasis {
            mission,
            capacity,
            evaluated_at: now,
            terrain: duskweave::m1_context::TerrainContextState::NotLoaded,
        },
        raw,
    )
}

pub fn clone_basis(b: &ContextBasis) -> ContextBasis {
    ContextBasis {
        mission: b.mission.clone(),
        capacity: ContextCapacity {
            version: b.capacity.version.clone(),
            epoch: b.capacity.epoch,
            session_claimed: b.capacity.session_claimed,
            active_holds: b.capacity.active_holds,
            charged: b.capacity.charged.clone(),
        },
        evaluated_at: b.evaluated_at,
        terrain: b.terrain.clone(),
    }
}

pub fn build_basis(
    raw: &Value,
    e: Uuid,
    c: Uuid,
    now: i64,
    capacity: Option<ContextCapacity>,
) -> ContextBasis {
    let op = Uuid::from_u128(0xa002);
    let mission: MissionRegistered = serde_json::from_value(json!({
        "event_id": Uuid::from_u128(0xa001),
        "operation_id": op,
        "engagement_id": e,
        "campaign_id": c,
        "producer": PRODUCER,
        "affected_entity": c,
        "owner_revision": 1,
        "kind": CONTRACT_KIND,
        "version": 3,
        "causation_id": op,
        "correlation_id": op,
        "occurred_at": now,
        "recorded_at": now,
        "fields": raw
    }))
    .unwrap();
    let cap = capacity.unwrap_or_else(|| ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 1,
        session_claimed: false,
        active_holds: 0,
        charged: ReservedCapacity {
            calls: 0,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    });
    ContextBasis {
        mission,
        capacity: cap,
        evaluated_at: now,
        terrain: duskweave::m1_context::TerrainContextState::NotLoaded,
    }
}

pub fn scoped_state(e: Uuid, c: Uuid) -> Value {
    crate::db_support::runtime_client()
        .query_one(
            "SELECT jsonb_build_object(
                'mission', (SELECT to_jsonb(m) FROM mission.missions m WHERE engagement_id=$1 AND campaign_id=$2),
                'publication', (SELECT to_jsonb(o) FROM mission.registration_outbox o WHERE engagement_id=$1 AND campaign_id=$2),
                'registration_history', coalesce((SELECT jsonb_agg(to_jsonb(rh) ORDER BY rh.event_id) FROM trajectory.registration_history rh WHERE engagement_id=$1 AND campaign_id=$2), '[]'::jsonb),
                'withdrawals', (SELECT to_jsonb(w) FROM mission.withdrawals w WHERE engagement_id=$1 AND campaign_id=$2),
                'withdrawal_history', coalesce((SELECT jsonb_agg(to_jsonb(wh) ORDER BY wh.event_id) FROM trajectory.withdrawal_history wh WHERE engagement_id=$1 AND campaign_id=$2), '[]'::jsonb),
                'ledger', coalesce((SELECT jsonb_agg(to_jsonb(r) ORDER BY r.generation, r.record_revision) FROM execution.inference_reservations r WHERE engagement_id=$1 AND campaign_id=$2), '[]'::jsonb),
                'fence', (SELECT to_jsonb(f) FROM execution.session_fences f WHERE engagement_id=$1 AND campaign_id=$2),
                'session_history', coalesce((SELECT jsonb_agg(to_jsonb(sh) ORDER BY sh.operation_id, sh.kind) FROM execution.session_history sh WHERE engagement_id=$1 AND campaign_id=$2), '[]'::jsonb)
            )",
            &[&e, &c],
        )
        .unwrap()
        .get(0)
}

pub fn assert_capacity_remainder(p: &Value, expected: [i64; 5]) {
    assert_eq!(p["remaining_capacity"]["campaign"]["calls"], expected[0]);
    assert_eq!(
        p["remaining_capacity"]["campaign"]["request_bytes"],
        expected[1]
    );
    assert_eq!(
        p["remaining_capacity"]["campaign"]["response_bytes"],
        expected[2]
    );
    assert_eq!(
        p["remaining_capacity"]["campaign"]["output_token_capacity"],
        expected[3]
    );
    assert_eq!(
        p["remaining_capacity"]["campaign"]["wall_time_ms"],
        expected[4]
    );
}

pub fn context_request(
    mission: &duskweave::m1_inference_budget::ReservationRequest,
) -> ContextRequest {
    ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission.clone(),
        candidates: vec![],
    }
}

pub fn prepare_pack(q: &ContextRequest) -> Value {
    let mut reader =
        duskweave::postgres_mission::PgContextReader::new(crate::session_support::broker_client());
    let prepared = duskweave::m1_context::prepare(&mut reader, q, crate::budget::op()).unwrap();
    serde_json::from_str(prepared.utf8()).unwrap()
}

pub fn run_request(q: &ContextRequest, tag: &str) -> Output {
    let raw = request(
        &serde_json::to_value(&q.mission).unwrap(),
        &q.candidates
            .iter()
            .map(|c| serde_json::to_value(c).unwrap())
            .collect::<Vec<_>>(),
    );
    let input = crate::wait_db::InputFile::new(&raw, tag);
    run(
        &[
            "m1-context-prepare".into(),
            "--input".into(),
            input.path().into(),
        ],
        &crate::session_support::broker_dsn(crate::session_support::db_port()),
    )
}

pub fn assert_failed_receipt(out: &Output, outcome: &str) {
    assert!(!out.status.success());
    let r = receipt(out);
    assert_eq!(r["outcome"], outcome);
    assert!(r["pack_utf8"].is_null());
    let decision: Uuid = serde_json::from_value(r["decision_id"].clone()).unwrap();
    assert!(!decision.is_nil());
    assert_false_flags(&r);
}

pub fn assert_dimensions(value: &Value, expected: [i64; 5]) {
    for (key, n) in [
        "calls",
        "request_bytes",
        "response_bytes",
        "output_token_capacity",
        "wall_time_ms",
    ]
    .into_iter()
    .zip(expected)
    {
        assert_eq!(value[key], n, "{key}");
    }
}

pub fn assert_charge(value: &ReservedCapacity, expected: [i64; 5]) {
    assert_eq!(
        [
            value.calls,
            value.request_bytes,
            value.response_bytes,
            value.output_token_capacity,
            value.wall_time_ms
        ],
        expected,
    );
}

pub fn assert_mission_frontier(pack: &Value, source: &MissionRegistered) {
    assert_eq!(pack["owner_views"][0]["owner"], "mission");
    assert_eq!(pack["owner_views"][0]["revision"], source.owner_revision);
    assert_eq!(pack["owner_views"][0]["frontier"], json!(source.event_id));
    assert_eq!(
        pack["references"][0]["lineage"],
        json!([source.event_id, source.operation_id])
    );
}

pub fn smaller_limits_fixture(
    seed: u128,
) -> (
    duskweave::m1_inference_budget::ReservationRequest,
    duskweave::mission::MissionRegistered,
) {
    let (e, c) = crate::db_support::scope(seed);
    let now = crate::session_support::now();
    let mut raw = crate::budget::registration_value(e, c, now);
    let perm = raw["m1_inference_permission"].as_object_mut().unwrap();
    let limits = perm["operation_limits"].as_object_mut().unwrap();
    limits.insert("request_bytes".into(), json!(1024));
    limits.insert("response_bytes".into(), json!(2048));
    limits.insert("decision_wall_time_ms".into(), json!(5000));
    limits.insert("connect_time_ms".into(), json!(1000));
    limits.insert("output_tokens_per_call".into(), json!(4096));

    let (_, source) = crate::budget::register(e, c, &raw);
    let req = crate::budget::request(e, c, &raw);
    (req, source)
}

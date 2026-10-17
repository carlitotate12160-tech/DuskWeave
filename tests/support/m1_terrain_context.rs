//! Shared combined Mission + Terrain DNS + native inference fixture for M1 Terrain context tests.
#![allow(dead_code)]

#[path = "m1_terrain.rs"]
pub mod terrain_support;

use crate::db_support;
use duskweave::input::parse_register;
use duskweave::m1_context_input::{Candidate, ContextRequest, Priority};
use duskweave::m1_inference_budget::ReservationRequest;
use duskweave::m1_terrain::{TerrainAcceptedEvent, TerrainClaimRevision};
use duskweave::mission::{AssetRef, CampaignId, EngagementId, MissionRegistered, OperationId};
use duskweave::postgres_terrain_history::record_terrain_history;
use duskweave::registration::{self, MissionStore};
use duskweave::trajectory::HistoryStatus;
use serde_json::{Value, json};
use uuid::Uuid;

pub fn combined_registration_json(
    e: EngagementId,
    c: CampaignId,
    now: i64,
    with_summaries: bool,
) -> Value {
    let mut raw = terrain_support::policy_support::registration(e, c, now);
    let permission_id = Uuid::from_u128(e.0.as_u128().wrapping_add(0x500));
    let mut classes = vec![
        "mission_question_constraints",
        "scoped_opaque_references",
        "omissions_unknown",
        "capacity",
        "admitted_operator_premises",
        "admitted_repair_fields_codes",
    ];
    if with_summaries {
        classes.push("owner_qualified_summaries");
    }
    let limits = json!({
        "context_bytes": 4096,
        "candidates": 8,
        "request_bytes": 16384,
        "configuration_bytes": 16384,
        "response_bytes": 65536,
        "calls_per_decision": 2,
        "output_tokens_per_call": 4096,
        "decision_wall_time_ms": 20000,
        "connect_time_ms": 5000,
        "proposal_bytes": 8192,
        "repair_bytes": 2048,
        "text_field_bytes": 1024,
        "repair_error_codes": 8,
        "json_depth": 32,
        "collection_entries": 64,
        "concurrency": 1
    });
    raw["m1_inference_permission"] = json!({
        "version": "m1_inference_permission_v2",
        "permission_id": permission_id,
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "authority_ref": raw["authority_ref"],
        "authority_revision": raw["authority_revision"],
        "mission_revision": 1,
        "goal_ref": raw["goal_ref"],
        "exercise_mode": "blind",
        "recipient": "https://api.deepseek.com/responses",
        "profile": "deepseek-responses-envelope32-v1",
        "field_classes": classes,
        "starts_at": now - 1800,
        "ends_at": now + 1800,
        "provider_terms_ref": Uuid::from_u128(0x32),
        "provider_terms_approved": true,
        "campaign_limits": {
            "calls": 20,
            "request_bytes": 200000,
            "response_bytes": 900000,
            "output_token_capacity": 80000,
            "wall_time_ms": 200000
        },
        "operation_limits": limits
    });
    raw
}

pub fn register_combined_campaign(
    slot: u128,
    with_summaries: bool,
) -> (EngagementId, CampaignId, MissionRegistered, Value, i64) {
    let (e, c) = db_support::scope(slot);
    let now: i64 = terrain_support::admin_client()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let raw = combined_registration_json(e, c, now, with_summaries);
    let input = parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut alloc, mut store, mut traj) = db_support::ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let receipt = registration::register(&mut alloc, &mut store, &mut traj, op, &input).unwrap();
    let ev = store
        .outbox_event(e, c, receipt.operation_id)
        .unwrap()
        .unwrap();
    (e, c, ev, raw, now)
}

pub fn context_request_for(
    e: EngagementId,
    c: CampaignId,
    raw_reg: &Value,
    candidates: Vec<(Uuid, Priority)>,
) -> (Value, ContextRequest) {
    let perm_id = raw_reg["m1_inference_permission"]["permission_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap();
    let mission_req = ReservationRequest {
        version: "m1_inference_reservation_request_v1".into(),
        engagement_id: e,
        campaign_id: c,
        operator_ref: duskweave::mission::OperatorRef(Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
        permission_id: perm_id,
        goal_ref: duskweave::mission::GoalRef(Uuid::from_u128(0x13)),
        exercise_mode: duskweave::mission::ExerciseMode::Blind,
    };
    let typed = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: candidates
            .into_iter()
            .map(|(id, priority)| Candidate {
                asset_ref: AssetRef(id),
                priority,
            })
            .collect(),
    };
    let val = serde_json::to_value(&typed).unwrap();
    (val, typed)
}

pub fn record_dns_accepted(
    _e: EngagementId,
    _c: CampaignId,
    req_val: &Value,
) -> (OperationId, TerrainAcceptedEvent, TerrainClaimRevision, i64) {
    let op = terrain_support::allocate_op();
    let req = terrain_support::parse_req(req_val);
    let (mut m_store, mut t_store, _) = terrain_support::stores();
    let (ev, rev, eval_at, authority_active) = t_store
        .record_dns(&mut m_store, &req, op)
        .expect("record_dns")
        .expect("admitted");
    assert!(authority_active);
    let mut tj = terrain_support::trajectory_client();
    let status = record_terrain_history(&mut tj, &ev).expect("record_terrain_history");
    assert_eq!(status, HistoryStatus::Completed);
    (op, ev, rev, eval_at)
}

pub fn inject_integrity_hold(
    _e: EngagementId,
    _c: CampaignId,
    op: OperationId,
    conflicting_req_val: &Value,
) {
    let req = terrain_support::parse_req(conflicting_req_val);
    let (mut m_store, mut t_store, _) = terrain_support::stores();
    let res = t_store.record_dns(&mut m_store, &req, op);
    assert!(
        res.is_err(),
        "conflicting replay must be rejected with hold"
    );
}

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
            Ok(b) => Ok(ContextBasis {
                mission: b.mission.clone(),
                capacity: ContextCapacity {
                    version: b.capacity.version.clone(),
                    epoch: b.capacity.epoch,
                    session_claimed: b.capacity.session_claimed,
                    active_holds: b.capacity.active_holds,
                    charged: b.capacity.charged.clone(),
                },
                evaluated_at: b.evaluated_at,
            }),
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

//! Shared fixture for the pre-send inference reservation slice: a registered
//! scope carrying both the crt.sh permission and initial inference consent,
//! the closed reservation request, port-level invocation, durable-state
//! readers and CLI argument construction. Broker logins come from
//! session_support; cluster-role fixtures serialize under db_support::db().
//! Included as a module by the real test crates; not a standalone target.
#![allow(dead_code)]

use crate::{consent, db_support, session_support};
use duskweave::m1_inference_budget::{
    ReservationAction, ReservationRecord, ReservationRequest, reservation,
};
use duskweave::mission::*;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::registration::{self, MissionStore};
use serde_json::{Value, json};

/// Registration document with both attachments and current server-clock
/// windows: mission +-3600s, permission/consent +-3500s.
pub fn registration_value(e: EngagementId, c: CampaignId, now: i64) -> Value {
    let mut raw = consent::registration(e, c);
    raw["m1_permission"] = session_support::registration(e, c, now, 0)["m1_permission"].clone();
    raw["m1_permission"]["starts_at"] = json!(now - 3500);
    raw["m1_permission"]["ends_at"] = json!(now + 3500);
    raw["starts_at"] = json!(now - 3600);
    raw["ends_at"] = json!(now + 3600);
    raw["m1_inference_permission"]["starts_at"] = json!(now - 3500);
    raw["m1_inference_permission"]["ends_at"] = json!(now + 3500);
    raw
}

/// The closed request bound to a registration document's real identities.
pub fn request(e: EngagementId, c: CampaignId, raw: &Value) -> ReservationRequest {
    serde_json::from_value(json!({"version":"m1_inference_reservation_request_v1",
        "engagement_id":e,"campaign_id":c,"operator_ref":raw["operator_ref"],
        "expected_mission_revision":1,"permission_id":raw["m1_inference_permission"]["permission_id"],
        "goal_ref":raw["goal_ref"],"exercise_mode":"blind"}))
    .unwrap()
}

/// Persist a registration document and return its owner outbox event.
pub fn register(e: EngagementId, c: CampaignId, raw: &Value) -> (OperationId, MissionRegistered) {
    let input = duskweave::input::parse_register(&serde_json::to_vec(raw).unwrap()).unwrap();
    let (mut a, mut s, mut t) = db_support::ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut s, &mut t, op, &input).unwrap();
    (op, s.outbox_event(e, c, op).unwrap().unwrap())
}

/// A current-window consenting scope: request plus its Mission publication.
/// The privilege-probe login is created here too: guarded-function probes
/// connect as PRIV_BROKER and must not depend on residual cluster state.
pub fn fixture(n: u128) -> (ReservationRequest, MissionRegistered) {
    session_support::ensure_broker_logins();
    session_support::ensure_priv_login();
    let (e, c) = db_support::scope(n);
    let raw = registration_value(e, c, session_support::now());
    let (_, source) = register(e, c, &raw);
    (request(e, c, &raw), source)
}

/// Fresh operation identity through the same allocation path as other slices.
pub fn op() -> OperationId {
    OperationId(consent::id())
}

/// Broker port call through the typed use case and the Mission reader.
pub fn run(
    q: &ReservationRequest,
    op: OperationId,
    a: ReservationAction,
) -> duskweave::Res<Option<ReservationRecord>> {
    let mut s = PgInferenceReservationStore::new(session_support::broker_client());
    let mut r = PgMissionStore::new(db_support::runtime_client());
    reservation(&mut s, Some(&mut r), a, q, op)
}

/// Complete scoped protected state: the full ledger plus the shared fence.
pub fn state(q: &ReservationRequest) -> Value {
    db_support::runtime_client().query_one("SELECT jsonb_build_object('ledger',\
       coalesce((SELECT jsonb_agg(to_jsonb(r) ORDER BY generation,record_revision) FROM execution.inference_reservations r \
       WHERE engagement_id=$1 AND campaign_id=$2),'[]'::jsonb),'fence',\
       (SELECT to_jsonb(f) FROM execution.session_fences f WHERE engagement_id=$1 AND campaign_id=$2))",
       &[&q.engagement_id.0,&q.campaign_id.0]).unwrap().get(0)
}

/// Direct guarded-function call, bypassing the Rust layer: raw SQL boundary
/// probes must fail at the same door.
pub fn sql(
    client: &mut postgres::Client,
    action: &str,
    q: &Value,
    op: OperationId,
    source: &Value,
) -> Result<Option<Value>, postgres::Error> {
    client
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation($1,$2,$3,$4))",
            &[&action, &op.0, q, source],
        )
        .map(|r| r.get(0))
}

/// CLI argument vector for `duskweave m1-inference-budget`.
pub fn args(action: &str, op: OperationId, path: &str) -> Vec<String> {
    vec![
        "m1-inference-budget".into(),
        "--action".into(),
        action.into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        path.into(),
    ]
}

/// The bounded receipt is always the first stdout line; a following
/// `error=<category>` line may close failures.
pub fn receipt(out: &std::process::Output) -> Value {
    serde_json::from_slice(out.stdout.split(|b| *b == b'\n').next().unwrap()).unwrap()
}

/// Every receipt keeps all five qualification flags explicitly false.
pub fn assert_flags_off(v: &Value) {
    for flag in [
        "current_permission",
        "dispatch_granted",
        "provider_qualified",
        "send_qualified",
        "acquisition_qualified",
    ] {
        assert_eq!(v[flag], false, "{flag} must be explicit false: {v}");
    }
}

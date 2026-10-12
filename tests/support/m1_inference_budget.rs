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
use uuid::Uuid;

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

/// Complete scoped protected state: the full ledger, the shared fence, the
/// owner Mission/publication rows and session history — everything an
/// invocation could mutate, compared as one value.
pub fn state(q: &ReservationRequest) -> Value {
    db_support::runtime_client().query_one("SELECT jsonb_build_object('ledger',\
       coalesce((SELECT jsonb_agg(to_jsonb(r) ORDER BY generation,record_revision) FROM execution.inference_reservations r \
       WHERE engagement_id=$1 AND campaign_id=$2),'[]'::jsonb),'fence',\
       (SELECT to_jsonb(f) FROM execution.session_fences f WHERE engagement_id=$1 AND campaign_id=$2),\
       'mission',(SELECT to_jsonb(m) FROM mission.missions m WHERE engagement_id=$1 AND campaign_id=$2),\
       'publication',(SELECT to_jsonb(o) FROM mission.registration_outbox o WHERE engagement_id=$1 AND campaign_id=$2),\
       'session_history',coalesce((SELECT jsonb_agg(to_jsonb(h) ORDER BY h.operation_id,h.kind) \
       FROM execution.session_history h WHERE engagement_id=$1 AND campaign_id=$2),'[]'::jsonb))",
       &[&q.engagement_id.0,&q.campaign_id.0]).unwrap().get(0)
}

/// The fence value after exactly `more` epoch increments; every other field
/// — phase, generation, claim operation, writer — must be identical.
pub fn fence_after(fence: &Value, more: i64) -> Value {
    let mut want = fence.clone();
    want["epoch"] = json!(fence["epoch"].as_i64().unwrap() + more);
    want
}

/// Accepted operation-profile bounds: field name, inclusive floor and
/// ceiling. The last three admit exactly one value.
pub const LIMIT_BOUNDS: &[(&str, i64, i64)] = &[
    ("request_bytes", 1, 16384),
    ("response_bytes", 1, 65536),
    ("decision_wall_time_ms", 1, 20000),
    ("context_bytes", 1, 4096),
    ("candidates", 1, 8),
    ("configuration_bytes", 1, 16384),
    ("connect_time_ms", 1, 5000),
    ("proposal_bytes", 1, 8192),
    ("repair_bytes", 1, 2048),
    ("text_field_bytes", 1, 1024),
    ("repair_error_codes", 1, 8),
    ("json_depth", 1, 8),
    ("collection_entries", 1, 64),
    ("calls_per_decision", 2, 2),
    ("output_tokens_per_call", 4096, 4096),
    ("concurrency", 1, 1),
];

/// A complete operation-limits object at one bound; connect stays inside the
/// shared decision envelope at both bounds.
pub fn limits_at(high: bool) -> Value {
    Value::Object(
        LIMIT_BOUNDS
            .iter()
            .map(|&(n, lo, hi)| (n.to_string(), json!(if high { hi } else { lo })))
            .collect(),
    )
}

/// The exact capacity a reservation charges for the given operation limits:
/// two call slots, two request/response allowances, 8192 output tokens and
/// one shared decision wall-time envelope.
pub fn capacity_for(limits: &Value) -> Value {
    json!({"calls":2,
        "request_bytes":2*limits["request_bytes"].as_i64().unwrap(),
        "response_bytes":2*limits["response_bytes"].as_i64().unwrap(),
        "output_token_capacity":8192,
        "wall_time_ms":limits["decision_wall_time_ms"].as_i64().unwrap()})
}

/// Assert a ledger row carries the complete admitted identity for this
/// fixture: scope, request, operation, generation/revision/kind, the real
/// Broker writer OID, owner registration provenance, two distinct nonnil
/// call identities and the exact derived capacity.
pub fn assert_full_record(
    row: &Value,
    q: &ReservationRequest,
    op: OperationId,
    kind: &str,
    revision: i64,
    generation: i64,
    source: &MissionRegistered,
) {
    let (reg_op, reg_event) =
        session_support::registration_identity(q.engagement_id, q.campaign_id);
    let nil = Uuid::nil().to_string();
    assert_eq!(row["engagement_id"], json!(q.engagement_id.0));
    assert_eq!(row["campaign_id"], json!(q.campaign_id.0));
    assert_eq!(row["operation_id"], json!(op.0));
    assert_eq!(row["generation"], json!(generation));
    assert_eq!(row["record_revision"], json!(revision));
    assert_eq!(row["kind"], json!(kind));
    assert_eq!(
        row["writer_oid"].as_str().unwrap(),
        session_support::broker_oid().to_string()
    );
    assert_eq!(row["request"], serde_json::to_value(q).unwrap());
    assert_eq!(row["registration_operation_id"], json!(reg_op));
    assert_eq!(row["registration_event_id"], json!(reg_event));
    assert_ne!(row["generation_call_id"], row["repair_call_id"]);
    for id in ["generation_call_id", "repair_call_id"] {
        assert_ne!(row[id].as_str().unwrap(), nil, "{id} must be nonnil");
    }
    let consent = &serde_json::to_value(source).unwrap()["fields"]["m1_inference_permission"];
    assert_eq!(row["capacity"], capacity_for(&consent["operation_limits"]));
    assert!(
        row["recorded_at"].as_str().is_some_and(|s| !s.is_empty()),
        "recorded_at must carry the database timestamp: {row}"
    );
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

/// The guarded function refuses every malformed call with one SQLSTATE.
pub fn refused(result: Result<Option<Value>, postgres::Error>) {
    let error = result.expect_err("guarded function must refuse");
    assert_eq!(
        error.code().map(postgres::error::SqlState::code),
        Some("P0001"),
        "{error}"
    );
}

/// Direct guarded-function call over the broker login.
pub fn sql_call(action: &str, qv: &Value, sv: &Value) -> Result<Option<Value>, postgres::Error> {
    sql(&mut session_support::broker_client(), action, qv, op(), sv)
}

/// Overwrite the stored publication; the guarded function then sees the
/// tampered contract as the authoritative source.
pub fn tamper(admin: &mut postgres::Client, q: &ReservationRequest, contract: &Value) {
    admin
        .execute(
            "UPDATE mission.registration_outbox SET contract=$1 \
             WHERE engagement_id=$2 AND campaign_id=$3",
            &[&contract, &q.engagement_id.0, &q.campaign_id.0],
        )
        .unwrap();
}

/// Tamper the stored contract, then prove the same call refuses at the door
/// with P0001 and the expected guard message — never an unrelated setup
/// error. The invocation itself must not mutate the complete scoped state
/// (the baseline is taken after the deliberate fixture tamper).
pub fn tamper_refuse(
    admin: &mut postgres::Client,
    q: &ReservationRequest,
    qv: &Value,
    original: &Value,
    expect: &str,
    mutate: impl Fn(&mut Value),
) {
    tamper_refuse_code(admin, q, qv, original, "P0001", Some(expect), mutate);
}

/// Same tamper/refuse cycle as tamper_refuse, asserting the SQLSTATE class
/// and optionally the guard message: used where Postgres's own eager cast
/// produces the refusal (the detail text is versioned), while the
/// mutation-free guarantee is identical.
pub fn tamper_refuse_code(
    admin: &mut postgres::Client,
    q: &ReservationRequest,
    qv: &Value,
    original: &Value,
    code: &str,
    message: Option<&str>,
    mutate: impl Fn(&mut Value),
) {
    let mut forged = original.clone();
    mutate(&mut forged);
    tamper(admin, q, &forged);
    let snapshot = state(q);
    let error = sql_call("reserve", qv, &forged).expect_err("guarded function must refuse");
    assert_eq!(
        error.code().map(postgres::error::SqlState::code),
        Some(code),
        "{error}"
    );
    if let Some(expect) = message {
        assert_eq!(
            error.as_db_error().map(|d| d.message()),
            Some(expect),
            "{error}"
        );
    }
    assert_eq!(state(q), snapshot, "refusal must not mutate scoped state");
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

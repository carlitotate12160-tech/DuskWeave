//! Prepared/no-effects session fence domain and input contract tests:
//! strict bounded ingress, the fixed lifecycle vocabulary, forged/mismatched
//! port records that fail without a grant, and composition refusals over
//! fake ports. Real fence/role/concurrency behavior lives in the dedicated
//! PostgreSQL suites.

use duskweave::input::parse_register;
use duskweave::m1_session::{
    self, FenceOutcome, SessionClaim, SessionFence, SessionRecord, SessionRequest,
};
use duskweave::m1_session_input::{parse_session_request, MAX_INPUT_BYTES};
use duskweave::mission::{
    CampaignId, EngagementId, EventId, Mission, MissionRegistered, OperationId, OperatorRef,
    RegistrationInput,
};
use duskweave::registration::{CommitEffect, MissionStore, MissionView};
use duskweave::{Fail, Res};
use uuid::Uuid;

const OPERATOR: u128 = 0x11;

fn operator() -> OperatorRef {
    OperatorRef(Uuid::from_u128(OPERATOR))
}

fn scope(n: u128) -> (EngagementId, CampaignId) {
    (
        EngagementId(Uuid::from_u128(0x1000 + n)),
        CampaignId(Uuid::from_u128(0x2000 + n)),
    )
}

fn session_reg_json(e: Uuid, c: Uuid, starts_at: i64, ends_at: i64) -> serde_json::Value {
    serde_json::json!({
        "engagement_id": e, "campaign_id": c,
        "operator_ref": Uuid::from_u128(OPERATOR), "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 1, "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21)], "excluded_assets": [],
        "exercise_mode": "blind", "starts_at": starts_at, "ends_at": ends_at,
        "m1_permission": {
            "policy_version": 1, "ct_base_domain": "example.internal",
            "provider_disclosure": "crt_sh", "vantage_ref": Uuid::from_u128(0x31),
            "resolver_ipv4": "127.0.0.1",
            "discovery_rules": [{"exact": "example.internal"}],
            "contact_rules": [{"asset_ref": Uuid::from_u128(0x21),
                "rule": {"exact": "host.example.internal"}, "priority": 1}],
            "excluded_names": [], "approved_path": "/",
            "starts_at": starts_at, "ends_at": ends_at,
            "campaign_limits": {"episodes": 1, "provider_calls": 1, "dns_questions": 1,
                "dns_followups": 1, "tcp_connections": 1, "head_requests": 1},
            "concurrency": 1
        }
    })
}

fn event_from(raw_json: serde_json::Value, at: i64) -> MissionRegistered {
    let input: RegistrationInput =
        parse_register(raw_json.to_string().as_bytes()).expect("fixture parses");
    let (_, event) = Mission::register(
        &input,
        OperationId(Uuid::from_u128(0x51)),
        EventId(Uuid::from_u128(0x52)),
        at,
    )
    .expect("fixture registers");
    event
}

fn mission_view(
    e: EngagementId,
    c: CampaignId,
    op: Uuid,
    starts_at: i64,
    ends_at: i64,
) -> MissionView {
    MissionView {
        operation_id: OperationId(op),
        revision: 1,
        exercise_mode: duskweave::mission::ExerciseMode::Blind,
        starts_at,
        ends_at,
    }
}

fn store_with(
    e: EngagementId,
    c: CampaignId,
    event: MissionRegistered,
    starts_at: i64,
    ends_at: i64,
) -> FakeMissionStore {
    FakeMissionStore {
        view: Some(mission_view(e, c, event.operation_id.0, starts_at, ends_at)),
        event: Some(event),
    }
}

#[derive(Default, Clone)]
struct FakeMissionStore {
    view: Option<MissionView>,
    event: Option<MissionRegistered>,
}

impl MissionStore for FakeMissionStore {
    fn commit_registration(
        &mut self,
        _: &duskweave::mission::Mission,
        _: &MissionRegistered,
    ) -> Res<CommitEffect> {
        Err(Fail::State("not_used"))
    }
    fn mission_view(&mut self, _e: EngagementId, _c: CampaignId) -> Res<Option<MissionView>> {
        Ok(self.view.clone())
    }
    fn outbox_event(
        &mut self,
        _e: EngagementId,
        _c: CampaignId,
        _op: OperationId,
    ) -> Res<Option<MissionRegistered>> {
        Ok(self.event.clone())
    }
}

#[derive(Default)]
struct FakeFence {
    prepare: Option<FenceOutcome>,
    release: Option<FenceOutcome>,
    recover: Option<FenceOutcome>,
    claims: Vec<SessionClaim>,
}

impl SessionFence for FakeFence {
    fn prepare(&mut self, claim: &SessionClaim) -> Res<FenceOutcome> {
        self.claims.push(claim.clone());
        Ok(self.prepare.clone().unwrap_or(FenceOutcome::Missing))
    }
    fn release(&mut self, _r: &SessionRequest, _o: OperationId) -> Res<FenceOutcome> {
        Ok(self.release.clone().unwrap_or(FenceOutcome::Missing))
    }
    fn recover(&mut self, _r: &SessionRequest, _o: OperationId) -> Res<FenceOutcome> {
        Ok(self.recover.clone().unwrap_or(FenceOutcome::Missing))
    }
}

fn durable_record(e: EngagementId, c: CampaignId, op: Uuid, kind: &str) -> SessionRecord {
    SessionRecord {
        engagement_id: e,
        campaign_id: c,
        operation_id: OperationId(op),
        operator_ref: operator(),
        expected_mission_revision: 1,
        registration_operation_id: OperationId(Uuid::from_u128(0x51)),
        registration_event_id: EventId(Uuid::from_u128(0x52)),
        generation: 1,
        writer_oid: 16384,
        kind: kind.to_string(),
        recorded_at: 1_700_000_000,
    }
}

fn request(e: EngagementId, c: CampaignId) -> SessionRequest {
    SessionRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: operator(),
        expected_mission_revision: 1,
    }
}

// --- strict bounded input ---

#[test]
fn input_accepts_the_fixed_fields_only() {
    let (e, c) = scope(1);
    let raw = serde_json::json!({
        "engagement_id": e, "campaign_id": c,
        "operator_ref": Uuid::from_u128(OPERATOR), "expected_mission_revision": 1
    })
    .to_string();
    let got = parse_session_request(raw.as_bytes()).expect("fixed fields parse");
    assert_eq!(got.expected_mission_revision, 1);
}

#[test]
fn input_rejects_unknown_endpoint_secret_and_effect_fields() {
    let (e, c) = scope(2);
    for (name, value) in [
        ("endpoint", serde_json::json!("https://127.0.0.1:1")),
        ("secret", serde_json::json!("sentinel-value")),
        ("effect", serde_json::json!("start")),
        ("revision_extra", serde_json::json!(1)),
    ] {
        let raw = serde_json::json!({
            "engagement_id": e, "campaign_id": c,
            "operator_ref": Uuid::from_u128(OPERATOR), "expected_mission_revision": 1,
            name: value
        })
        .to_string();
        assert_eq!(
            parse_session_request(raw.as_bytes()),
            Err(Fail::Input("schema_violation")),
            "unknown field {name} must be rejected"
        );
    }
}

#[test]
fn input_rejects_nil_identity_and_wrong_revision() {
    let (e, c) = scope(3);
    for (field, value) in [
        ("engagement_id", Uuid::nil()),
        ("campaign_id", Uuid::nil()),
        ("operator_ref", Uuid::nil()),
    ] {
        let raw = serde_json::json!({
            "engagement_id": e, "campaign_id": c,
            "operator_ref": Uuid::from_u128(OPERATOR), "expected_mission_revision": 1,
            field: value
        })
        .to_string();
        assert_eq!(
            parse_session_request(raw.as_bytes()),
            Err(Fail::Input("nil_reference")),
        );
    }
    for revision in [0u64, 2, u64::MAX] {
        let raw = serde_json::json!({
            "engagement_id": e, "campaign_id": c,
            "operator_ref": Uuid::from_u128(OPERATOR),
            "expected_mission_revision": revision
        })
        .to_string();
        assert_eq!(
            parse_session_request(raw.as_bytes()),
            Err(Fail::Input("invalid_revision")),
            "revision {revision} must be rejected"
        );
    }
}

#[test]
fn input_rejects_malformed_oversize_and_empty_payloads() {
    assert_eq!(parse_session_request(b"{"), Err(Fail::Input("malformed_json")));
    assert_eq!(parse_session_request(b""), Err(Fail::Input("size_limit")));
    let oversized = vec![b' '; MAX_INPUT_BYTES + 1];
    assert_eq!(
        parse_session_request(&oversized),
        Err(Fail::Input("size_limit"))
    );
    assert_eq!(
        parse_session_request(b"[]"),
        Err(Fail::Input("schema_violation"))
    );
}

// --- record vocabulary and binding ---

fn raw_record(e: Uuid, c: Uuid, op: Uuid) -> serde_json::Value {
    serde_json::json!({
        "engagement_id": e, "campaign_id": c, "operation_id": op,
        "operator_ref": Uuid::from_u128(OPERATOR), "expected_mission_revision": 1,
        "registration_operation_id": Uuid::from_u128(0x51),
        "registration_event_id": Uuid::from_u128(0x52),
        "generation": 1, "writer_oid": 16384,
        "kind": "prepared_no_effects", "recorded_at": 1_700_000_000
    })
}

#[test]
fn record_decodes_the_fixed_lifecycle_vocabulary() {
    let (e, c) = scope(4);
    let op = Uuid::from_u128(0x61);
    let record = SessionRecord::decode(raw_record(e.0, c.0, op)).expect("valid record decodes");
    assert_eq!(record.kind, "prepared_no_effects");
    let mut released = raw_record(e.0, c.0, op);
    released["kind"] = serde_json::json!("released_no_effects");
    assert_eq!(
        SessionRecord::decode(released)
            .expect("released decodes")
            .kind,
        "released_no_effects"
    );
    for kind in ["", "prepared", "prepared_no_effects2", "released"] {
        let mut raw = raw_record(e.0, c.0, op);
        raw["kind"] = serde_json::json!(kind);
        assert_eq!(
            SessionRecord::decode(raw),
            Err(Fail::Store("contract_decode")),
            "kind {kind:?} is outside the fixed vocabulary"
        );
    }
}

#[test]
fn record_rejects_unsafe_field_values() {
    let (e, c) = scope(5);
    let op = Uuid::from_u128(0x62);
    let base = || raw_record(e.0, c.0, op);
    let with = |field: &str, value: serde_json::Value| {
        let mut raw = base();
        raw[field] = value;
        raw
    };
    let cases = [
        ("revision", with("expected_mission_revision", serde_json::json!(2))),
        ("generation", with("generation", serde_json::json!(0))),
        ("writer", with("writer_oid", serde_json::json!(0))),
        ("operation", with("operation_id", serde_json::json!(Uuid::nil()))),
        ("operator", with("operator_ref", serde_json::json!(Uuid::nil()))),
        (
            "registration",
            with("registration_operation_id", serde_json::json!(Uuid::nil())),
        ),
        (
            "event",
            with("registration_event_id", serde_json::json!(Uuid::nil())),
        ),
        ("scope", with("engagement_id", serde_json::json!(Uuid::nil()))),
    ];
    for (name, raw) in cases {
        assert_eq!(
            SessionRecord::decode(raw),
            Err(Fail::Store("contract_decode")),
            "{name} must fail decoding"
        );
    }
}

#[test]
fn record_binding_rejects_mismatched_identities() {
    let (e, c) = scope(6);
    let (other_e, _other_c) = scope(7);
    let op = OperationId(Uuid::from_u128(0x63));
    let record = SessionRecord::decode(raw_record(e.0, c.0, op.0)).expect("decodes");
    let req = request(e, c);
    assert!(record.binds(&req, op, Some(record.registration_operation_id)));
    assert!(record.binds(&req, op, None), "absent expected registration binds");
    assert!(!record.binds(&request(other_e, c), op, None), "wrong engagement");
    assert!(
        !record.binds(&req, OperationId(Uuid::from_u128(0x64)), None),
        "wrong operation"
    );
    let mut wrong_operator = request(e, c);
    wrong_operator.operator_ref = OperatorRef(Uuid::from_u128(0x99));
    assert!(!record.binds(&wrong_operator, op, None), "wrong operator");
    let mut wrong_revision = request(e, c);
    wrong_revision.expected_mission_revision = 2;
    assert!(!record.binds(&wrong_revision, op, None), "wrong revision");
    assert!(
        !record.binds(&req, op, Some(OperationId(Uuid::from_u128(0x77)))),
        "different registration binding"
    );
}

#[test]
fn refusal_decode_covers_the_fixed_reasons() {
    for reason in [
        "already_claimed",
        "writer_mismatch",
        "operator_mismatch",
        "source_conflict",
        "window_closed",
        "permission_absent",
        "mission_missing",
        "withdrawn",
        "stale_revision",
        "operation_reuse",
        "not_owner",
        "no_prepared_claim",
        "not_authorized",
    ] {
        assert_eq!(m1_session::decode_refusal(reason), reason, "decode {reason}");
    }
    assert_eq!(
        m1_session::decode_refusal("anything_unrecognized"),
        "unrecognized_refusal"
    );
}

// --- composition over fake ports ---

fn current_window() -> (i64, i64) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    (now - 60, now + 3600)
}

#[test]
fn prepare_passes_a_bound_claim_with_current_source() {
    let (e, c) = scope(8);
    let (starts, ends) = current_window();
    let event = event_from(session_reg_json(e.0, c.0, starts, ends), starts);
    let mut store = store_with(e, c, event, starts, ends);
    let op = OperationId(Uuid::from_u128(0x71));
    let mut fence = FakeFence {
        prepare: Some(FenceOutcome::Durable(durable_record(
            e,
            c,
            op.0,
            "prepared_no_effects",
        ))),
        ..Default::default()
    };
    let outcome = m1_session::prepare(&mut store, &mut fence, &request(e, c), op)
        .expect("prepare composes");
    assert_eq!(
        outcome,
        FenceOutcome::Durable(durable_record(e, c, op.0, "prepared_no_effects"))
    );
    assert_eq!(fence.claims.len(), 1, "one claim handed to the fence port");
    let claim = &fence.claims[0];
    assert_eq!(claim.engagement_id, e);
    assert_eq!(claim.campaign_id, c);
    assert_eq!(claim.operation_id, op);
    assert_eq!(
        claim.registration_operation_id,
        OperationId(Uuid::from_u128(0x51))
    );
    assert_eq!(
        claim.contract["fields"]["m1_permission"]["policy_version"],
        serde_json::json!(1)
    );
}

#[test]
fn prepare_refuses_closed_permission_windows() {
    let (e, c) = scope(9);
    let (starts, ends) = current_window();
    let mut expired = session_reg_json(e.0, c.0, starts, ends);
    // Inside the current mission window but already elapsed.
    expired["m1_permission"]["starts_at"] = serde_json::json!(starts);
    expired["m1_permission"]["ends_at"] = serde_json::json!(starts + 30);
    let event = event_from(expired, starts);
    let mut store = store_with(e, c, event, starts, ends);
    let op = OperationId(Uuid::from_u128(0x72));
    let outcome =
        m1_session::prepare(&mut store, &mut FakeFence::default(), &request(e, c), op)
            .expect("composes");
    assert_eq!(outcome, FenceOutcome::Refused("window_closed"));
}

#[test]
fn prepare_refuses_missing_permission_attachment() {
    let (e, c) = scope(15);
    let (starts, ends) = current_window();
    let mut bare = session_reg_json(e.0, c.0, starts, ends);
    bare.as_object_mut().unwrap().remove("m1_permission");
    let event = event_from(bare, starts);
    let mut store = store_with(e, c, event, starts, ends);
    let op = OperationId(Uuid::from_u128(0x78));
    let outcome =
        m1_session::prepare(&mut store, &mut FakeFence::default(), &request(e, c), op)
            .expect("composes");
    assert_eq!(outcome, FenceOutcome::Refused("permission_absent"));
}

#[test]
fn prepare_refuses_operator_mismatch() {
    let (e, c) = scope(16);
    let (starts, ends) = current_window();
    let event = event_from(session_reg_json(e.0, c.0, starts, ends), starts);
    let mut store = store_with(e, c, event, starts, ends);
    let mut wrong = request(e, c);
    wrong.operator_ref = OperatorRef(Uuid::from_u128(0x99));
    let op = OperationId(Uuid::from_u128(0x79));
    let outcome =
        m1_session::prepare(&mut store, &mut FakeFence::default(), &wrong, op)
            .expect("composes");
    assert_eq!(outcome, FenceOutcome::Refused("operator_mismatch"));
}

#[test]
fn prepare_refuses_missing_mission_and_source() {
    let (e, c) = scope(10);
    let (starts, ends) = current_window();
    let op = OperationId(Uuid::from_u128(0x73));
    let mut empty = FakeMissionStore::default();
    assert!(matches!(
        m1_session::prepare(&mut empty, &mut FakeFence::default(), &request(e, c), op),
        Err(Fail::State("mission_missing"))
    ));
    // Catalog present but the validated original source missing.
    let mut no_source = FakeMissionStore {
        view: Some(mission_view(e, c, Uuid::from_u128(0x51), starts, ends)),
        event: None,
    };
    assert!(matches!(
        m1_session::prepare(
            &mut no_source,
            &mut FakeFence::default(),
            &request(e, c),
            op
        ),
        Err(Fail::State("mission_missing"))
    ));
}

#[test]
fn prepare_maps_forged_and_mismatched_durable_records_to_unknown() {
    let (e, c) = scope(11);
    let (other_e, _other_c) = scope(12);
    let (starts, ends) = current_window();
    let event = event_from(session_reg_json(e.0, c.0, starts, ends), starts);
    let mut store = store_with(e, c, event, starts, ends);
    let op = OperationId(Uuid::from_u128(0x74));
    // A forged scope binding must never surface as a granted record.
    let forged = durable_record(other_e, c, op.0, "prepared_no_effects");
    let mut fence = FakeFence {
        prepare: Some(FenceOutcome::Durable(forged)),
        ..Default::default()
    };
    let outcome = m1_session::prepare(&mut store, &mut fence, &request(e, c), op)
        .expect("composes");
    assert_eq!(
        outcome,
        FenceOutcome::Unknown,
        "forged record is unresolved, never a grant"
    );
    // A refused outcome passes through untouched.
    let mut refused = FakeFence {
        prepare: Some(FenceOutcome::Refused("already_claimed")),
        ..Default::default()
    };
    let outcome = m1_session::prepare(&mut store, &mut refused, &request(e, c), op)
        .expect("composes");
    assert_eq!(outcome, FenceOutcome::Refused("already_claimed"));
}

#[test]
fn release_and_recover_validate_returned_records() {
    let (e, c) = scope(13);
    let op = OperationId(Uuid::from_u128(0x75));
    // A forged release record is unresolved, never a release confirmation.
    let mut fence = FakeFence {
        release: Some(FenceOutcome::Durable(durable_record(
            e,
            c,
            Uuid::from_u128(0x76),
            "released_no_effects",
        ))),
        ..Default::default()
    };
    let outcome = m1_session::release(&mut fence, &request(e, c), op).expect("composes");
    assert_eq!(outcome, FenceOutcome::Unknown);
    // Recover with a wrong-operator record is a decode failure, not a grant.
    let mut wrong_operator = durable_record(e, c, op.0, "prepared_no_effects");
    wrong_operator.operator_ref = OperatorRef(Uuid::from_u128(0x99));
    let mut fence = FakeFence {
        recover: Some(FenceOutcome::Durable(wrong_operator)),
        ..Default::default()
    };
    assert!(matches!(
        m1_session::recover(&mut fence, &request(e, c), op),
        Err(Fail::Store("contract_decode"))
    ));
    // Missing recovery passes through without mutation claims.
    let mut fence = FakeFence {
        recover: Some(FenceOutcome::Missing),
        ..Default::default()
    };
    let outcome = m1_session::recover(&mut fence, &request(e, c), op).expect("composes");
    assert_eq!(outcome, FenceOutcome::Missing);
}

#[test]
fn composition_rejects_nil_operation_identity() {
    let (e, c) = scope(14);
    let (starts, ends) = current_window();
    let event = event_from(session_reg_json(e.0, c.0, starts, ends), starts);
    let mut store = store_with(e, c, event, starts, ends);
    let mut fence = FakeFence::default();
    let nil = OperationId(Uuid::nil());
    assert_eq!(
        m1_session::prepare(&mut store, &mut fence, &request(e, c), nil),
        Err(Fail::Input("nil_identity"))
    );
    assert_eq!(
        m1_session::release(&mut fence, &request(e, c), nil),
        Err(Fail::Input("nil_identity"))
    );
    assert_eq!(
        m1_session::recover(&mut fence, &request(e, c), nil),
        Err(Fail::Input("nil_identity"))
    );
    assert!(
        fence.claims.is_empty(),
        "nil identity never reaches the fence port"
    );
}

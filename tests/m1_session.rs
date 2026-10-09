//! m1_session contract coverage without a database: request/input bounds,
//! forged-source and forged-record negative controls through the narrow
//! ports, and proof that recovery/release never touch the Mission read port.

use duskweave::m1_session::{SessionAction, SessionRecord, SessionRequest, SessionStore, session};
use duskweave::m1_session_input::parse_session;
use duskweave::mission::*;
use duskweave::registration::{CommitEffect, MissionStore, MissionView};
use duskweave::{Fail, Res};
use serde_json::json;
use uuid::Uuid;

const E: u128 = 0xA1;
const C: u128 = 0xA2;
const REG_OP: u128 = 0xA3;
const SESSION_OP: u128 = 0xA4;

fn request() -> SessionRequest {
    SessionRequest {
        engagement_id: EngagementId(Uuid::from_u128(E)),
        campaign_id: CampaignId(Uuid::from_u128(C)),
        operator_ref: OperatorRef(Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
    }
}

fn registration_json(permission: bool) -> serde_json::Value {
    let mut value = json!({
        "engagement_id": Uuid::from_u128(E), "campaign_id": Uuid::from_u128(C),
        "operator_ref": Uuid::from_u128(0x11), "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 1, "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21)],
        "excluded_assets": [Uuid::from_u128(0x23)],
        "exercise_mode": "blind", "starts_at": 100, "ends_at": 100000
    });
    if permission {
        value["m1_permission"] = json!({
            "policy_version": 1, "ct_base_domain": "example.invalid",
            "provider_disclosure": "crt_sh", "vantage_ref": Uuid::from_u128(0x31),
            "resolver_ipv4": "192.0.2.53",
            "discovery_rules": [{"label_suffix": "example.invalid"}],
            "contact_rules": [{"asset_ref": Uuid::from_u128(0x21),
                "rule": {"exact": "api.example.invalid"}, "priority": 1}],
            "excluded_names": [{"exact": "excluded.example.invalid"}],
            "approved_path": "/", "starts_at": 200, "ends_at": 90000,
            "campaign_limits": {"episodes": 2, "provider_calls": 3,
                "dns_questions": 10, "dns_followups": 2, "tcp_connections": 5,
                "head_requests": 2},
            "concurrency": 1
        });
    }
    value
}

fn registered_event(permission: bool) -> MissionRegistered {
    let raw = serde_json::to_vec(&registration_json(permission)).unwrap();
    let input = duskweave::input::parse_register(&raw).unwrap();
    Mission::register(
        &input,
        OperationId(Uuid::from_u128(REG_OP)),
        EventId(Uuid::from_u128(0x42)),
        500,
    )
    .unwrap()
    .1
}

fn good_record() -> SessionRecord {
    SessionRecord {
        engagement_id: EngagementId(Uuid::from_u128(E)),
        campaign_id: CampaignId(Uuid::from_u128(C)),
        operation_id: OperationId(Uuid::from_u128(SESSION_OP)),
        operator_ref: OperatorRef(Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
        registration_operation_id: OperationId(Uuid::from_u128(REG_OP)),
        registration_event_id: EventId(Uuid::from_u128(0x42)),
        generation: 1,
        writer_oid: 77,
        kind: "prepared_no_effects".into(),
        recorded_at: "2026-10-09T00:00:00+00:00".into(),
    }
}

struct Store {
    reply: Res<Option<SessionRecord>>,
    source: Option<MissionRegistered>,
}

impl SessionStore for Store {
    fn execute(
        &mut self,
        _: SessionAction,
        _: &SessionRequest,
        _: OperationId,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<SessionRecord>> {
        self.source = source.cloned();
        self.reply.clone()
    }
}

struct Reader {
    view: Option<MissionView>,
    event: Option<MissionRegistered>,
}

impl MissionStore for Reader {
    fn commit_registration(&mut self, _: &Mission, _: &MissionRegistered) -> Res<CommitEffect> {
        panic!("session admission never registers")
    }
    fn mission_view(&mut self, _: EngagementId, _: CampaignId) -> Res<Option<MissionView>> {
        Ok(self.view.clone())
    }
    fn outbox_event(
        &mut self,
        _: EngagementId,
        _: CampaignId,
        _: OperationId,
    ) -> Res<Option<MissionRegistered>> {
        Ok(self.event.clone())
    }
}

/// Recover/release must not consult the Mission port at all.
struct Never;
impl MissionStore for Never {
    fn commit_registration(&mut self, _: &Mission, _: &MissionRegistered) -> Res<CommitEffect> {
        panic!("recovery never writes")
    }
    fn mission_view(&mut self, _: EngagementId, _: CampaignId) -> Res<Option<MissionView>> {
        panic!("recovery never reads the Mission catalog")
    }
    fn outbox_event(
        &mut self,
        _: EngagementId,
        _: CampaignId,
        _: OperationId,
    ) -> Res<Option<MissionRegistered>> {
        panic!("recovery never reads the Mission outbox")
    }
}

fn view() -> MissionView {
    MissionView {
        operation_id: OperationId(Uuid::from_u128(REG_OP)),
        revision: 1,
        exercise_mode: ExerciseMode::Blind,
        starts_at: 100,
        ends_at: 100000,
    }
}

fn op() -> OperationId {
    OperationId(Uuid::from_u128(SESSION_OP))
}

#[test]
fn request_validation_rejects_nil_identity_and_wrong_revision() {
    for nil_field in 0..4 {
        let mut r = request();
        match nil_field {
            0 => r.engagement_id = EngagementId(Uuid::nil()),
            1 => r.campaign_id = CampaignId(Uuid::nil()),
            2 => r.operator_ref = OperatorRef(Uuid::nil()),
            _ => return,
        }
        assert_eq!(
            r.validate(op()),
            Err(Fail::Input("invalid_session_request"))
        );
    }
    assert_eq!(
        request().validate(OperationId(Uuid::nil())),
        Err(Fail::Input("invalid_session_request"))
    );
    for revision in [0, 2, -1] {
        let mut r = request();
        r.expected_mission_revision = revision;
        assert_eq!(
            r.validate(op()),
            Err(Fail::Input("invalid_session_request"))
        );
    }
    assert_eq!(request().validate(op()), Ok(()));
}

#[test]
fn bounded_input_rejects_size_malformed_unknown_and_secret_fields() {
    assert_eq!(parse_session(b""), Err(Fail::Input("size_limit")));
    let oversize = vec![b' '; 16 * 1024 + 1];
    assert_eq!(parse_session(&oversize), Err(Fail::Input("size_limit")));
    for raw in [
        b"{".as_slice(),
        b"[]".as_slice(),
        br#"{"engagement_id":"x"}"#.as_slice(),
        &serde_json::to_vec(&json!({
            "engagement_id": Uuid::from_u128(E), "campaign_id": Uuid::from_u128(C),
            "operator_ref": Uuid::from_u128(0x11), "expected_mission_revision": 1,
            "endpoint": "tcp://target", "password": "SENTINEL"}))
        .unwrap(),
        &serde_json::to_vec(&json!({
            "engagement_id": Uuid::from_u128(E), "campaign_id": Uuid::from_u128(C),
            "operator_ref": Uuid::from_u128(0x11), "expected_mission_revision": 1,
            "effect": "start"}))
        .unwrap(),
    ] {
        assert_eq!(
            parse_session(raw),
            Err(Fail::Input("invalid_session_request"))
        );
    }
    let valid = serde_json::to_vec(&json!({
        "engagement_id": Uuid::from_u128(E), "campaign_id": Uuid::from_u128(C),
        "operator_ref": Uuid::from_u128(0x11), "expected_mission_revision": 1}))
    .unwrap();
    assert!(parse_session(&valid).is_ok());
}

#[test]
fn prepare_composes_validated_source_and_binds_returned_record() {
    let mut store = Store {
        reply: Ok(Some(good_record())),
        source: None,
    };
    let mut reader = Reader {
        view: Some(view()),
        event: Some(registered_event(true)),
    };
    let got = session(
        &mut store,
        &mut reader,
        SessionAction::Prepare,
        &request(),
        op(),
    );
    assert_eq!(got, Ok(Some(good_record())));
    assert_eq!(store.source, Some(registered_event(true)));
}

#[test]
fn prepare_refuses_missing_permission_or_operator_mismatch() {
    for (event, r) in [
        (registered_event(false), request()),
        (registered_event(true), {
            let mut r = request();
            r.operator_ref = OperatorRef(Uuid::from_u128(0x99));
            r
        }),
    ] {
        let mut store = Store {
            reply: Ok(Some(good_record())),
            source: None,
        };
        let mut reader = Reader {
            view: Some(view()),
            event: Some(event),
        };
        assert_eq!(
            session(&mut store, &mut reader, SessionAction::Prepare, &r, op()),
            Err(Fail::State("session_prepare_refused"))
        );
        assert!(store.source.is_none());
    }
}

#[test]
fn forged_source_identity_and_contract_fail_closed() {
    let mut forged = registered_event(true);
    forged.campaign_id = CampaignId(Uuid::from_u128(0xFF));
    let mut wrong_op = registered_event(true);
    wrong_op.operation_id = OperationId(Uuid::from_u128(0xFE));
    let mut corrupt = registered_event(true);
    corrupt.producer = "forged".into();
    for event in [forged, wrong_op, corrupt] {
        let mut store = Store {
            reply: Ok(Some(good_record())),
            source: None,
        };
        let mut reader = Reader {
            view: Some(view()),
            event: Some(event),
        };
        assert_eq!(
            session(
                &mut store,
                &mut reader,
                SessionAction::Prepare,
                &request(),
                op()
            ),
            Err(Fail::Store("contract_decode"))
        );
        assert!(store.source.is_none());
    }
    let mut missing = Reader {
        view: None,
        event: None,
    };
    assert_eq!(
        session(
            &mut Store {
                reply: Ok(None),
                source: None
            },
            &mut missing,
            SessionAction::Prepare,
            &request(),
            op()
        ),
        Err(Fail::State("session_mission_missing"))
    );
}

#[test]
fn forged_or_mismatched_records_fail_decode_without_grant() {
    for mutate in [
        |r: &mut SessionRecord| r.engagement_id = EngagementId(Uuid::from_u128(0xF1)),
        |r: &mut SessionRecord| r.operation_id = OperationId(Uuid::from_u128(0xF2)),
        |r: &mut SessionRecord| r.operator_ref = OperatorRef(Uuid::from_u128(0xF3)),
        |r: &mut SessionRecord| r.expected_mission_revision = 2,
        |r: &mut SessionRecord| r.generation = 0,
        |r: &mut SessionRecord| r.writer_oid = 0,
        |r: &mut SessionRecord| r.registration_operation_id = OperationId(Uuid::nil()),
        |r: &mut SessionRecord| r.registration_event_id = EventId(Uuid::nil()),
        |r: &mut SessionRecord| r.kind = "effect_enabled".into(),
        |r: &mut SessionRecord| r.kind = "prepared".into(),
    ] {
        let mut record = good_record();
        mutate(&mut record);
        let mut store = Store {
            reply: Ok(Some(record)),
            source: None,
        };
        for action in [SessionAction::Recover, SessionAction::Release] {
            assert_eq!(
                session(&mut store, &mut Never, action, &request(), op()),
                Err(Fail::Unresolved("session_contract_decode"))
            );
        }
    }
}

#[test]
fn recover_returns_missing_and_release_never_touches_mission_port() {
    let mut store = Store {
        reply: Ok(None),
        source: None,
    };
    assert_eq!(
        session(
            &mut store,
            &mut Never,
            SessionAction::Recover,
            &request(),
            op()
        ),
        Ok(None)
    );
    let mut released = good_record();
    released.kind = "released_no_effects".into();
    let mut store = Store {
        reply: Ok(Some(released.clone())),
        source: None,
    };
    assert_eq!(
        session(
            &mut store,
            &mut Never,
            SessionAction::Release,
            &request(),
            op()
        ),
        Ok(Some(released))
    );
    assert!(store.source.is_none());
}

//! Typed request/record and use-case contract for the pre-send inference
//! reservation; deliberately database-free.
use duskweave::m1_inference_budget::*;
use duskweave::mission::*;
use duskweave::registration::{CommitEffect, MissionStore, MissionView};
use duskweave::{Fail, Res};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_session.rs"]
mod session_support;

fn id(n: u128) -> OperationId {
    OperationId(Uuid::from_u128(n))
}

fn request() -> ReservationRequest {
    serde_json::from_value(json!({"version":"m1_inference_reservation_request_v1",
        "engagement_id":EngagementId(Uuid::from_u128(1)),"campaign_id":CampaignId(Uuid::from_u128(2)),
        "operator_ref":OperatorRef(Uuid::from_u128(3)),"expected_mission_revision":1,
        "permission_id":Uuid::from_u128(4),"goal_ref":GoalRef(Uuid::from_u128(5)),
        "exercise_mode":"blind"}))
    .unwrap()
}

fn input() -> Value {
    serde_json::to_value(request()).unwrap()
}

fn capacity() -> ReservedCapacity {
    ReservedCapacity {
        calls: 2,
        request_bytes: 32768,
        response_bytes: 131072,
        output_token_capacity: 8192,
        wall_time_ms: 20000,
    }
}

fn record() -> ReservationRecord {
    ReservationRecord {
        engagement_id: request().engagement_id,
        campaign_id: request().campaign_id,
        operation_id: id(6),
        generation: 1,
        record_revision: 1,
        writer_oid: "77".into(),
        request: request(),
        registration_operation_id: id(7),
        registration_event_id: EventId(Uuid::from_u128(8)),
        generation_call_id: Uuid::from_u128(9),
        repair_call_id: Uuid::from_u128(10),
        capacity: capacity(),
        kind: "reserved_not_sent".into(),
        recorded_at: "2026-10-11 00:00:00+00".into(),
    }
}

fn source() -> MissionRegistered {
    let e = request().engagement_id;
    let c = request().campaign_id;
    let input = consent::input(e, c);
    Mission::register(&input, id(7), EventId(Uuid::from_u128(8)), 1700000000)
        .unwrap()
        .1
}

struct Store {
    reply: Res<Option<ReservationRecord>>,
    source: Option<MissionRegistered>,
    calls: usize,
}
impl InferenceReservationStore for Store {
    fn execute(
        &mut self,
        _: ReservationAction,
        _: &ReservationRequest,
        _: OperationId,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<ReservationRecord>> {
        self.calls += 1;
        self.source = source.cloned();
        self.reply.clone()
    }
}
fn store(reply: Res<Option<ReservationRecord>>) -> Store {
    Store {
        reply,
        source: None,
        calls: 0,
    }
}

struct Reader {
    event: Option<MissionRegistered>,
    missing: bool,
    forbidden: bool,
}
impl MissionStore for Reader {
    fn commit_registration(&mut self, _: &Mission, _: &MissionRegistered) -> Res<CommitEffect> {
        panic!("reservation never registers")
    }
    fn mission_view(&mut self, _: EngagementId, _: CampaignId) -> Res<Option<MissionView>> {
        assert!(!self.forbidden, "recover/cancel must not read the source");
        Ok((!self.missing).then(|| MissionView {
            operation_id: id(7),
            revision: 1,
            exercise_mode: ExerciseMode::Blind,
            starts_at: 1600000000,
            ends_at: 1800000000,
        }))
    }
    fn outbox_event(
        &mut self,
        _: EngagementId,
        _: CampaignId,
        _: OperationId,
    ) -> Res<Option<MissionRegistered>> {
        assert!(!self.forbidden);
        Ok(self.event.clone())
    }
}
fn reader() -> Reader {
    Reader {
        event: Some(source()),
        missing: false,
        forbidden: false,
    }
}

#[test]
fn invalid_id_version_and_revision_never_reach_store() {
    let mut cases = Vec::new();
    for mutate in [
        |r: &mut ReservationRequest| r.engagement_id = EngagementId(Uuid::nil()),
        |r: &mut ReservationRequest| r.campaign_id = CampaignId(Uuid::nil()),
        |r: &mut ReservationRequest| r.operator_ref = OperatorRef(Uuid::nil()),
        |r: &mut ReservationRequest| r.permission_id = Uuid::nil(),
        |r: &mut ReservationRequest| r.goal_ref = GoalRef(Uuid::nil()),
        |r: &mut ReservationRequest| r.expected_mission_revision = 0,
        |r: &mut ReservationRequest| r.expected_mission_revision = 2,
        |r: &mut ReservationRequest| r.version = "v2".into(),
        |r: &mut ReservationRequest| r.version = "m1_inference_reservation_request_v2".into(),
    ] {
        let mut r = request();
        mutate(&mut r);
        cases.push((r, id(6)));
    }
    cases.push((request(), OperationId(Uuid::nil())));
    for (r, op) in cases {
        for action in [
            ReservationAction::Reserve,
            ReservationAction::Recover,
            ReservationAction::Cancel,
        ] {
            let mut s = store(Ok(Some(record())));
            assert!(reservation(&mut s, Some(&mut reader()), action, &r, op).is_err());
            assert_eq!(s.calls, 0, "invalid identity must not reach the store");
        }
    }
    assert_eq!(ReservationAction::Reserve.name(), "reserve");
    assert_eq!(ReservationAction::Recover.name(), "recover");
    assert_eq!(ReservationAction::Cancel.name(), "cancel");
}

#[test]
fn closed_json_schema_rejects_unknown_missing_null_and_trailing() {
    let canonical = serde_json::to_string(&input()).unwrap();
    assert_eq!(
        serde_json::from_str::<ReservationRequest>(&canonical).unwrap(),
        request()
    );
    let mut extra = input();
    extra["extra"] = json!(1);
    let mut nested_extra = input();
    nested_extra["m1_permission_sentinel"] = json!({"x": 1});
    let mut missing = input();
    missing.as_object_mut().unwrap().remove("operator_ref");
    let mut nul = input();
    nul["operator_ref"] = Value::Null;
    let mut bad_mode = input();
    bad_mode["exercise_mode"] = json!("observe");
    for candidate in [extra, nested_extra, missing, nul, bad_mode] {
        assert!(serde_json::from_value::<ReservationRequest>(candidate).is_err());
    }
    for raw in [
        format!("{canonical} {{}}"),
        format!("{canonical} garbage"),
        canonical[..canonical.len() - 1].to_string(),
    ] {
        assert!(serde_json::from_str::<ReservationRequest>(&raw).is_err());
    }
}

#[test]
fn reserve_requires_mission_reader_recover_and_cancel_do_not() {
    let mut s = store(Ok(Some(record())));
    assert_eq!(
        reservation(
            &mut s,
            None::<&mut Reader>,
            ReservationAction::Reserve,
            &request(),
            id(6)
        ),
        Err(Fail::State("inference_authority_unavailable"))
    );
    assert_eq!(s.calls, 0);
    for action in [ReservationAction::Recover, ReservationAction::Cancel] {
        let mut s = store(Ok(Some(record())));
        let mut never = reader();
        never.forbidden = true;
        assert_eq!(
            reservation(&mut s, None::<&mut Reader>, action, &request(), id(6)),
            Ok(Some(record()))
        );
        assert_eq!(s.calls, 1);
        assert!(s.source.is_none(), "recover/cancel never pass a source");
        drop(never);
    }
}

#[test]
fn composed_source_reaches_store_and_consentless_source_refuses() {
    let mut s = store(Ok(Some(record())));
    assert_eq!(
        reservation(
            &mut s,
            Some(&mut reader()),
            ReservationAction::Reserve,
            &request(),
            id(6)
        ),
        Ok(Some(record()))
    );
    assert_eq!(s.source, Some(source()));
    let mut absent = source();
    let mut raw = serde_json::to_value(&absent).unwrap();
    raw["fields"]
        .as_object_mut()
        .unwrap()
        .remove("m1_inference_permission");
    absent = serde_json::from_value(raw).unwrap();
    // Consentless stored event: the use case refuses before the store.
    {
        let mut s = store(Ok(Some(record())));
        let mut r = reader();
        r.event = Some(absent);
        assert_eq!(
            reservation(
                &mut s,
                Some(&mut r),
                ReservationAction::Reserve,
                &request(),
                id(6)
            ),
            Err(Fail::State("inference_consent_absent"))
        );
        assert_eq!(s.calls, 0);
    }
    // A missing outbox event is decode integrity, never absence of consent.
    {
        let mut s = store(Ok(Some(record())));
        let mut r = reader();
        r.event = None;
        assert_eq!(
            reservation(
                &mut s,
                Some(&mut r),
                ReservationAction::Reserve,
                &request(),
                id(6)
            ),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(s.calls, 0);
    }
    let mut r = reader();
    r.missing = true;
    assert_eq!(
        reservation(
            &mut store(Ok(None)),
            Some(&mut r),
            ReservationAction::Reserve,
            &request(),
            id(6)
        ),
        Err(Fail::State("mission_missing"))
    );
}

#[test]
fn store_none_is_missing_only_for_recover() {
    for (action, expected) in [
        (
            ReservationAction::Reserve,
            Err(Fail::Unresolved("inference_record_decode")),
        ),
        (ReservationAction::Recover, Ok(None)),
        (
            ReservationAction::Cancel,
            Err(Fail::Unresolved("inference_record_decode")),
        ),
    ] {
        let mut s = store(Ok(None));
        assert_eq!(
            reservation(&mut s, Some(&mut reader()), action, &request(), id(6)),
            expected
        );
    }
}

#[test]
fn mismatched_or_corrupt_record_is_unknown_never_absence() {
    for mutate in [
        |r: &mut ReservationRecord| r.engagement_id = EngagementId(Uuid::from_u128(90)),
        |r: &mut ReservationRecord| r.campaign_id = CampaignId(Uuid::from_u128(90)),
        |r: &mut ReservationRecord| r.operation_id = id(90),
        |r: &mut ReservationRecord| r.generation = 0,
        |r: &mut ReservationRecord| r.generation = -1,
        |r: &mut ReservationRecord| r.record_revision = 0,
        |r: &mut ReservationRecord| r.record_revision = 3,
        |r: &mut ReservationRecord| r.writer_oid = "0".into(),
        |r: &mut ReservationRecord| r.writer_oid = "not-a-oid".into(),
        |r: &mut ReservationRecord| r.registration_operation_id = OperationId(Uuid::nil()),
        |r: &mut ReservationRecord| r.registration_event_id = EventId(Uuid::nil()),
        |r: &mut ReservationRecord| r.generation_call_id = r.repair_call_id,
        |r: &mut ReservationRecord| r.generation_call_id = Uuid::nil(),
        |r: &mut ReservationRecord| r.repair_call_id = Uuid::nil(),
        |r: &mut ReservationRecord| r.capacity.calls = 1,
        |r: &mut ReservationRecord| r.capacity.output_token_capacity = 8191,
        |r: &mut ReservationRecord| r.capacity.request_bytes = 0,
        |r: &mut ReservationRecord| r.capacity.response_bytes = 0,
        |r: &mut ReservationRecord| r.capacity.wall_time_ms = 0,
        |r: &mut ReservationRecord| r.kind = "send_qualified".into(),
        |r: &mut ReservationRecord| r.kind = "reservation".into(),
        |r: &mut ReservationRecord| r.request.operator_ref = OperatorRef(Uuid::from_u128(90)),
        |r: &mut ReservationRecord| r.request.permission_id = Uuid::from_u128(90),
        |r: &mut ReservationRecord| r.request.goal_ref = GoalRef(Uuid::from_u128(90)),
        |r: &mut ReservationRecord| r.request.expected_mission_revision = 2,
    ] {
        let mut value = record();
        mutate(&mut value);
        for action in [
            ReservationAction::Reserve,
            ReservationAction::Recover,
            ReservationAction::Cancel,
        ] {
            assert_eq!(
                reservation(
                    &mut store(Ok(Some(value.clone()))),
                    Some(&mut reader()),
                    action,
                    &request(),
                    id(6)
                ),
                Err(Fail::Unresolved("inference_record_decode"))
            );
        }
    }
    for (revision, kind) in [(1i64, "cancelled_never_sent"), (2, "reserved_not_sent")] {
        let mut value = record();
        value.record_revision = revision;
        value.kind = kind.into();
        assert_eq!(
            reservation(
                &mut store(Ok(Some(value))),
                Some(&mut reader()),
                ReservationAction::Recover,
                &request(),
                id(6)
            ),
            Err(Fail::Unresolved("inference_record_decode"))
        );
    }
    // A well-formed cancelled record decodes cleanly on recover.
    let mut cancelled = record();
    cancelled.record_revision = 2;
    cancelled.kind = "cancelled_never_sent".into();
    assert_eq!(
        reservation(
            &mut store(Ok(Some(cancelled.clone()))),
            Some(&mut reader()),
            ReservationAction::Recover,
            &request(),
            id(6)
        ),
        Ok(Some(cancelled))
    );
}

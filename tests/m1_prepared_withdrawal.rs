//! Typed input and Broker application contract; deliberately database-free.
use duskweave::m1_prepared_withdrawal::*;
use duskweave::m1_session_input::{parse_prepared_withdrawal, read_prepared_withdrawal_file};
use duskweave::mission::*;
use duskweave::registration::{CommitEffect, MissionStore, MissionView};
use duskweave::withdrawal::MissionAuthorityWithdrawn;
use duskweave::{Fail, Res};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_session.rs"]
mod session_support;

fn id(n: u128) -> OperationId {
    OperationId(Uuid::from_u128(n))
}
fn request() -> PreparedWithdrawalRequest {
    PreparedWithdrawalRequest {
        withdrawal: session_support::withdraw_req(
            EngagementId(Uuid::from_u128(1)),
            CampaignId(Uuid::from_u128(2)),
        ),
        session_operation_id: id(3),
        expected_session_generation: 1,
    }
}
fn input() -> Value {
    let r = request();
    json!({"withdrawal":r.withdrawal,"session_operation_id":r.session_operation_id,
        "expected_session_generation":1})
}
fn source() -> MissionRegistered {
    let raw = session_support::registration(
        request().withdrawal.engagement_id,
        request().withdrawal.campaign_id,
        1700000000,
        0,
    )
    .to_string();
    let input = duskweave::input::parse_register(raw.as_bytes()).unwrap();
    Mission::register(&input, id(4), EventId(Uuid::from_u128(5)), 1700000000)
        .unwrap()
        .1
}
fn record() -> PreparedWithdrawalRecord {
    let r = request();
    PreparedWithdrawalRecord {
        engagement_id: r.withdrawal.engagement_id,
        campaign_id: r.withdrawal.campaign_id,
        operation_id: id(6),
        session_operation_id: id(3),
        generation: 1,
        writer_oid: 77,
        event: MissionAuthorityWithdrawn::new(
            r.withdrawal,
            id(6),
            id(4),
            EventId(Uuid::from_u128(7)),
            1700000001,
        )
        .unwrap(),
    }
}
struct Store {
    reply: Res<Option<PreparedWithdrawalRecord>>,
    source: Option<MissionRegistered>,
    calls: usize,
}
impl PreparedWithdrawalStore for Store {
    fn execute(
        &mut self,
        _: &PreparedWithdrawalRequest,
        _: OperationId,
        _: bool,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<PreparedWithdrawalRecord>> {
        self.calls += 1;
        self.source = source.cloned();
        self.reply.clone()
    }
}
fn store(reply: Res<Option<PreparedWithdrawalRecord>>) -> Store {
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
        panic!("withdrawal never registers")
    }
    fn mission_view(&mut self, _: EngagementId, _: CampaignId) -> Res<Option<MissionView>> {
        assert!(!self.forbidden, "recovery must not read the source");
        Ok((!self.missing).then(|| MissionView {
            operation_id: id(4),
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
fn invalid_id_revision_generation_and_collision_never_reach_store() {
    let mut cases = Vec::new();
    for mutate in [
        |r: &mut PreparedWithdrawalRequest| r.withdrawal.engagement_id = EngagementId(Uuid::nil()),
        |r: &mut PreparedWithdrawalRequest| r.withdrawal.campaign_id = CampaignId(Uuid::nil()),
        |r: &mut PreparedWithdrawalRequest| r.withdrawal.operator_ref = OperatorRef(Uuid::nil()),
        |r: &mut PreparedWithdrawalRequest| r.withdrawal.expected_mission_revision = 0,
        |r: &mut PreparedWithdrawalRequest| r.withdrawal.expected_mission_revision = 2,
        |r: &mut PreparedWithdrawalRequest| r.expected_session_generation = 0,
        |r: &mut PreparedWithdrawalRequest| r.expected_session_generation = -1,
        |r: &mut PreparedWithdrawalRequest| r.session_operation_id = OperationId(Uuid::nil()),
    ] {
        let mut r = request();
        mutate(&mut r);
        cases.push((r, id(6)));
    }
    cases.extend([(request(), OperationId(Uuid::nil())), (request(), id(3))]);
    for (r, op) in cases {
        let mut s = store(Ok(Some(record())));
        assert!(withdraw_prepared(&mut s, None::<&mut Reader>, &r, op, true).is_err());
        assert_eq!(
            s.calls, 0,
            "invalid identity must NOT pass because the port is available"
        );
    }
    assert!(request().validate(id(6)).is_ok());
}

#[test]
fn strict_json_has_outer_and_nested_bounds_without_secret_echo() {
    for raw in [b"".as_slice(), &vec![b' '; 16385]] {
        assert_eq!(
            parse_prepared_withdrawal(raw),
            Err(Fail::Input("size_limit"))
        );
    }
    for raw in [b"{".as_slice(), b"[]", b"null", b"{}"] {
        assert_eq!(
            parse_prepared_withdrawal(raw),
            Err(Fail::Input("invalid_prepared_withdrawal"))
        );
    }
    for path in ["endpoint", "effect", "password", "configuration"] {
        let mut outer = input();
        outer[path] = json!("SYNTHETIC_SECRET_SENTINEL");
        let mut nested = input();
        nested["withdrawal"][path] = json!("SYNTHETIC_SECRET_SENTINEL");
        for value in [outer, nested] {
            let error = parse_prepared_withdrawal(value.to_string().as_bytes()).unwrap_err();
            assert_eq!(error, Fail::Input("invalid_prepared_withdrawal"));
            assert!(!format!("{error:?}").contains("SYNTHETIC_SECRET_SENTINEL"));
        }
    }
    let mut missing = input();
    missing.as_object_mut().unwrap().remove("withdrawal");
    assert!(parse_prepared_withdrawal(missing.to_string().as_bytes()).is_err());
    let mut bad = input();
    bad["withdrawal"]["reason"] = json!("activate");
    assert!(parse_prepared_withdrawal(bad.to_string().as_bytes()).is_err());
    let mut bytes = input().to_string().into_bytes();
    bytes.resize(16384, b' ');
    assert_eq!(parse_prepared_withdrawal(&bytes).unwrap(), request());
    assert_eq!(
        read_prepared_withdrawal_file(std::path::Path::new(
            "missing-SYNTHETIC_SECRET_SENTINEL.json"
        )),
        Err(Fail::Input("unreadable_input"))
    );
}

#[test]
fn original_mission_port_source_is_composed_and_missing_reader_refuses() {
    let mut s = store(Ok(Some(record())));
    assert_eq!(
        withdraw_prepared(&mut s, Some(&mut reader()), &request(), id(6), false),
        Ok(Some(record()))
    );
    assert_eq!(s.source, Some(source()));
    let mut s = store(Ok(None));
    assert_eq!(
        withdraw_prepared(&mut s, None::<&mut Reader>, &request(), id(6), false),
        Err(Fail::Config("session_reader_required"))
    );
    assert_eq!(s.calls, 0);
}

#[test]
fn forged_missing_and_unqualified_source_never_reaches_mutation() {
    let mut forged = source();
    forged.event_id = EventId(Uuid::nil());
    let mut wrong_scope = source();
    wrong_scope.campaign_id = CampaignId(Uuid::from_u128(90));
    let mut wrong_op = source();
    wrong_op.operation_id = id(90);
    let mut raw = serde_json::to_value(source()).unwrap();
    raw["fields"]["operator_ref"] = json!(Uuid::from_u128(90));
    let wrong_owner = serde_json::from_value(raw).unwrap();
    let mut raw = serde_json::to_value(source()).unwrap();
    raw["fields"]
        .as_object_mut()
        .unwrap()
        .remove("m1_permission");
    let no_permission = serde_json::from_value(raw).unwrap();
    for event in [
        None,
        Some(forged),
        Some(wrong_scope),
        Some(wrong_op),
        Some(wrong_owner),
        Some(no_permission),
    ] {
        let mut s = store(Ok(Some(record())));
        let mut r = reader();
        r.event = event;
        assert!(withdraw_prepared(&mut s, Some(&mut r), &request(), id(6), false).is_err());
        assert_eq!(s.calls, 0);
    }
    let mut r = reader();
    r.missing = true;
    assert_eq!(
        withdraw_prepared(&mut store(Ok(None)), Some(&mut r), &request(), id(6), false),
        Err(Fail::State("session_mission_missing"))
    );
}

#[test]
fn mismatched_or_corrupt_record_is_unknown_never_absence_or_rollback() {
    for mutate in [
        |r: &mut PreparedWithdrawalRecord| r.engagement_id = EngagementId(Uuid::from_u128(90)),
        |r: &mut PreparedWithdrawalRecord| r.campaign_id = CampaignId(Uuid::from_u128(90)),
        |r: &mut PreparedWithdrawalRecord| r.operation_id = id(90),
        |r: &mut PreparedWithdrawalRecord| r.session_operation_id = id(90),
        |r: &mut PreparedWithdrawalRecord| r.generation = 2,
        |r: &mut PreparedWithdrawalRecord| r.writer_oid = 0,
        |r: &mut PreparedWithdrawalRecord| r.event.operation_id = id(90),
        |r: &mut PreparedWithdrawalRecord| {
            r.event.request.operator_ref = OperatorRef(Uuid::from_u128(90))
        },
        |r: &mut PreparedWithdrawalRecord| r.event.producer = "forged".into(),
    ] {
        let mut value = record();
        mutate(&mut value);
        assert_eq!(
            withdraw_prepared(
                &mut store(Ok(Some(value))),
                None::<&mut Reader>,
                &request(),
                id(6),
                true
            ),
            Err(Fail::Unresolved("prepared_withdrawal_decode"))
        );
    }
}

#[test]
fn recovery_returns_durable_missing_and_errors_without_source_or_allocation() {
    for reply in [
        Ok(Some(record())),
        Ok(None),
        Err(Fail::Store("serialization_retry")),
        Err(Fail::Unresolved("prepared_withdrawal_unknown")),
    ] {
        let mut s = store(reply.clone());
        let mut never = reader();
        never.forbidden = true;
        assert_eq!(
            withdraw_prepared(&mut s, Some(&mut never), &request(), id(6), true),
            reply
        );
        assert_eq!(s.calls, 1);
        assert!(s.source.is_none());
    }
}

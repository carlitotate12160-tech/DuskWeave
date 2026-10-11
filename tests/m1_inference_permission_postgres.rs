use duskweave::Fail;
use duskweave::mission::*;
use duskweave::registration::{self, MissionStore, ReconcileOutcome, TrajectoryPort};
use duskweave::trajectory::{Delivered, HistoryStatus};
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_prepared_withdrawal.rs"]
mod prepared;
#[path = "support/m1_session.rs"]
mod session_support;

#[test]
fn unpublished_forged_and_cross_scope_first_delivery_remain_unaccepted() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0xa101);
    let (_, ev) = Mission::register(
        &consent::input(e, c),
        OperationId(Uuid::from_u128(3)),
        EventId(Uuid::from_u128(4)),
        5,
    )
    .unwrap();
    let (_, mut s, mut t) = db_support::ports();
    assert_eq!(
        t.deliver(&ev).unwrap(),
        Delivered::Unresolved("missing_committed_publication")
    );
    assert_eq!(
        db_support::count("trajectory.registration_history", e, c),
        0
    );
    let (mission, ev) =
        Mission::register(&consent::input(e, c), ev.operation_id, ev.event_id, 5).unwrap();
    s.commit_registration(&mission, &ev).unwrap();
    let mut forged = ev.clone();
    forged.event_id = EventId(Uuid::from_u128(6));
    assert_eq!(
        t.deliver(&forged).unwrap(),
        Delivered::Unresolved("missing_committed_publication")
    );
    let mut wire = serde_json::to_value(&ev).unwrap();
    wire["fields"]["m1_inference_permission"]["campaign_limits"]["calls"] = json!(11);
    let changed: MissionRegistered = serde_json::from_value(wire).unwrap();
    assert_eq!(
        t.deliver(&changed).unwrap(),
        Delivered::Unresolved("missing_committed_publication")
    );
    let (e2, c2) = db_support::scope(0xa102);
    let (_, cross) =
        Mission::register(&consent::input(e2, c2), ev.operation_id, ev.event_id, 5).unwrap();
    assert_eq!(
        t.deliver(&cross).unwrap(),
        Delivered::Unresolved("missing_committed_publication")
    );
    assert_eq!(
        db_support::count("trajectory.registration_history", e2, c2),
        0
    );
    assert_eq!(
        db_support::count("trajectory.registration_history", e, c),
        0
    );
    assert_eq!(t.deliver(&ev).unwrap(), Delivered::Completed);
    assert_eq!(t.deliver(&ev).unwrap(), Delivered::Duplicate);
    assert_eq!(t.deliver(&changed).unwrap(), Delivered::Anomaly);
    assert_eq!(t.deliver(&ev).unwrap(), Delivered::Anomaly);
    assert_eq!(t.registration_status(&ev).unwrap(), HistoryStatus::Anomaly);
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, ev.operation_id).unwrap(),
        ReconcileOutcome::Conflicted
    );
    consent::assert_rows(e, c, 2);
}

#[test]
fn raw_owner_contract_unknown_or_normalized_content_fails_closed() {
    let _g = db_support::db();
    for (index, pointer, value) in [
        (1, "/unknown", json!(true)),
        (2, "/fields/unknown", json!(true)),
        (3, "/fields/m1_inference_permission/unknown", json!(true)),
        (4, "/fields/m1_permission", Value::Null),
        (5, "/version", json!(1)),
        (6, "/version", json!(2)),
        (
            7,
            "/fields/m1_inference_permission/mission_revision",
            json!(2),
        ),
        (
            8,
            "/fields/m1_inference_permission/campaign_id",
            json!(Uuid::from_u128(99)),
        ),
        (
            9,
            "/fields/m1_inference_permission/permission_id",
            json!("00000000-0000-0000-0000-000000ABCDEF"),
        ),
    ] {
        let (e, c) = db_support::scope(0xa200 + index);
        let ev = consent::committed(e, c);
        let mut wire = serde_json::to_value(&ev).unwrap();
        if pointer == "/unknown" {
            wire["unknown"] = value;
        } else if pointer == "/fields/unknown" {
            wire["fields"]["unknown"] = value;
        } else if pointer.ends_with("/unknown") {
            wire["fields"]["m1_inference_permission"]["unknown"] = value;
        } else if pointer == "/fields/m1_permission" {
            wire["fields"]["m1_permission"] = value;
        } else {
            *wire.pointer_mut(pointer).unwrap() = value;
        }
        consent::admin().execute("UPDATE mission.registration_outbox SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0,&c.0,&wire]).unwrap();
        let (_, mut s, mut t) = db_support::ports();
        assert_eq!(
            s.outbox_event(e, c, ev.operation_id),
            Err(Fail::Store("contract_decode")),
            "{pointer}"
        );
        assert_eq!(
            registration::inspect(&mut s, &mut t, e, c),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(
            registration::reconcile(&mut s, &mut t, e, c, ev.operation_id),
            Err(Fail::Store("contract_decode"))
        );
    }
}

#[test]
fn accepted_scope_identity_conflicts_remain_monotonic_anomalies() {
    let _g = db_support::db();
    for index in 1..=3 {
        let (e, c) = db_support::scope(0xa700 + index);
        let ev = consent::committed(e, c);
        let mut conflict = ev.clone();
        if index != 2 {
            conflict.event_id = EventId(Uuid::from_u128(0xf1));
        }
        if index != 1 {
            conflict.operation_id = OperationId(Uuid::from_u128(0xf2));
            conflict.causation_id = conflict.operation_id;
            conflict.correlation_id = conflict.operation_id;
        }
        let (_, mut s, mut t) = db_support::ports();
        assert_eq!(t.deliver(&conflict).unwrap(), Delivered::Anomaly);
        assert_eq!(t.registration_status(&ev).unwrap(), HistoryStatus::Anomaly);
        assert_eq!(t.deliver(&ev).unwrap(), Delivered::Anomaly);
        assert_eq!(t.deliver(&conflict).unwrap(), Delivered::Anomaly);
        assert_eq!(
            registration::reconcile(&mut s, &mut t, e, c, ev.operation_id).unwrap(),
            ReconcileOutcome::Conflicted
        );
        consent::assert_rows(e, c, 2);
    }
}

#[test]
fn owner_catalog_and_original_row_are_bound_during_decoding() {
    let _g = db_support::db();
    for (index, assignment) in [
        (1, "event_id=gen_random_uuid()"),
        (
            2,
            "contract=jsonb_set(contract,'{operation_id}',to_jsonb(gen_random_uuid()))",
        ),
        (
            3,
            "contract=jsonb_set(contract,'{fields,goal_ref}',to_jsonb(gen_random_uuid()))",
        ),
    ] {
        let (e, c) = db_support::scope(0xa300 + index);
        let ev = consent::committed(e, c);
        let mut admin = consent::admin();
        if index == 3 {
            // Change both parent and attachment: locally valid, but foreign to original row.
            let mut wire = serde_json::to_value(&ev).unwrap();
            wire["fields"]["goal_ref"] = json!(Uuid::from_u128(99));
            wire["fields"]["m1_inference_permission"]["goal_ref"] = json!(Uuid::from_u128(99));
            admin.execute("UPDATE mission.registration_outbox SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
                &[&e.0,&c.0,&wire]).unwrap();
        } else {
            admin.execute(&format!("UPDATE mission.registration_outbox SET {assignment} WHERE engagement_id=$1 AND campaign_id=$2"),
                &[&e.0,&c.0]).unwrap();
        }
        let (_, mut s, mut t) = db_support::ports();
        assert_eq!(
            s.outbox_event(e, c, ev.operation_id),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(
            registration::inspect(&mut s, &mut t, e, c),
            Err(Fail::Store("contract_decode"))
        );
    }
}

#[test]
fn accepted_contract_catalog_completion_corruption_cannot_report_completed() {
    let _g = db_support::db();
    for (index, assignment) in [
        (
            1,
            "contract=jsonb_set(contract,'{fields,m1_inference_permission,campaign_limits,calls}','11')",
        ),
        (2, "operation_id=gen_random_uuid()"),
        (3, "producer='forged'"),
        (4, "obligation='unsupported'"),
        (5, "completed_at=NULL"),
        (6, "contract=jsonb_set(contract,'{version}','1')"),
        (7, "event_id=gen_random_uuid()"),
        (
            8,
            "event_id=gen_random_uuid(),operation_id=gen_random_uuid()",
        ),
    ] {
        let (e, c) = db_support::scope(0xa400 + index);
        let ev = consent::committed(e, c);
        consent::admin().execute(&format!("UPDATE trajectory.registration_history SET {assignment} WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'"),
            &[&e.0,&c.0]).unwrap();
        let (_, mut s, mut t) = db_support::ports();
        assert_eq!(
            t.registration_status(&ev),
            Err(Fail::Store("contract_decode")),
            "{assignment}"
        );
        assert_eq!(
            registration::inspect(&mut s, &mut t, e, c),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(
            registration::reconcile(&mut s, &mut t, e, c, ev.operation_id),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(t.deliver(&ev).unwrap(), Delivered::Anomaly);
        assert_eq!(t.registration_status(&ev).unwrap(), HistoryStatus::Anomaly);
        assert_eq!(
            db_support::count_where(
                "trajectory.registration_history",
                "AND status='accepted'",
                e,
                c
            ),
            1,
            "catalog corruption cannot create a second accepted history"
        );
    }
}

#[test]
fn concurrent_registrations_commit_one_original_and_no_amendment() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0xa500);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let (mut a, mut s, mut t) = db_support::ports();
                let op = registration::prepare_operation(&mut a).unwrap();
                barrier.wait();
                db_support::retry(|| {
                    registration::register(&mut a, &mut s, &mut t, op, &consent::input(e, c))
                })
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let receipt = results.into_iter().find_map(Result::ok).unwrap();
    let (mut a, mut s, mut t) = db_support::ports();
    assert_eq!(
        registration::register(
            &mut a,
            &mut s,
            &mut t,
            receipt.operation_id,
            &consent::input(e, c)
        )
        .unwrap()
        .event_id,
        receipt.event_id
    );
    let new_op = registration::prepare_operation(&mut a).unwrap();
    assert_eq!(
        registration::register(&mut a, &mut s, &mut t, new_op, &consent::input(e, c)),
        Err(Fail::Conflict("already_registered"))
    );
    consent::assert_rows(e, c, 1);
}

#[test]
fn v3_prepared_session_retains_guard_and_stale_snapshot_fence() {
    use duskweave::m1_session::{SessionAction, session};
    use duskweave::postgres_m1_session::PgSessionStore;
    use duskweave::postgres_mission::PgMissionStore;
    use duskweave::withdrawal::WithdrawalStore;
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    for (slot, isolation) in [
        (0xa601, postgres::IsolationLevel::RepeatableRead),
        (0xa602, postgres::IsolationLevel::Serializable),
    ] {
        let (e, c) = db_support::scope(slot);
        let now = session_support::now();
        let mut raw = session_support::registration(e, c, now, 0);
        raw["m1_inference_permission"] =
            consent::registration(e, c)["m1_inference_permission"].clone();
        raw["m1_inference_permission"]["starts_at"] = json!(now - 3600);
        raw["m1_inference_permission"]["ends_at"] = json!(now + 3600);
        let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
        let (mut a, mut s, mut t) = db_support::ports();
        let op = registration::prepare_operation(&mut a).unwrap();
        registration::register(&mut a, &mut s, &mut t, op, &input).unwrap();
        let source = s.outbox_event(e, c, op).unwrap().unwrap();
        assert_eq!(source.version, 3);
        let session_op = session_support::session_op();
        let first = session_support::prepare(e, c, session_op).unwrap().unwrap();
        assert_eq!(first["registration_event_id"], json!(source.event_id));
        let case = prepared::Case {
            e,
            c,
            session: session_op,
            operation: session_support::session_op(),
            generation: 1,
            source: source.clone(),
        };
        let mut alloc = duskweave::postgres_mission::PgAllocator::new(db_support::runtime_client());
        assert!(
            s.withdraw(
                &session_support::withdraw_req(e, c),
                case.operation,
                false,
                &mut alloc
            )
            .is_err(),
            "ordinary writer cannot bypass v3 active prepared session"
        );
        let mut stale = session_support::broker_client();
        let mut tx = stale
            .build_transaction()
            .isolation_level(isolation)
            .start()
            .unwrap();
        tx.query_one("SELECT generation FROM execution.session_fences WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0,&c.0]).unwrap();
        session(
            &mut PgSessionStore::new(session_support::broker_client()),
            None::<&mut PgMissionStore>,
            SessionAction::Release,
            &session_support::req(e, c),
            session_op,
        )
        .unwrap();
        let newer = session_support::session_op();
        session_support::prepare(e, c, newer).unwrap().unwrap();
        let before = case.snapshot();
        let error = prepared::raw(
            &mut tx,
            &case,
            case.operation,
            &case.request(),
            &serde_json::to_value(&source).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.code().map(postgres::error::SqlState::code),
            Some("40001")
        );
        tx.rollback().unwrap();
        assert_eq!(case.snapshot(), before);
        assert!(
            case.submit().is_err(),
            "stale generation cannot withdraw v3 campaign"
        );
        assert_eq!(case.snapshot(), before);
        assert_eq!(s.outbox_event(e, c, op).unwrap().unwrap(), source);
        assert_eq!(
            t.registration_status(&source).unwrap(),
            HistoryStatus::Completed
        );
    }
}

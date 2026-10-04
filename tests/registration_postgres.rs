//! Real disposable-PostgreSQL integration tests for M0A: durable commit,
//! dedup/conflict, anomaly persistence and concurrency. Shared fixture in
//! tests/support/registration_db.rs; absent env config fails loudly.

use duskweave::Fail;
use duskweave::input::parse_register;
use duskweave::mission::*;
use duskweave::registration::{self, MissionStore, ReconcileOutcome, TrajectoryPort};
use duskweave::trajectory::{Delivered, HistoryStatus};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

#[test]
fn registration_lifecycle_idempotency_and_conflicts() {
    let _g = db();
    let (e, c) = scope(0x1000);
    let (mut alloc, mut store, mut traj) = ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let r = reg(&mut alloc, &mut store, &mut traj, op, e, c).unwrap();
    assert_eq!(r.history, HistoryStatus::Completed);
    for t in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(count(t, e, c), 1, "{t}");
    }
    // Same operation + canonical same intent dedups to the committed event.
    let r2 = reg(&mut alloc, &mut store, &mut traj, op, e, c).unwrap();
    assert_eq!(r2.event_id, r.event_id);
    assert_eq!(count("mission.missions", e, c), 1);
    // Changed content under the same operation is an integrity conflict.
    let mut alt_j = reg_json(e, c);
    alt_j["goal_ref"] = serde_json::json!(Uuid::from_u128(0x99));
    let alt = parse_register(&serde_json::to_vec(&alt_j).unwrap()).unwrap();
    assert_eq!(
        registration::register(&mut alloc, &mut store, &mut traj, op, &alt),
        Err(Fail::Conflict("integrity_conflict"))
    );
    // New operation on an occupied campaign is refused, never renewed.
    let op2 = registration::prepare_operation(&mut alloc).unwrap();
    assert!(reg(&mut alloc, &mut store, &mut traj, op2, e, c).is_err());
    assert_eq!(count("mission.missions", e, c), 1);
    // Fresh-process equivalent: new connections see the same records.
    let (mut _a, mut s2, mut t2) = ports();
    let v = registration::inspect(&mut s2, &mut t2, e, c)
        .unwrap()
        .unwrap();
    assert_eq!(v.mission.revision, 1);
    assert_eq!(v.event_id, Some(r.event_id));
    assert_eq!(v.history, HistoryStatus::Completed);
    assert_eq!(
        registration::reconcile(&mut s2, &mut t2, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    // Another scope must never expose this record.
    let (e2, c2) = scope(0x7fff);
    assert!(
        registration::inspect(&mut s2, &mut t2, e2, c2)
            .unwrap()
            .is_none()
    );
    assert!(
        registration::inspect(&mut s2, &mut t2, e, c2)
            .unwrap()
            .is_none()
    );
}

#[test]
fn consumer_conflict_records_append_only_anomaly() {
    let _g = db();
    let (e, c) = scope(0x3000);
    let (mut alloc, mut store, mut traj) = ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let r = reg(&mut alloc, &mut store, &mut traj, op, e, c).unwrap();
    let ev = store.outbox_event(e, c, op).unwrap().unwrap();
    let mut v = serde_json::to_value(&ev).unwrap();
    v["fields"]["authority_revision"] = serde_json::json!(2);
    let ev: MissionRegistered = serde_json::from_value(v).unwrap();
    assert_eq!(traj.deliver(&ev).unwrap(), Delivered::Anomaly);
    assert_eq!(
        traj.status(e, c, r.event_id).unwrap(),
        HistoryStatus::Anomaly
    );
    // Redelivering the original variant cannot resolve the anomaly.
    let orig = store.outbox_event(e, c, op).unwrap().unwrap();
    assert_eq!(traj.deliver(&orig).unwrap(), Delivered::Anomaly);
    assert_eq!(
        registration::reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Conflicted)
    );
    assert_eq!(
        count_where(
            "trajectory.registration_history",
            "AND status='anomaly' AND anomaly_category='conflicting_identity'",
            e,
            c
        ),
        1
    );
    // The accepted history row still exists: anomaly blocks, never erases.
    assert_eq!(
        count_where(
            "trajectory.registration_history",
            "AND status='accepted'",
            e,
            c
        ),
        1
    );
}

#[test]
fn history_rows_preserve_admitted_contract_and_effects() {
    let _g = db();
    let (engagement, campaign) = scope(0x3500);
    let (mut allocator, mut store, mut trajectory) = ports();
    let receipt = accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    let original = store
        .outbox_event(engagement, campaign, receipt.operation_id)
        .unwrap()
        .unwrap();
    let original_contract = serde_json::to_value(&original).unwrap();
    // Full accepted-row snapshot before duplicate/conflicting delivery; every
    // later read uses a fresh connection, so equality also proves what fresh
    // recovery sees is byte-identical.
    let accepted_row = || {
        runtime_client()
            .query_one(
                "SELECT to_jsonb(h) FROM trajectory.registration_history h \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
                &[&engagement.0, &campaign.0],
            )
            .unwrap()
            .get::<_, serde_json::Value>(0)
    };
    let before = accepted_row();
    assert_eq!(trajectory.deliver(&original).unwrap(), Delivered::Duplicate);
    assert_eq!(accepted_row(), before);
    assert_eq!(
        count("trajectory.registration_history", engagement, campaign),
        1
    );

    let mut changed = original_contract.clone();
    changed["fields"]["authority_revision"] = serde_json::json!(2);
    let changed: MissionRegistered = serde_json::from_value(changed).unwrap();
    assert_eq!(trajectory.deliver(&changed).unwrap(), Delivered::Anomaly);
    assert_eq!(trajectory.deliver(&original).unwrap(), Delivered::Anomaly);
    assert_eq!(
        trajectory
            .status(engagement, campaign, receipt.event_id)
            .unwrap(),
        HistoryStatus::Anomaly
    );
    assert_eq!(accepted_row(), before);

    let rows = runtime_client()
        .query(
            "SELECT producer, operation_id, event_id, obligation, status, anomaly_category, \
             contract, completed_at IS NOT NULL, recorded_at = completed_at \
             FROM trajectory.registration_history \
             WHERE engagement_id=$1 AND campaign_id=$2 ORDER BY status",
            &[&engagement.0, &campaign.0],
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        assert_eq!(row.get::<_, &str>(0), PRODUCER);
        assert_eq!(row.get::<_, Uuid>(1), receipt.operation_id.0);
        assert_eq!(row.get::<_, Uuid>(2), receipt.event_id.0);
        assert_eq!(row.get::<_, &str>(3), "trajectory.registration_history.v1");
    }
    assert_eq!(rows[0].get::<_, &str>(4), "accepted");
    assert_eq!(rows[0].get::<_, Option<&str>>(5), None);
    assert_eq!(rows[0].get::<_, serde_json::Value>(6), original_contract);
    assert!(rows[0].get::<_, bool>(7));
    assert!(rows[0].get::<_, Option<bool>>(8).unwrap());
    assert_eq!(rows[1].get::<_, &str>(4), "anomaly");
    assert_eq!(
        rows[1].get::<_, Option<&str>>(5),
        Some("conflicting_identity")
    );
    assert_eq!(rows[1].get::<_, Option<serde_json::Value>>(6), None);
    assert!(!rows[1].get::<_, bool>(7));
    assert_eq!(
        count_where(
            "trajectory.registration_history",
            "AND status='accepted'",
            engagement,
            campaign
        ),
        1
    );
    assert_eq!(accepted_row(), before);
}

#[test]
fn unsupported_contract_stays_unresolved_no_mutation() {
    let _g = db();
    let (e, c) = scope(0x4000);
    let (mut alloc, mut store, mut traj) = ports();
    let op = accepted(&mut alloc, &mut store, &mut traj, e, c).operation_id;
    let mut ev = store.outbox_event(e, c, op).unwrap().unwrap();
    ev.version = 99;
    assert!(matches!(
        traj.deliver(&ev).unwrap(),
        Delivered::Unresolved(_)
    ));
    ev.version = 1;
    ev.producer = "not_mission".into();
    assert!(matches!(
        traj.deliver(&ev).unwrap(),
        Delivered::Unresolved(_)
    ));
    ev.producer = PRODUCER.into();
    ev.owner_revision = 9;
    assert!(matches!(
        traj.deliver(&ev).unwrap(),
        Delivered::Unresolved(_)
    ));
    assert_eq!(
        count_where(
            "trajectory.registration_history",
            "AND status='anomaly'",
            e,
            c
        ),
        0
    );
}

#[test]
fn concurrent_registration_single_winner() {
    let _g = db();
    let (e, c) = scope(0x9000);
    let handles: Vec<_> = (0..2)
        .map(|_| {
            std::thread::spawn(move || {
                let (mut a, mut s, mut t) = ports();
                let op = registration::prepare_operation(&mut a).unwrap();
                retry(|| reg(&mut a, &mut s, &mut t, op, e, c))
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let wins = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        wins, 1,
        "exactly one concurrent registration may win: {results:?}"
    );
    for r in &results {
        if let Err(f) = r {
            assert!(
                matches!(f, Fail::Conflict(_) | Fail::Store("serialization_retry")),
                "unexpected failure: {f:?}"
            );
        }
    }
    assert_eq!(count("mission.missions", e, c), 1);
}

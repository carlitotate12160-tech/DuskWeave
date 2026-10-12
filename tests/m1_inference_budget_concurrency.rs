//! Fixed-snapshot, mutual-exclusion and ordering coverage for the pre-send
//! inference reservation slice on the owned disposable database: concurrent
//! writers produce at most one hold, session and inference owners exclude
//! each other in both directions, withdrawn authority blocks new holds while
//! durable history stays readable. Admin access is fixture setup only;
//! runtime assertions use the restricted logins. Cluster-role fixtures
//! serialize under db().
//!
//! Requires the packet's authorized disposable PostgreSQL service.

use duskweave::m1_inference_budget::{ReservationAction::*, reservation};
use duskweave::m1_session::{SessionAction, session as session_case};
use duskweave::mission::*;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_m1_session::PgSessionStore;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::withdrawal::WithdrawalStore;
use serde_json::json;
use std::sync::Barrier;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;
use session_support::*;
#[path = "support/upgrade_db.rs"]
mod upgrade_db;

#[test]
fn fixed_snapshots_abort_stale_reservation_and_stale_cancellation() {
    let _g = db();
    let mut n = 0xf003u128;
    for isolation in ["REPEATABLE READ", "SERIALIZABLE"] {
        // Stale new reservation: the snapshot is taken before a competing
        // writer's committed hold/epoch change, then the mutation attempt
        // must abort rather than spend capacity it cannot see.
        let (q, source) = budget::fixture(n);
        let request = serde_json::to_value(&q).unwrap();
        let source_v = serde_json::to_value(&source).unwrap();
        let before = budget::state(&q);
        let mut stale = broker_client();
        stale
            .batch_execute(&format!("BEGIN ISOLATION LEVEL {isolation}"))
            .unwrap();
        stale
            .query_one("SELECT count(*) FROM execution.inference_reservations", &[])
            .unwrap();
        // The competing writer commits synchronously before the attempt:
        // observable ordering, no assumed scheduling.
        let first = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
        let error = stale
            .query_one(
                "SELECT to_jsonb(execution.inference_reservation($1,$2,$3,$4))",
                &[&"reserve", &budget::op().0, &request, &source_v],
            )
            .expect_err("stale snapshot must abort");
        assert_eq!(
            error.code().map(postgres::error::SqlState::code),
            Some("40001"),
            "{isolation}: {error}"
        );
        stale.batch_execute("ROLLBACK").unwrap();
        // Only the committed hold and its one epoch increment exist.
        let after = budget::state(&q);
        assert_eq!(after["fence"], budget::fence_after(&before["fence"], 1));
        assert_eq!(after["ledger"].as_array().unwrap().len(), 1);
        assert_eq!(
            after["ledger"][0]["operation_id"],
            first.operation_id.to_string()
        );
        for key in ["mission", "publication", "session_history"] {
            assert_eq!(after[key], before[key], "{isolation}: {key} unchanged");
        }
        n += 0x40;
        // Stale first cancellation: the snapshot still sees a live hold
        // while a competing cancellation already committed the fence epoch.
        // The stale mutation must abort rather than refund from old data.
        let (q2, _s2) = budget::fixture(n);
        let qv2 = serde_json::to_value(&q2).unwrap();
        let held = budget::run(&q2, budget::op(), Reserve).unwrap().unwrap();
        // Same-writer stale transaction: a different session_user would be
        // refused on writer identity before the epoch write, not aborted.
        let mut stale = broker_client();
        stale
            .batch_execute(&format!("BEGIN ISOLATION LEVEL {isolation}"))
            .unwrap();
        stale
            .query_one("SELECT count(*) FROM execution.inference_reservations", &[])
            .unwrap();
        let before = budget::state(&q2);
        // A competing first cancellation commits its epoch bump.
        let cancelled = budget::run(&q2, held.operation_id, Cancel)
            .unwrap()
            .unwrap();
        let error = stale
            .query_one(
                "SELECT to_jsonb(execution.inference_reservation($1,$2,$3,$4))",
                &[
                    &"cancel",
                    &held.operation_id.0,
                    &qv2,
                    &serde_json::Value::Null,
                ],
            )
            .expect_err("stale cancellation must abort");
        assert_eq!(
            error.code().map(postgres::error::SqlState::code),
            Some("40001"),
            "{isolation}: {error}"
        );
        stale.batch_execute("ROLLBACK").unwrap();
        let after = budget::state(&q2);
        assert_eq!(after["fence"], budget::fence_after(&before["fence"], 1));
        assert_eq!(after["ledger"].as_array().unwrap().len(), 2);
        assert_eq!(
            after["ledger"][1],
            serde_json::to_value(&cancelled).unwrap()
        );
        for key in ["mission", "publication", "session_history"] {
            assert_eq!(after[key], before[key], "{isolation}: {key} unchanged");
        }
        n += 0x40;
    }
}

#[test]
fn session_and_inference_owners_exclude_each_other_in_both_directions() {
    let _g = db();
    // Direction 1: an active inference hold blocks session prepare.
    let (q, _source) = budget::fixture(0xf007);
    let record = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    let mut session_store = PgSessionStore::new(broker_client());
    let mut reader = PgMissionStore::new(runtime_client());
    let req = req(q.engagement_id, q.campaign_id);
    let prepare = session_case(
        &mut session_store,
        Some(&mut reader),
        SessionAction::Prepare,
        &req,
        session_op(),
    );
    assert!(
        prepare.is_err(),
        "inference hold must exclude session prepare"
    );
    // Cancelling the hold re-opens session prepare.
    budget::run(&q, record.operation_id, Cancel)
        .unwrap()
        .unwrap();
    let mut session_store = PgSessionStore::new(broker_client());
    let mut reader = PgMissionStore::new(runtime_client());
    let session_id = session_op();
    let prepared = session_case(
        &mut session_store,
        Some(&mut reader),
        SessionAction::Prepare,
        &req,
        session_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(prepared.kind, "prepared_no_effects");
    // Direction 2: the held session fence now blocks a new reservation.
    assert_eq!(
        budget::run(&q, budget::op(), Reserve),
        Err(duskweave::Fail::State("inference_reservation_refused"))
    );
    // Releasing the session re-opens reservation.
    let mut session_store = PgSessionStore::new(broker_client());
    session_case(
        &mut session_store,
        None::<&mut PgMissionStore>,
        SessionAction::Release,
        &req,
        session_id,
    )
    .unwrap();
    assert!(budget::run(&q, budget::op(), Reserve).unwrap().is_some());
}

#[test]
fn two_competing_reservations_produce_exactly_one_winner() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf008);
    let barrier = std::sync::Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let q = q.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                budget::run(&q, budget::op(), Reserve)
            })
        })
        .collect();
    let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let winners = outcomes.iter().filter(|o| matches!(o, Ok(Some(_)))).count();
    assert_eq!(winners, 1, "outcomes: {outcomes:?}");
    // Exactly one durable hold; the loser leaves no partial record.
    let ledger = budget::state(&q)["ledger"].clone();
    assert_eq!(ledger.as_array().unwrap().len(), 1);
}

#[test]
fn campaign_capacity_exhaustion_blocks_the_next_hold() {
    let _g = db();
    session_support::ensure_broker_logins();
    let (e, c) = scope(0xf009);
    let now = session_support::now();
    let mut raw = budget::registration_value(e, c, now);
    raw["m1_inference_permission"]["campaign_limits"]["calls"] = json!(3);
    raw["m1_inference_permission"]["campaign_limits"]["wall_time_ms"] = json!(60000);
    let (_, source) = budget::register(e, c, &raw);
    let q = budget::request(e, c, &raw);
    // Two holds consume calls=2 each only when the campaign allows them;
    // with calls=3 a single hold (2 calls) leaves no room for a second.
    assert!(budget::run(&q, budget::op(), Reserve).unwrap().is_some());
    // The raw boundary reports the capacity door, not merely a busy scope.
    let error = budget::sql(
        &mut broker_client(),
        "reserve",
        &serde_json::to_value(&q).unwrap(),
        budget::op(),
        &serde_json::to_value(&source).unwrap(),
    )
    .expect_err("second hold must refuse");
    assert_eq!(
        error.as_db_error().map(|d| d.message()),
        Some("inference_capacity_exhausted")
    );
    // Cancelling the hold frees the accounted capacity.
    let ledger = budget::state(&q)["ledger"].clone();
    let op = OperationId(ledger[0]["operation_id"].as_str().unwrap().parse().unwrap());
    budget::run(&q, op, Cancel).unwrap().unwrap();
    assert!(budget::run(&q, budget::op(), Reserve).unwrap().is_some());
}

#[test]
fn withdrawn_authority_blocks_new_reservation_but_history_survives() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf00a);
    let held = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    // Withdraw the Mission through the real authority path.
    let mut store = PgMissionStore::new(runtime_client());
    let mut alloc = PgAllocator::new(runtime_client());
    store
        .withdraw(
            &session_support::withdraw_req(q.engagement_id, q.campaign_id),
            session_op(),
            false,
            &mut alloc,
        )
        .unwrap()
        .unwrap();
    // Durable history stays readable: recover returns the held record and
    // cancel still completes the ledger-local lifecycle.
    assert_eq!(
        budget::run(&q, held.operation_id, Recover)
            .unwrap()
            .unwrap(),
        held
    );
    let cancelled = budget::run(&q, held.operation_id, Cancel).unwrap().unwrap();
    assert_eq!(cancelled.kind, "cancelled_never_sent");
    // New reserve is refused: the publication port excludes withdrawn scope.
    assert_eq!(
        budget::run(&q, budget::op(), Reserve),
        Err(duskweave::Fail::State("inference_reservation_refused"))
    );
    // Recovery after the lifecycle still returns the final record.
    assert_eq!(
        budget::run(&q, held.operation_id, Recover)
            .unwrap()
            .unwrap(),
        cancelled
    );
}
#[test]
fn upgrade_fixture_applies_all_migrations_and_reapply_is_idempotent() {
    let _g = db();
    session_support::ensure_broker_logins();
    let owned = upgrade_db::OwnedUpgradeDatabase::create().unwrap();
    {
        let mut admin = upgrade_db::admin_in(&owned.name);
        for sql in [
            MIGRATION,
            PLANNING_MIGRATION,
            HISTORY_MIGRATION,
            V2_MIGRATION,
            WITHDRAWAL_MIGRATION,
            WITHDRAWAL_REFUSAL_MIGRATION,
            ELIGIBILITY_MIGRATION,
            SESSION_MIGRATION,
            PREPARED_WITHDRAWAL_MIGRATION,
        ] {
            admin.batch_execute(sql).unwrap();
        }
        // A scope registered under the pre-0010 schema reserves afterwards.
        let (e, c) = scope(0xf050);
        let raw = budget::registration_value(e, c, session_support::now());
        let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
        let mut alloc = PgAllocator::new(upgrade_db::runtime_in(&owned.name));
        let mut reader = PgMissionStore::new(upgrade_db::runtime_in(&owned.name));
        let mut history = PgTrajectory::new(upgrade_db::runtime_in(&owned.name));
        let op = registration::prepare_operation(&mut alloc).unwrap();
        registration::register(&mut alloc, &mut reader, &mut history, op, &input).unwrap();
        for _ in 0..2 {
            admin
                .batch_execute(INFERENCE_RESERVATION_MIGRATION)
                .unwrap();
        }
        let mut cfg: postgres::Config = session_support::broker_dsn(session_support::db_port())
            .parse()
            .unwrap();
        cfg.dbname(&owned.name);
        let mut store = PgInferenceReservationStore::new(cfg.connect(postgres::NoTls).unwrap());
        let q = budget::request(e, c, &raw);
        let held = reservation(&mut store, Some(&mut reader), Reserve, &q, budget::op())
            .unwrap()
            .unwrap();
        assert_eq!(held.kind, "reserved_not_sent");
        // The upgrade installed table, RLS, triggers, functions and grants.
        let installed: bool = upgrade_db::runtime_in(&owned.name)
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_trigger \
                     WHERE tgname='inference_reservations_no_truncate') \
                 AND EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='inference_owner_fence') \
                 AND EXISTS(SELECT 1 FROM pg_policies \
                     WHERE tablename='inference_reservations') \
                 AND (SELECT relrowsecurity FROM pg_class \
                     WHERE relname='inference_reservations' \
                     AND relnamespace='execution'::regnamespace) \
                 AND has_function_privilege('dw_m1_broker',\
                     'execution.inference_reservation(text,uuid,jsonb,jsonb)','EXECUTE') \
                 AND has_function_privilege('dw_m1_broker',\
                     'mission.current_inference_publication(uuid,uuid,uuid)','EXECUTE')",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(
            installed,
            "0010 must install table+RLS+triggers+functions+grants"
        );
    }
    owned.finish().unwrap();
}

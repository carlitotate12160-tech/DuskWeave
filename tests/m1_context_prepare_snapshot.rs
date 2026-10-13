//! Real concurrency, barrier-synchronized races, and Repeatable Read snapshot honesty.
use duskweave::m1_context::prepare;
use duskweave::m1_context_input::ContextRequest;
use duskweave::m1_inference_budget::ReservationAction::*;
use duskweave::postgres_mission::PgContextReader;
use serde_json::Value;
use std::sync::{Arc, Barrier};
use std::time::Duration;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/m1_context_fault.rs"]
mod context_fault;
#[path = "support/m1_context_prepare.rs"]
mod context_support;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;

#[test]
fn overlapping_concurrent_readers_produce_distinct_coherent_decisions() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9601);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);

    let barrier = Arc::new(Barrier::new(3));
    let q = Arc::new(q);

    let t1 = {
        let barrier = barrier.clone();
        let q = q.clone();
        std::thread::spawn(move || {
            let op = budget::op();
            let mut reader = PgContextReader::new(session_support::broker_client());
            barrier.wait();
            prepare(&mut reader, &q, op).unwrap()
        })
    };

    let t2 = {
        let barrier = barrier.clone();
        let q = q.clone();
        std::thread::spawn(move || {
            let op = budget::op();
            let mut reader = PgContextReader::new(session_support::broker_client());
            barrier.wait();
            prepare(&mut reader, &q, op).unwrap()
        })
    };

    barrier.wait();
    let prep1 = t1.join().unwrap();
    let prep2 = t2.join().unwrap();

    let after_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(
        before_state, after_state,
        "concurrent reads must not mutate scoped state"
    );

    let p1: Value = serde_json::from_str(prep1.utf8()).unwrap();
    let p2: Value = serde_json::from_str(prep2.utf8()).unwrap();

    assert_eq!(p1["version"], "m1_context_pack_v1");
    assert_eq!(p2["version"], "m1_context_pack_v1");
    assert_eq!(p1["question"], p2["question"]);
    assert_eq!(p1["remaining_capacity"], p2["remaining_capacity"]);
    assert_ne!(
        p1["decision_id"], p2["decision_id"],
        "each preparation has its own distinct allocated decision ID"
    );
}

#[test]
fn snapshot_reader_holds_coherent_view_across_independent_reserve_race() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9602);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);

    // Install capacity barrier hook
    let mut fault =
        context_fault::CapacityBarrierFault::install(session_support::admin_db_client());

    let (q_arc, req_arc) = (Arc::new(q), Arc::new(req.clone()));
    let reader_thread = {
        let q = q_arc.clone();
        std::thread::spawn(move || {
            let op = budget::op();
            let mut reader = PgContextReader::new(session_support::broker_client());
            prepare(&mut reader, &q, op).unwrap()
        })
    };

    // Verify reader is blocked on the capacity advisory barrier
    assert!(
        fault.wait_blocked(Duration::from_secs(10)),
        "reader must block on barrier"
    );

    // Concurrently commit a real reservation while reader is blocked in its transaction
    let res_op = budget::op();
    let held = budget::run(&req_arc, res_op, Reserve)
        .unwrap()
        .expect("reservation must succeed");
    assert_eq!(held.operation_id, res_op);

    let post_mutation_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_ne!(before_state, post_mutation_state);

    // Release the barrier so reader unblocks and completes
    fault.release();
    let prep = reader_thread.join().unwrap();
    drop(fault);

    // State after reader finishes equals post_mutation_state (prepare does not mutate)
    let final_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(
        post_mutation_state, final_state,
        "prepare itself leaves scoped state unchanged"
    );

    let p: Value = serde_json::from_str(prep.utf8()).unwrap();
    // In Repeatable Read, reader snapshot was established before reserve committed!
    // So its capacity view remains coherent as-of-snapshot (zero holds charged).
    assert_eq!(p["remaining_capacity"]["campaign"]["calls"], 10);
    assert_eq!(p["remaining_capacity"]["campaign"]["request_bytes"], 100000);
    assert_eq!(
        p["remaining_capacity"]["campaign"]["response_bytes"],
        500000
    );
    assert_eq!(
        p["remaining_capacity"]["campaign"]["output_token_capacity"],
        20000
    );
    assert_eq!(p["remaining_capacity"]["campaign"]["wall_time_ms"], 100000);
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], false);

    // A fresh reader sees the newly charged capacity
    let mut fresh_reader = PgContextReader::new(session_support::broker_client());
    let fresh_prep = prepare(&mut fresh_reader, &q_arc, budget::op()).unwrap();
    let fresh_p: Value = serde_json::from_str(fresh_prep.utf8()).unwrap();
    assert_eq!(fresh_p["remaining_capacity"]["campaign"]["calls"], 8);
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["request_bytes"],
        100000 - 32768
    );
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["response_bytes"],
        500000 - 131072
    );
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["output_token_capacity"],
        20000 - 8192
    );
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["wall_time_ms"],
        100000 - 20000
    );
    assert_eq!(fresh_p["remaining_capacity"]["exclusive_owner_busy"], true);
}

#[test]
fn snapshot_reader_holds_coherent_view_across_cancel_and_withdrawal_races() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9603);

    // Pre-reserve an operation
    let res_op = budget::op();
    let _held = budget::run(&req, res_op, Reserve).unwrap().unwrap();

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let mut fault =
        context_fault::CapacityBarrierFault::install(session_support::admin_db_client());
    let q_arc = Arc::new(q);
    let req_arc = Arc::new(req.clone());

    let reader_thread = {
        let q = q_arc.clone();
        std::thread::spawn(move || {
            let op = budget::op();
            let mut reader = PgContextReader::new(session_support::broker_client());
            prepare(&mut reader, &q, op).unwrap()
        })
    };

    assert!(fault.wait_blocked(Duration::from_secs(10)));

    // Concurrently cancel the reservation while reader is paused
    let cancelled = budget::run(&req_arc, res_op, Cancel)
        .unwrap()
        .expect("cancel succeeds");
    assert_eq!(cancelled.record_revision, 2);

    fault.release();
    let prep = reader_thread.join().unwrap();
    drop(fault);

    let p: Value = serde_json::from_str(prep.utf8()).unwrap();
    // Paused reader sees capacity as of its snapshot
    assert_eq!(p["version"], "m1_context_pack_v1");
    assert_eq!(p["remaining_capacity"]["campaign"]["calls"], 8);
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);

    // Fresh reader sees release of holds across all 5 dimensions
    let mut fresh_reader = PgContextReader::new(session_support::broker_client());
    let fresh_prep = prepare(&mut fresh_reader, &q_arc, budget::op()).unwrap();
    let fresh_p: Value = serde_json::from_str(fresh_prep.utf8()).unwrap();
    assert_eq!(fresh_p["remaining_capacity"]["campaign"]["calls"], 10);
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["request_bytes"],
        100000
    );
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["response_bytes"],
        500000
    );
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["output_token_capacity"],
        20000
    );
    assert_eq!(
        fresh_p["remaining_capacity"]["campaign"]["wall_time_ms"],
        100000
    );
    assert_eq!(fresh_p["remaining_capacity"]["exclusive_owner_busy"], false);
}

#[test]
fn real_cli_snapshot_path_across_concurrent_withdrawal_race() {
    use duskweave::withdrawal::WithdrawalStore;

    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9604);
    let req_val = context_support::request(&serde_json::to_value(&req).unwrap(), &[]);
    let broker_dsn = session_support::broker_dsn(session_support::db_port());

    let mut fault =
        context_fault::CapacityBarrierFault::install(session_support::admin_db_client());

    let input = wait_db::InputFile::new(&req_val, "cli_race");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let (child, _) = wait_db::spawn_piped(&mut wait_db::cli(&args, &broker_dsn));
    assert!(
        fault.wait_blocked(Duration::from_secs(10)),
        "CLI subprocess must reach capacity snapshot barrier"
    );

    // While CLI is blocked, withdraw the mission through owner API
    let mut store = duskweave::postgres_mission::PgMissionStore::new(db_support::runtime_client());
    let mut alloc = duskweave::postgres_mission::PgAllocator::new(db_support::runtime_client());
    let withdraw_req = duskweave::withdrawal::WithdrawalRequest {
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        operator_ref: req.operator_ref,
        expected_mission_revision: 1,
        reason: duskweave::withdrawal::WithdrawalReason::OperatorRequested,
    };
    let w_res = store.withdraw(&withdraw_req, budget::op(), false, &mut alloc);
    assert!(w_res.is_ok(), "withdrawal must succeed: {w_res:?}");

    // Release the barrier so CLI completes
    fault.release();
    let out = child.wait_with_output().unwrap();
    drop(fault);

    // CLI had established Repeatable Read snapshot before withdrawal, so its receipt succeeds
    assert!(
        out.status.success(),
        "CLI snapshot established before withdrawal completes: {out:?}"
    );
    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "prepared");

    // But a fresh CLI started after withdrawal cannot prepare!
    let out_fresh = context_support::run(&args, &broker_dsn);
    assert!(
        !out_fresh.status.success(),
        "fresh CLI after withdrawal must fail"
    );
    let r_fresh = context_support::receipt(&out_fresh);
    assert!(
        r_fresh["outcome"] == "refused" || r_fresh["outcome"] == "deferred",
        "withdrawn mission cannot produce prepared pack"
    );
    assert!(r_fresh["pack_utf8"].is_null());
}

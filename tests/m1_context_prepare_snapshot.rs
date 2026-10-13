//! Real concurrency, barrier-synchronized races, and Repeatable Read snapshot honesty.
use duskweave::m1_context::{ContextReader, prepare};
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
    assert_ne!(p1["decision_id"], p2["decision_id"]);
}

#[test]
fn snapshot_reader_holds_coherent_view_across_independent_reserve_race() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, source) = budget::fixture(0x9602);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    let _pre_fence = session_support::fence(req.engagement_id, req.campaign_id).unwrap();

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

    assert!(fault.wait_blocked(Duration::from_secs(10)));

    let res_op = budget::op();
    let held = budget::run(&req_arc, res_op, Reserve)
        .unwrap()
        .expect("reservation must succeed");
    assert_eq!(held.operation_id, res_op);

    let _post_fence = session_support::fence(req.engagement_id, req.campaign_id).unwrap();
    let post_mutation_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_ne!(before_state, post_mutation_state);

    fault.release();
    let prep = reader_thread.join().unwrap();
    fault.finish().expect("restoration check must succeed");

    let final_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(post_mutation_state, final_state);

    let p: Value = serde_json::from_str(prep.utf8()).unwrap();
    assert_eq!(
        p["remaining_capacity"]["snapshot_epoch"],
        before_state["fence"]["epoch"]
    );
    context_support::assert_capacity_remainder(&p, [10, 100000, 500000, 20000, 100000]);
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], false);
    assert_eq!(p["owner_views"][0]["revision"], 1);
    assert_eq!(
        p["owner_views"][0]["frontier"],
        serde_json::to_value(source.event_id).unwrap()
    );

    let mut fresh_reader = PgContextReader::new(session_support::broker_client());
    let fresh_prep = prepare(&mut fresh_reader, &q_arc, budget::op()).unwrap();
    let fresh_p: Value = serde_json::from_str(fresh_prep.utf8()).unwrap();
    assert_eq!(
        fresh_p["remaining_capacity"]["snapshot_epoch"],
        post_mutation_state["fence"]["epoch"]
    );
    context_support::assert_capacity_remainder(
        &fresh_p,
        [
            8,
            100000 - 32768,
            500000 - 131072,
            20000 - 8192,
            100000 - 20000,
        ],
    );
    assert_eq!(fresh_p["remaining_capacity"]["exclusive_owner_busy"], true);
}

#[test]
fn snapshot_reader_holds_coherent_view_across_cancel_and_withdrawal_races() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9603);

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

    let cancelled = budget::run(&req_arc, res_op, Cancel)
        .unwrap()
        .expect("cancel succeeds");
    assert_eq!(cancelled.record_revision, 2);
    let post_cancel_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);

    fault.release();
    let prep = reader_thread.join().unwrap();
    fault.finish().expect("restoration check must succeed");

    let state_after = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(post_cancel_state, state_after);

    let p: Value = serde_json::from_str(prep.utf8()).unwrap();
    context_support::assert_capacity_remainder(
        &p,
        [
            8,
            100000 - 32768,
            500000 - 131072,
            20000 - 8192,
            100000 - 20000,
        ],
    );
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);

    let mut fresh_reader = PgContextReader::new(session_support::broker_client());
    let fresh_prep = prepare(&mut fresh_reader, &q_arc, budget::op()).unwrap();
    let fresh_p: Value = serde_json::from_str(fresh_prep.utf8()).unwrap();
    context_support::assert_capacity_remainder(&fresh_p, [10, 100000, 500000, 20000, 100000]);
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
    assert!(fault.wait_blocked(Duration::from_secs(10)));

    let mut store = duskweave::postgres_mission::PgMissionStore::new(db_support::runtime_client());
    let mut alloc = duskweave::postgres_mission::PgAllocator::new(db_support::runtime_client());
    let withdraw_req = duskweave::withdrawal::WithdrawalRequest {
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        operator_ref: req.operator_ref,
        expected_mission_revision: 1,
        reason: duskweave::withdrawal::WithdrawalReason::OperatorRequested,
    };
    store
        .withdraw(&withdraw_req, budget::op(), false, &mut alloc)
        .unwrap();
    let post_withdrawal_state =
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);

    fault.release();
    let out = child.wait_with_output().unwrap();
    fault.finish().expect("restoration check must succeed");

    assert_eq!(
        post_withdrawal_state,
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
    );
    assert!(out.status.success());
    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "prepared");
    let pack: Value = serde_json::from_str(r["pack_utf8"].as_str().unwrap()).unwrap();
    assert_eq!(pack["owner_views"][0]["revision"], 1);
    assert_eq!(pack["references"][0]["status"], "owner_published");
    assert_eq!(pack["references"][0]["freshness"], "as_of_snapshot");
    context_support::assert_false_flags(&r);

    let out_fresh = context_support::run(&args, &broker_dsn);
    assert!(!out_fresh.status.success());
    let r_fresh = context_support::receipt(&out_fresh);
    assert!(r_fresh["outcome"] == "refused" || r_fresh["outcome"] == "deferred");
    assert!(r_fresh["pack_utf8"].is_null());
    context_support::assert_false_flags(&r_fresh);
    assert_eq!(
        post_withdrawal_state,
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
    );
}

#[test]
fn smaller_positive_limits_reservation_charges_and_remainder() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = context_support::smaller_limits_fixture(0x9605);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let held = budget::run(&req, budget::op(), Reserve)
        .unwrap()
        .expect("reserve succeeds");
    assert_eq!(held.capacity.calls, 2);
    assert_eq!(held.capacity.request_bytes, 2048);
    assert_eq!(held.capacity.response_bytes, 4096);
    assert_eq!(held.capacity.output_token_capacity, 8192);
    assert_eq!(held.capacity.wall_time_ms, 5000);

    let mut reader = PgContextReader::new(session_support::broker_client());
    let prep = prepare(&mut reader, &q, budget::op()).unwrap();
    let p: Value = serde_json::from_str(prep.utf8()).unwrap();
    let mut direct_reader = PgContextReader::new(session_support::broker_client());
    let basis = direct_reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis.capacity.active_holds, 1);
    assert_eq!(basis.capacity.charged.calls, 2);
    assert_eq!(basis.capacity.charged.request_bytes, 2048);
    assert_eq!(basis.capacity.charged.response_bytes, 4096);
    assert_eq!(basis.capacity.charged.output_token_capacity, 8192);
    assert_eq!(basis.capacity.charged.wall_time_ms, 5000);
    let req_dims = &p["remaining_capacity"]["required_for_full_decision"];
    assert_eq!(req_dims["calls"], 2);
    assert_eq!(req_dims["request_bytes"], 2048);
    assert_eq!(req_dims["response_bytes"], 4096);
    assert_eq!(req_dims["output_token_capacity"], 8192);
    assert_eq!(req_dims["wall_time_ms"], 5000);
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);
    context_support::assert_capacity_remainder(
        &p,
        [
            10 - 2,
            100000 - 2048,
            500000 - 4096,
            20000 - 8192,
            100000 - 5000,
        ],
    );
}

#[test]
fn busy_session_exposes_snapshot_without_charge_or_send_permission() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9606);
    let raw = budget::registration_value(e, c, session_support::now());
    let (_op, _source) = budget::register(e, c, &raw);
    let s_op = session_support::session_op();
    assert!(session_support::prepare(e, c, s_op).unwrap().is_some());

    let req = budget::request(e, c, &raw);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![],
    };

    let before_state = context_support::scoped_state(e.0, c.0);
    let mut reader = PgContextReader::new(session_support::broker_client());
    let prep = prepare(&mut reader, &q, budget::op()).unwrap();
    let p: Value = serde_json::from_str(prep.utf8()).unwrap();

    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);
    context_support::assert_capacity_remainder(&p, [10, 100000, 500000, 20000, 100000]);
    let mut direct_reader = PgContextReader::new(session_support::broker_client());
    let basis = direct_reader.read(&q, budget::op()).unwrap();
    assert!(basis.capacity.session_claimed);
    assert_eq!(basis.capacity.charged.calls, 0);
    assert_eq!(before_state, context_support::scoped_state(e.0, c.0));

    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let input = wait_db::InputFile::new(
        &context_support::request(&serde_json::to_value(&q.mission).unwrap(), &[]),
        "busy_cli",
    );
    let out = context_support::run(
        &[
            "m1-context-prepare".into(),
            "--input".into(),
            input.path().into(),
        ],
        &broker_dsn,
    );
    assert!(out.status.success());
    let r = context_support::receipt(&out);
    context_support::assert_false_flags(&r);
}

#[test]
fn new_generation_reservation_charges_second_active_hold_cleanly() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9607);
    let op1 = budget::op();
    let h1 = budget::run(&req, op1, Reserve).unwrap().unwrap();
    assert_eq!(h1.generation, 1);
    let c1 = budget::run(&req, op1, Cancel).unwrap().unwrap();
    assert_eq!(c1.record_revision, 2);

    let op2 = budget::op();
    let h2 = budget::run(&req, op2, Reserve).unwrap().unwrap();
    assert_ne!(h1.operation_id, h2.operation_id);
    assert_eq!(h2.generation, 2);

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    let mut reader = PgContextReader::new(session_support::broker_client());
    let prep = prepare(&mut reader, &q, budget::op()).unwrap();
    let p: Value = serde_json::from_str(prep.utf8()).unwrap();

    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);
    context_support::assert_capacity_remainder(
        &p,
        [
            8,
            100000 - 32768,
            500000 - 131072,
            20000 - 8192,
            100000 - 20000,
        ],
    );

    let mut direct_reader = PgContextReader::new(session_support::broker_client());
    let basis = direct_reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis.capacity.active_holds, 1);
    assert_eq!(basis.capacity.charged.calls, 2);

    assert_eq!(
        before_state,
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
    );
}

#[test]
fn capacity_barrier_fault_unwind_preserves_restoration_and_restricted_read() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9608);

    let panic_res = std::panic::catch_unwind(|| {
        let _fault =
            context_fault::CapacityBarrierFault::install(session_support::admin_db_client());
        panic!("simulated panic to verify unwind teardown");
    });
    assert!(panic_res.is_err(), "must catch panic");

    let mut fresh_admin = session_support::admin_db_client();
    let snap = context_fault::capture_metadata(&mut fresh_admin).expect("metadata captures ok");
    assert_eq!(snap.lang, "sql");
    assert_eq!(snap.volatile, "s");
    assert!(!snap.secdef);

    let alias_exists: bool = fresh_admin
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
             WHERE n.nspname = 'execution' AND p.proname = 'dw_context_capacity_fixture')",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(!alias_exists, "alias must be absent after unwind drop");

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![],
    };
    let mut broker_reader = PgContextReader::new(session_support::broker_client());
    assert!(
        prepare(&mut broker_reader, &q, budget::op()).is_ok(),
        "restricted reader must work normally after unwind teardown"
    );
}

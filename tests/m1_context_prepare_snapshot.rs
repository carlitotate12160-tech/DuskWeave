//! Real concurrency, barrier-synchronized races, and Repeatable Read snapshot honesty.
use duskweave::m1_context::{ContextReader, prepare};
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

const FULL: [i64; 5] = [2, 32768, 131072, 8192, 20000];
const TOTALS: [i64; 5] = [10, 100000, 500000, 20000, 100000];
const REMAINDER: [i64; 5] = [8, 67232, 368928, 11808, 80000];

#[test]
fn overlapping_concurrent_readers_produce_distinct_coherent_decisions() {
    let _g = db_support::db();
    let (req, _) = budget::fixture(0x9601);
    let q = Arc::new(context_support::context_request(&req));
    let before = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    let barrier = Arc::new(Barrier::new(3));
    let readers: Vec<_> = (0..2)
        .map(|_| {
            let q = q.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let op = budget::op();
                let mut reader = PgContextReader::new(session_support::broker_client());
                barrier.wait();
                prepare(&mut reader, &q, op).unwrap()
            })
        })
        .collect();
    barrier.wait();
    let packs: Vec<Value> = readers
        .into_iter()
        .map(|reader| {
            let prepared = context_fault::join_bounded(reader, Duration::from_secs(10));
            serde_json::from_str(prepared.utf8()).unwrap()
        })
        .collect();
    assert_eq!(
        before,
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
    );
    assert_eq!(packs[0]["version"], "m1_context_pack_v1");
    assert_eq!(packs[1]["version"], "m1_context_pack_v1");
    assert_eq!(packs[0]["question"], packs[1]["question"]);
    assert_eq!(
        packs[0]["remaining_capacity"],
        packs[1]["remaining_capacity"]
    );
    assert_ne!(packs[0]["decision_id"], packs[1]["decision_id"]);
}

#[test]
fn snapshot_reader_holds_coherent_view_across_independent_reserve_race() {
    let _g = db_support::db();
    let (req, source) = budget::fixture(0x9602);
    let q = Arc::new(context_support::context_request(&req));
    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let before = context_support::scoped_state(e, c);
    let mut fault =
        context_fault::CapacityBarrierFault::install(session_support::admin_db_client());
    let reader_thread = {
        let q = q.clone();
        std::thread::spawn(move || {
            let mut reader = PgContextReader::new(session_support::broker_client());
            prepare(&mut reader, &q, budget::op()).unwrap()
        })
    };
    assert!(fault.wait_blocked(Duration::from_secs(10)));
    let res_op = budget::op();
    let held = budget::run(&req, res_op, Reserve).unwrap().unwrap();
    assert_eq!(held.operation_id, res_op);
    context_support::assert_charge(&held.capacity, FULL);
    let committed = context_support::scoped_state(e, c);
    assert_ne!(before, committed);
    fault.release().expect("checked barrier release");
    let prep = context_fault::join_bounded(reader_thread, Duration::from_secs(10));
    fault.finish().expect("restoration check must succeed");
    assert_eq!(committed, context_support::scoped_state(e, c));
    let p: Value = serde_json::from_str(prep.utf8()).unwrap();
    assert_eq!(
        p["remaining_capacity"]["snapshot_epoch"],
        before["fence"]["epoch"]
    );
    context_support::assert_capacity_remainder(&p, TOTALS);
    context_support::assert_dimensions(
        &p["remaining_capacity"]["required_for_full_decision"],
        FULL,
    );
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], false);
    context_support::assert_mission_frontier(&p, &source);
    let mut fresh_reader = PgContextReader::new(session_support::broker_client());
    let basis = fresh_reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis.capacity.active_holds, 1);
    assert!(!basis.capacity.session_claimed);
    context_support::assert_charge(&basis.capacity.charged, FULL);
    let fresh = context_support::prepare_pack(&q);
    assert_eq!(
        fresh["remaining_capacity"]["snapshot_epoch"],
        committed["fence"]["epoch"]
    );
    context_support::assert_capacity_remainder(&fresh, REMAINDER);
    context_support::assert_dimensions(
        &fresh["remaining_capacity"]["required_for_full_decision"],
        FULL,
    );
    assert_eq!(fresh["remaining_capacity"]["exclusive_owner_busy"], true);
    context_support::assert_mission_frontier(&fresh, &source);
    assert_eq!(committed, context_support::scoped_state(e, c));
}

#[test]
fn snapshot_reader_holds_coherent_view_across_independent_cancel_race() {
    let _g = db_support::db();
    let (req, source) = budget::fixture(0x9603);
    let res_op = budget::op();
    let held = budget::run(&req, res_op, Reserve).unwrap().unwrap();
    context_support::assert_charge(&held.capacity, FULL);
    let q = Arc::new(context_support::context_request(&req));
    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let pre_cancel = context_support::scoped_state(e, c);
    let mut fault =
        context_fault::CapacityBarrierFault::install(session_support::admin_db_client());
    let reader_thread = {
        let q = q.clone();
        std::thread::spawn(move || {
            let mut reader = PgContextReader::new(session_support::broker_client());
            prepare(&mut reader, &q, budget::op()).unwrap()
        })
    };
    assert!(fault.wait_blocked(Duration::from_secs(10)));
    let cancelled = budget::run(&req, res_op, Cancel).unwrap().unwrap();
    assert_eq!(cancelled.record_revision, 2);
    assert_eq!(cancelled.kind, "cancelled_never_sent");
    context_support::assert_charge(&cancelled.capacity, FULL);
    let post_cancel = context_support::scoped_state(e, c);
    assert_ne!(pre_cancel["fence"]["epoch"], post_cancel["fence"]["epoch"]);
    fault.release().expect("checked barrier release");
    let prep = context_fault::join_bounded(reader_thread, Duration::from_secs(10));
    fault.finish().expect("restoration check must succeed");
    assert_eq!(post_cancel, context_support::scoped_state(e, c));
    let p: Value = serde_json::from_str(prep.utf8()).unwrap();
    assert_eq!(
        p["remaining_capacity"]["snapshot_epoch"],
        pre_cancel["fence"]["epoch"]
    );
    context_support::assert_capacity_remainder(&p, REMAINDER);
    context_support::assert_dimensions(
        &p["remaining_capacity"]["required_for_full_decision"],
        FULL,
    );
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);
    context_support::assert_mission_frontier(&p, &source);
    let mut fresh_reader = PgContextReader::new(session_support::broker_client());
    let basis = fresh_reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis.capacity.active_holds, 0);
    assert!(!basis.capacity.session_claimed);
    context_support::assert_charge(&basis.capacity.charged, [0; 5]);
    let fresh = context_support::prepare_pack(&q);
    assert_eq!(
        fresh["remaining_capacity"]["snapshot_epoch"],
        post_cancel["fence"]["epoch"]
    );
    context_support::assert_capacity_remainder(&fresh, TOTALS);
    context_support::assert_dimensions(
        &fresh["remaining_capacity"]["required_for_full_decision"],
        FULL,
    );
    assert_eq!(fresh["remaining_capacity"]["exclusive_owner_busy"], false);
    context_support::assert_mission_frontier(&fresh, &source);
    assert_eq!(post_cancel, context_support::scoped_state(e, c));
}

#[test]
fn real_cli_snapshot_path_across_concurrent_withdrawal_race() {
    use duskweave::withdrawal::WithdrawalStore;
    let _g = db_support::db();
    let (req, source) = budget::fixture(0x9604);
    let raw = context_support::request(&serde_json::to_value(&req).unwrap(), &[]);
    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let mut fault =
        context_fault::CapacityBarrierFault::install(session_support::admin_db_client());
    let input = wait_db::InputFile::new(&raw, "cli_race");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];
    let (child, started) = wait_db::spawn_piped(&mut wait_db::cli(&args, &broker_dsn));
    assert!(fault.wait_blocked(Duration::from_secs(10)));
    let mut store = duskweave::postgres_mission::PgMissionStore::new(db_support::runtime_client());
    let mut alloc = duskweave::postgres_mission::PgAllocator::new(db_support::runtime_client());
    store
        .withdraw(
            &session_support::withdraw_req(req.engagement_id, req.campaign_id),
            budget::op(),
            false,
            &mut alloc,
        )
        .unwrap();
    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let committed = context_support::scoped_state(e, c);
    fault.release().expect("checked barrier release");
    let out = wait_db::wait_bounded(child, Duration::from_secs(15), started).0;
    fault.finish().expect("restoration check must succeed");
    assert_eq!(committed, context_support::scoped_state(e, c));
    assert!(out.status.success());
    let receipt = context_support::receipt(&out);
    assert_eq!(receipt["outcome"], "prepared");
    let pack: Value = serde_json::from_str(receipt["pack_utf8"].as_str().unwrap()).unwrap();
    context_support::assert_mission_frontier(&pack, &source);
    assert_eq!(pack["references"][0]["status"], "owner_published");
    assert_eq!(pack["references"][0]["freshness"], "as_of_snapshot");
    context_support::assert_false_flags(&receipt);
    let out_fresh = context_support::run(&args, &broker_dsn);
    context_support::assert_failed_receipt(&out_fresh, "deferred");
    assert_eq!(committed, context_support::scoped_state(e, c));
}

#[test]
fn capacity_barrier_fault_unwind_preserves_restoration_and_restricted_read() {
    let _g = db_support::db();
    let (req, _) = budget::fixture(0x9608);
    let mut before_admin = session_support::admin_db_client();
    let before = context_fault::capture_metadata(&mut before_admin).unwrap();
    drop(before_admin);
    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    let panic_res = std::panic::catch_unwind(|| {
        let _fault =
            context_fault::CapacityBarrierFault::install(session_support::admin_db_client());
        panic!("simulated panic to verify unwind teardown");
    });
    assert!(panic_res.is_err(), "must catch panic");
    let mut fresh_admin = session_support::admin_db_client();
    let snap = context_fault::capture_metadata(&mut fresh_admin).expect("metadata captures ok");
    assert_eq!(snap, before);
    let alias_exists: bool = fresh_admin.query_one(
        "SELECT to_regprocedure('execution.dw_context_capacity_fixture(uuid,uuid)') IS NOT NULL",
        &[],
    ).unwrap().get(0);
    assert!(!alias_exists, "alias must be absent after unwind drop");
    let q = context_support::context_request(&req);
    let mut broker_reader = PgContextReader::new(session_support::broker_client());
    assert!(
        prepare(&mut broker_reader, &q, budget::op()).is_ok(),
        "restricted reader must work normally after unwind teardown"
    );
    assert_eq!(
        before_state,
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
    );
}

use duskweave::m1_context::ContextReader;
use duskweave::m1_inference_budget::ReservationAction::*;
use duskweave::postgres_mission::PgContextReader;
use serde_json::{Value, json};

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
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
fn smaller_positive_limits_reservation_charges_and_remainder() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, source) = context_support::smaller_limits_fixture(0x9605);
    let q = context_support::context_request(&req);
    let held = budget::run(&req, budget::op(), Reserve).unwrap().unwrap();
    let expected = [2, 2048, 4096, 8192, 5000];
    context_support::assert_charge(&held.capacity, expected);
    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let before = context_support::scoped_state(e, c);
    let mut reader = PgContextReader::new(session_support::broker_client());
    let basis = reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis.capacity.active_holds, 1);
    assert!(!basis.capacity.session_claimed);
    assert_eq!(json!(basis.capacity.epoch), before["fence"]["epoch"]);
    context_support::assert_charge(&basis.capacity.charged, expected);
    assert_eq!(before, context_support::scoped_state(e, c));
    let p = context_support::prepare_pack(&q);
    assert_eq!(
        p["remaining_capacity"]["snapshot_epoch"],
        before["fence"]["epoch"]
    );
    context_support::assert_dimensions(
        &p["remaining_capacity"]["required_for_full_decision"],
        expected,
    );
    context_support::assert_capacity_remainder(&p, [8, 97952, 495904, 11808, 95000]);
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);
    context_support::assert_mission_frontier(&p, &source);
    assert_eq!(before, context_support::scoped_state(e, c));
}

#[test]
fn busy_session_exposes_snapshot_without_charge_or_send_permission() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9606);
    let raw = budget::registration_value(e, c, session_support::now());
    let (_, source) = budget::register(e, c, &raw);
    assert!(
        session_support::prepare(e, c, session_support::session_op())
            .unwrap()
            .is_some()
    );
    let req = budget::request(e, c, &raw);
    let q = context_support::context_request(&req);
    let before = context_support::scoped_state(e.0, c.0);
    let mut reader = PgContextReader::new(session_support::broker_client());
    let basis = reader.read(&q, budget::op()).unwrap();
    assert!(basis.capacity.session_claimed);
    assert_eq!(basis.capacity.active_holds, 0);
    context_support::assert_charge(&basis.capacity.charged, [0; 5]);
    assert_eq!(json!(basis.capacity.epoch), before["fence"]["epoch"]);
    assert_eq!(before, context_support::scoped_state(e.0, c.0));
    let p = context_support::prepare_pack(&q);
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);
    assert_eq!(
        p["remaining_capacity"]["snapshot_epoch"],
        before["fence"]["epoch"]
    );
    context_support::assert_capacity_remainder(&p, TOTALS);
    context_support::assert_dimensions(
        &p["remaining_capacity"]["required_for_full_decision"],
        FULL,
    );
    assert_eq!(before, context_support::scoped_state(e.0, c.0));
    let out = context_support::run_request(&q, "busy_cli");
    assert!(out.status.success());
    let receipt = context_support::receipt(&out);
    assert_eq!(receipt["outcome"], "prepared");
    context_support::assert_false_flags(&receipt);
    let cli_pack: Value = serde_json::from_str(receipt["pack_utf8"].as_str().unwrap()).unwrap();
    assert_eq!(cli_pack["remaining_capacity"], p["remaining_capacity"]);
    context_support::assert_mission_frontier(&cli_pack, &source);
    assert_eq!(before, context_support::scoped_state(e.0, c.0));
}

#[test]
fn new_generation_reservation_charges_second_active_hold_cleanly() {
    let _g = db_support::db();
    let (req, source) = budget::fixture(0x9607);
    let op1 = budget::op();
    let h1 = budget::run(&req, op1, Reserve).unwrap().unwrap();
    assert_eq!(h1.generation, 1);
    assert_eq!(h1.operation_id, op1);
    let cancelled = budget::run(&req, op1, Cancel).unwrap().unwrap();
    assert_eq!(cancelled.kind, "cancelled_never_sent");
    assert_eq!(cancelled.record_revision, 2);
    assert_eq!(cancelled.generation_call_id, h1.generation_call_id);
    assert_eq!(cancelled.repair_call_id, h1.repair_call_id);
    let op2 = budget::op();
    let h2 = budget::run(&req, op2, Reserve).unwrap().unwrap();
    assert_eq!(h2.operation_id, op2);
    assert_ne!(op1, op2);
    assert_eq!(h2.generation, 2);
    assert_eq!(h2.kind, "reserved_not_sent");
    let mut calls = vec![
        h1.generation_call_id,
        h1.repair_call_id,
        h2.generation_call_id,
        h2.repair_call_id,
    ];
    assert!(calls.iter().all(|id| !id.is_nil()));
    calls.sort();
    calls.dedup();
    assert_eq!(calls.len(), 4);
    context_support::assert_charge(&h2.capacity, FULL);
    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let before = context_support::scoped_state(e, c);
    let ledger = before["ledger"].as_array().unwrap();
    assert_eq!(ledger.len(), 3);
    for (op, kind, revision, generation) in [
        (op1, "reserved_not_sent", 1, 1),
        (op1, "cancelled_never_sent", 2, 1),
        (op2, "reserved_not_sent", 1, 2),
    ] {
        let row = ledger
            .iter()
            .find(|r| r["operation_id"] == json!(op) && r["record_revision"] == revision)
            .unwrap();
        budget::assert_full_record(row, &req, op, kind, revision, generation, &source);
    }
    assert_eq!(budget::run(&req, op1, Recover).unwrap().unwrap(), cancelled);
    assert_eq!(budget::run(&req, op2, Recover).unwrap().unwrap(), h2);
    let q = context_support::context_request(&req);
    let mut reader = PgContextReader::new(session_support::broker_client());
    let basis = reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis.capacity.active_holds, 1);
    assert!(!basis.capacity.session_claimed);
    context_support::assert_charge(&basis.capacity.charged, FULL);
    assert_eq!(json!(basis.capacity.epoch), before["fence"]["epoch"]);
    let p = context_support::prepare_pack(&q);
    assert_eq!(
        p["remaining_capacity"]["snapshot_epoch"],
        before["fence"]["epoch"]
    );
    assert_eq!(p["remaining_capacity"]["exclusive_owner_busy"], true);
    context_support::assert_dimensions(
        &p["remaining_capacity"]["required_for_full_decision"],
        FULL,
    );
    context_support::assert_capacity_remainder(&p, REMAINDER);
    assert_eq!(before, context_support::scoped_state(e, c));
}

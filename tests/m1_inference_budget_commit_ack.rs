//! Physical commit-acknowledgment-loss and rollback coverage for the pre-send
//! inference reservation slice on the owned disposable database. A real
//! PostgreSQL protocol relay withholds bytes at controlled frames; durable
//! state is asserted through an independent restricted connection while the
//! CLI child is blocked, and reconciliation/replay is exercised afterward.
//! Admin access is fixture setup only; cluster-role fixtures serialize
//! under db().
//!
//! Requires the packet's authorized disposable PostgreSQL service.

use duskweave::m1_inference_budget::ReservationAction::*;
use std::net::SocketAddr;
use std::time::Duration;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/database_wait_db.rs"]
mod wait_db;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;

fn relay_dsn(port: u16) -> String {
    session_support::broker_dsn(port)
}

/// The relay consumes backend bytes after the wire frame containing the
/// needle is forwarded; the client's completion is withheld from there.
fn start(needle: &'static [u8]) -> proxy::Relay {
    proxy::Relay::start(
        SocketAddr::from(([127, 0, 0, 1], session_support::db_port())),
        needle,
    )
}

#[test]
fn physical_reserve_commit_ack_loss_keeps_exact_durable_capacity() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf008);
    let relay = start(b"COMMIT");
    let input = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-ack-reserve");
    let id = budget::op();
    let (child, _started) = wait_db::spawn_piped(&mut wait_db::cli(
        &budget::args("reserve", id, input.path()),
        &relay_dsn(relay.port()),
    ));
    // The durable record must be committed before the caller is released.
    assert!(
        relay.observed(Duration::from_secs(30)),
        "COMMIT frame never sent"
    );
    assert!(
        relay.consumed(Duration::from_secs(30)),
        "COMMIT completion never confirmed"
    );
    let committed = budget::state(&q)["ledger"].clone();
    assert_eq!(committed.as_array().unwrap().len(), 1);
    assert_eq!(committed[0]["operation_id"], id.0.to_string());
    assert_eq!(committed[0]["kind"], "reserved_not_sent");
    // The relay drops the connection; the caller learns nothing certain.
    drop(relay);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let receipt = budget::receipt(&output);
    assert_eq!(receipt["outcome"], "unknown");
    budget::assert_flags_off(&receipt);
    // Reconciliation returns the durable record; replay does not duplicate.
    let recovered = budget::run(&q, id, Recover).unwrap().unwrap();
    assert_eq!(recovered.generation, 1);
    assert_eq!(recovered.kind, "reserved_not_sent");
    let replay = budget::run(&q, id, Reserve).unwrap().unwrap();
    assert_eq!(replay, recovered);
    let ledger = budget::state(&q)["ledger"].clone();
    assert_eq!(ledger.as_array().unwrap().len(), 1);
}

#[test]
fn physical_cancel_commit_ack_loss_keeps_exact_durable_cancellation() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf009);
    let held = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    let relay = start(b"COMMIT");
    let input = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-ack-cancel");
    let (child, _started) = wait_db::spawn_piped(&mut wait_db::cli(
        &budget::args("cancel", held.operation_id, input.path()),
        &relay_dsn(relay.port()),
    ));
    assert!(
        relay.observed(Duration::from_secs(30)),
        "COMMIT frame never sent"
    );
    assert!(
        relay.consumed(Duration::from_secs(30)),
        "COMMIT completion never confirmed"
    );
    // The appended cancelled row (record_revision 2) is durable before the
    // caller learns anything; the ledger keeps both revisions.
    let committed = budget::state(&q)["ledger"].clone();
    assert_eq!(committed.as_array().unwrap().len(), 2);
    assert_eq!(committed[1]["kind"], "cancelled_never_sent");
    assert_eq!(committed[1]["record_revision"], 2);
    drop(relay);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let receipt = budget::receipt(&output);
    assert_eq!(receipt["outcome"], "unknown");
    budget::assert_flags_off(&receipt);
    // Recover and a replayed cancel both return the same durable record.
    let recovered = budget::run(&q, held.operation_id, Recover)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.kind, "cancelled_never_sent");
    assert_eq!(recovered.record_revision, 2);
    assert_eq!(
        budget::run(&q, held.operation_id, Cancel).unwrap().unwrap(),
        recovered
    );
    // The freed capacity admits a fresh hold at the next generation.
    let next = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    assert_eq!(next.generation, 2);
}

#[test]
fn uncommitted_rollback_leaves_no_durable_record() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf00b);
    // Withhold backend replies once the Parse frame carrying the guarded
    // function call is forwarded: the work may execute inside the open
    // transaction but can never commit while the caller is blocked.
    let relay = start(b"execution.inference_reservation");
    let input = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-uncommit");
    let id = budget::op();
    let (mut child, _started) = wait_db::spawn_piped(&mut wait_db::cli(
        &budget::args("reserve", id, input.path()),
        &relay_dsn(relay.port()),
    ));
    // Wait for the relay to forward the matched frame and confirm the child
    // is genuinely blocked.
    assert!(
        relay.observed(Duration::from_secs(30)),
        "function call never sent"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        child.try_wait().unwrap().is_none(),
        "child must still be blocked"
    );
    // Nothing durable may exist while the transaction is uncommitted.
    assert!(
        budget::state(&q)["ledger"].as_array().unwrap().is_empty(),
        "uncommitted work must be invisible"
    );
    drop(relay);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let receipt = budget::receipt(&output);
    assert_eq!(receipt["outcome"], "unknown");
    budget::assert_flags_off(&receipt);
    // The operation truly does not exist: recover reports missing.
    assert_eq!(budget::run(&q, id, Recover).unwrap(), None);
    // And a clean retry performs the reservation normally.
    let retried = budget::run(&q, id, Reserve).unwrap().unwrap();
    assert_eq!(retried.operation_id, id);
}

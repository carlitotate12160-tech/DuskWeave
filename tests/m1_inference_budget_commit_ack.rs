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
use serde_json::Value;
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

/// A fresh real CLI process running `recover` against the direct qualified
/// Broker connection; asserts success, durable outcome and flags.
fn cli_recover(q: &Value, id: duskweave::mission::OperationId, path: &str) -> Value {
    let (out, _) = wait_db::run_bounded(
        &mut wait_db::cli(
            &budget::args("recover", id, path),
            &session_support::broker_dsn(session_support::db_port()),
        ),
        wait_db::COMMAND_ENVELOPE,
    );
    assert!(out.status.success(), "recover CLI failed: {out:?}");
    let receipt = budget::receipt(&out);
    assert_eq!(receipt["request"], *q);
    assert_eq!(receipt["operation_id"], id.0.to_string());
    assert_eq!(receipt["outcome"], "durable");
    budget::assert_flags_off(&receipt);
    receipt
}

#[test]
fn physical_reserve_commit_ack_loss_keeps_exact_durable_capacity() {
    let _g = db();
    let (q, source) = budget::fixture(0xf008);
    let before = budget::state(&q);
    let relay = start(b"COMMIT");
    let input = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-ack-reserve");
    let id = budget::op();
    let (mut child, _started) = wait_db::spawn_piped(&mut wait_db::cli(
        &budget::args("reserve", id, input.path()),
        &relay_dsn(relay.port()),
    ));
    // The relay proves the COMMIT request left and its successful backend
    // CommandComplete arrived — the durable commit exists before the caller
    // can learn anything. The caller must still be blocked while we look.
    assert!(
        relay.observed(Duration::from_secs(30)),
        "COMMIT frame never sent"
    );
    assert!(
        relay.consumed(Duration::from_secs(30)),
        "COMMIT completion never confirmed"
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "caller must still be blocked on the withheld acknowledgment"
    );
    let committed = budget::state(&q);
    let rows = committed["ledger"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    budget::assert_full_record(&rows[0], &q, id, "reserved_not_sent", 1, 1, &source);
    // Exactly one epoch increment; phase/generation/claim/writer untouched.
    assert_eq!(committed["fence"], budget::fence_after(&before["fence"], 1));
    // Source owner, publication and history are untouched by the invocation.
    for key in ["mission", "publication", "session_history"] {
        assert_eq!(committed[key], before[key], "{key} must not change");
    }
    // The relay drops the connection; the caller learns nothing certain.
    drop(relay);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let receipt = budget::receipt(&output);
    assert_eq!(receipt["outcome"], "unknown");
    budget::assert_flags_off(&receipt);
    // A fresh real CLI process reconciles: the complete typed record equals
    // the independently observed committed row — both render recorded_at as
    // the same to_jsonb timestamptz string, so equality is exact.
    let recovered = cli_recover(&serde_json::to_value(&q).unwrap(), id, input.path());
    assert_eq!(recovered["record"], rows[0]);
    // Recovery and a duplicate reserve are read-only: identical snapshot,
    // no extra record, epoch bump or charge.
    assert_eq!(budget::state(&q), committed);
    let replay = budget::run(&q, id, Reserve).unwrap().unwrap();
    assert_eq!(serde_json::to_value(&replay).unwrap(), rows[0]);
    assert_eq!(budget::state(&q), committed);
}

#[test]
fn physical_cancel_commit_ack_loss_keeps_exact_durable_cancellation() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf009);
    let held = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    let before = budget::state(&q);
    let relay = start(b"COMMIT");
    let input = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-ack-cancel");
    let (mut child, _started) = wait_db::spawn_piped(&mut wait_db::cli(
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
    assert!(
        child.try_wait().unwrap().is_none(),
        "caller must still be blocked on the withheld acknowledgment"
    );
    // The appended cancelled row (record_revision 2) is durable while the
    // caller learns nothing; the ledger keeps both revisions.
    let committed = budget::state(&q);
    let rows = committed["ledger"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    // Revision 1 is byte-identical to the record the original call returned.
    assert_eq!(rows[0], serde_json::to_value(&held).unwrap());
    // Revision 2 preserves every immutable identity and capacity field; only
    // record_revision, kind and the database timestamp may differ.
    let mut expected = rows[0].clone();
    expected["record_revision"] = serde_json::json!(2);
    expected["kind"] = serde_json::json!("cancelled_never_sent");
    expected["recorded_at"] = rows[1]["recorded_at"].clone();
    assert_eq!(rows[1], expected);
    assert!(
        rows[1]["recorded_at"].as_str().unwrap() >= rows[0]["recorded_at"].as_str().unwrap(),
        "cancel timestamp must not precede the hold"
    );
    // Exactly one epoch increment for this first linked cancellation; every
    // other fence field untouched.
    assert_eq!(committed["fence"], budget::fence_after(&before["fence"], 1));
    for key in ["mission", "publication", "session_history"] {
        assert_eq!(committed[key], before[key], "{key} must not change");
    }
    drop(relay);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let receipt = budget::receipt(&output);
    assert_eq!(receipt["outcome"], "unknown");
    budget::assert_flags_off(&receipt);
    // Fresh-process recovery returns the latest committed row verbatim.
    let recovered = cli_recover(
        &serde_json::to_value(&q).unwrap(),
        held.operation_id,
        input.path(),
    );
    assert_eq!(recovered["record"], rows[1]);
    // Recovery and a duplicate cancel are read-only: no mutation at all.
    assert_eq!(budget::state(&q), committed);
    assert_eq!(
        serde_json::to_value(budget::run(&q, held.operation_id, Cancel).unwrap().unwrap()).unwrap(),
        rows[1]
    );
    assert_eq!(budget::state(&q), committed);
    // Only after those comparisons: the freed capacity admits a fresh hold
    // at the next generation, unaffected by the old cancellation.
    let next = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    assert_eq!(next.generation, 2);
}

#[test]
fn uncommitted_rollback_leaves_no_durable_record() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf00b);
    let before = budget::state(&q);
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
    // The rolled-back invocation left no trace in the complete scoped state.
    assert_eq!(budget::state(&q), before, "rollback must leave no trace");
    // And a clean retry performs the reservation normally.
    let retried = budget::run(&q, id, Reserve).unwrap().unwrap();
    assert_eq!(retried.operation_id, id);
}

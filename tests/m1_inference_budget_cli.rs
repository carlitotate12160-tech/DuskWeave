//! Real end-to-end CLI coverage for `duskweave m1-inference-budget` on the
//! owned disposable database: reserve/recover/cancel over the Broker DSN,
//! the bounded pre-send receipt, missing-outcome reporting, refused
//! categories and bounded input handling without credential or request echo.
//! Admin access is fixture setup only; cluster-role fixtures serialize
//! under db().
//!
//! Requires the packet's authorized disposable PostgreSQL service.

use serde_json::{Value, json};
use std::process::Output;
use uuid::Uuid;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;

fn invoke(args: &[String]) -> Output {
    wait_db::cli(
        args,
        &session_support::broker_dsn(session_support::db_port()),
    )
    .output()
    .unwrap()
}

fn stdout_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn real_cli_reserve_recover_cancel_emit_only_pre_send_receipts() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf004);
    let input = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-lifecycle");
    let id = budget::op();

    for (action, outcome, kind) in [
        ("reserve", "durable", "reserved_not_sent"),
        ("recover", "durable", "reserved_not_sent"),
        ("cancel", "durable", "cancelled_never_sent"),
        ("recover", "durable", "cancelled_never_sent"),
    ] {
        let out = invoke(&budget::args(action, id, input.path()));
        assert!(
            out.status.success(),
            "{action} failed: {}",
            stdout_text(&out)
        );
        let receipt = budget::receipt(&out);
        assert_eq!(receipt["kind"], "m1_inference_reservation_receipt_v1");
        assert_eq!(receipt["outcome"], outcome);
        assert_eq!(receipt["operation_id"], id.0.to_string());
        assert_eq!(receipt["record"]["kind"], kind);
        budget::assert_flags_off(&receipt);
    }
    // Replay of reserve under a new operation allocates the next generation.
    let out = invoke(&budget::args("reserve", budget::op(), input.path()));
    assert!(out.status.success());
    let receipt = budget::receipt(&out);
    assert_eq!(receipt["record"]["generation"], 2);
    budget::assert_flags_off(&receipt);
}

#[test]
fn cli_missing_recover_reports_missing_and_cancel_of_missing_is_refused() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf005);
    let input = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-missing");
    // Recover on a never-reserved operation is a clean negative answer.
    let out = invoke(&budget::args("recover", budget::op(), input.path()));
    assert!(out.status.success(), "{}", stdout_text(&out));
    let receipt = budget::receipt(&out);
    assert_eq!(receipt["outcome"], "missing");
    assert_eq!(receipt["record"], Value::Null);
    budget::assert_flags_off(&receipt);
    // Cancel of a missing record refuses: it must not claim absence, and the
    // guarded function raises the missing-reservation refusal.
    let out = invoke(&budget::args("cancel", budget::op(), input.path()));
    assert!(!out.status.success());
    let receipt = budget::receipt(&out);
    assert_eq!(receipt["outcome"], "refused");
    assert_eq!(receipt["record"], Value::Null);
    budget::assert_flags_off(&receipt);
}

#[test]
fn cli_refused_and_invalid_inputs_never_touch_or_echo_state() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf006);
    let before = budget::state(&q);
    let broker = session_support::broker_dsn(session_support::db_port());
    let good = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-ok");
    // Unknown action / malformed operation flag / missing flags.
    for args in [
        budget::args("hold", budget::op(), good.path()),
        vec![
            "m1-inference-budget".into(),
            "--action".into(),
            "reserve".into(),
            "--operation".into(),
            "not-a-uuid".into(),
            "--input".into(),
            good.path().into(),
        ],
        vec!["m1-inference-budget".into()],
    ] {
        let out = wait_db::cli(&args, &broker).output().unwrap();
        assert!(!out.status.success());
    }
    // Unreadable and malformed input files.
    let malformed = wait_db::InputFile::new(&json!({"broken": true}), "budget-bad");
    for args in [
        budget::args("reserve", budget::op(), "missing-SYNTHETIC_SENTINEL.json"),
        budget::args("reserve", budget::op(), malformed.path()),
    ] {
        let out = wait_db::cli(&args, &broker).output().unwrap();
        assert!(!out.status.success());
        assert!(
            !stdout_text(&out).contains("SYNTHETIC"),
            "input content or path must not echo: {}",
            stdout_text(&out)
        );
    }
    // Oversize input is refused by the same bound as every other ingress.
    let oversized = std::env::temp_dir().join(format!("dw-budget-big-{}.json", std::process::id()));
    std::fs::write(&oversized, " ".repeat(16385)).unwrap();
    let out = invoke(&budget::args(
        "reserve",
        budget::op(),
        oversized.to_str().unwrap(),
    ));
    assert!(!out.status.success());
    std::fs::remove_file(&oversized).unwrap();
    // A consent-less scope refuses through the receipt.
    session_support::ensure_broker_logins();
    let (e, c) = scope(0xf007);
    let mut raw = budget::registration_value(e, c, session_support::now());
    raw.as_object_mut()
        .unwrap()
        .remove("m1_inference_permission");
    budget::register(e, c, &raw);
    let mut qc = budget::request(
        e,
        c,
        &budget::registration_value(e, c, session_support::now()),
    );
    qc.permission_id = Uuid::from_u128(0xf007);
    let consentless =
        wait_db::InputFile::new(&serde_json::to_value(&qc).unwrap(), "budget-noconsent");
    let out = invoke(&budget::args("reserve", budget::op(), consentless.path()));
    assert!(!out.status.success());
    let receipt = budget::receipt(&out);
    assert_eq!(receipt["outcome"], "refused");
    assert_eq!(receipt["record"], Value::Null);
    budget::assert_flags_off(&receipt);
    assert_eq!(budget::state(&q), before);
}

#[test]
fn cli_receipt_is_bounded_and_never_echoes_credentials() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf008);
    let mut value = serde_json::to_value(&q).unwrap();
    value["goal_ref"] = json!("SYNTHETIC_SECRET_SENTINEL");
    let input = wait_db::InputFile::new(&value, "budget-sentinel");
    // Rejected input never echoes its content onto the receipt stream.
    let out = invoke(&budget::args("reserve", budget::op(), input.path()));
    let text = stdout_text(&out);
    assert!(!text.contains("SYNTHETIC_SECRET_SENTINEL"));
    // No DSN material may appear on stdout either.
    let dsn = session_support::broker_dsn(session_support::db_port());
    for part in dsn.split(' ') {
        if let Some(secret) = part.strip_prefix("password=") {
            let bare = secret.trim_matches('\'');
            assert!(!text.contains(bare), "credential material in receipt");
        }
    }
    // A well-formed run's receipt fields stay within the declared catalog.
    let good = wait_db::InputFile::new(&serde_json::to_value(&q).unwrap(), "budget-good");
    let out = invoke(&budget::args("reserve", budget::op(), good.path()));
    assert!(out.status.success());
    let receipt = budget::receipt(&out);
    let keys: Vec<&str> = receipt
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    for key in &keys {
        assert!(
            [
                "kind",
                "request",
                "operation_id",
                "record",
                "outcome",
                "current_permission",
                "dispatch_granted",
                "provider_qualified",
                "send_qualified",
                "acquisition_qualified",
            ]
            .contains(key),
            "unexpected receipt field {key}"
        );
    }
}

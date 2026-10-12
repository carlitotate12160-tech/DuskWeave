//! Shared fixtures and helpers for m1-context-prepare test suites.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::process::Output;
use std::time::Duration;
use uuid::Uuid;

pub fn candidate(asset: Uuid, priority: &str) -> Value {
    json!({
        "asset_ref": asset,
        "priority": priority
    })
}

pub fn request(mission: &Value, candidates: &[Value]) -> Value {
    json!({
        "version": "m1_context_prepare_request_v1",
        "mission": mission,
        "candidates": candidates
    })
}

pub fn run(args: &[String], dsn: &str) -> Output {
    crate::wait_db::run_bounded(&mut crate::wait_db::cli(args, dsn), Duration::from_secs(15)).0
}

pub fn receipt(out: &Output) -> Value {
    let line = out
        .stdout
        .split(|b| *b == b'\n')
        .find(|line| !line.is_empty())
        .expect("receipt line present");
    serde_json::from_slice(line).expect("receipt is valid JSON")
}

pub fn assert_false_flags(r: &Value) {
    assert_eq!(r["kind"], "m1_context_preparation_receipt_v1");
    assert_eq!(r["current_permission"], false);
    assert_eq!(r["dispatch_granted"], false);
    assert_eq!(r["provider_qualified"], false);
    assert_eq!(r["acquisition_qualified"], false);
    assert_eq!(r["send_qualified"], false);
    assert_eq!(r["reservation_granted"], false);
    assert_eq!(r["decision_published"], false);
    assert_eq!(r["history_completed"], false);
    assert_eq!(r["request_binding_qualified"], false);
}

//! Subprocess integration tests for duskweave m1-context-prepare:
//! Real CLI binary, CommandGuard, exit codes, safe receipts, and false flags.
use serde_json::json;
use std::fs::File;
use std::io::Write;

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

#[test]
fn cli_context_prepare_happy_path_with_safe_receipt_and_flags() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9501);
    let mut raw = budget::registration_value(e, c, session_support::now());
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));
    let (_, source) = budget::register(e, c, &raw);
    let mission_req = budget::request(e, c, &raw);
    let a1 = source.fields.included_assets()[0];

    let req_val = context_support::request(
        &serde_json::to_value(&mission_req).unwrap(),
        &[context_support::candidate(a1.0, "high")],
    );

    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let input = wait_db::InputFile::new(&req_val, "prep_happy");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let out = context_support::run(&args, &broker_dsn);
    assert!(out.status.success(), "CLI preparation must exit with 0");

    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "prepared");
    assert!(r["pack_utf8"].is_string());
    context_support::assert_false_flags(&r);

    let pack: serde_json::Value = serde_json::from_str(r["pack_utf8"].as_str().unwrap()).unwrap();
    assert_eq!(pack["version"], "m1_context_pack_v1");
    assert_eq!(pack["decision_id"], r["decision_id"]);
}

#[test]
fn cli_context_prepare_refused_outcome_on_out_of_scope_candidate() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9502);
    let mut raw = budget::registration_value(e, c, session_support::now());
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));
    let (_, _source) = budget::register(e, c, &raw);
    let mission_req = budget::request(e, c, &raw);

    // Asset not in included_assets
    let unincluded = uuid::Uuid::from_u128(0x9999);
    let req_val = context_support::request(
        &serde_json::to_value(&mission_req).unwrap(),
        &[context_support::candidate(unincluded, "high")],
    );

    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let input = wait_db::InputFile::new(&req_val, "prep_refused");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let out = context_support::run(&args, &broker_dsn);
    assert!(
        !out.status.success(),
        "refused outcome must exit with non-zero"
    );

    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "refused");
    assert!(r["pack_utf8"].is_null());
    context_support::assert_false_flags(&r);
}

#[test]
fn cli_context_prepare_deferred_outcome_when_consent_expired() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9503);
    let now = session_support::now();
    let mut raw = budget::registration_value(e, c, now);
    // Expired consent window relative to database clock
    raw["m1_inference_permission"]["starts_at"] = json!(now - 2000);
    raw["m1_inference_permission"]["ends_at"] = json!(now - 100);
    let (_, _source) = budget::register(e, c, &raw);
    let mission_req = budget::request(e, c, &raw);

    let req_val = context_support::request(&serde_json::to_value(&mission_req).unwrap(), &[]);

    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let input = wait_db::InputFile::new(&req_val, "prep_deferred");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let out = context_support::run(&args, &broker_dsn);
    assert!(
        !out.status.success(),
        "deferred outcome must exit with non-zero"
    );

    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "deferred");
    assert!(r["pack_utf8"].is_null());
    context_support::assert_false_flags(&r);
}

#[test]
fn cli_context_prepare_unavailable_outcome_without_broker_privilege() {
    let _g = db_support::db();
    let (req, _source) = budget::fixture(0x9504);
    let req_val = context_support::request(&serde_json::to_value(&req).unwrap(), &[]);

    // Connecting with runtime DSN (without broker role) -> unavailable outcome
    let runtime_dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let input = wait_db::InputFile::new(&req_val, "prep_unavail");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let out = context_support::run(&args, &runtime_dsn);
    assert!(!out.status.success());

    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "unavailable");
    assert!(r["pack_utf8"].is_null());
    context_support::assert_false_flags(&r);
}

#[test]
fn cli_input_parsing_errors_and_size_limit() {
    let dsn = std::env::var("DW_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5432/test".into());

    // Missing file
    let args_missing = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        "nonexistent_file_path_12345.json".into(),
    ];
    let out_missing = context_support::run(&args_missing, &dsn);
    assert!(!out_missing.status.success());
    let txt = String::from_utf8_lossy(&out_missing.stdout);
    assert!(txt.contains("error=unreadable_input"));

    // Malformed JSON
    let bad_json_input = wait_db::InputFile::new(&json!("not_an_object"), "bad_json");
    let args_bad = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        bad_json_input.path().into(),
    ];
    let out_bad = context_support::run(&args_bad, &dsn);
    assert!(!out_bad.status.success());
    let txt_bad = String::from_utf8_lossy(&out_bad.stdout);
    assert!(txt_bad.contains("error=invalid_context_input"));

    // Oversized input file (> 16384 bytes)
    let temp_dir = std::env::temp_dir();
    let oversize_path = temp_dir.join("oversize_context_input.json");
    {
        let mut f = File::create(&oversize_path).unwrap();
        f.write_all(&vec![b' '; 16385]).unwrap();
    }
    let args_over = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        oversize_path.to_str().unwrap().into(),
    ];
    let out_over = context_support::run(&args_over, &dsn);
    let _ = std::fs::remove_file(&oversize_path);
    assert!(!out_over.status.success());
    let txt_over = String::from_utf8_lossy(&out_over.stdout);
    assert!(txt_over.contains("error=size_limit"));
}

#[test]
fn cli_missing_flags_fails_command_guard() {
    let dsn = std::env::var("DW_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5432/test".into());
    let args = vec!["m1-context-prepare".into()];
    let out = context_support::run(&args, &dsn);
    assert!(!out.status.success());
    let txt = String::from_utf8_lossy(&out.stdout);
    assert!(txt.contains("error=invalid_args"));
}

#[test]
fn cli_context_prepare_refused_outcome_on_basis_mismatch() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9506);
    let raw = budget::registration_value(e, c, session_support::now());
    let (_, _source) = budget::register(e, c, &raw);
    let mut mission_req = budget::request(e, c, &raw);
    mission_req.permission_id = uuid::Uuid::from_u128(0xbeef);

    let req_val = context_support::request(&serde_json::to_value(&mission_req).unwrap(), &[]);
    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let input = wait_db::InputFile::new(&req_val, "prep_basis_mismatch");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];
    let out = context_support::run(&args, &broker_dsn);
    assert!(!out.status.success());
    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "refused");
    assert!(r["pack_utf8"].is_null());
    context_support::assert_false_flags(&r);
}

//! Real prepared-only withdrawal entrypoint; owner/history and fence are observed independently.
use duskweave::mission::OperationId;
use duskweave::registration::MissionStore;
use serde_json::{Value, json};
use std::process::Output;
use std::time::Duration;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_prepared_withdrawal.rs"]
mod prepared;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;

fn run(args: Vec<String>, dsn: &str) -> Output {
    wait_db::run_bounded(&mut wait_db::cli(&args, dsn), Duration::from_secs(15)).0
}

fn args(command: &str, op: OperationId, file: &str, tail: &[&str]) -> Vec<String> {
    let mut args = vec![
        command.into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        file.into(),
    ];
    args.extend(tail.iter().map(|s| s.to_string()));
    args
}

fn receipt(out: &Output) -> Value {
    serde_json::from_slice(out.stdout.split(|b| *b == b'\n').next().unwrap()).unwrap()
}

#[test]
fn prepared_withdrawal_cli_keeps_fence_and_consumes_existing_history() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x7901);
    let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let broker = session_support::broker_dsn(session_support::db_port());
    let registered = session_support::session_op();
    let input = wait_db::InputFile::new(
        &session_support::registration(e, c, session_support::now(), 0),
        &e.to_string(),
    );
    let out = run(args("register", registered, input.path(), &[]), &runtime);
    assert!(out.status.success(), "registration baseline must succeed");
    let session = session_support::session_op();
    let input = wait_db::InputFile::new(&session_support::request(e, c), &session.to_string());
    let out = run(
        args(
            "m1-session",
            session,
            input.path(),
            &["--action", "prepare"],
        ),
        &broker,
    );
    assert!(out.status.success(), "prepare baseline must succeed");
    let original_fence = session_support::fence(e, c).unwrap();
    let withdrawal = session_support::session_op();
    let input = wait_db::InputFile::new(
        &json!({"withdrawal":session_support::withdraw_req(e,c),
            "session_operation_id":session,"expected_session_generation":1}),
        &withdrawal.to_string(),
    );
    let out = run(
        args(
            "m1-session-withdraw",
            withdrawal,
            input.path(),
            &["--recover", "false"],
        ),
        &broker,
    );
    assert!(
        out.status.success(),
        "prepared withdrawal must commit through its original Broker without release: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let got = receipt(&out);
    assert_eq!(got["result"], "durable");
    assert_eq!(got["complete_history"], true);
    assert_eq!(got["operation"], json!(withdrawal));
    assert_eq!(got["session_operation_id"], json!(session));
    assert_eq!(got["expected_session_generation"], 1);
    for flag in [
        "current_permission",
        "dispatch_granted",
        "acquisition_qualified",
        "host_control_qualified",
    ] {
        assert_eq!(got[flag], false);
    }
    assert_eq!(session_support::fence(e, c).unwrap(), original_fence);
    let mut mission = db_support::ports().1;
    assert_eq!(mission.mission_view(e, c).unwrap().unwrap().revision, 2);
    assert_eq!(db_support::count("mission.withdrawals", e, c), 1);
    assert_eq!(db_support::count("execution.prepared_withdrawals", e, c), 1);
    assert_eq!(db_support::count("trajectory.withdrawal_history", e, c), 1);
}

fn submit_cli(case: &prepared::Case, recover: bool, dsn: &str) -> Output {
    let file = wait_db::InputFile::new(&case.input(), &case.operation.to_string());
    run(
        args(
            "m1-session-withdraw",
            case.operation,
            file.path(),
            &["--recover", if recover { "true" } else { "false" }],
        ),
        dsn,
    )
}
fn safe_flags(out: &Output) -> Value {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for secret in ["SYNTHETIC_SECRET_SENTINEL", "password", "postgresql://"] {
        assert!(
            !text.contains(secret),
            "bounded output leaked a prohibited marker"
        );
    }
    let got = receipt(out);
    for flag in [
        "current_permission",
        "dispatch_granted",
        "acquisition_qualified",
        "host_control_qualified",
    ] {
        assert_eq!(got[flag], false);
    }
    got
}

#[test]
fn committed_owner_with_failed_history_recovers_read_only_then_explicit_retry_completes_once() {
    let _g = db_support::db();
    let case = prepared::Case::new(0x7902);
    let broker = session_support::broker_dsn(session_support::db_port());
    let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let fence = session_support::fence(case.e, case.c);
    let first;
    {
        let _fault = prepared::Grants::install(
            &format!(
            "CREATE FUNCTION trajectory.dw_pw_history_fault() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='fixture_history_fault'; END $$;
             CREATE TRIGGER dw_pw_history_fault BEFORE INSERT ON trajectory.withdrawal_history
             FOR EACH ROW WHEN (NEW.engagement_id='{}'::uuid)
             EXECUTE FUNCTION trajectory.dw_pw_history_fault();",case.e),
            "DROP TRIGGER dw_pw_history_fault ON trajectory.withdrawal_history;
             DROP FUNCTION trajectory.dw_pw_history_fault();",
        );
        let out = submit_cli(&case, false, &broker);
        assert!(out.status.success());
        first = safe_flags(&out);
        assert_eq!(first["result"], "durable");
        assert_eq!(first["history"], "unknown");
        assert_eq!(first["complete_history"], false);
    }
    let before = case.snapshot();
    let out = submit_cli(&case, true, &runtime);
    assert!(out.status.success());
    let recovered = safe_flags(&out);
    assert_eq!(recovered["result"], "durable");
    assert_eq!(recovered["history"], "pending");
    assert_eq!(recovered["contract"], first["contract"]);
    assert_eq!(case.snapshot(), before);
    let out = submit_cli(&case, false, &broker);
    assert!(out.status.success());
    let completed = safe_flags(&out);
    assert_eq!(completed["contract"], first["contract"]);
    assert_eq!(completed["complete_history"], true);
    assert_eq!(session_support::fence(case.e, case.c), fence);
    let after = case.snapshot();
    for recover in [false, true] {
        let out = submit_cli(&case, recover, &broker);
        assert!(out.status.success());
        assert_eq!(safe_flags(&out)["contract"], first["contract"]);
        assert_eq!(case.snapshot(), after);
    }
    assert_eq!(
        db_support::count("trajectory.withdrawal_history", case.e, case.c),
        1
    );
}

#[test]
fn invalid_or_missing_input_is_rejected_before_database_access_without_echo() {
    let r = serde_json::json!({"withdrawal":session_support::withdraw_req(
        duskweave::mission::EngagementId(uuid::Uuid::from_u128(1)),
        duskweave::mission::CampaignId(uuid::Uuid::from_u128(2))),
        "session_operation_id":uuid::Uuid::from_u128(3),"expected_session_generation":1});
    let op = OperationId(uuid::Uuid::from_u128(4));
    let mut values = vec![serde_json::json!({})];
    let mut outer = r.clone();
    outer["endpoint"] = serde_json::json!("SYNTHETIC_SECRET_SENTINEL");
    values.push(outer);
    let mut nested = r.clone();
    nested["withdrawal"]["effect"] = serde_json::json!("SYNTHETIC_SECRET_SENTINEL");
    values.push(nested);
    let mut bad = r.clone();
    bad["expected_session_generation"] = serde_json::json!(0);
    values.push(bad);
    for value in values {
        let file = wait_db::InputFile::new(&value, "pw-invalid");
        let out = run(
            args(
                "m1-session-withdraw",
                op,
                file.path(),
                &["--recover", "false"],
            ),
            "not-a-dsn-SYNTHETIC_SECRET_SENTINEL",
        );
        assert_eq!(out.status.code(), Some(1));
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("error=invalid_prepared_withdrawal"));
        assert!(!text.contains("SYNTHETIC_SECRET_SENTINEL"));
    }
    let oversized = wait_db::InputFile::new(&serde_json::json!("x".repeat(16385)), "pw-oversize");
    for (path, category) in [
        (oversized.path(), "size_limit"),
        ("missing-SYNTHETIC_SECRET_SENTINEL.json", "unreadable_input"),
    ] {
        let out = run(
            args("m1-session-withdraw", op, path, &["--recover", "false"]),
            "not-a-dsn",
        );
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            format!("error={category}")
        );
    }
    let file = wait_db::InputFile::new(&r, "pw-argv");
    for argv in [
        vec!["m1-session-withdraw".into()],
        args(
            "m1-session-withdraw",
            op,
            file.path(),
            &["--recover", "TRUE"],
        ),
        args(
            "m1-session-withdraw",
            op,
            file.path(),
            &["--recover", "false", "--effect", "start"],
        ),
    ] {
        let out = run(argv, "not-a-dsn");
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&out.stdout).contains("error=invalid_args"));
    }
}

#[test]
fn database_unavailability_missing_recovery_and_wrong_writer_have_honest_receipts() {
    let _g = db_support::db();
    let case = prepared::Case::new(0x7903);
    let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let missing = submit_cli(&case, true, &runtime);
    assert!(missing.status.success());
    assert_eq!(safe_flags(&missing)["result"], "not_committed");
    let before = case.snapshot();
    let other = session_support::other_broker_dsn(session_support::db_port());
    let denied = submit_cli(&case, false, &other);
    assert_eq!(denied.status.code(), Some(1));
    assert_eq!(safe_flags(&denied)["result"], "rejected");
    assert_eq!(case.snapshot(), before);
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let unavailable = submit_cli(&case, false, &session_support::broker_dsn(port));
    assert_eq!(unavailable.status.code(), Some(1));
    let got = safe_flags(&unavailable);
    assert_eq!(got["result"], "rejected");
    assert_eq!(got["reason"], "connect_failed");
    assert_eq!(got["operation"], serde_json::json!(case.operation));
    assert_eq!(got["session_operation_id"], serde_json::json!(case.session));
    assert_eq!(got["expected_session_generation"], 1);
    assert_eq!(case.snapshot(), before);
}

//! Actual-entrypoint coverage for `duskweave m1-session`: strict argv, safe
//! receipts with all four qualification flags false, role denial end to end,
//! lost-ACK proof with fresh-process recovery, and exit-124 uncertainty.

use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::registration;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::process::Output;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;
use session_support::*;

fn input(e: EngagementId, c: CampaignId) -> Value {
    json!({
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(OPERATOR),
        "expected_mission_revision": 1
    })
}

fn session_args(action: &str, op: OperationId, path: &str) -> Vec<String> {
    vec![
        "m1-session".into(),
        "--action".into(),
        action.into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        path.into(),
    ]
}

fn register_args(op: OperationId, path: &str) -> Vec<String> {
    vec![
        "register".into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        path.into(),
    ]
}

fn withdraw_args(op: OperationId, path: &str) -> Vec<String> {
    vec![
        "withdraw".into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        path.into(),
        "--recover".into(),
        "false".into(),
    ]
}

fn run_cli(args: &[String], dsn_text: &str) -> Output {
    wait_db::run_bounded(&mut wait_db::cli(args, dsn_text), Duration::from_secs(15)).0
}

fn receipt(out: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.lines().next().unwrap_or(""))
        .unwrap_or_else(|e| panic!("receipt must be the first stdout line ({e}): {stdout}"))
}

fn assert_flags_off(v: &Value) {
    for flag in [
        "current_permission",
        "dispatch_granted",
        "acquisition_qualified",
        "host_control_qualified",
    ] {
        assert_eq!(v[flag], false, "{flag} must be explicit false: {v}");
    }
}

fn assert_no_leaks(out: &Output) {
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for marker in ["postgresql", "password", "_probe", "DSN"] {
        assert!(!all.contains(marker), "leaked marker {marker}: {all}");
    }
}

fn db_addr() -> SocketAddr {
    let cfg = dsn("DW_TEST_DATABASE_URL");
    let host = match cfg.get_hosts() {
        [postgres::config::Host::Tcp(host)] => host.clone(),
        _ => panic!("test runtime DSN must be one TCP host"),
    };
    let port = *cfg.get_ports().first().unwrap_or(&5432);
    SocketAddr::new(host.parse().expect("literal loopback IP"), port)
}

fn register_scope(e: EngagementId, c: CampaignId, dsn_text: &str) -> OperationId {
    let reg = registration(e, c, now(), 0);
    let file = wait_db::InputFile::new(&reg, &format!("{e}reg"));
    let op = registration::prepare_operation(&mut db_support::ports().0).unwrap();
    let out = run_cli(&register_args(op, file.path()), dsn_text);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    op
}

#[test]
fn strict_argv_and_unknown_command_reject() {
    let _g = db();
    ensure_broker_logins();
    let dsn_text = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let (e, c) = scope(0x7700);
    let op = OperationId(Uuid::from_u128(0xE001));
    let file = wait_db::InputFile::new(&input(e, c), &op.to_string());
    let base = session_args("prepare", op, file.path());
    for args in [
        vec!["m1-session".to_string()],
        base[..5].to_vec(),
        [base.clone(), vec!["--action".into(), "recover".into()]].concat(),
        [base.clone(), vec!["--bogus".into(), "x".into()]].concat(),
        {
            let mut v = base.clone();
            v[2] = "start".into();
            v
        },
        vec!["m1-sessiond".into()],
        vec!["unknown".into()],
    ] {
        let out = run_cli(&args, &dsn_text);
        assert_eq!(out.status.code(), Some(1), "argv must reject: {args:?}");
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("error="),
            "no receipt on argv rejection: {args:?}"
        );
        assert_no_leaks(&out);
    }
}

#[test]
fn prepare_recover_release_emit_false_flags_and_durable_outcomes() {
    let _g = db();
    ensure_broker_logins();
    let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let broker = broker_dsn(db_port());
    let (e, c) = scope(0x7701);
    register_scope(e, c, &runtime);
    let op = registration::prepare_operation(&mut db_support::ports().0).unwrap();
    let file = wait_db::InputFile::new(&input(e, c), &op.to_string());
    let out = run_cli(&session_args("prepare", op, file.path()), &broker);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let r = receipt(&out);
    assert_eq!(r["outcome"], "durable");
    assert_eq!(r["record"]["generation"], 1);
    assert_eq!(r["record"]["kind"], "prepared_no_effects");
    assert_flags_off(&r);
    assert_no_leaks(&out);
    // Fresh-process recovery through the ordinary login is read-only and
    // returns the durable record.
    let out = run_cli(&session_args("recover", op, file.path()), &runtime);
    assert!(out.status.success());
    let r = receipt(&out);
    assert_eq!(r["outcome"], "durable");
    assert_eq!(r["record"]["generation"], 1);
    assert_flags_off(&r);
    // Ordinary login may recover but never releases or prepares.
    let denied = run_cli(&session_args("release", op, file.path()), &runtime);
    assert_eq!(denied.status.code(), Some(1));
    assert_eq!(receipt(&denied)["outcome"], "refused");
    assert_flags_off(&receipt(&denied));
    assert_eq!(fence(e, c).unwrap().0, "prepared_no_effects");
    // Explicit release by the original login; idempotent replay matches.
    let released = run_cli(&session_args("release", op, file.path()), &broker);
    assert!(released.status.success());
    let r = receipt(&released);
    assert_eq!(r["outcome"], "durable");
    assert_eq!(r["record"]["kind"], "released_no_effects");
    let replay = run_cli(&session_args("release", op, file.path()), &broker);
    assert_eq!(receipt(&replay)["record"]["generation"], 1);
    assert_eq!(fence(e, c).unwrap().0, "idle");
    // Missing identity recovers as `missing` at exit 0.
    let absent = OperationId(Uuid::from_u128(0xE002));
    let out = run_cli(&session_args("recover", absent, file.path()), &runtime);
    assert!(out.status.success());
    assert_eq!(receipt(&out)["outcome"], "missing");
    assert_flags_off(&receipt(&out));
}

#[test]
fn ordinary_login_prepare_is_refused_and_fenced_withdraw_rejected() {
    let _g = db();
    ensure_broker_logins();
    let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let broker = broker_dsn(db_port());
    let (e, c) = scope(0x7702);
    register_scope(e, c, &runtime);
    let op = registration::prepare_operation(&mut db_support::ports().0).unwrap();
    let file = wait_db::InputFile::new(&input(e, c), &op.to_string());
    let denied = run_cli(&session_args("prepare", op, file.path()), &runtime);
    assert_eq!(denied.status.code(), Some(1));
    assert_eq!(receipt(&denied)["outcome"], "refused");
    assert_flags_off(&receipt(&denied));
    assert_eq!(fence(e, c).unwrap().0, "idle");
    // Claim with the Broker login, then prove the ordinary withdraw CLI is
    // fenced: rejection, not dispatch, and the claim survives.
    let claimed = run_cli(&session_args("prepare", op, file.path()), &broker);
    assert!(claimed.status.success());
    let w_op = registration::prepare_operation(&mut db_support::ports().0).unwrap();
    let w_file = wait_db::InputFile::new(
        &json!({"engagement_id": e, "campaign_id": c,
                "operator_ref": Uuid::from_u128(OPERATOR),
                "expected_mission_revision": 1,
                "reason": "operator_requested"}),
        &w_op.to_string(),
    );
    let withdrawn = run_cli(&withdraw_args(w_op, w_file.path()), &runtime);
    assert_eq!(withdrawn.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&withdrawn.stdout).contains("rejected")
            || String::from_utf8_lossy(&withdrawn.stdout).contains("error="),
        "{}",
        String::from_utf8_lossy(&withdrawn.stdout)
    );
    assert_eq!(fence(e, c).unwrap().0, "prepared_no_effects");
    assert_eq!(count("mission.withdrawals", e, c), 0);
    let released = run_cli(&session_args("release", op, file.path()), &broker);
    assert!(released.status.success());
}

#[test]
fn lost_ack_after_durable_prepare_recovers_without_new_claim() {
    let _g = db();
    ensure_broker_logins();
    let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let (e, c) = scope(0x7703);
    register_scope(e, c, &runtime);
    let op = registration::prepare_operation(&mut db_support::ports().0).unwrap();
    let file = wait_db::InputFile::new(&input(e, c), &op.to_string());
    let relay = proxy::Relay::start(db_addr(), b"COMMIT");
    let mut cmd = wait_db::cli(
        &session_args("prepare", op, file.path()),
        &broker_dsn(relay.port()),
    );
    let (child, started) = wait_db::spawn_piped(&mut cmd);
    assert!(
        relay.observed(Duration::from_secs(10)),
        "producer COMMIT never reached the server"
    );
    assert!(
        relay.consumed(Duration::from_secs(10)),
        "the server never reported CommandComplete COMMIT; commit not server-confirmed"
    );
    // The server committed the prepared claim while its reply was withheld.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if history_count(e, c) == 1 {
            break;
        }
        assert!(Instant::now() < deadline, "prepared record never committed");
        std::thread::sleep(Duration::from_millis(50));
    }
    let (output, _) = wait_db::wait_bounded(
        child,
        wait_db::COMMAND_ENVELOPE + Duration::from_secs(5),
        started,
    );
    wait_db::expect_watchdog_stop(
        &output,
        started.elapsed(),
        wait_db::COMMAND_FLOOR,
        wait_db::COMMAND_ENVELOPE,
    );
    drop(relay);
    // Fresh-process recovery sees the durable claim; no new generation or
    // duplicate record exists, and no retry is needed to see the truth.
    let out = run_cli(&session_args("recover", op, file.path()), &runtime);
    assert!(out.status.success());
    let r = receipt(&out);
    assert_eq!(r["outcome"], "durable");
    assert_eq!(r["record"]["generation"], 1);
    assert_eq!(r["record"]["kind"], "prepared_no_effects");
    assert_flags_off(&r);
    let (_, generation, claimed, _) = fence(e, c).unwrap();
    assert_eq!((generation, claimed), (1, Some(op.0)));
    assert_eq!(history_count(e, c), 1);
}

#[test]
fn lost_ack_after_durable_release_cannot_clear_newer_generation() {
    let _g = db();
    ensure_broker_logins();
    let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let broker = broker_dsn(db_port());
    let (e, c) = scope(0x7704);
    register_scope(e, c, &runtime);
    let op = registration::prepare_operation(&mut db_support::ports().0).unwrap();
    let file = wait_db::InputFile::new(&input(e, c), &op.to_string());
    assert!(
        run_cli(&session_args("prepare", op, file.path()), &broker)
            .status
            .success()
    );
    let relay = proxy::Relay::start(db_addr(), b"COMMIT");
    let mut cmd = wait_db::cli(
        &session_args("release", op, file.path()),
        &broker_dsn(relay.port()),
    );
    let (child, started) = wait_db::spawn_piped(&mut cmd);
    assert!(
        relay.observed(Duration::from_secs(10)),
        "producer COMMIT never reached the server"
    );
    assert!(
        relay.consumed(Duration::from_secs(10)),
        "the server never reported CommandComplete COMMIT; commit not server-confirmed"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if history_count(e, c) == 2 {
            break;
        }
        assert!(Instant::now() < deadline, "released record never committed");
        std::thread::sleep(Duration::from_millis(50));
    }
    let (output, _) = wait_db::wait_bounded(
        child,
        wait_db::COMMAND_ENVELOPE + Duration::from_secs(5),
        started,
    );
    wait_db::expect_watchdog_stop(
        &output,
        started.elapsed(),
        wait_db::COMMAND_FLOOR,
        wait_db::COMMAND_ENVELOPE,
    );
    drop(relay);
    // Generation 2 claims after the lost ACK; replaying the generation-1
    // release returns its durable record without clearing generation 2.
    let op2 = registration::prepare_operation(&mut db_support::ports().0).unwrap();
    let file2 = wait_db::InputFile::new(&input(e, c), &op2.to_string());
    assert!(
        run_cli(&session_args("prepare", op2, file2.path()), &broker)
            .status
            .success()
    );
    assert_eq!(fence(e, c).unwrap().1, 2);
    let replay = run_cli(&session_args("release", op, file.path()), &broker);
    let r = receipt(&replay);
    assert_eq!(r["outcome"], "durable");
    assert_eq!(r["record"]["kind"], "released_no_effects");
    assert_eq!(r["record"]["generation"], 1);
    let (_, generation, claimed, _) = fence(e, c).unwrap();
    assert_eq!((generation, claimed), (2, Some(op2.0)));
    // Recovery reports the durable released record, not a claim.
    let out = run_cli(&session_args("recover", op, file.path()), &runtime);
    assert_eq!(receipt(&out)["record"]["kind"], "released_no_effects");
    assert_flags_off(&receipt(&out));
}

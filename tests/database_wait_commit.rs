//! COMMIT-boundary uncertainty and recovery bounds for the CLI under the
//! wait watchdog. Server-confirmed COMMIT aborts with unclassified
//! SQLSTATEs stay UNKNOWN with recover-before-retry; a relay that forwards
//! a real COMMIT but withholds its acknowledgment leaves the outcome
//! unresolved — the caller stops at 124 with a proven durable effect, and a
//! fresh direct connection recovers the identical contract once.

use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::registration;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::{accepted, count, db, dsn, ports, runtime_client, scope};

#[path = "support/database_wait_db.rs"]
mod wait_db;

#[path = "support/database_wait_proxy.rs"]
mod proxy;

fn admin_db() -> postgres::Client {
    wait_db::admin_db_client(
        dsn("DW_TEST_ADMIN_DATABASE_URL"),
        &dsn("DW_TEST_DATABASE_URL"),
    )
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

fn input(e: EngagementId, c: CampaignId) -> Value {
    json!({"engagement_id": e, "campaign_id": c, "operator_ref": Uuid::from_u128(99),
        "expected_mission_revision": 1, "reason": "authorization_ended"})
}

fn withdraw_args(op: OperationId, path: &str, recover: bool) -> Vec<String> {
    vec![
        "withdraw".into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        path.to_string(),
        "--recover".into(),
        recover.to_string(),
    ]
}

fn run_cli(args: &[String], dsn_text: &str) -> std::process::Output {
    wait_db::run_bounded(&mut wait_db::cli(args, dsn_text), Duration::from_secs(15)).0
}

fn receipt(output: &std::process::Output) -> Value {
    assert!(output.stderr.is_empty());
    let text = std::str::from_utf8(&output.stdout).unwrap();
    let view: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(view["current_permission"], false);
    assert_eq!(view["continuation_blocked"], true);
    view
}

/// The exact durable producer contract for one operation identity.
fn stored_contract(e: EngagementId, c: CampaignId, op: OperationId) -> Value {
    runtime_client()
        .query_opt(
            "SELECT contract FROM mission.withdrawals \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[&e.0, &c.0, &op.0],
        )
        .unwrap()
        .map(|row| row.get(0))
        .expect("durable withdrawal contract")
}

#[test]
fn unclassified_commit_abort_sqlstates_stay_unknown() {
    let _guard = db();
    let dsn_text = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    for (n, errcode) in [(201u128, "57014"), (202, "55P03")] {
        let (e, c) = scope(n);
        let (mut alloc, mut store, mut traj) = ports();
        accepted(&mut alloc, &mut store, &mut traj, e, c);
        let op = registration::prepare_operation(&mut alloc).unwrap();
        let file = wait_db::InputFile::new(&input(e, c), &op.to_string());
        let aborted = {
            let _fault = wait_db::DeferredFault::install(admin_db(), &e.to_string(), errcode);
            run_cli(&withdraw_args(op, file.path(), false), &dsn_text)
        };
        assert!(!aborted.status.success());
        let view = receipt(&aborted);
        assert_eq!(view["result"], "unknown");
        assert_eq!(view["reason"], "commit_unknown");
        assert_eq!(view["action"], "recover_before_retry");
        assert_eq!(view["operation"], op.to_string());
        let text = String::from_utf8_lossy(&aborted.stdout);
        assert!(!text.contains("SYNTHETIC_SECRET_SENTINEL"));
        assert!(!text.contains(file.path()));
        assert_eq!(count("mission.withdrawals", e, c), 0);
        assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
        let recovered = receipt(&run_cli(&withdraw_args(op, file.path(), true), &dsn_text));
        assert_eq!(recovered["result"], "not_committed");
        assert_eq!(count("mission.withdrawals", e, c), 0);
    }
}

#[test]
fn withheld_commit_ack_exits_124_and_recovers_identical_contract() {
    let _guard = db();
    let (e, c) = scope(203);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let file = wait_db::InputFile::new(&input(e, c), &op.to_string());
    let relay = proxy::Relay::start(db_addr(), b"COMMIT");
    let dsn_kw = wait_db::relay_dsn(&dsn("DW_TEST_DATABASE_URL"), relay.port());
    let mut cmd = wait_db::cli(&withdraw_args(op, file.path(), false), &dsn_kw);
    let (child, started) = wait_db::spawn_piped(&mut cmd);
    assert!(
        relay.observed(Duration::from_secs(10)),
        "producer COMMIT never reached the server; fault never engaged"
    );
    assert!(
        relay.consumed(Duration::from_secs(10)),
        "the server never reported CommandComplete COMMIT; commit not server-confirmed"
    );
    // Before caller timeout and before recovery, the durable effect exists:
    // exactly one producer row with the identical contract and operation
    // identity, and zero history effects.
    let contract = stored_contract(e, c, op);
    assert_eq!(contract["operation_id"], op.to_string());
    assert_eq!(contract["kind"], "mission_authority_withdrawn");
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
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
    // A fresh direct connection recovers the identical contract; no
    // duplicate producer effect is created and continuation stays blocked.
    let dsn_text = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let recovered = receipt(&run_cli(&withdraw_args(op, file.path(), true), &dsn_text));
    assert_eq!(recovered["result"], "durable");
    assert_eq!(
        recovered["contract"]["operation_id"],
        contract["operation_id"]
    );
    assert_eq!(recovered["contract"]["event_id"], contract["event_id"]);
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    // Explicit existing delivery completes history exactly once.
    let completed = receipt(&run_cli(&withdraw_args(op, file.path(), false), &dsn_text));
    assert_eq!(completed["result"], "durable");
    assert_eq!(completed["contract"]["event_id"], contract["event_id"]);
    assert_eq!(completed["complete_history"], true);
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 1);
}

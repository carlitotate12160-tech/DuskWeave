use duskweave::mission::*;
use duskweave::registration::{self, MissionStore};
use postgres::NoTls;
use serde_json::Value;
use serde_json::json;
use std::process::Command;
use uuid::Uuid;

#[path = "support/authority_confirmation_cli.rs"]
mod confirm_support;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

#[test]
fn real_cli_withdraws_expired_registration_and_records_history() {
    let _guard = db();
    let (e, c) = scope(1);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let operation = duskweave::registration::prepare_operation(&mut alloc).unwrap();
    let input = json!({
        "engagement_id": e, "campaign_id": c, "operator_ref": uuid::Uuid::from_u128(99),
        "expected_mission_revision": 1, "reason": "operator_requested"
    });
    let path = std::env::temp_dir().join(format!("dw-c1a-{operation}.json"));
    std::fs::write(&path, input.to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        )
        .args(["withdraw", "--operation", &operation.to_string(), "--input"])
        .arg(&path)
        .args(["--recover", "false"])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "withdraw must accept restrictive intent: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["result"], "durable");
    assert_eq!(receipt["authority_state"], "withdrawn");
    assert_eq!(receipt["current_permission"], false);
    assert_eq!(receipt["continuation_blocked"], true);
    assert_eq!(receipt["complete_history"], true);
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 1);
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        )
        .args(args)
        .output()
        .unwrap()
}

fn receipt(output: &std::process::Output) -> Value {
    assert!(output.stderr.is_empty());
    let text = std::str::from_utf8(&output.stdout).unwrap();
    let view: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(view["current_permission"], false);
    assert_eq!(view["continuation_blocked"], true);
    view
}

fn input(e: EngagementId, c: CampaignId) -> Value {
    json!({"engagement_id": e, "campaign_id": c, "operator_ref": Uuid::from_u128(99),
        "expected_mission_revision": 1, "reason": "authorization_ended"})
}

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(value: &Value, op: OperationId) -> Self {
        let path = std::env::temp_dir().join(format!("dw-c1a-cli-{op}.json"));
        std::fs::write(&path, value.to_string()).unwrap();
        Self(path)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for InputFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}
fn withdraw(op: OperationId, path: &str, recover: bool) -> std::process::Output {
    cli(&[
        "withdraw",
        "--operation",
        &op.to_string(),
        "--input",
        path,
        "--recover",
        if recover { "true" } else { "false" },
    ])
}
fn admin() -> postgres::Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    config.connect(NoTls).unwrap()
}

#[test]
fn failed_unknown_and_unavailable_history_views_remain_honest_and_blocked() {
    let _guard = db();
    let (e, c) = scope(2);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let file = InputFile::new(&input(e, c), op);
    let not_committed = receipt(&withdraw(op, file.path(), true));
    assert_eq!(not_committed["result"], "not_committed");
    let mut admin = admin();
    admin
        .batch_execute("REVOKE INSERT ON mission.withdrawals FROM dw_runtime")
        .unwrap();
    let rejected = withdraw(op, file.path(), false);
    admin
        .batch_execute("GRANT INSERT ON mission.withdrawals TO dw_runtime")
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(receipt(&rejected)["result"], "rejected");
    assert_eq!(count("mission.withdrawals", e, c), 0);
    // Commit-time failure is conservatively UNKNOWN, not a lost-ACK simulation.
    admin
        .batch_execute(&format!(
            "CREATE FUNCTION mission.c1a_commit_fault() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'SYNTHETIC_SECRET_SENTINEL'; END $$; \
         CREATE CONSTRAINT TRIGGER c1a_commit_fault AFTER INSERT ON mission.withdrawals \
         DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN (NEW.engagement_id='{e}'::uuid) \
         EXECUTE FUNCTION mission.c1a_commit_fault();"
        ))
        .unwrap();
    let unknown = withdraw(op, file.path(), false);
    admin.batch_execute("DROP TRIGGER c1a_commit_fault ON mission.withdrawals; DROP FUNCTION mission.c1a_commit_fault();").unwrap();
    assert!(!unknown.status.success());
    let unknown_view = receipt(&unknown);
    assert_eq!(unknown_view["result"], "unknown");
    assert_eq!(unknown_view["operation"], op.to_string());
    assert_eq!(unknown_view["action"], "recover_before_retry");
    assert!(!String::from_utf8_lossy(&unknown.stdout).contains("SYNTHETIC_SECRET_SENTINEL"));
    assert_eq!(count("mission.withdrawals", e, c), 0);
    assert_eq!(
        receipt(&withdraw(op, file.path(), true))["result"],
        "not_committed"
    );
    admin
        .batch_execute("REVOKE SELECT ON trajectory.withdrawal_history FROM dw_runtime")
        .unwrap();
    let accepted = withdraw(op, file.path(), false);
    admin
        .batch_execute("GRANT SELECT ON trajectory.withdrawal_history TO dw_runtime")
        .unwrap();
    assert!(accepted.status.success());
    let view = receipt(&accepted);
    assert_eq!(view["result"], "durable");
    assert_eq!(view["authority_state"], "withdrawn");
    assert_eq!(view["history"], "unknown");
    assert_eq!(view["history_reason"], "history_unavailable");
    assert_eq!(view["complete_history"], false);
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    assert_eq!(
        receipt(&withdraw(op, file.path(), true))["history"],
        "pending"
    );
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    let completed = receipt(&withdraw(op, file.path(), false));
    assert_eq!(completed["contract"], view["contract"]);
    assert_eq!(completed["complete_history"], true);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 1);
}

#[test]
fn missing_predecessor_is_pending_and_recovery_remains_read_only() {
    let _guard = db();
    let (e, c) = scope(3);
    let (mut alloc, mut store, _trajectory) = ports();
    let reg_op = registration::prepare_operation(&mut alloc).unwrap();
    let (mission, registered) =
        Mission::register(&reg_input(e, c), reg_op, EventId(Uuid::from_u128(70)), 1).unwrap();
    store.commit_registration(&mission, &registered).unwrap();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let file = InputFile::new(&input(e, c), op);
    let pending = receipt(&withdraw(op, file.path(), false));
    assert_eq!(pending["result"], "durable");
    assert_eq!(pending["history_reason"], "missing_predecessor");
    assert_eq!(pending["complete_history"], false);
    let recovered = receipt(&withdraw(op, file.path(), true));
    assert_eq!(recovered["contract"], pending["contract"]);
    assert_eq!(recovered["history_reason"], "not_recorded");
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
}

#[test]
fn strict_flags_and_sensitive_rejected_input_never_echo_paths_or_values() {
    let _guard = db();
    let (e, c) = scope(4);
    let (mut alloc, _store, _trajectory) = ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let mut invalid = input(e, c);
    invalid["secret"] = "SYNTHETIC_SECRET_SENTINEL".into();
    let file = InputFile::new(&invalid, op);
    let operation = op.to_string();
    for args in [
        vec!["withdraw"],
        vec![
            "withdraw",
            "--operation",
            &operation,
            "--input",
            file.path(),
            "--recover",
            "yes",
        ],
        vec![
            "withdraw",
            "--operation",
            &operation,
            "--input",
            file.path(),
            "--recover",
            "false",
            "--recover",
            "true",
        ],
        vec![
            "withdraw",
            "--operation",
            &operation,
            "--input",
            file.path(),
            "--recover",
            "true",
            "--secret",
            "SYNTHETIC_SECRET_SENTINEL",
        ],
        vec![
            "withdraw",
            "--operation",
            &operation,
            "--input",
            file.path(),
            "--recover",
            "false",
        ],
        vec![
            "withdraw",
            "--operation",
            &operation,
            "--input",
            "SYNTHETIC_SECRET_SENTINEL",
            "--recover",
            "false",
        ],
    ] {
        let output = cli(&args);
        assert!(!output.status.success());
        assert_eq!(receipt(&output)["result"], "rejected");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(!text.contains("SYNTHETIC_SECRET_SENTINEL"));
        assert!(!text.contains(file.path()));
    }
    assert_eq!(count("mission.withdrawals", e, c), 0);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
}

#[test]
fn fresh_dialogue_cannot_override_marker_and_unrecorded_uncertainty_requires_reconciliation() {
    let _guard = db();
    let (e, c) = scope(5);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let withdraw_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawal_file = InputFile::new(&input(e, c), withdraw_op);
    let mut admin = admin();
    admin
        .batch_execute("REVOKE INSERT ON mission.withdrawals FROM dw_runtime")
        .unwrap();
    let failed = withdraw(withdraw_op, withdrawal_file.path(), false);
    admin
        .batch_execute("GRANT INSERT ON mission.withdrawals TO dw_runtime")
        .unwrap();
    assert_eq!(receipt(&failed)["result"], "rejected");
    assert_eq!(count("mission.withdrawals", e, c), 0);
    let planning = json!({"engagement_id": e, "campaign_id": c,
        "purpose_ref": Uuid::from_u128(0x13), "asset_ref": Uuid::from_u128(0x21),
        "expected_mission_revision": 1, "current_authority_confirmed": true});
    for response in [None, Some(b"{}".as_slice())] {
        let op = registration::prepare_operation(&mut alloc).unwrap();
        let file = InputFile::new(&planning, op);
        let mut session = confirm_support::assess_session(
            &op.to_string(),
            file.path(),
            false,
            &std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        );
        session.challenge();
        if let Some(response) = response {
            session.respond(response);
        }
        let finished = session.finish();
        assert!(!finished.status.success());
        assert_eq!(count("mission.planning_assessments", e, c), 0);
    }
    // C0 is a trusted-operator boundary for unrecorded uncertainty, not a durable barrier.
    assert!(
        receipt(&withdraw(withdraw_op, withdrawal_file.path(), false))["complete_history"]
            .as_bool()
            .unwrap()
    );
    for confirmed in [true, false] {
        let op = registration::prepare_operation(&mut alloc).unwrap();
        let mut changed = planning.clone();
        changed["current_authority_confirmed"] = confirmed.into();
        let file = InputFile::new(&changed, op);
        let view = if confirmed {
            // Known withdrawal skips the fresh challenge and exits with open
            // stdin: no challenge line, a durable v3 refusal, exit success.
            let session = confirm_support::assess_session(
                &op.to_string(),
                file.path(),
                false,
                &std::env::var("DW_TEST_DATABASE_URL").unwrap(),
            );
            let finished = session.finish_with_open_input();
            assert!(finished.status.success());
            assert!(
                finished.challenge_line.is_empty(),
                "known withdrawal must not prompt"
            );
            serde_json::from_slice::<Value>(&finished.stdout).unwrap()
        } else {
            let output = cli(&[
                "assess",
                "--operation",
                &op.to_string(),
                "--input",
                file.path(),
                "--recover",
                "false",
            ]);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            serde_json::from_slice::<Value>(&output.stdout).unwrap()
        };
        assert_eq!(view["result"], "durable");
        assert_eq!(view["contract"]["decision"], "refused_authority_withdrawn");
        assert_eq!(view["current_permission"], false);
        assert_eq!(view["complete_assessment"], false);
    }
    assert_eq!(count("mission.planning_assessments", e, c), 2);
    assert_eq!(count("mission.withdrawals", e, c), 1);
    let inspected = cli(&[
        "inspect",
        "--engagement",
        &e.to_string(),
        "--campaign",
        &c.to_string(),
    ]);
    assert!(String::from_utf8_lossy(&inspected.stdout).contains("mission.revision=2"));
}

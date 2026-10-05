use duskweave::mission::*;
use duskweave::registration;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest, WithdrawalStore};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Output};
use uuid::Uuid;

#[path = "support/authority_confirmation_cli.rs"]
mod confirm_support;
#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

struct InputFile(PathBuf);
impl InputFile {
    fn new(label: &str, op: &str, body: &Value) -> Self {
        let file = Self(std::env::temp_dir().join(format!("dw-positive-{label}-{op}.json")));
        std::fs::write(&file.0, body.to_string()).unwrap();
        file
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

fn cli(args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear().env(
        "DW_DATABASE_URL",
        std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    );
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.args(args).output().unwrap()
}
fn receipt(out: Output) -> Value {
    assert!(
        out.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn prepare(e: EngagementId, c: CampaignId) -> String {
    let out = cli(&[
        "prepare-operation",
        "--engagement",
        &e.to_string(),
        "--campaign",
        &c.to_string(),
    ]);
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .find_map(|s| s.strip_prefix("operation=").map(str::to_owned))
        .unwrap()
}
fn planning(e: EngagementId, c: CampaignId) -> Value {
    json!({"engagement_id":e, "campaign_id":c, "purpose_ref":Uuid::from_u128(0x13),
        "asset_ref":Uuid::from_u128(0x21), "expected_mission_revision":1,
        "current_authority_confirmed":true})
}
fn command(name: &str, op: &str, file: &InputFile, recover: bool) -> Output {
    cli(&[
        name,
        "--operation",
        op,
        "--input",
        file.path(),
        "--recover",
        if recover { "true" } else { "false" },
    ])
}
fn register(e: EngagementId, c: CampaignId) {
    let now: i64 = runtime_client()
        .query_one("SELECT floor(extract(epoch FROM now()))::bigint", &[])
        .unwrap()
        .get(0);
    let mut body = reg_json(e, c);
    body["starts_at"] = json!(now - 7_200);
    body["ends_at"] = json!(now + 7_200);
    let op = prepare(e, c);
    let file = InputFile::new("registration", &op, &body);
    let out = cli(&["register", "--operation", &op, "--input", file.path()]);
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("history=completed")
    );
}
fn live_assess(e: EngagementId, c: CampaignId) -> (String, InputFile, Value) {
    let op = prepare(e, c);
    let file = InputFile::new("assessment", &op, &planning(e, c));
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
    session.affirm(&e.to_string(), &c.to_string(), &op, 1);
    let done = session.finish();
    assert!(done.status.success());
    assert!(done.stderr_tail.is_empty());
    let view: Value = serde_json::from_slice(&done.stdout).unwrap();
    assert_eq!(view["contract"]["version"], 4);
    assert_eq!(view["contract"]["decision"], "eligible");
    assert_eq!(view["scope"], "matched");
    assert_eq!(view["window"], "within_window");
    assert_eq!(view["history"], "pending");
    assert_eq!(view["complete_assessment"], false);
    assert_eq!(view["current_permission"], false);
    (op, file, view)
}

#[test]
fn cli_live_confirmation_positive_history_withdrawal_and_archival_restart() {
    let _guard = db();
    let (e, c) = scope(0xd50);
    register(e, c);
    let op = prepare(e, c);
    let file = InputFile::new("assessment", &op, &planning(e, c));
    let unaffirmed = command("assess", &op, &file, false);
    assert!(!unaffirmed.status.success());
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    let mut fake = confirm_support::assess_session(
        &op,
        file.path(),
        false,
        &std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    );
    let challenge = fake.challenge();
    assert_eq!(challenge["operation_id"], op);
    fake.respond(
        json!({"challenge_id":Uuid::from_u128(0x55),"statement":confirm_support::STATEMENT})
            .to_string()
            .as_bytes(),
    );
    let failed = fake.finish();
    assert!(!failed.status.success());
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    let (op, file, produced) = {
        let mut session = confirm_support::assess_session(
            &op,
            file.path(),
            false,
            &std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        );
        session.affirm(&e.to_string(), &c.to_string(), &op, 1);
        let finished = session.finish();
        assert!(finished.status.success());
        assert!(finished.stderr_tail.is_empty());
        let view: Value = serde_json::from_slice(&finished.stdout).unwrap();
        (op, file, view)
    };
    assert_eq!(produced["contract"]["version"], 4);
    assert_eq!(produced["contract"]["decision"], "eligible");
    assert_eq!(produced["history"], "pending");
    assert_eq!(produced["complete_assessment"], false);
    assert_eq!(produced["current_permission"], false);
    let pending = receipt(command("planning-history", &op, &file, true));
    assert_eq!(pending["history"], "pending");
    assert_eq!(pending["complete_assessment"], false);
    let completed = receipt(command("planning-history", &op, &file, false));
    assert_eq!(completed["history"], "completed");
    assert_eq!(completed["contract"], produced["contract"]);
    assert_eq!(completed["complete_assessment"], true);
    assert_eq!(completed["current_permission"], false);
    let wop = prepare(e, c);
    let wfile = InputFile::new(
        "withdraw",
        &wop,
        &json!({"engagement_id":e,"campaign_id":c,
        "operator_ref":Uuid::from_u128(99),"expected_mission_revision":1,"reason":"operator_requested"}),
    );
    let wview = receipt(command("withdraw", &wop, &wfile, false));
    assert_eq!(wview["authority_state"], "withdrawn");
    let new_op = prepare(e, c);
    let new_file = InputFile::new("assessment", &new_op, &planning(e, c));
    let session = confirm_support::assess_session(
        &new_op,
        new_file.path(),
        false,
        &std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    );
    let refused = session.finish_with_open_input();
    assert!(refused.status.success());
    assert!(refused.challenge_line.is_empty());
    let refusal: Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert_eq!(refusal["contract"]["version"], 3);
    assert_eq!(
        refusal["contract"]["decision"],
        "refused_authority_withdrawn"
    );
    assert_eq!(refusal["complete_assessment"], false);
    assert_eq!(
        receipt(command("planning-history", &op, &file, true)),
        completed
    );
    assert_eq!(receipt(command("assess", &op, &file, true)), produced);
    assert_eq!(count("mission.planning_assessments", e, c), 2);
    assert_eq!(
        count_where(
            "trajectory.planning_history",
            "AND status='accepted' AND version=4",
            e,
            c
        ),
        1
    );
}

#[test]
fn historical_positive_completes_after_withdrawal_even_when_withdrawal_history_is_unavailable() {
    let _guard = db();
    let (e, c) = scope(0xd51);
    register(e, c);
    let (op, file, produced) = live_assess(e, c);
    let (mut allocator, mut store, _) = ports();
    let wop = registration::prepare_operation(&mut allocator).unwrap();
    store
        .withdraw(
            &WithdrawalRequest {
                engagement_id: e,
                campaign_id: c,
                operator_ref: OperatorRef(Uuid::from_u128(99)),
                expected_mission_revision: 1,
                reason: WithdrawalReason::OperatorRequested,
            },
            wop,
            false,
            &mut allocator,
        )
        .unwrap()
        .unwrap();
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    let fresh_op = prepare(e, c);
    let fresh_file = InputFile::new("assessment", &fresh_op, &planning(e, c));
    let refused = confirm_support::assess_session(
        &fresh_op,
        fresh_file.path(),
        false,
        &std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    )
    .finish_with_open_input();
    assert!(refused.status.success());
    assert!(refused.challenge_line.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&refused.stdout).unwrap()["contract"]["version"],
        3
    );
    let historical = receipt(command("planning-history", &op, &file, false));
    assert_eq!(historical["contract"], produced["contract"]);
    assert_eq!(historical["complete_assessment"], true);
    assert_eq!(historical["current_permission"], false);
    assert_eq!(
        receipt(command("planning-history", &op, &file, true)),
        historical
    );
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
}

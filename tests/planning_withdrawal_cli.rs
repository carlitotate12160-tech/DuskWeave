//! Real CLI/restart assertions for the version-3 recorded-withdrawal refusal:
//! a fresh `assess` after a committed withdrawal records a durable refusal
//! with no challenge (including open stdin), and `planning-history` publishes
//! or inspects the original decision.

use duskweave::mission::*;
use duskweave::registration;
use duskweave::trajectory::Delivered;
use duskweave::withdrawal::*;
use serde_json::{Value, json};
use std::process::{Command, Output};
use uuid::Uuid;

#[path = "support/authority_confirmation_cli.rs"]
mod confirm_support;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .env_remove("DW_DATABASE_URL_FILE")
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        )
        .args(args)
        .output()
        .unwrap()
}

fn planning_json(e: EngagementId, c: CampaignId, revision: u64, confirmed: bool) -> Value {
    json!({
        "engagement_id": e, "campaign_id": c,
        "purpose_ref": Uuid::from_u128(0x13), "asset_ref": Uuid::from_u128(0x21),
        "expected_mission_revision": revision, "current_authority_confirmed": confirmed,
    })
}

struct InputFile(std::path::PathBuf);
impl InputFile {
    fn new(value: &Value, op: OperationId) -> Self {
        let path = std::env::temp_dir().join(format!("dw-c1b-cli-{op}.json"));
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

fn assess(op: &str, path: &str, recover: bool) -> Output {
    cli(&[
        "assess",
        "--operation",
        op,
        "--input",
        path,
        "--recover",
        if recover { "true" } else { "false" },
    ])
}

fn assert_v3_receipt(view: &Value) {
    assert_eq!(view["result"], "durable");
    assert_eq!(view["contract"]["version"], 3);
    assert_eq!(view["contract"]["owner_revision"], 2);
    assert_eq!(view["contract"]["decision"], "refused_authority_withdrawn");
    assert_eq!(view["current_permission"], false);
    assert_eq!(view["complete_assessment"], false);
    assert_eq!(view["scope"], "not_evaluated");
    assert_eq!(view["window"], "not_evaluated");
}

#[test]
fn cli_assess_after_withdrawal_records_v3_without_prompt_and_history_completes() {
    let _guard = db();
    let (e, c) = scope(0xc20);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawn = store
        .withdraw(
            &WithdrawalRequest {
                engagement_id: e,
                campaign_id: c,
                operator_ref: OperatorRef(Uuid::from_u128(99)),
                expected_mission_revision: 1,
                reason: WithdrawalReason::AuthorizationEnded,
            },
            w_op,
            false,
            &mut alloc,
        )
        .unwrap()
        .unwrap();
    assert_eq!(traj.publish(&withdrawn).unwrap(), Delivered::Completed);
    // A brand-new confirmed request after withdrawal: no challenge, exit
    // success with open stdin, and a durable v3 refusal.
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let file = InputFile::new(&planning_json(e, c, 1, true), op);
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
    let view: Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_v3_receipt(&view);
    assert_eq!(
        view["contract"]["withdrawal"]["operation_id"],
        w_op.to_string()
    );
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    // planning-history publishes the original v3 decision and completes.
    let history = cli(&[
        "planning-history",
        "--operation",
        &op.to_string(),
        "--input",
        file.path(),
        "--recover",
        "false",
    ]);
    assert!(
        history.status.success(),
        "{}",
        String::from_utf8_lossy(&history.stdout)
    );
    let hview: Value = serde_json::from_slice(&history.stdout).unwrap();
    assert_eq!(hview["result"], "durable");
    assert_eq!(hview["contract"]["decision"], "refused_authority_withdrawn");
    assert_eq!(hview["complete_history"], true);
    assert_eq!(hview["current_permission"], false);
    assert_eq!(
        count_where("trajectory.planning_history", "AND status='accepted'", e, c),
        1
    );
    // A fresh process inspect is read-only and returns the same decision.
    let inspect = cli(&[
        "planning-history",
        "--operation",
        &op.to_string(),
        "--input",
        file.path(),
        "--recover",
        "true",
    ]);
    assert!(inspect.status.success());
    let iview: Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(iview["contract"], view["contract"]);
    assert_eq!(
        count_where("trajectory.planning_history", "AND status='accepted'", e, c),
        1
    );
}

#[test]
fn cli_stale_revision_after_withdrawal_still_records_v3_refusal() {
    let _guard = db();
    let (e, c) = scope(0xc22);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    store
        .withdraw(
            &WithdrawalRequest {
                engagement_id: e,
                campaign_id: c,
                operator_ref: OperatorRef(Uuid::from_u128(99)),
                expected_mission_revision: 1,
                reason: WithdrawalReason::ScopeConcern,
            },
            w_op,
            false,
            &mut alloc,
        )
        .unwrap()
        .unwrap();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let file = InputFile::new(&planning_json(e, c, 99, false), op);
    let output = assess(&op.to_string(), file.path(), false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let view: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_v3_receipt(&view);
    assert_eq!(count("mission.planning_assessments", e, c), 1);
}

#[test]
fn cli_rejects_malformed_after_withdrawal_and_recovery_stays_read_only() {
    let _guard = db();
    let (e, c) = scope(0xc24);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    store
        .withdraw(
            &WithdrawalRequest {
                engagement_id: e,
                campaign_id: c,
                operator_ref: OperatorRef(Uuid::from_u128(99)),
                expected_mission_revision: 1,
                reason: WithdrawalReason::OperatorRequested,
            },
            w_op,
            false,
            &mut alloc,
        )
        .unwrap()
        .unwrap();
    // Malformed input rejects without effects or echo.
    let mut invalid = planning_json(e, c, 1, false);
    invalid["secret"] = "SYNTHETIC_SECRET_SENTINEL".into();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let file = InputFile::new(&invalid, op);
    let output = assess(&op.to_string(), file.path(), false);
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("SYNTHETIC_SECRET_SENTINEL"));
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    // Recovery of an unknown operation is read-only and empty.
    let unknown = registration::prepare_operation(&mut alloc).unwrap();
    let file2 = InputFile::new(&planning_json(e, c, 1, false), unknown);
    let output = assess(&unknown.to_string(), file2.path(), true);
    assert!(output.status.success());
    let view: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(view["result"], "not_committed");
    assert_eq!(count("mission.planning_assessments", e, c), 0);
}

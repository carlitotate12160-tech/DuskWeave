//! Real-process proof that the confirmation exchange completes on one
//! newline-delimited response line without waiting for stdin EOF: the child
//! must exit on its own while this harness keeps its stdin open. A CLI that
//! blocked on EOF would miss the helper's bounded deadline instead.

use duskweave::mission::{CampaignId, EngagementId};
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
    fn new(label: &str, value: &Value) -> Self {
        let file = Self(std::env::temp_dir().join(format!("dw-m0c-c0s-{label}.json")));
        std::fs::write(&file.0, value.to_string()).unwrap();
        file
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn cli(args: &[&str], dsn: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    cmd.env("DW_DATABASE_URL", dsn);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.args(args).output().expect("spawn CLI")
}

fn prepare(e: &str, c: &str, dsn: &str) -> String {
    let out = cli(
        &["prepare-operation", "--engagement", e, "--campaign", c],
        dsn,
    );
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .find_map(|s| s.strip_prefix("operation=").map(str::to_owned))
        .unwrap()
}

fn register(e: &str, c: &str, dsn: &str) {
    let body = reg_json(
        EngagementId::parse(e).unwrap(),
        CampaignId::parse(c).unwrap(),
    );
    let file = InputFile::new(&format!("reg-{e}"), &body);
    let op = prepare(e, c, dsn);
    let out = cli(
        &["register", "--operation", &op, "--input", file.path()],
        dsn,
    );
    assert!(out.status.success(), "register failed");
}

fn request_json(e: &EngagementId, c: &CampaignId) -> Value {
    json!({
        "engagement_id": e.0, "campaign_id": c.0,
        "purpose_ref": Uuid::from_u128(0x13),
        "asset_ref": Uuid::from_u128(0x21),
        "expected_mission_revision": 1,
        "current_authority_confirmed": true,
    })
}

#[test]
fn complete_response_line_exits_without_stdin_eof() {
    let _guard = db();
    let (e, c) = scope(0xc5a1);
    let (es, cs) = (e.to_string(), c.to_string());
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    register(&es, &cs, &dsn);
    let op = prepare(&es, &cs, &dsn);
    let file = InputFile::new(&op, &request_json(&e, &c));

    let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
    let challenge = session.affirm(&es, &cs, &op, 1);
    assert_eq!(challenge["operation_id"], op);

    // stdin stays open through exit; a child waiting for EOF would miss the
    // bounded deadline inside finish_with_open_input rather than proceed.
    let finished = session.finish_with_open_input();
    assert!(finished.status.success());
    assert!(finished.stderr_tail.is_empty());
    let receipt: Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_eq!(receipt["result"], "durable");
    assert_eq!(receipt["contract"]["version"], 2);
    assert_eq!(receipt["contract"]["operation_id"], op);
    assert_eq!(receipt["contract"]["request"], request_json(&e, &c));
    assert_eq!(receipt["decision_origin"], "durable_record");
    assert_eq!(
        receipt["publication_obligation"],
        "trajectory.planning_history.v1"
    );
    assert_eq!(receipt["complete_assessment"], false);
    assert_eq!(receipt["current_permission"], false);
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    assert_eq!(count("trajectory.planning_history", e, c), 0);
}

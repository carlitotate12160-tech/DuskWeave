//! Native subprocess coverage of v2 purpose/asset/window refusals through the
//! real CLI assess -> planning-history -> recovery path. Window fixtures use
//! wide deterministic intervals around a database-observed timestamp.

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
        let file = Self(std::env::temp_dir().join(format!("dw-b2-{label}.json")));
        file.write(value);
        file
    }
    fn write(&self, value: &Value) {
        std::fs::write(&self.0, value.to_string()).unwrap();
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

fn json_output(output: Output) -> Value {
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn prepare(e: &str, c: &str) -> String {
    let output = cli(&["prepare-operation", "--engagement", e, "--campaign", c]);
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .find_map(|s| s.strip_prefix("operation=").map(str::to_owned))
        .unwrap()
}

fn planning(command: &str, op: &str, file: &InputFile, recover: &str) -> Output {
    cli(&[
        command,
        "--operation",
        op,
        "--input",
        file.path(),
        "--recover",
        recover,
    ])
}

fn db_epoch() -> i64 {
    runtime_client()
        .query_one("SELECT floor(extract(epoch FROM now()))::bigint", &[])
        .unwrap()
        .get(0)
}

fn register_scope(e: &str, c: &str, starts_at: i64, ends_at: i64) {
    let mut body = reg_json(
        duskweave::mission::EngagementId::parse(e).unwrap(),
        duskweave::mission::CampaignId::parse(c).unwrap(),
    );
    body["starts_at"] = json!(starts_at);
    body["ends_at"] = json!(ends_at);
    let input = InputFile::new(&format!("reg-{e}"), &body);
    let op = prepare(e, c);
    let output = cli(&["register", "--operation", &op, "--input", input.path()]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("history=completed")
    );
}

fn request_json(e: &str, c: &str, revision: u64, confirmed: bool) -> Value {
    json!({
        "engagement_id": e,
        "campaign_id": c,
        "purpose_ref": Uuid::from_u128(0x13),
        "asset_ref": Uuid::from_u128(0x21),
        "expected_mission_revision": revision,
        "current_authority_confirmed": confirmed,
    })
}

fn assert_labels(receipt: &Value, scope: &str, window: &str, complete: bool) {
    assert_eq!(receipt["scope"], scope);
    assert_eq!(receipt["window"], window);
    assert_eq!(receipt["complete_assessment"], complete);
    assert_eq!(receipt["current_permission"], false);
}

struct CliCase {
    window: Option<(i64, i64)>,
    overrides: Value,
    decision: &'static str,
    scope: &'static str,
    window_label: &'static str,
}

#[test]
fn cli_scope_and_window_refusals_publish_and_recover_identically() {
    let _guard = db();
    let now = db_epoch();
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let case = |window, overrides, decision, scope, window_label| CliCase {
        window,
        overrides,
        decision,
        scope,
        window_label,
    };
    let cases = vec![
        case(
            None,
            json!({}),
            "unresolved_mission_basis",
            "not_evaluated",
            "not_evaluated",
        ),
        case(
            Some((now - 14_400, now - 7_200)),
            json!({}),
            "refused_expired",
            "matched",
            "expired",
        ),
        case(
            Some((now + 7_200, now + 14_400)),
            json!({}),
            "refused_not_yet_valid",
            "matched",
            "not_yet_valid",
        ),
        case(
            Some((now - 7_200, now + 7_200)),
            json!({}),
            "eligible",
            "matched",
            "within_window",
        ),
        case(
            Some((now - 7_200, now + 7_200)),
            json!({"purpose_ref": Uuid::from_u128(0xb2c9)}),
            "refused_purpose_mismatch",
            "purpose_mismatch",
            "not_evaluated",
        ),
        case(
            Some((now - 7_200, now + 7_200)),
            json!({"asset_ref": Uuid::from_u128(0x23)}),
            "refused_asset_excluded",
            "excluded",
            "not_evaluated",
        ),
        case(
            Some((now - 7_200, now + 7_200)),
            json!({"asset_ref": Uuid::from_u128(0xb2ca)}),
            "refused_asset_unknown",
            "unknown",
            "not_evaluated",
        ),
        case(
            Some((now - 7_200, now + 7_200)),
            json!({"expected_mission_revision": 2}),
            "refused_revision_mismatch",
            "not_evaluated",
            "not_evaluated",
        ),
        case(
            Some((now - 7_200, now + 7_200)),
            json!({"current_authority_confirmed": false}),
            "unresolved_authority_unconfirmed",
            "not_evaluated",
            "not_evaluated",
        ),
    ];
    for (i, case) in cases.iter().enumerate() {
        let (e, c) = scope(0xb320 + i as u128);
        let (engagement, campaign) = (e.to_string(), c.to_string());
        if let Some((starts_at, ends_at)) = case.window {
            register_scope(&engagement, &campaign, starts_at, ends_at);
        }
        let mut body = request_json(&engagement, &campaign, 1, true);
        for (key, value) in case.overrides.as_object().unwrap() {
            body[key] = value.clone();
        }
        let file = InputFile::new(&format!("req-{e}"), &body);
        let op = prepare(&engagement, &campaign);
        // A saved `current_authority_confirmed=true` request must pass the
        // fresh live exchange in this invocation; false takes the unchanged
        // non-interactive path.
        let assessed = if body["current_authority_confirmed"].as_bool().unwrap() {
            let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
            session.affirm(
                &engagement,
                &campaign,
                &op,
                body["expected_mission_revision"].as_u64().unwrap(),
            );
            let finished = session.finish();
            assert!(finished.status.success());
            assert!(finished.stderr_tail.is_empty());
            serde_json::from_slice(&finished.stdout).unwrap()
        } else {
            json_output(planning("assess", &op, &file, "false"))
        };
        let eligible = case.decision == "eligible";
        assert_eq!(
            assessed["contract"]["version"],
            if eligible { 4 } else { 2 }
        );
        assert_eq!(assessed["contract"]["decision"], case.decision);
        assert_labels(&assessed, case.scope, case.window_label, false);
        let pending = json_output(planning("planning-history", &op, &file, "true"));
        assert_eq!(pending["history"], "pending");
        assert_labels(&pending, case.scope, case.window_label, false);
        let completed = json_output(planning("planning-history", &op, &file, "false"));
        assert_eq!(completed["history"], "completed");
        assert_eq!(completed["contract"], assessed["contract"]);
        assert_labels(&completed, case.scope, case.window_label, eligible);
        // A fresh process recovers the exact original durable record.
        let restarted = json_output(planning("planning-history", &op, &file, "true"));
        assert_eq!(restarted, completed);
        let recovered = json_output(planning("assess", &op, &file, "true"));
        assert_eq!(recovered, assessed);
        assert_eq!(count("mission.planning_assessments", e, c), 1);
        assert_eq!(
            count_where("trajectory.planning_history", "AND status='accepted'", e, c),
            1
        );
    }
}

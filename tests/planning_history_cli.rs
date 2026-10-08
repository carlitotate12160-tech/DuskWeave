//! Native subprocess path; recovery runs a new process against real storage.
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Output};
use uuid::Uuid;

#[path = "support/authority_confirmation_cli.rs"]
mod confirm_support;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use db_support::*;
use history_support::*;

struct InputFile(PathBuf);

impl InputFile {
    fn new(label: &str, value: &Value) -> Self {
        let file = Self(std::env::temp_dir().join(format!("dw-b1b-{label}.json")));
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
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn cli(args: &[&str], configured: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear().env("DW_DATABASE_CONFIG_MODE", "env-local");
    if configured {
        cmd.env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        );
    }
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

fn operation(e: &str, c: &str) -> String {
    let output = cli(
        &["prepare-operation", "--engagement", e, "--campaign", c],
        true,
    );
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .find_map(|s| s.strip_prefix("operation=").map(str::to_owned))
        .unwrap()
}

fn planning(command: &str, op: &str, file: &InputFile, recover: &str) -> Output {
    cli(
        &[
            command,
            "--operation",
            op,
            "--input",
            file.path(),
            "--recover",
            recover,
        ],
        true,
    )
}

#[test]
fn real_cli_register_assess_publish_restart_inspect_covers_all_nonpositive_decisions() {
    let _guard = db();
    for (slot, registered, revision, confirmed, decision, scope_label, window_label) in [
        (
            0xb1c0,
            false,
            1,
            true,
            "unresolved_mission_basis",
            "not_evaluated",
            "not_evaluated",
        ),
        (
            0xb1c1,
            true,
            2,
            true,
            "refused_revision_mismatch",
            "not_evaluated",
            "not_evaluated",
        ),
        (
            0xb1c2,
            true,
            1,
            false,
            "unresolved_authority_unconfirmed",
            "not_evaluated",
            "not_evaluated",
        ),
        // The fixed fixture window is in the past: v2 evaluates it to refused_expired.
        (
            0xb1c3,
            true,
            1,
            true,
            "refused_expired",
            "matched",
            "expired",
        ),
    ] {
        let (e, c) = scope(slot);
        let (engagement, campaign) = (e.to_string(), c.to_string());
        if registered {
            let input = InputFile::new(&format!("registration-{e}"), &reg_json(e, c));
            let op = operation(&engagement, &campaign);
            let output = cli(
                &["register", "--operation", &op, "--input", input.path()],
                true,
            );
            assert!(output.status.success());
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("history=completed")
            );
        }
        let mut value = serde_json::to_value(request(e, c)).unwrap();
        value["expected_mission_revision"] = json!(revision);
        value["current_authority_confirmed"] = json!(confirmed);
        let file = InputFile::new(&e.to_string(), &value);
        let op = operation(&engagement, &campaign);
        // Saved `true` assertions now pass a fresh live exchange in this
        // invocation; the durable contract and receipt stay unchanged.
        let assessed = if confirmed {
            let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
            let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
            session.affirm(&engagement, &campaign, &op, revision);
            let finished = session.finish();
            assert!(finished.status.success());
            assert!(finished.stderr_tail.is_empty());
            serde_json::from_slice(&finished.stdout).unwrap()
        } else {
            json_output(planning("assess", &op, &file, "false"))
        };
        assert_eq!(assessed["contract"]["decision"], decision);
        assert_eq!(assessed["contract"]["version"], 2);
        assert_eq!(assessed["scope"], scope_label);
        assert_eq!(assessed["window"], window_label);
        assert_eq!(assessed["history_view"], "producer_receipt_as_of_decision");
        let pending = json_output(planning("planning-history", &op, &file, "true"));
        assert_eq!(pending["history"], "pending");
        assert_eq!(count("trajectory.planning_history", e, c), 0);
        let completed = json_output(planning("planning-history", &op, &file, "false"));
        assert_eq!(completed["contract"], assessed["contract"]);
        assert_eq!(completed["history"], "completed");
        assert_eq!(completed["complete_history"], true);
        assert_eq!(completed["history_source"], "trajectory");
        for receipt in [&pending, &completed] {
            assert_eq!(receipt["complete_assessment"], false);
            assert_eq!(receipt["current_permission"], false);
            assert_eq!(receipt["scope"], scope_label);
            assert_eq!(receipt["window"], window_label);
        }
        let restarted = json_output(planning("planning-history", &op, &file, "true"));
        assert_eq!(restarted, completed);
        let duplicate = json_output(planning("planning-history", &op, &file, "false"));
        assert_eq!(duplicate, completed);
        let producer_recovery = json_output(planning("assess", &op, &file, "true"));
        assert_eq!(producer_recovery, assessed);
        assert_eq!(count("mission.planning_assessments", e, c), 1);
        assert_eq!(count("trajectory.planning_history", e, c), 1);
    }
}

#[test]
fn changed_intent_fails_before_consumer_and_missing_producer_has_no_effect() {
    let _guard = db();
    let ev = decision(0xb1c4, false);
    let op = ev.operation_id.to_string();
    let original = serde_json::to_value(&ev.request).unwrap();
    let file = InputFile::new(&op, &original);
    for (field, value) in [
        ("purpose_ref", json!(Uuid::from_u128(900))),
        ("asset_ref", json!(Uuid::from_u128(901))),
        ("expected_mission_revision", json!(2)),
        ("current_authority_confirmed", json!(false)),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        file.write(&changed);
        for recover in ["true", "false"] {
            let output = planning("planning-history", &op, &file, recover);
            assert!(!output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                "error=integrity_conflict"
            );
            effects(&ev, 0, 0);
        }
    }
    file.write(&original);
    for recover in ["true", "false"] {
        let absent = json_output(planning(
            "planning-history",
            &Uuid::from_u128(902).to_string(),
            &file,
            recover,
        ));
        assert_eq!(absent["result"], "not_committed");
        effects(&ev, 0, 0);
    }
}

#[test]
fn invalid_input_is_rejected_without_configuration_or_sensitive_echo() {
    let sentinel = "B1B_REJECTED_SENSITIVE_CANARY";
    let op = Uuid::from_u128(903).to_string();
    let file = InputFile::new(
        &format!("invalid-{}", std::process::id()),
        &json!({"secret": sentinel}),
    );
    let valid_args = [
        "planning-history",
        "--operation",
        &op,
        "--input",
        file.path(),
        "--recover",
        "true",
    ];
    for bad in [
        format!("{{\"secret\":\"{sentinel}\"}}"),
        sentinel.to_string(),
        "x".repeat(16385),
    ] {
        std::fs::write(&file.0, bad).unwrap();
        let out = cli(&valid_args, false);
        assert!(!out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(!text.contains(sentinel));
        assert!(!text.contains("missing_env"));
        assert!(out.stderr.is_empty());
    }
    file.write(&serde_json::to_value(request(scope(0xb1c5).0, scope(0xb1c5).1)).unwrap());
    for args in [
        vec!["planning-history"],
        vec![
            "planning-history",
            "--operation",
            &op,
            "--input",
            file.path(),
            "--recover",
            "maybe",
        ],
        vec![
            "planning-history",
            "--operation",
            "00000000-0000-0000-0000-000000000000",
            "--input",
            file.path(),
            "--recover",
            "true",
        ],
        vec![
            "planning-history",
            "--operation",
            &op,
            "--input",
            file.path(),
            "--recover",
            "true",
            "--unknown",
            sentinel,
        ],
        vec![
            "planning-history",
            "--operation",
            &op,
            "--input",
            file.path(),
            "--recover",
            "true",
            "--recover",
            "false",
        ],
    ] {
        let out = cli(&args, false);
        assert!(!out.status.success());
        assert_eq!(
            String::from_utf8(out.stdout).unwrap().trim(),
            "error=invalid_args"
        );
        assert!(out.stderr.is_empty());
    }
}

#[test]
fn noncanonical_recover_rejected_before_file_or_config() {
    let sentinel = format!("dw-sensitive-{}", std::process::id());
    let op = Uuid::from_u128(0x5a5a_0000_0000_0000_0000_0000_0000_0001).to_string();
    let missing = std::env::temp_dir()
        .join(format!("dw-absent-{sentinel}.json"))
        .to_string_lossy()
        .to_string();
    assert!(!std::path::Path::new(&missing).exists());
    let out = cli(
        &[
            "planning-history",
            "--operation",
            &op,
            "--input",
            &missing,
            "--recover",
            &sentinel,
        ],
        false,
    );
    assert!(!out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "error=invalid_args");
    assert!(!stdout.contains(&sentinel));
    assert!(out.stderr.is_empty());
}

#[test]
fn consumer_connection_failure_retains_durable_decision_and_recovery_has_no_effect() {
    let _guard = db();
    let original = decision(0xb1c6, false);
    let op = original.operation_id.to_string();
    let file = InputFile::new(&op, &serde_json::to_value(&original.request).unwrap());
    let role = dsn("DW_TEST_DATABASE_URL").get_user().unwrap().to_owned();
    assert!(role.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    let mut administrator = admin();
    // The producer client occupies the sole connection; opening the consumer
    // client then fails deterministically, without sleeps or a port race.
    administrator
        .batch_execute(&format!("ALTER ROLE {role} CONNECTION LIMIT 1"))
        .unwrap();
    let output = planning("planning-history", &op, &file, "false");
    administrator
        .batch_execute(&format!("ALTER ROLE {role} CONNECTION LIMIT -1"))
        .unwrap();
    let receipt = json_output(output);
    assert_eq!(
        receipt["contract"],
        serde_json::to_value(&original).unwrap()
    );
    assert_eq!(receipt["history"], "unknown");
    assert_eq!(receipt["history_reason"], "history_unavailable");
    assert_eq!(receipt["action"], "recover_history_before_retry");
    assert_eq!(receipt["complete_history"], false);
    effects(&original, 0, 0);
    let recovery = json_output(planning("planning-history", &op, &file, "true"));
    assert_eq!(recovery["history"], "pending");
    effects(&original, 0, 0);
    let published = json_output(planning("planning-history", &op, &file, "false"));
    assert_eq!(published["history"], "completed");
    effects(&original, 1, 0);
}

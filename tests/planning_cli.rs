//! Real CLI duplicate and conflict controls for durable M0B assessment.

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
    fn new(operation: &str) -> Self {
        Self(std::env::temp_dir().join(format!("dw-b1a-planning-{operation}.json")))
    }

    fn write(&self, input: &Value) {
        std::fs::write(&self.0, input.to_string()).expect("write planning input");
    }

    fn path(&self) -> &str {
        self.0.to_str().expect("input path UTF-8")
    }
}

impl Drop for InputFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).expect("remove owned planning input");
    }
}

fn cli(args: &[&str], dsn: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    command.env_clear().env("DW_DATABASE_URL", dsn);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    command.args(args).output().expect("spawn CLI")
}

fn assess(operation: &str, input: &str, recover: bool, dsn: &str) -> Output {
    cli(
        &[
            "assess",
            "--operation",
            operation,
            "--input",
            input,
            "--recover",
            if recover { "true" } else { "false" },
        ],
        dsn,
    )
}

/// A new `current_authority_confirmed=true` assessment requires the live
/// challenge/response exchange in the current invocation.
fn assess_confirmed(
    operation: &str,
    input: &str,
    engagement: &str,
    campaign: &str,
    revision: u64,
    dsn: &str,
) -> Value {
    let mut session = confirm_support::assess_session(operation, input, false, dsn);
    session.affirm(engagement, campaign, operation, revision);
    let finished = session.finish();
    assert!(finished.status.success(), "assess failed");
    assert!(finished.stderr_tail.is_empty(), "unexpected CLI stderr");
    serde_json::from_slice(&finished.stdout).expect("durable JSON receipt")
}

fn prepare(engagement: &str, campaign: &str, dsn: &str) -> String {
    let output = cli(
        &[
            "prepare-operation",
            "--engagement",
            engagement,
            "--campaign",
            campaign,
        ],
        dsn,
    );
    assert!(output.status.success(), "prepare-operation failed");
    String::from_utf8(output.stdout)
        .expect("operation receipt UTF-8")
        .split_whitespace()
        .find_map(|part| part.strip_prefix("operation=").map(str::to_owned))
        .expect("prepared operation")
}

fn durable(output: Output) -> Value {
    assert!(output.status.success(), "assess failed");
    assert!(output.stderr.is_empty(), "unexpected CLI stderr");
    serde_json::from_slice(&output.stdout).expect("durable JSON receipt")
}

fn effects(engagement: EngagementId, campaign: CampaignId, planning: i64) {
    for (table, expected) in [
        ("mission.missions", 1),
        ("mission.registration_outbox", 1),
        ("trajectory.registration_history", 1),
        ("mission.planning_assessments", planning),
    ] {
        assert_eq!(count(table, engagement, campaign), expected, "{table}");
    }
}

#[test]
fn cli_fresh_duplicate_conflicts_and_recovery_keep_one_original() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1af);
    let (mut allocator, mut store, mut trajectory) = ports();
    let registration = accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    effects(engagement, campaign, 0);

    let dsn = std::env::var("DW_TEST_DATABASE_URL").expect("DW_TEST_DATABASE_URL required");
    let operation = prepare(&engagement.to_string(), &campaign.to_string(), &dsn);
    assert_ne!(Uuid::parse_str(&operation).unwrap(), Uuid::nil());
    let input_file = InputFile::new(&operation);
    let input = json!({
        "engagement_id": engagement.0,
        "campaign_id": campaign.0,
        "purpose_ref": Uuid::from_u128(0x13),
        "asset_ref": Uuid::from_u128(0x21),
        "expected_mission_revision": 1,
        "current_authority_confirmed": true,
    });
    input_file.write(&input);

    let original = assess_confirmed(
        &operation,
        input_file.path(),
        &engagement.to_string(),
        &campaign.to_string(),
        1,
        &dsn,
    );
    let contract = &original["contract"];
    assert_eq!(contract["operation_id"], operation);
    assert_eq!(contract["request"], input);
    assert_eq!(
        contract["basis"]["registration_operation_id"],
        registration.operation_id.to_string()
    );
    assert_ne!(contract["event_id"], Uuid::nil().to_string());
    assert_eq!(contract["version"], 2);
    for (field, expected) in [
        ("producer", "mission"),
        ("kind", "planning_assessed"),
        ("decision", "refused_expired"),
    ] {
        assert_eq!(contract[field], expected, "contract.{field}");
    }
    for (field, expected) in [
        ("result", "durable"),
        ("decision_origin", "durable_record"),
        ("publication_obligation", "trajectory.planning_history.v1"),
        ("history", "pending"),
        ("history_reason", "not_published_at_decision"),
        ("basis_status", "available"),
        ("scope", "matched"),
        ("window", "expired"),
    ] {
        assert_eq!(original[field], expected, "receipt.{field}");
    }
    assert_eq!(original["complete_assessment"], false);
    assert_eq!(original["current_permission"], false);
    effects(engagement, campaign, 1);

    assert_eq!(
        durable(assess(&operation, input_file.path(), false, &dsn)),
        original
    );
    effects(engagement, campaign, 1);

    for (field, value) in [
        ("purpose_ref", json!(Uuid::from_u128(0x14))),
        ("asset_ref", json!(Uuid::from_u128(0x22))),
        ("expected_mission_revision", json!(2)),
        ("current_authority_confirmed", json!(false)),
    ] {
        let mut changed = input.clone();
        changed[field] = value;
        input_file.write(&changed);
        for recover in [false, true] {
            let output = assess(&operation, input_file.path(), recover, &dsn);
            assert!(!output.status.success(), "changed {field} accepted");
            assert!(
                output.stdout == b"error=integrity_conflict\n",
                "expected bounded integrity_conflict error"
            );
            assert!(output.stderr.is_empty(), "unexpected CLI stderr");
        }
        effects(engagement, campaign, 1);
    }

    input_file.write(&input);
    assert_eq!(
        durable(assess(&operation, input_file.path(), true, &dsn)),
        original
    );
    let row = runtime_client()
        .query_one(
            "SELECT contract, publication_obligation FROM mission.planning_assessments \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[
                &engagement.0,
                &campaign.0,
                &Uuid::parse_str(&operation).unwrap(),
            ],
        )
        .expect("one scoped planning row");
    assert_eq!(row.get::<_, Value>(0), original["contract"]);
    assert_eq!(row.get::<_, String>(1), "trajectory.planning_history.v1");
    effects(engagement, campaign, 1);
}

#[test]
fn cli_assess_and_fresh_process_recovery_preserve_pending_original() {
    let _guard = db();
    let dsn = std::env::var("DW_TEST_DATABASE_URL").expect("DW_TEST_DATABASE_URL required");
    // (register, expected revision, confirmed, decision, scope, window)
    let cases = [
        (
            false,
            1,
            true,
            "unresolved_mission_basis",
            "not_evaluated",
            "not_evaluated",
        ),
        (
            true,
            2,
            false,
            "refused_revision_mismatch",
            "not_evaluated",
            "not_evaluated",
        ),
        (
            true,
            1,
            false,
            "unresolved_authority_unconfirmed",
            "not_evaluated",
            "not_evaluated",
        ),
        (true, 1, true, "refused_expired", "matched", "expired"),
    ];
    for (i, (register, revision, confirmed, decision, scope_label, window_label)) in
        cases.iter().enumerate()
    {
        let (engagement, campaign) = scope(0xb2d0 + i as u128);
        let (es, cs) = (engagement.to_string(), campaign.to_string());
        let operation = prepare(&es, &cs, &dsn);
        if *register {
            // Mission registration stays on the real CLI. The fixture keeps
            // its fixed past window [1700000000, 1700086400).
            let registration_file = InputFile::new(&format!("reg-{operation}"));
            registration_file.write(&reg_json(engagement, campaign));
            let registration_operation = prepare(&es, &cs, &dsn);
            let registered = cli(
                &[
                    "register",
                    "--operation",
                    &registration_operation,
                    "--input",
                    registration_file.path(),
                ],
                &dsn,
            );
            assert!(registered.status.success(), "register failed");
        }
        let file = InputFile::new(&operation);
        file.write(&json!({
            "engagement_id": engagement.0,
            "campaign_id": campaign.0,
            "purpose_ref": Uuid::from_u128(0x13),
            "asset_ref": Uuid::from_u128(0x21),
            "expected_mission_revision": revision,
            "current_authority_confirmed": confirmed,
        }));

        let absent = assess(&operation, file.path(), true, &dsn);
        assert!(absent.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&absent.stdout).unwrap()["result"],
            "not_committed"
        );
        let original = if *confirmed {
            assess_confirmed(&operation, file.path(), &es, &cs, *revision, &dsn)
        } else {
            durable(assess(&operation, file.path(), false, &dsn))
        };
        assert_eq!(original["result"], "durable");
        assert_eq!(original["contract"]["decision"], *decision);
        assert_eq!(original["history"], "pending");
        assert_eq!(original["history_reason"], "not_published_at_decision");
        assert_eq!(original["complete_assessment"], false);
        assert_eq!(original["current_permission"], false);
        assert_eq!(original["scope"], *scope_label);
        assert_eq!(original["window"], *window_label);
        assert_eq!(
            original["basis_status"],
            if *register {
                "available"
            } else {
                "unavailable"
            }
        );
        if *decision == "refused_expired" {
            assert_eq!(original["contract"]["version"], 2);
            assert!(
                original["contract"]["evaluated_at"].as_i64().unwrap() >= 1_700_086_400,
                "v2 expiry is judged at the recorded evaluation time"
            );
        }
        let recovered = durable(assess(&operation, file.path(), true, &dsn));
        assert_eq!(recovered, original);
        assert_eq!(
            count("mission.planning_assessments", engagement, campaign),
            1
        );
    }
}

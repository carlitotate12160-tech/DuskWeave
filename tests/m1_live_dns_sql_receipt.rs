//! Dedicated schema-0030 restricted-SQL regression target: the ordinary
//! suite runs only this bootstrap wrapper, which re-enters this executable
//! inside a supervised dw_native_ fixture for the ignored `sql_live` group.
#![allow(dead_code)]

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_live_sql.rs"]
mod sql;
#[path = "support/m1_terrain.rs"]
mod terrain_support;

use duskweave::m1_effect_control::authority::RestrictedWithdrawal;
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use duskweave::postgres_m1_effect::PgStopWriter;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest};
use postgres::NoTls;
use serde_json::{Value, json};
use sql::*;
use uuid::Uuid;

const GROUP: &str = "sql_live";

#[test]
fn dedicated_schema0030_fixture_executes_the_receipt_assertion_group() {
    if worker() {
        return;
    }
    let unique = format!(
        "{:x}_{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let database = format!("dw_native_livesql_{unique}");
    let database = &database[..database.len().min(63)];
    let report = std::env::temp_dir().join(format!("dw-livesql-{unique}"));
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    });
    let mut command = std::process::Command::new(python);
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(sql::MARKER, "sql-worker-v1")
        .env_remove("DEEPSEEK_API_KEY")
        .arg("tests/support/m1_native_bootstrap.py")
        .args([
            "--container",
            if cfg!(windows) { "dwpg-r3" } else { "dwpg" },
        ]);
    if !cfg!(windows) {
        command.arg("--ci");
    }
    command
        .args([
            "--database",
            "fixture",
            "--database-name",
            database,
            "--schema-revision",
            "0030",
            "--role-prefix",
            &format!("dw_lsq{}", &unique[..unique.len().min(10)]),
            "--report",
        ])
        .arg(&report)
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([GROUP, "--ignored", "--test-threads=1", "--nocapture"]);
    let output = command.output().expect("bootstrap supervisor spawn");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dedicated schema-0030 fixture failed; report {}: {stdout}\n{stderr}",
        report.display()
    );
    let passed = stdout
        .split("test result: ok. ")
        .nth(1)
        .and_then(|rest| rest.split(" passed").next())
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(0);
    assert!(passed >= 5, "SQL assertion group must execute: {stdout}");
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_receipt_binds_qualification_fields() {
    assert!(worker());
    let (e, c, _operator) = register_live(0x5d01, true);
    let (run, _, _) = fabricate_live_run(e, c, false, true);
    let qualified = |run: Uuid| -> bool {
        broker()
            .query_one(
                "SELECT execution.run_summary_without_diagnostics($1)",
                &[&run],
            )
            .unwrap()
            .get::<_, Value>(0)["live_qualified"]
            .as_bool()
            .unwrap()
    };
    assert!(
        qualified(run),
        "matching manifest qualification is live_qualified"
    );
    // Receipt comparison binds release, not merely the qualification ref.
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET release_sha256=$1 WHERE run_profile=$2",
            &[&"f".repeat(64), &LAB],
        )
        .unwrap();
    assert!(!qualified(run), "release mismatch cannot be live_qualified");
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET valid_until_ms=2 WHERE run_profile=$1",
            &[&LAB],
        )
        .unwrap();
    assert!(
        !qualified(run),
        "expired qualification cannot be live_qualified"
    );
}

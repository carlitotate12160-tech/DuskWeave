//! Target-private native bootstrap wrapper for schema-0035 SQL assertions.
use super::fixture::{GROUP, MARKER, worker};

pub fn run_fixture_wrapper() {
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
        .env(MARKER, "sql-worker-v1")
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
            "0035",
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
        "dedicated schema-0035 fixture failed; report {}: {stdout}\n{stderr}",
        report.display()
    );
    let log = std::fs::read_to_string(report.join("qualification.log"))
        .expect("complete sanitized worker log");
    let passed = log
        .split("test result: ok. ")
        .nth(1)
        .and_then(|rest| rest.split(" passed").next())
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(0);
    for name in [
        "pro_budget::sql_live_pro_v2_and_mirror_admission_and_cross_profile_matrix",
        "pro_budget::sql_live_fixture_pro_v2_cli_feedback_repair_incomplete_and_key_free_recovery",
        "pro_profile::sql_live_pro_intent_generation_and_repair_are_profile_bound",
        "pro_profile::sql_live_pro_accounting_and_terminal_diagnostics_preserve_model_identity",
        "pro_profile::sql_live_pro_missing_key_refuses_and_key_free_recovery_preserves_identity",
        "successor_pack::sql_live_successor_pack_v2_v3_v4_contract_matrix",
        "diagnostic_episode::sql_live_proposal_diagnostics_survive_terminal_repair_and_recovery",
        "successor_episode::sql_live_sdk_v3_cli_feedback_and_key_free_recovery_on_schema0030",
        "audit007::sql_live_aud007_original_writer_isolation_survives_temp_pg_roles",
        "audit007::sql_live_aud007_inference_and_live_stop_refuse_foreign_writer",
        "audit007::sql_live_aud007_catalog_paths_and_qualification_use_real_catalogs",
        "lease_coordination::sql_live_lease_renewal_does_not_own_release_fence",
        "lease_coordination::sql_live_lease_coordination_rejects_invalid_renewals",
        "lease_coordination::sql_live_authority_mutations_still_fence_release",
        "lease_coordination_races::sql_live_lease_renewal_preserves_stop_and_expiry",
        "pro_budget::diagnostic::sql_live_pro_v2_diagnostic_version_elapsed_matrix",
        "pro_budget::accounting::sql_live_pro_v2_production_profile_intent_settlement_and_ledger",
        "pro_budget::accounting::sql_live_pro_v2_capacity_exhaustion_and_unknown_charge",
        "pro_budget::live_binding::sql_live_pro_v2_live_qualification_start_and_current",
    ] {
        let line = format!("test {name} ... ok");
        assert_eq!(log.matches(&line).count(), 1, "{name} missing: {log}");
    }
    assert!(passed >= 27, "SQL assertion group must execute: {log}");
    eprintln!(
        "schema0035_sql_group passed={passed} report={}",
        report.display()
    );
}

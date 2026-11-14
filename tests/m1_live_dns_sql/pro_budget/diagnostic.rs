//! Schema-0035 diagnostic version and elapsed bound matrix through the restricted writer.
use super::super::fixture::*;
use super::super::run_support::*;
use super::register_pro_v2;
use duskweave::m1_run::RunPhase;
use duskweave::postgres_m1_run::PgRunStore;
use serde_json::{Value, json};
use uuid::Uuid;

fn diagnostic_json(version: &str, run: Uuid, elapsed_ms: i64, guard: Option<&str>) -> Value {
    let mut payload = json!({
        "version": version,
        "run_id": run,
        "phase": "decision_one",
        "cause": "child_exit",
        "cause_source": "parent_observed",
        "child_error": null,
        "exit_code": 124,
        "readiness_observed": false,
        "elapsed_ms": elapsed_ms
    });
    if let Some(guard) = guard {
        payload["reported_guard"] = json!(guard);
    }
    payload
}

fn started_run(slot: u128, profile: Option<&str>) -> Uuid {
    let (e, c, _) = match profile {
        Some(p) => register_pro_v2(slot, p),
        None => register_live(slot, false),
    };
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    let request = successor_request(&publication, run, "m1_autonomous_run_request_v3");
    let mut runs = PgRunStore::new(broker());
    assert!(matches!(runs.start(&request, RELEASE), Ok(true)));
    runs.start_phase(run, RunPhase::DecisionOne).unwrap();
    run
}

fn row_count(run: Uuid) -> i64 {
    admin()
        .query_one(
            "SELECT count(*) FROM execution.run_phase_diagnostics WHERE run_id=$1",
            &[&run],
        )
        .unwrap()
        .get(0)
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_pro_v2_diagnostic_version_elapsed_matrix() {
    assert!(worker());
    let cases = [
        (
            0xaa10_u128,
            None,
            "m1_run_phase_diagnostic_v1",
            150000_i64,
            None,
            true,
        ),
        (
            0xaa11,
            None,
            "m1_run_phase_diagnostic_v1",
            150001,
            None,
            false,
        ),
        (
            0xaa12,
            Some("deepseek-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v1",
            200000,
            None,
            false,
        ),
        (
            0xaa13,
            Some("fixture-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v1",
            200000,
            None,
            false,
        ),
        (
            0xaa14,
            Some("deepseek-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v2",
            200000,
            Some("transport_hard_deadline"),
            true,
        ),
        (
            0xaa15,
            Some("deepseek-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v2",
            650000,
            None,
            true,
        ),
        (
            0xaa16,
            Some("fixture-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v2",
            650000,
            None,
            true,
        ),
        (
            0xaa17,
            Some("deepseek-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v2",
            650001,
            None,
            false,
        ),
        (
            0xaa18,
            Some("fixture-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v2",
            650001,
            None,
            false,
        ),
        (
            0xaa19,
            None,
            "m1_run_phase_diagnostic_v2",
            200000,
            None,
            false,
        ),
        (
            0xaa1a,
            Some("deepseek-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v3",
            1000,
            None,
            false,
        ),
        (
            0xaa1b,
            Some("deepseek-genai-v4-pro-low-v2"),
            "m1_run_phase_diagnostic_v1",
            1000,
            Some("command_watchdog"),
            false,
        ),
    ];

    let mut port = broker();
    for (slot, profile, version, elapsed_ms, guard, expected_ok) in cases {
        let run = started_run(slot, profile);
        let payload = diagnostic_json(version, run, elapsed_ms, guard);
        let res = port.execute("SELECT execution.run_phase_diagnostic($1)", &[&payload]);
        assert_eq!(
            res.is_ok(),
            expected_ok,
            "diagnostic matrix mismatch for profile={profile:?} version={version} elapsed_ms={elapsed_ms}: {res:?}"
        );
        assert_eq!(
            row_count(run),
            i64::from(expected_ok),
            "diagnostic row persistence mismatch for profile={profile:?} version={version} elapsed_ms={elapsed_ms}"
        );
    }
}

#![allow(dead_code)]
//! Privileged Linux ARM64 active-HEAD STOP and control-loss fault tests (`#[ignore]`).

#[cfg(target_os = "linux")]
use super::active_effect_probe::linux::{probe_attempt_disposal, wait_for_active_head};
#[cfg(target_os = "linux")]
use super::active_effect_probe::{
    ActiveHeadBinding, ActiveWindowObservation, DisposalEvidence, LauncherSessionEvidence,
    validate_disposal_evidence,
};
#[cfg(target_os = "linux")]
use super::support::linux_host::NativeHost;
#[cfg(target_os = "linux")]
use super::support::start_loopback_sdk;
#[cfg(target_os = "linux")]
use crate::lab_support::sanitize_inspect_receipt;
#[cfg(target_os = "linux")]
use crate::sdk_fixture::Server;
#[cfg(target_os = "linux")]
use serde_json::{Value, json};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use uuid::Uuid;

#[cfg(target_os = "linux")]
fn wait_for_disposal(
    attempt: Uuid,
    launcher: &LauncherSessionEvidence,
    timeout: Duration,
) -> Result<DisposalEvidence, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let ev = probe_attempt_disposal(
            attempt,
            &launcher.observed_pids,
            &launcher.observed_container_ids,
        );
        if validate_disposal_evidence(&ev).is_ok() {
            return Ok(ev);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "attempt {attempt} disposal unresolved: {}",
                serde_json::to_string(&ev).unwrap_or_default()
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(target_os = "linux")]
fn run_fresh_cli_recovery(
    host: &mut NativeHost,
    sdk: &Server,
    run_id: Uuid,
    label: &str,
    expected_counters: &Value,
    expected_sdk_calls: usize,
    extra: Value,
) -> (Value, Value, Value) {
    let run_str = run_id.to_string();
    let assert_zero_replay = |h: &NativeHost, step: &str| {
        assert_eq!(
            &h.https_counters(),
            expected_counters,
            "{label} {step} replayed network/DNS I/O"
        );
        assert_eq!(
            sdk.count(),
            expected_sdk_calls,
            "{label} {step} replayed SDK inference calls"
        );
    };

    assert_zero_replay(host, "pre_inspect");
    let insp = host.cli(&["m1-run-inspect", "--run", &run_str]);
    assert!(
        insp.status.success(),
        "m1-run-inspect failed on {label}: {}",
        String::from_utf8_lossy(&insp.stderr)
    );
    assert_zero_replay(host, "post_inspect");

    let rec1 = host.cli(&["m1-run-recover", "--run", &run_str]);
    assert!(
        rec1.status.success(),
        "m1-run-recover #1 failed on {label}: {}",
        String::from_utf8_lossy(&rec1.stderr)
    );
    assert_zero_replay(host, "post_recover_1");

    let rec2 = host.cli(&["m1-run-recover", "--run", &run_str]);
    assert!(
        rec2.status.success(),
        "m1-run-recover #2 failed on {label}: {}",
        String::from_utf8_lossy(&rec2.stderr)
    );
    assert_zero_replay(host, "post_recover_2");

    let insp_san = sanitize_inspect_receipt(&NativeHost::receipt(&insp)).unwrap();
    let rec1_san = sanitize_inspect_receipt(&NativeHost::receipt(&rec1)).unwrap();
    let rec2_san = sanitize_inspect_receipt(&NativeHost::receipt(&rec2)).unwrap();
    assert_eq!(rec1_san, rec2_san, "{label} recover must be idempotent");
    let insp_stdout = String::from_utf8_lossy(&insp.stdout);
    assert!(
        !insp_stdout.contains("://")
            && !insp_stdout.contains("password=")
            && !insp_stdout.contains("DW_"),
        "unsafe inspect stdout"
    );

    let artifact = json!({
        "label": label,
        "source_sha": host.source_sha,
        "run_id": run_id,
        "fault_evidence": extra,
        "counters": host.https_counters(),
        "inspect_stdout": insp_stdout.trim(),
        "inspect_sanitized": insp_san,
        "recover_1_sanitized": rec1_san,
        "recover_2_sanitized": rec2_san,
    });
    host.save_structured_artifact(label, &artifact);
    (insp_san, rec1_san, rec2_san)
}

#[cfg(target_os = "linux")]
fn find_session_row(snapshot: &Value, attempt_id: Uuid) -> &Value {
    let target = attempt_id.to_string();
    snapshot["sessions"]
        .as_array()
        .and_then(|arr| arr.iter().find(|s| s["attempt"].as_str() == Some(&target)))
        .expect("head attempt session missing from authoritative SQL snapshot")
}

#[cfg(target_os = "linux")]
fn start_timeout_run(host: &mut NativeHost) -> (Uuid, Server, crate::sdk_fixture::FixtureChild) {
    let run_id = host.id();
    host.start_topology_for("timeout", run_id, 30, 30);
    let sdk = start_loopback_sdk("none");
    let run_file = host.prepare_run_input(run_id, |_| {});
    let parent = host.spawn_cli(&[
        "m1-run",
        "--input",
        run_file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ]);
    (run_id, sdk, parent)
}

#[cfg(target_os = "linux")]
fn assert_one_head_counters(hc: &Value, run_id: Uuid) {
    assert_eq!(hc["run_id"], run_id.to_string());
    assert_eq!(hc["scenario"], "timeout");
    assert_eq!(hc["dns_answered"], 1);
    assert_eq!(hc["connections"], 1);
    assert_eq!(hc["tls_handshakes"], 1);
    assert_eq!(hc["head_requests"], 1);
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_active_stop_during_head_revokes_without_replay() {
    let mut host = NativeHost::new();
    let (run_id, sdk, mut parent) = start_timeout_run(&mut host);
    let (binding, obs): (ActiveHeadBinding, ActiveWindowObservation) = wait_for_active_head(
        &mut host,
        &mut parent,
        run_id,
        "timeout",
        Duration::from_secs(20),
    )
    .unwrap();
    let before_sql = host.sql_snapshot(run_id).unwrap();
    let before_counters = host.https_counters();
    let sdk_calls_before = sdk.count();
    let fault_at_ms = host.db_now_s() * 1000;

    let stop_out = host.cli(&["m1-run-stop", "--run", &run_id.to_string()]);
    assert!(
        stop_out.status.success(),
        "m1-run-stop failed: {}",
        String::from_utf8_lossy(&stop_out.stderr)
    );
    let stop_receipt = NativeHost::receipt(&stop_out);
    assert_eq!(stop_receipt["local_stop"]["stop_latched"], true);

    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(15))
            .is_some(),
        "m1-run did not exit within 15s after active HEAD STOP"
    );
    let disposal =
        wait_for_disposal(binding.attempt_id, &obs.launcher, Duration::from_secs(8)).unwrap();
    let after_counters = host.https_counters();
    assert_one_head_counters(&after_counters, run_id);
    assert_eq!(sdk.count(), sdk_calls_before);

    let after_sql = host.sql_snapshot(run_id).unwrap();
    assert_eq!(after_sql["control"]["status"], "stopped");
    let head_sess = find_session_row(&after_sql, binding.attempt_id);
    assert_eq!(head_sess["possibly_started"], true);
    assert_eq!(head_sess["granted_phase"], "head");

    let extra = json!({
        "fault": "m1-run-stop",
        "step_index": binding.step_index,
        "attempt_id": binding.attempt_id,
        "session_generation": binding.session_generation,
        "fault_at_ms": fault_at_ms,
        "launcher_before": obs.launcher,
        "before_counters": before_counters,
        "after_counters": after_counters,
        "stop_receipt": stop_receipt,
        "before_sql": before_sql,
        "after_sql": after_sql,
        "disposal": disposal,
    });
    let (insp_san, rec1_san, _) = run_fresh_cli_recovery(
        &mut host,
        &sdk,
        run_id,
        "active-head-stop",
        &after_counters,
        sdk_calls_before,
        extra,
    );
    assert_eq!(insp_san["status"], "stopped");
    assert_eq!(rec1_san["status"], "stopped");
    host.dispose().unwrap();
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_active_control_loss_during_head_recovers_without_replay() {
    let mut host = NativeHost::new();
    let (run_id, sdk, mut parent) = start_timeout_run(&mut host);
    let (binding, obs) = wait_for_active_head(
        &mut host,
        &mut parent,
        run_id,
        "timeout",
        Duration::from_secs(20),
    )
    .unwrap();
    let before_sql = host.sql_snapshot(run_id).unwrap();
    let before_counters = host.https_counters();
    let sdk_calls_before = sdk.count();
    let fault_at_ms = host.db_now_s() * 1000;

    let faulted_dir = host.inject_control_root_loss("active-head").unwrap();
    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(15))
            .is_some(),
        "m1-run did not exit within 15s after control-root loss"
    );
    let disposal =
        wait_for_disposal(binding.attempt_id, &obs.launcher, Duration::from_secs(8)).unwrap();
    let after_counters = host.https_counters();
    assert_one_head_counters(&after_counters, run_id);
    assert_eq!(sdk.count(), sdk_calls_before);

    let after_sql = host.sql_snapshot(run_id).unwrap();
    let head_sess = find_session_row(&after_sql, binding.attempt_id);
    assert_eq!(head_sess["possibly_started"], true);
    assert_eq!(head_sess["granted_phase"], "head");

    let extra = json!({
        "fault": "private_core_control_root_loss",
        "faulted_sibling_name": faulted_dir.file_name().and_then(|s| s.to_str()),
        "step_index": binding.step_index,
        "attempt_id": binding.attempt_id,
        "session_generation": binding.session_generation,
        "fault_at_ms": fault_at_ms,
        "launcher_before": obs.launcher,
        "before_counters": before_counters,
        "after_counters": after_counters,
        "before_sql": before_sql,
        "after_sql": after_sql,
        "disposal": disposal,
    });
    run_fresh_cli_recovery(
        &mut host,
        &sdk,
        run_id,
        "active-head-control-loss",
        &after_counters,
        sdk_calls_before,
        extra,
    );
    assert!(faulted_dir.exists());
    host.dispose().unwrap();
}

#[cfg(test)]
mod tests {
    use super::super::active_effect_probe::*;
    use uuid::Uuid;

    fn valid_observation(run_id: Uuid, attempt: Uuid) -> ActiveWindowObservation {
        ActiveWindowObservation {
            sql: SqlActiveState {
                run_id,
                step_index: 2,
                step_status: "acquisition_running".into(),
                resolved_endpoint_ref: Some("endpoint_0".into()),
                dns_result_present: true,
                head_attempt_id: Some(attempt),
                attempt_kind: Some("head".into()),
                session_generation: Some(2),
                control_phase: Some("may_start".into()),
                granted_phase: Some("head".into()),
                possibly_started: true,
                disposal_confirmed: false,
                has_may_start_event: true,
                has_phase_head_event: true,
                has_result_recorded_event: false,
            },
            launcher: LauncherSessionEvidence {
                run_id,
                attempt,
                session_generation: 2,
                granted_phase: Some("head".into()),
                started_marker_present: true,
                observed_pids: vec![1234],
                observed_container_ids: vec!["abc".into()],
            },
            responder: ResponderCounterEvidence {
                run_id: run_id.to_string(),
                scenario: "timeout".into(),
                dns_answered: 1,
                connections: 1,
                tls_handshakes: 1,
                head_requests: 1,
            },
        }
    }

    #[test]
    fn active_head_validator_rejects_pre_head_completed_and_mismatched_identities() {
        let (run_id, attempt) = (Uuid::new_v4(), Uuid::new_v4());
        let ok_obs = valid_observation(run_id, attempt);
        let binding = validate_active_head_window(run_id, "timeout", &ok_obs).unwrap();
        assert_eq!(
            (
                binding.run_id,
                binding.attempt_id,
                binding.session_generation
            ),
            (run_id, attempt, 2)
        );
        let expect_err = |name: &str, f: fn(&mut ActiveWindowObservation)| {
            let mut obs = ok_obs.clone();
            f(&mut obs);
            assert!(
                validate_active_head_window(run_id, "timeout", &obs).is_err(),
                "{name}"
            );
        };
        expect_err("pre_head_tls", |o| o.sql.granted_phase = Some("tls".into()));
        expect_err("reservation_only", |o| o.sql.possibly_started = false);
        expect_err("missing_phase_head", |o| o.sql.has_phase_head_event = false);
        expect_err("already_completed", |o| {
            o.sql.step_status = "completed".into()
        });
        expect_err("result_recorded", |o| {
            o.sql.has_result_recorded_event = true
        });
        expect_err("missing_started", |o| {
            o.launcher.started_marker_present = false
        });
        expect_err("attempt_mismatch", |o| o.launcher.attempt = Uuid::new_v4());
        expect_err("gen_mismatch", |o| o.launcher.session_generation = 9);
        expect_err("run_mismatch", |o| {
            o.responder.run_id = Uuid::new_v4().to_string()
        });
        expect_err("zero_head", |o| o.responder.head_requests = 0);
    }

    #[test]
    fn disposal_validator_fails_closed_on_residuals_and_probe_errors() {
        let clean = DisposalEvidence {
            attempt_id: Uuid::new_v4(),
            ..Default::default()
        };
        assert!(validate_disposal_evidence(&clean).is_ok());
        let expect_err = |name: &str, f: fn(&mut DisposalEvidence)| {
            let mut bad = clean.clone();
            f(&mut bad);
            assert!(validate_disposal_evidence(&bad).is_err(), "{name}");
        };
        expect_err("probe_error", |e| e.probe_errors.push("nft err".into()));
        expect_err("launcher_dir", |e| e.launcher_dir_present = true);
        expect_err("pending", |e| {
            e.pending_markers.push("dwcap.pending".into())
        });
        expect_err("network_slot", |e| e.network_slot_held = true);
        expect_err("container", |e| e.containers_present.push("dwcap1".into()));
        expect_err("netns", |e| e.netns_present = true);
        expect_err("veth", |e| e.veth_present = true);
        expect_err("nft", |e| e.nft_table_present = true);
        expect_err("pid", |e| e.residual_pids.push(4321));
        expect_err("cgroup", |e| e.residual_cgroups.push("/proc/4321".into()));
    }
}

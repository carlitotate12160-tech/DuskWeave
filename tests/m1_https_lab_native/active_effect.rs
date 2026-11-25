#![allow(dead_code)]
//! Privileged Linux ARM64 active-HEAD STOP and control-loss fault tests (`#[ignore]`).

use super::active_effect_evidence::{ActiveHeadBinding, SqlActiveState, check_all};
#[cfg(target_os = "linux")]
use super::active_effect_evidence::{
    DisposalEvidence, EXPECTED_PRE_FAULT_SDK_CALLS, MIN_INJECTION_MARGIN_MS, WorkerPairBaseline,
    execute_armed_fault, validate_disposal_evidence,
};
#[cfg(target_os = "linux")]
use super::active_effect_probe::linux::{
    prepare_armed_head_window, probe_attempt_disposal, read_sql_active_state,
};
#[cfg(target_os = "linux")]
use super::support::linux_host::NativeHost;
#[cfg(target_os = "linux")]
use super::support::start_loopback_sdk;
#[cfg(target_os = "linux")]
use crate::lab_support::sanitize_inspect_receipt;
#[cfg(target_os = "linux")]
use crate::sdk_fixture::Server;
use serde_json::Value;
#[cfg(target_os = "linux")]
use serde_json::json;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExpectedFaultOutcome {
    Stopped,
    ControlRootLossUnknown,
}

impl ExpectedFaultOutcome {
    fn expected_status(self) -> &'static str {
        if self == Self::Stopped {
            "stopped"
        } else {
            "unknown"
        }
    }

    fn allowed_step3_status(self, status: &str) -> bool {
        let wanted = if self == Self::Stopped {
            "stopped"
        } else {
            "pending"
        };
        status == wanted
    }
}

fn valid_no_result_head_outcome(v: &Value) -> bool {
    v.is_null() || v.as_str() == Some("unknown")
}

fn step_by_index(v: &Value, idx: i64) -> Result<&Value, String> {
    v["steps"]
        .as_array()
        .ok_or("snapshot missing steps")?
        .iter()
        .find(|s| s["step_index"].as_i64() == Some(idx))
        .ok_or_else(|| format!("missing step {idx}"))
}

fn validate_sql_after_fault(
    expected: ExpectedFaultOutcome,
    binding: &ActiveHeadBinding,
    sql: &SqlActiveState,
) -> Result<(), String> {
    let st = expected.expected_status();
    let att_ok = sql.head_attempt_id == Some(binding.attempt_id)
        && sql.attempt_kind.as_deref() == Some("head");
    let phase_ok = sql.possibly_started && sql.granted_phase.as_deref() == Some("head");
    let ev_ok = sql.has_may_start_event && sql.has_phase_head_event;
    let no_res = !sql.has_result_recorded_event && !sql.head_result_present;
    check_all(&[
        (
            sql.run_id == binding.run_id && sql.step_index == 2,
            "run/step mismatch",
        ),
        (
            sql.run_status == st && sql.step_status == st,
            "run/step status mismatch",
        ),
        (att_ok, "head_attempt_id mismatch"),
        (
            sql.session_generation == Some(binding.session_generation as i64),
            "generation mismatch",
        ),
        (phase_ok && ev_ok, "HEAD charge/phase/events lost"),
        (no_res && !sql.d3_executed, "HEAD result or D3 recorded"),
    ])
}

pub(super) fn validate_post_fault_state(
    expected: ExpectedFaultOutcome,
    binding: &ActiveHeadBinding,
    after_sql: &SqlActiveState,
    snap: &Value,
    sdk_calls: usize,
) -> Result<(), String> {
    validate_sql_after_fault(expected, binding, after_sql)?;
    check_all(&[(sdk_calls == 2, "extra SDK call executed")])?;
    let st = expected.expected_status();
    let att = binding.attempt_id.to_string();
    let steps = snap["steps"].as_array().ok_or("snapshot missing steps")?;
    let s2 = steps
        .iter()
        .find(|s| s["step_index"].as_i64() == Some(2))
        .ok_or("missing step 2")?;
    let s3_ok = steps
        .iter()
        .find(|s| s["step_index"].as_i64() == Some(3))
        .is_none_or(|s| {
            s["status"]
                .as_str()
                .is_some_and(|v| expected.allowed_step3_status(v))
        });
    let sess = snap["sessions"]
        .as_array()
        .and_then(|arr| arr.iter().find(|s| s["attempt"].as_str() == Some(&att)))
        .ok_or("missing HEAD session row")?;
    let evs: Vec<&str> = snap["events"]
        .as_array()
        .ok_or("missing events")?
        .iter()
        .filter(|e| e["attempt"].as_str() == Some(&att))
        .filter_map(|e| e["phase"].as_str())
        .collect();
    let s2_ok = s2["status"].as_str() == Some(st) && s2["head_attempt_id"].as_str() == Some(&att);
    let s2_empty =
        s2["head_result_id"].is_null() && valid_no_result_head_outcome(&s2["head_outcome"]);
    let sess_ok = sess["step_index"].as_i64() == Some(2)
        && sess["attempt_kind"].as_str() == Some("head")
        && sess["generation"].as_u64() == Some(binding.session_generation)
        && sess["possibly_started"].as_bool() == Some(true)
        && sess["granted_phase"].as_str() == Some("head");
    let evs_ok = evs.contains(&"may_start")
        && evs.contains(&"phase_head")
        && !evs.contains(&"result_recorded");
    check_all(&[
        (
            snap["control"]["status"].as_str() == Some(st),
            "control.status mismatch",
        ),
        (s2_ok && s2_empty && s3_ok, "snapshot step 2/3 mismatch"),
        (sess_ok && evs_ok, "snapshot session/events invalid"),
    ])
}

fn validate_receipt_projection(
    expected: ExpectedFaultOutcome,
    binding: &ActiveHeadBinding,
    snap_s2: &Value,
    v: &Value,
) -> Result<(), String> {
    let st = expected.expected_status();
    let r = binding.run_id.to_string();
    let att = binding.attempt_id.to_string();
    let s2 = step_by_index(v, 2)?;
    let s3_ok = v["steps"]
        .as_array()
        .and_then(|arr| arr.iter().find(|s| s["step_index"].as_i64() == Some(3)))
        .is_none_or(|s| {
            s["status"]
                .as_str()
                .is_some_and(|w| expected.allowed_step3_status(w))
        });
    let step2_ok = s2["status"].as_str() == Some(st)
        && s2["head_attempt_id"].as_str() == Some(&att)
        && s2["head_result_id"].is_null()
        && valid_no_result_head_outcome(&s2["head_outcome"])
        && s2["head_outcome"] == snap_s2["head_outcome"]
        && s2["disposition"] == snap_s2["disposition"];
    let counts_ok = v["run_id"].as_str() == Some(&r)
        && v["status"].as_str() == Some(st)
        && v["decision_count"].as_u64() == Some(2)
        && v["dns_effect_count"].as_u64() == Some(1)
        && v["head_effect_count"].as_u64() == Some(0);
    check_all(&[(
        counts_ok && step2_ok && s3_ok,
        "recovery receipt fields mismatch",
    )])
}

pub(super) fn validate_recovery_receipts(
    expected: ExpectedFaultOutcome,
    binding: &ActiveHeadBinding,
    post_fault_snap: &Value,
    insp: &Value,
    rec1: &Value,
    rec2: &Value,
) -> Result<(), String> {
    let snap_s2 = step_by_index(post_fault_snap, 2)?;
    check_all(&[(
        valid_no_result_head_outcome(&snap_s2["head_outcome"]),
        "unexpected post-fault head_outcome",
    )])?;
    validate_receipt_projection(expected, binding, snap_s2, insp)?;
    validate_receipt_projection(expected, binding, snap_s2, rec1)?;
    validate_receipt_projection(expected, binding, snap_s2, rec2)?;
    check_all(&[(rec1 == rec2, "m1-run-recover not idempotent")])
}

pub(super) fn validate_post_recover_sql(
    expected: ExpectedFaultOutcome,
    binding: &ActiveHeadBinding,
    before_recover: &SqlActiveState,
    after_recover: &SqlActiveState,
) -> Result<(), String> {
    validate_sql_after_fault(expected, binding, after_recover)?;
    let same = after_recover.disposal_confirmed == before_recover.disposal_confirmed;
    check_all(&[(same, "recover must not fabricate disposal_confirmed")])
}

#[cfg(target_os = "linux")]
fn wait_for_disposal(
    attempt: Uuid,
    baseline: &WorkerPairBaseline,
    timeout: Duration,
) -> Result<DisposalEvidence, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let ev = probe_attempt_disposal(attempt, baseline);
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
) -> Result<(Value, Value, Value), String> {
    let run_str = run_id.to_string();
    let zero_replay = |h: &NativeHost, step: &str| -> Result<(), String> {
        let ok = &h.https_counters() == expected_counters && sdk.count() == expected_sdk_calls;
        check_all(&[(ok, "replayed network/DNS/SDK I/O")])
            .map_err(|e| format!("{label} {step}: {e}"))
    };
    zero_replay(host, "pre_inspect")?;
    let insp = host.cli(&["m1-run-inspect", "--run", &run_str]);
    check_all(&[(insp.status.success(), "m1-run-inspect failed")])?;
    zero_replay(host, "post_inspect")?;
    let rec1 = host.cli(&["m1-run-recover", "--run", &run_str]);
    check_all(&[(rec1.status.success(), "m1-run-recover #1 failed")])?;
    zero_replay(host, "post_recover_1")?;
    let rec2 = host.cli(&["m1-run-recover", "--run", &run_str]);
    check_all(&[(rec2.status.success(), "m1-run-recover #2 failed")])?;
    zero_replay(host, "post_recover_2")?;

    let insp_raw = NativeHost::receipt(&insp);
    let rec1_raw = NativeHost::receipt(&rec1);
    let rec2_raw = NativeHost::receipt(&rec2);
    let insp_san = sanitize_inspect_receipt(&insp_raw).map_err(str::to_string)?;
    let rec1_san = sanitize_inspect_receipt(&rec1_raw).map_err(str::to_string)?;
    let rec2_san = sanitize_inspect_receipt(&rec2_raw).map_err(str::to_string)?;
    let insp_stdout = String::from_utf8_lossy(&insp.stdout);
    if insp_stdout.contains("://")
        || insp_stdout.contains("password=")
        || insp_stdout.contains("DW_")
    {
        return Err("unsafe inspect stdout".into());
    }
    host.save_structured_artifact(
        label,
        &json!({
            "label": label, "source_sha": host.source_sha, "run_id": run_id,
            "fault_evidence": extra, "counters": host.https_counters(),
            "inspect_stdout": insp_stdout.trim(), "inspect_sanitized": insp_san,
            "recover_1_sanitized": rec1_san, "recover_2_sanitized": rec2_san,
        }),
    );
    Ok((insp_raw, rec1_raw, rec2_raw))
}

#[cfg(target_os = "linux")]
fn run_active_fault_scenario<F>(
    host: &mut NativeHost,
    label: &str,
    fault_name: &str,
    expected: ExpectedFaultOutcome,
    inject: F,
) -> Result<(), String>
where
    F: FnOnce(&mut NativeHost, Uuid) -> Result<Value, String>,
{
    let run_id = host.id();
    host.start_topology_for("timeout", run_id, 30, 30);
    let sdk = start_loopback_sdk("none");
    let run_file = host.prepare_run_input(run_id, |_| {});
    let args = [
        "m1-run",
        "--input",
        run_file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ];
    let mut parent = host.spawn_cli(&args);
    let (armed, before_sql, before_counters) = prepare_armed_head_window(
        host,
        &mut parent,
        &sdk,
        run_id,
        "timeout",
        Duration::from_secs(20),
    )?;
    let binding = armed.binding.clone();
    let workers = armed.observation.launcher.workers.clone();

    let (fault_detail, timing) =
        execute_armed_fault(&armed, MIN_INJECTION_MARGIN_MS, Instant::now, || {
            inject(host, run_id)
        })?;

    if parent
        .wait_until(Instant::now() + Duration::from_secs(15))
        .is_none()
    {
        return Err(format!("m1-run did not exit within 15s after {fault_name}"));
    }
    let disposal = wait_for_disposal(binding.attempt_id, &workers, Duration::from_secs(8))?;
    let after_counters = host.https_counters();
    let counters_ok = after_counters["run_id"] == run_id.to_string()
        && after_counters["scenario"] == "timeout"
        && after_counters["dns_answered"] == 1
        && after_counters["connections"] == 1
        && after_counters["tls_handshakes"] == 1
        && after_counters["head_requests"] == 1;
    check_all(&[(counters_ok, "unexpected timeout responder counters")])?;
    let after_sql =
        read_sql_active_state(&mut host.admin, run_id)?.ok_or("step 2 missing after fault")?;
    let after_snapshot = host.sql_snapshot(run_id)?;
    validate_post_fault_state(expected, &binding, &after_sql, &after_snapshot, sdk.count())?;

    let extra = json!({
        "fault": fault_name, "fault_detail": fault_detail, "step_index": binding.step_index,
        "attempt_id": binding.attempt_id, "session_generation": binding.session_generation,
        "workers": workers, "timing": timing, "before_counters": before_counters,
        "after_counters": after_counters, "before_sql": before_sql, "after_sql": after_snapshot,
        "sql_disposal_confirmed_before_recover": after_sql.disposal_confirmed, "disposal": disposal,
    });
    let (insp, rec1, rec2) = run_fresh_cli_recovery(
        host,
        &sdk,
        run_id,
        label,
        &after_counters,
        EXPECTED_PRE_FAULT_SDK_CALLS,
        extra,
    )?;
    validate_recovery_receipts(expected, &binding, &after_snapshot, &insp, &rec1, &rec2)?;
    let post_recover =
        read_sql_active_state(&mut host.admin, run_id)?.ok_or("step 2 missing after recover")?;
    let post_recover_snap = host.sql_snapshot(run_id)?;
    let before_s2 = step_by_index(&after_snapshot, 2)?;
    let after_s2 = step_by_index(&post_recover_snap, 2)?;
    check_all(&[(
        after_s2["head_outcome"] == before_s2["head_outcome"]
            && after_s2["disposition"] == before_s2["disposition"],
        "recover mutated step 2 uncertainty/disposition",
    )])?;
    validate_post_recover_sql(expected, &binding, &after_sql, &post_recover)
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_active_stop_during_head_revokes_without_replay() {
    let mut host = NativeHost::new();
    let res = run_active_fault_scenario(
        &mut host,
        "active-head-stop",
        "m1-run-stop",
        ExpectedFaultOutcome::Stopped,
        |h, run_id| {
            let stop_out = h.cli(&["m1-run-stop", "--run", &run_id.to_string()]);
            if !stop_out.status.success() {
                return Err(format!(
                    "m1-run-stop failed: {}",
                    String::from_utf8_lossy(&stop_out.stderr)
                ));
            }
            let rcpt = NativeHost::receipt(&stop_out);
            if rcpt["local_stop"]["stop_latched"] != true {
                return Err(format!("stop_latched != true: {rcpt}"));
            }
            Ok(json!({ "stop_receipt": rcpt }))
        },
    );
    let dispose_res = host.dispose();
    res.unwrap();
    dispose_res.unwrap();
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_active_control_loss_during_head_recovers_without_replay() {
    let mut host = NativeHost::new();
    let res = run_active_fault_scenario(
        &mut host,
        "active-head-control-loss",
        "private_core_control_root_loss",
        ExpectedFaultOutcome::ControlRootLossUnknown,
        |h, _| {
            let dir = h.inject_control_root_loss("active-head")?;
            if !dir.exists() {
                return Err("faulted sibling dir missing after rename".into());
            }
            Ok(json!({ "faulted_sibling_name": dir.file_name().and_then(|s| s.to_str()) }))
        },
    );
    let dispose_res = host.dispose();
    res.unwrap();
    dispose_res.unwrap();
}

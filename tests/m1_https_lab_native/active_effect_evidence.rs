#![allow(dead_code)]
//! Extracted active-HEAD window and disposal evidence types, validators, and regressions.

use crate::lab_support::is_hex;
use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) const WORKER_OWNER_LABEL: &str = "live-dns-20261030";
pub(super) const MIN_INJECTION_MARGIN_MS: u64 = 2_500;
pub(super) const EXPECTED_PRE_FAULT_SDK_CALLS: usize = 2;
pub(super) const DOCKER_INSPECT_FORMAT: &str = "{{.Id}}\t{{.Name}}\t{{.State.Pid}}\t{{.State.Running}}\t{{index .Config.Labels \"org.duskweave.attempt\"}}\t{{index .Config.Labels \"org.duskweave.owner\"}}";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WorkerRole {
    Capture,
    Exporter,
}

impl WorkerRole {
    pub fn expected_name(self, attempt: Uuid) -> String {
        let p = if self == Self::Capture {
            "dwcap"
        } else {
            "dwexp"
        };
        format!("{p}{}", attempt.simple())
    }

    pub fn label(self) -> &'static str {
        if self == Self::Capture {
            "capture"
        } else {
            "exporter"
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct WorkerIdentity {
    pub role: WorkerRole,
    pub name: String,
    pub container_id: String,
    pub pid: u32,
    pub running: bool,
    pub attempt_label: String,
    pub owner_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct WorkerPairBaseline {
    pub capture: WorkerIdentity,
    pub exporter: WorkerIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SqlActiveState {
    pub run_id: Uuid,
    pub run_status: String,
    pub step_index: i32,
    pub step_status: String,
    pub resolved_endpoint_ref: Option<String>,
    pub dns_result_present: bool,
    pub head_attempt_id: Option<Uuid>,
    pub attempt_kind: Option<String>,
    pub session_generation: Option<i64>,
    pub control_phase: Option<String>,
    pub granted_phase: Option<String>,
    pub possibly_started: bool,
    pub disposal_confirmed: bool,
    pub has_may_start_event: bool,
    pub has_phase_head_event: bool,
    pub has_result_recorded_event: bool,
    pub head_result_present: bool,
    pub d3_executed: bool,
    pub attempt_deadline_at_ms: Option<i64>,
    pub db_now_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct LauncherSessionMeta {
    pub run_id: Uuid,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub granted_phase: Option<String>,
    pub started_marker_present: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct LauncherSessionEvidence {
    #[serde(flatten)]
    pub meta: LauncherSessionMeta,
    pub workers: WorkerPairBaseline,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ResponderCounterEvidence {
    pub run_id: String,
    pub scenario: String,
    pub dns_answered: u64,
    pub connections: u64,
    pub tls_handshakes: u64,
    pub head_requests: u64,
}

impl ResponderCounterEvidence {
    pub fn from_json(hc: &Value) -> Self {
        Self {
            run_id: hc["run_id"].as_str().unwrap_or_default().to_string(),
            scenario: hc["scenario"].as_str().unwrap_or_default().to_string(),
            dns_answered: hc["dns_answered"].as_u64().unwrap_or(0),
            connections: hc["connections"].as_u64().unwrap_or(0),
            tls_handshakes: hc["tls_handshakes"].as_u64().unwrap_or(0),
            head_requests: hc["head_requests"].as_u64().unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActiveWindowObservation {
    pub sql: SqlActiveState,
    pub launcher: LauncherSessionEvidence,
    pub responder: ResponderCounterEvidence,
    pub sdk_calls: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActiveHeadBinding {
    pub run_id: Uuid,
    pub step_index: i32,
    pub attempt_id: Uuid,
    pub session_generation: u64,
}

#[derive(Clone, Debug)]
pub(super) struct ArmedFaultWindow {
    pub binding: ActiveHeadBinding,
    pub observation: ActiveWindowObservation,
    pub anchor_instant: Instant,
    pub monotonic_cutoff: Instant,
    pub db_now_ms: i64,
    pub attempt_deadline_at_ms: i64,
    pub remaining_at_db_sample_ms: u64,
    pub remaining_at_arm_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct FaultTimingEvidence {
    pub db_now_ms: i64,
    pub attempt_deadline_at_ms: i64,
    pub remaining_at_db_sample_ms: u64,
    pub remaining_at_arm_ms: u64,
    pub invoked_since_anchor_ms: u64,
    pub completed_since_anchor_ms: u64,
    pub remaining_at_complete_ms: u64,
    pub completed_before_cutoff: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(super) struct DisposalEvidence {
    pub attempt_id: Uuid,
    pub baseline: Option<WorkerPairBaseline>,
    pub launcher_dir_present: bool,
    pub pending_markers: Vec<String>,
    pub network_slot_held: bool,
    pub containers_present: Vec<String>,
    pub netns_present: bool,
    pub veth_present: bool,
    pub nft_table_present: bool,
    pub residual_pids: Vec<u32>,
    pub residual_cgroups: Vec<String>,
    pub probe_errors: Vec<String>,
}

pub(super) fn check_all(checks: &[(bool, &str)]) -> Result<(), String> {
    for &(ok, msg) in checks {
        if !ok {
            return Err(msg.to_string());
        }
    }
    Ok(())
}

pub(super) fn parse_worker_inspect(role: WorkerRole, raw: &str) -> Result<WorkerIdentity, String> {
    let line = raw.trim_end_matches(['\r', '\n']);
    let parts: Vec<&str> = line.split('\t').collect();
    if line.is_empty() || line.contains(['\r', '\n']) || parts.len() != 6 {
        return Err(format!("malformed inspect for {}", role.label()));
    }
    let pid = parts[2]
        .parse::<u32>()
        .map_err(|_| format!("invalid PID for {}", role.label()))?;
    let running = match parts[3] {
        "true" => true,
        "false" => false,
        _ => return Err(format!("invalid running state for {}", role.label())),
    };
    Ok(WorkerIdentity {
        role,
        name: parts[1].strip_prefix('/').unwrap_or(parts[1]).to_string(),
        container_id: parts[0].to_string(),
        pid,
        running,
        attempt_label: parts[4].to_string(),
        owner_label: parts[5].to_string(),
    })
}

pub(super) fn validate_worker_identity(
    id: &WorkerIdentity,
    expected_role: WorkerRole,
    attempt: Uuid,
) -> Result<(), String> {
    let exp_name = expected_role.expected_name(attempt);
    let exp_att = attempt.to_string();
    let checks = [
        (id.role == expected_role, "worker role mismatch"),
        (id.name == exp_name, "worker name mismatch"),
        (is_hex(&id.container_id, 64), "worker CID malformed"),
        (id.pid > 0, "worker PID non-positive"),
        (id.running, "worker not running"),
        (id.attempt_label == exp_att, "worker attempt mismatch"),
        (
            id.owner_label == WORKER_OWNER_LABEL,
            "worker owner mismatch",
        ),
    ];
    check_all(&checks)
}

pub(super) fn validate_worker_pair(pair: &WorkerPairBaseline, attempt: Uuid) -> Result<(), String> {
    validate_worker_identity(&pair.capture, WorkerRole::Capture, attempt)?;
    validate_worker_identity(&pair.exporter, WorkerRole::Exporter, attempt)?;
    let dup_cid = pair.capture.container_id == pair.exporter.container_id;
    let dup_pid = pair.capture.pid == pair.exporter.pid;
    check_all(&[(!dup_cid, "duplicate CIDs"), (!dup_pid, "duplicate PIDs")])
}

pub(super) fn sql_head_candidate_ready(sql: &SqlActiveState, expected_run: Uuid) -> bool {
    validate_sql_active(sql, expected_run).is_ok()
}

pub(super) fn sql_window_missed(sql: &SqlActiveState) -> bool {
    let terminal = matches!(
        sql.step_status.as_str(),
        "completed" | "terminal" | "stopped" | "unknown"
    );
    sql.has_result_recorded_event
        || sql.head_result_present
        || sql.d3_executed
        || sql.run_status != "running"
        || terminal
}

fn validate_sql_active(sql: &SqlActiveState, expected_run: Uuid) -> Result<(Uuid, u64), String> {
    let ep_ok = sql.dns_result_present
        && sql
            .resolved_endpoint_ref
            .as_deref()
            .is_some_and(|s| !s.is_empty());
    let no_res = !sql.has_result_recorded_event && !sql.head_result_present;
    let head_phase = sql.possibly_started && sql.granted_phase.as_deref() == Some("head");
    let events_ok = sql.has_may_start_event && sql.has_phase_head_event;
    check_all(&[
        (
            sql.run_id == expected_run && sql.step_index == 2,
            "sql run/step mismatch",
        ),
        (sql.run_status == "running", "run not running"),
        (ep_ok, "pre-HEAD DNS/endpoint incomplete"),
        (
            sql.step_status == "acquisition_running",
            "step 2 not acquisition_running",
        ),
        (no_res, "HEAD already recorded result"),
        (!sql.d3_executed, "D3 already executed"),
        (
            sql.attempt_kind.as_deref() == Some("head"),
            "attempt_kind not head",
        ),
        (
            sql.control_phase.as_deref() == Some("may_start"),
            "control_phase not may_start",
        ),
        (head_phase, "SQL HEAD not in phase_head"),
        (events_ok, "missing may_start/phase_head event"),
    ])?;
    let attempt_id = sql.head_attempt_id.ok_or("missing head_attempt_id")?;
    let generation = sql
        .session_generation
        .filter(|&g| g > 0)
        .ok_or("invalid generation")? as u64;
    Ok((attempt_id, generation))
}

pub(super) fn validate_active_head_window(
    expected_run: Uuid,
    expected_scenario: &str,
    obs: &ActiveWindowObservation,
) -> Result<ActiveHeadBinding, String> {
    let (attempt_id, generation) = validate_sql_active(&obs.sql, expected_run)?;
    let meta = &obs.launcher.meta;
    let meta_ids = meta.run_id == expected_run && meta.attempt == attempt_id;
    let meta_phase =
        meta.session_generation == generation && meta.granted_phase.as_deref() == Some("head");
    let rsp = &obs.responder;
    let rsp_id = rsp.run_id == expected_run.to_string() && rsp.scenario == expected_scenario;
    let rsp_cnt = rsp.dns_answered >= 1
        && rsp.connections >= 1
        && rsp.tls_handshakes >= 1
        && rsp.head_requests >= 1;
    check_all(&[
        (meta_ids && meta_phase, "launcher session mismatch"),
        (
            meta.started_marker_present,
            "launcher started marker missing",
        ),
    ])?;
    validate_worker_pair(&obs.launcher.workers, attempt_id)?;
    check_all(&[
        (rsp_id && rsp_cnt, "responder has not observed active HEAD"),
        (
            obs.sdk_calls == EXPECTED_PRE_FAULT_SDK_CALLS,
            "unexpected SDK calls",
        ),
    ])?;
    Ok(ActiveHeadBinding {
        run_id: expected_run,
        step_index: 2,
        attempt_id,
        session_generation: generation,
    })
}

pub(super) fn arm_fault_window(
    expected_run: Uuid,
    expected_scenario: &str,
    obs: &ActiveWindowObservation,
    anchor_instant: Instant,
    after_sql_instant: Instant,
    min_margin_ms: u64,
) -> Result<ArmedFaultWindow, String> {
    let binding = validate_active_head_window(expected_run, expected_scenario, obs)?;
    let dl = obs
        .sql
        .attempt_deadline_at_ms
        .ok_or("missing request.deadline_at_ms")?;
    let rem_u64 = dl
        .checked_sub(obs.sql.db_now_ms)
        .filter(|&ms| ms > 0)
        .and_then(|ms| u64::try_from(ms).ok())
        .ok_or("HEAD deadline expired at DB sample")?;
    let cutoff = anchor_instant
        .checked_add(Duration::from_millis(rem_u64))
        .ok_or("cutoff overflow")?;
    let rem_arm = cutoff
        .checked_duration_since(after_sql_instant)
        .filter(|d| *d >= Duration::from_millis(min_margin_ms))
        .ok_or("HEAD fault window missed: insufficient margin at arm")?;
    Ok(ArmedFaultWindow {
        binding,
        observation: obs.clone(),
        anchor_instant,
        monotonic_cutoff: cutoff,
        db_now_ms: obs.sql.db_now_ms,
        attempt_deadline_at_ms: dl,
        remaining_at_db_sample_ms: rem_u64,
        remaining_at_arm_ms: rem_arm.as_millis() as u64,
    })
}

pub(super) fn execute_armed_fault<T>(
    armed: &ArmedFaultWindow,
    min_margin_ms: u64,
    mut now: impl FnMut() -> Instant,
    inject: impl FnOnce() -> Result<T, String>,
) -> Result<(T, FaultTimingEvidence), String> {
    let invoke_at = now();
    armed
        .monotonic_cutoff
        .checked_duration_since(invoke_at)
        .filter(|d| *d >= Duration::from_millis(min_margin_ms))
        .ok_or("HEAD fault window missed before invocation")?;
    let out = inject()?;
    let complete_at = now();
    let rem_done = armed
        .monotonic_cutoff
        .checked_duration_since(complete_at)
        .filter(|d| !d.is_zero())
        .ok_or("fault completed at or after cutoff")?;
    let since = |t: Instant| {
        t.saturating_duration_since(armed.anchor_instant)
            .as_millis() as u64
    };
    Ok((
        out,
        FaultTimingEvidence {
            db_now_ms: armed.db_now_ms,
            attempt_deadline_at_ms: armed.attempt_deadline_at_ms,
            remaining_at_db_sample_ms: armed.remaining_at_db_sample_ms,
            remaining_at_arm_ms: armed.remaining_at_arm_ms,
            invoked_since_anchor_ms: since(invoke_at),
            completed_since_anchor_ms: since(complete_at),
            remaining_at_complete_ms: rem_done.as_millis() as u64,
            completed_before_cutoff: true,
        },
    ))
}

pub(super) fn validate_disposal_evidence(ev: &DisposalEvidence) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    match ev.baseline.as_ref() {
        Some(pair) => {
            if let Err(err) = validate_worker_pair(pair, ev.attempt_id) {
                errs.push(format!("invalid worker baseline: {err}"));
            }
        }
        None => errs.push("missing original worker baseline".into()),
    }
    if !ev.probe_errors.is_empty() {
        errs.push(format!("probe errors: {}", ev.probe_errors.join("; ")));
    }
    for (present, label) in [
        (ev.launcher_dir_present, "launcher dir"),
        (!ev.pending_markers.is_empty(), "pending markers"),
        (ev.network_slot_held, "network slot"),
        (!ev.containers_present.is_empty(), "docker containers"),
        (ev.netns_present, "attempt netns"),
        (ev.veth_present, "attempt veth"),
        (ev.nft_table_present, "attempt nft table"),
        (!ev.residual_pids.is_empty(), "residual PIDs"),
        (!ev.residual_cgroups.is_empty(), "residual cgroups"),
    ] {
        if present {
            errs.push(format!("{label} still present"));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

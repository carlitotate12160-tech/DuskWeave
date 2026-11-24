#![allow(dead_code)]
//! Active-HEAD 3-way window observation, exact-attempt disposal probing, and fail-closed validation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SqlActiveState {
    pub run_id: Uuid,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct LauncherSessionEvidence {
    pub run_id: Uuid,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub granted_phase: Option<String>,
    pub started_marker_present: bool,
    pub observed_pids: Vec<u32>,
    pub observed_container_ids: Vec<String>,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActiveHeadBinding {
    pub run_id: Uuid,
    pub step_index: i32,
    pub attempt_id: Uuid,
    pub session_generation: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(super) struct DisposalEvidence {
    pub attempt_id: Uuid,
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

pub(super) fn validate_active_head_window(
    expected_run: Uuid,
    expected_scenario: &str,
    obs: &ActiveWindowObservation,
) -> Result<ActiveHeadBinding, String> {
    let sql = &obs.sql;
    if sql.run_id != expected_run || sql.step_index != 2 {
        return Err("sql run/step identity mismatch".into());
    }
    if !sql.dns_result_present
        || sql
            .resolved_endpoint_ref
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return Err("pre-HEAD: DNS or endpoint resolution incomplete".into());
    }
    if sql.step_status != "acquisition_running" {
        return Err(format!(
            "step 2 status is {}, expected acquisition_running",
            sql.step_status
        ));
    }
    if sql.has_result_recorded_event {
        return Err("HEAD attempt already recorded result".into());
    }
    let attempt_id = sql
        .head_attempt_id
        .ok_or("missing step 2 head_attempt_id")?;
    if sql.attempt_kind.as_deref() != Some("head") {
        return Err("attempt_kind is not head".into());
    }
    let generation = sql
        .session_generation
        .filter(|&g| g > 0)
        .ok_or("missing or invalid session_generation")? as u64;
    if sql.control_phase.as_deref() != Some("may_start")
        || !sql.possibly_started
        || sql.granted_phase.as_deref() != Some("head")
        || !sql.has_may_start_event
        || !sql.has_phase_head_event
    {
        return Err("SQL HEAD attempt has not reached committed phase_head".into());
    }
    let ln = &obs.launcher;
    if ln.run_id != expected_run
        || ln.attempt != attempt_id
        || ln.session_generation != generation
        || ln.granted_phase.as_deref() != Some("head")
        || !ln.started_marker_present
    {
        return Err("launcher session does not match active phase_head attempt".into());
    }
    let rsp = &obs.responder;
    if rsp.run_id != expected_run.to_string()
        || rsp.scenario != expected_scenario
        || rsp.dns_answered < 1
        || rsp.connections < 1
        || rsp.tls_handshakes < 1
        || rsp.head_requests < 1
    {
        return Err("responder counters have not observed active HEAD request".into());
    }
    Ok(ActiveHeadBinding {
        run_id: expected_run,
        step_index: 2,
        attempt_id,
        session_generation: generation,
    })
}

pub(super) fn validate_disposal_evidence(ev: &DisposalEvidence) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
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

#[derive(Deserialize)]
struct RawSessionReq {
    attempt: Uuid,
    session_generation: u64,
}

#[derive(Deserialize)]
struct RawSessionJson {
    run_id: Uuid,
    granted_phase: Option<String>,
    request: RawSessionReq,
}

#[cfg(target_os = "linux")]
pub(super) mod linux {
    use super::super::support::linux_host::NativeHost;
    use super::super::topology::{CmdRunner, LAUNCHER_ROOT};
    use super::super::topology_linux::OsCmdRunner;
    use super::*;
    use crate::sdk_fixture::FixtureChild;
    use postgres::Client;
    use std::io::{ErrorKind, Read};
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn path_exists_checked(path: &Path, errs: &mut Vec<String>) -> bool {
        match std::fs::symlink_metadata(path) {
            Ok(_) => true,
            Err(e) if e.kind() == ErrorKind::NotFound => false,
            Err(e) => {
                errs.push(format!("{} check: {e}", path.display()));
                false
            }
        }
    }

    pub(in super::super) fn read_sql_active_state(
        admin: &mut Client,
        run_id: Uuid,
    ) -> Result<Option<SqlActiveState>, String> {
        let row = admin
            .query_opt(
                "SELECT s.status, s.resolved_endpoint_ref,
                        EXISTS(SELECT 1 FROM execution.live_dns_results d WHERE d.run_id = $1),
                        s.head_attempt_id, les.attempt_kind, les.generation,
                        lec.phase, lec.granted_phase,
                        coalesce(lec.possibly_started, false),
                        coalesce(lec.disposal_confirmed, false),
                        EXISTS(SELECT 1 FROM execution.live_effect_events e
                               WHERE e.attempt = s.head_attempt_id AND e.phase = 'may_start'),
                        EXISTS(SELECT 1 FROM execution.live_effect_events e
                               WHERE e.attempt = s.head_attempt_id AND e.phase = 'phase_head'),
                        EXISTS(SELECT 1 FROM execution.live_effect_events e
                               WHERE e.attempt = s.head_attempt_id AND e.phase = 'result_recorded')
                 FROM execution.https_loop_steps s
                 LEFT JOIN execution.live_effect_sessions les ON les.attempt = s.head_attempt_id
                 LEFT JOIN execution.live_effect_control lec ON lec.attempt = s.head_attempt_id
                 WHERE s.run_id = $1 AND s.step_index = 2",
                &[&run_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(row.map(|r| SqlActiveState {
            run_id,
            step_index: 2,
            step_status: r.get(0),
            resolved_endpoint_ref: r.get(1),
            dns_result_present: r.get(2),
            head_attempt_id: r.get(3),
            attempt_kind: r.get(4),
            session_generation: r.get(5),
            control_phase: r.get(6),
            granted_phase: r.get(7),
            possibly_started: r.get(8),
            disposal_confirmed: r.get(9),
            has_may_start_event: r.get(10),
            has_phase_head_event: r.get(11),
            has_result_recorded_event: r.get(12),
        }))
    }

    pub(in super::super) fn read_launcher_session(
        attempt: Uuid,
    ) -> Result<Option<LauncherSessionEvidence>, String> {
        let dir = Path::new(LAUNCHER_ROOT).join(attempt.simple().to_string());
        let sess_path = dir.join("session.json");
        let meta = match std::fs::symlink_metadata(&sess_path) {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
            Ok(m) => m,
        };
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.uid() != 0
            || (meta.mode() & 0o077) != 0
        {
            return Err(format!(
                "unsafe session.json custody: {}",
                sess_path.display()
            ));
        }
        let file = std::fs::File::open(&sess_path).map_err(|e| e.to_string())?;
        let parsed: RawSessionJson =
            serde_json::from_reader(file.take(16_385)).map_err(|e| e.to_string())?;
        let started_marker_present = match std::fs::symlink_metadata(dir.join("started")) {
            Ok(m) => m.is_file() && !m.file_type().is_symlink() && m.uid() == 0,
            Err(e) if e.kind() == ErrorKind::NotFound => false,
            Err(e) => return Err(e.to_string()),
        };
        let (mut observed_pids, mut observed_container_ids) = (Vec::new(), Vec::new());
        for prefix in ["dwcap", "dwexp"] {
            let name = format!("{prefix}{}", attempt.simple());
            if let Ok(out) = OsCmdRunner.run(
                "/usr/bin/docker",
                &["inspect", "--format", "{{.Id}} {{.State.Pid}}", &name],
            ) && out.success
                && let Some((cid, pid_s)) = out.stdout.trim().split_once(' ')
            {
                if !cid.is_empty() {
                    observed_container_ids.push(cid.to_string());
                }
                if let Ok(pid) = pid_s.parse::<u32>()
                    && pid > 0
                {
                    observed_pids.push(pid);
                }
            }
        }
        Ok(Some(LauncherSessionEvidence {
            run_id: parsed.run_id,
            attempt: parsed.request.attempt,
            session_generation: parsed.request.session_generation,
            granted_phase: parsed.granted_phase,
            started_marker_present,
            observed_pids,
            observed_container_ids,
        }))
    }

    pub(in super::super) fn wait_for_active_head(
        host: &mut NativeHost,
        parent: &mut FixtureChild,
        run_id: Uuid,
        scenario: &str,
        timeout: Duration,
    ) -> Result<(ActiveHeadBinding, ActiveWindowObservation), String> {
        let deadline = Instant::now() + timeout;
        let mut last_err = "no active step 2 state observed".to_string();
        while Instant::now() < deadline {
            match parent.try_wait() {
                Ok(Some(status)) => {
                    return Err(format!(
                        "m1-run exited before active HEAD ({status}): {last_err}"
                    ));
                }
                Ok(None) => {}
                Err(e) => return Err(format!("try_wait polling m1-run failed: {e}")),
            }
            if let Some(sql) = read_sql_active_state(&mut host.admin, run_id)? {
                if sql.has_result_recorded_event
                    || matches!(
                        sql.step_status.as_str(),
                        "completed" | "terminal" | "stopped" | "unknown"
                    )
                {
                    return Err(format!(
                        "active HEAD window missed (status={}): {last_err}",
                        sql.step_status
                    ));
                }
                if let Some(attempt) = sql.head_attempt_id
                    && let Some(launcher) = read_launcher_session(attempt)?
                {
                    let responder = ResponderCounterEvidence::from_json(&host.https_counters());
                    let obs = ActiveWindowObservation {
                        sql,
                        launcher,
                        responder,
                    };
                    match validate_active_head_window(run_id, scenario, &obs) {
                        Ok(binding) => return Ok((binding, obs)),
                        Err(err) => last_err = err,
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(format!(
            "timed out waiting for active HEAD window: {last_err}"
        ))
    }

    pub(in super::super) fn probe_attempt_disposal(
        attempt: Uuid,
        observed_pids: &[u32],
        observed_cids: &[String],
    ) -> DisposalEvidence {
        let mut ev = DisposalEvidence {
            attempt_id: attempt,
            ..Default::default()
        };
        let simple = attempt.simple().to_string();
        let short = &simple[..12];
        let root = Path::new(LAUNCHER_ROOT);
        ev.launcher_dir_present = path_exists_checked(&root.join(&simple), &mut ev.probe_errors);
        for prefix in ["dwcap", "dwexp"] {
            for ext in ["pending", "complete"] {
                let p = root.join(format!("{prefix}{simple}.{ext}"));
                if path_exists_checked(&p, &mut ev.probe_errors) {
                    ev.pending_markers.push(p.display().to_string());
                }
            }
        }
        let slot = root.join(format!("network-{}", attempt.as_bytes()[6] & 0xfc));
        match std::fs::read_to_string(&slot) {
            Ok(owner) if owner == attempt.to_string() => ev.network_slot_held = true,
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => ev.probe_errors.push(format!("network slot check: {e}")),
        }
        let filter = format!("label=org.duskweave.attempt={attempt}");
        match OsCmdRunner.run(
            "/usr/bin/docker",
            &["ps", "-a", "--filter", &filter, "--format", "{{.Names}}"],
        ) {
            Ok(out) if out.success => {
                ev.containers_present = out
                    .stdout
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            Ok(out) => ev
                .probe_errors
                .push(format!("docker ps failed: {}", out.stderr.trim())),
            Err(e) => ev.probe_errors.push(e),
        }
        ev.netns_present = path_exists_checked(
            Path::new(&format!("/run/netns/dw{short}")),
            &mut ev.probe_errors,
        );
        ev.veth_present = path_exists_checked(
            Path::new(&format!("/sys/class/net/dvh{short}")),
            &mut ev.probe_errors,
        );
        match OsCmdRunner.run(
            "/usr/sbin/nft",
            &["list", "table", "inet", &format!("dwf{short}")],
        ) {
            Ok(out) if out.success => ev.nft_table_present = true,
            Ok(out) => {
                let low = out.stderr.to_ascii_lowercase();
                if !low.contains("no such file or directory") && !low.contains("does not exist") {
                    ev.probe_errors
                        .push(format!("nft check: {}", out.stderr.trim()));
                }
            }
            Err(e) => ev.probe_errors.push(e),
        }
        for &pid in observed_pids {
            if path_exists_checked(Path::new(&format!("/proc/{pid}")), &mut ev.probe_errors) {
                ev.residual_pids.push(pid);
            }
        }
        let needles: Vec<String> = observed_cids
            .iter()
            .cloned()
            .chain([format!("dwcap{simple}"), format!("dwexp{simple}")])
            .collect();
        match std::fs::read_dir("/proc") {
            Err(e) => ev.probe_errors.push(format!("/proc scan failed: {e}")),
            Ok(rd) => {
                for ent in rd {
                    let Ok(ent) = ent else {
                        ev.probe_errors.push("/proc iteration failed".into());
                        continue;
                    };
                    if ent.file_name().to_string_lossy().parse::<u32>().is_err() {
                        continue;
                    }
                    let pid_dir = ent.path();
                    match std::fs::read_to_string(pid_dir.join("cgroup")) {
                        Ok(cg) if needles.iter().any(|n| cg.contains(n)) => {
                            ev.residual_cgroups.push(pid_dir.display().to_string())
                        }
                        Ok(_) => {}
                        Err(e) if e.kind() == ErrorKind::NotFound && !pid_dir.exists() => {}
                        Err(e) => ev
                            .probe_errors
                            .push(format!("cgroup {}: {e}", pid_dir.display())),
                    }
                }
            }
        }
        ev
    }
}

#![allow(dead_code)]
//! Active-HEAD 3-way window observation, exact-attempt disposal probing, and fail-closed validation.

use super::active_effect_evidence::{
    DOCKER_INSPECT_FORMAT, LauncherSessionMeta, WorkerIdentity, WorkerPairBaseline, WorkerRole,
    parse_worker_inspect, validate_worker_pair,
};
use super::topology::CmdRunner;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::Path;
use uuid::Uuid;

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

pub(super) fn inspect_worker_role(
    runner: &mut dyn CmdRunner,
    role: WorkerRole,
    attempt: Uuid,
) -> Result<WorkerIdentity, String> {
    let name = role.expected_name(attempt);
    let args = ["inspect", "--format", DOCKER_INSPECT_FORMAT, &name];
    let out = runner
        .run("/usr/bin/docker", &args)
        .map_err(|e| format!("docker inspect {} failed: {e}", role.label()))?;
    if !out.success {
        return Err(format!("docker inspect {} non-zero exit", role.label()));
    }
    parse_worker_inspect(role, &out.stdout)
}

pub(super) fn inspect_worker_pair_with(
    runner: &mut dyn CmdRunner,
    attempt: Uuid,
) -> Result<WorkerPairBaseline, String> {
    let capture = inspect_worker_role(runner, WorkerRole::Capture, attempt)?;
    let exporter = inspect_worker_role(runner, WorkerRole::Exporter, attempt)?;
    let pair = WorkerPairBaseline { capture, exporter };
    validate_worker_pair(&pair, attempt)?;
    Ok(pair)
}

fn collect_docker_ps_lines(
    runner: &mut dyn CmdRunner,
    filter: &str,
    label: &str,
    found: &mut BTreeSet<String>,
    errs: &mut Vec<String>,
) {
    let args = [
        "ps",
        "-a",
        "--filter",
        filter,
        "--format",
        "{{.ID}} {{.Names}}",
    ];
    match runner.run("/usr/bin/docker", &args) {
        Ok(out) if out.success => {
            found.extend(
                out.stdout
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            );
        }
        Ok(_) => errs.push(format!("docker ps ({label}) failed")),
        Err(e) => errs.push(format!("docker ps ({label}): {e}")),
    }
}

pub(super) fn inspect_attempt_containers_with(
    runner: &mut dyn CmdRunner,
    attempt: Uuid,
    baseline: &WorkerPairBaseline,
    errs: &mut Vec<String>,
) -> Vec<String> {
    let mut found = BTreeSet::new();
    let label_filter = format!("label=org.duskweave.attempt={attempt}");
    collect_docker_ps_lines(runner, &label_filter, "attempt-label", &mut found, errs);
    for worker in [&baseline.capture, &baseline.exporter] {
        let id_filter = format!("id={}", worker.container_id);
        collect_docker_ps_lines(runner, &id_filter, worker.role.label(), &mut found, errs);
    }
    found.into_iter().collect()
}

pub(super) fn scan_proc_disposal_at(
    proc_root: &Path,
    baseline: &WorkerPairBaseline,
    errs: &mut Vec<String>,
) -> (Vec<u32>, Vec<String>) {
    let mut residual_pids = Vec::new();
    for pid in [baseline.capture.pid, baseline.exporter.pid] {
        if path_exists_checked(&proc_root.join(pid.to_string()), errs) {
            residual_pids.push(pid);
        }
    }
    let needles = [
        baseline.capture.container_id.as_str(),
        baseline.exporter.container_id.as_str(),
        baseline.capture.name.as_str(),
        baseline.exporter.name.as_str(),
    ];
    let mut residual_cgroups = Vec::new();
    let Ok(rd) = std::fs::read_dir(proc_root) else {
        errs.push(format!("{} scan failed", proc_root.display()));
        return (residual_pids, residual_cgroups);
    };
    for ent in rd {
        let Ok(ent) = ent else {
            errs.push("/proc iteration failed".into());
            continue;
        };
        if ent.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let pid_dir = ent.path();
        match std::fs::read_to_string(pid_dir.join("cgroup")) {
            Ok(cg) if needles.iter().any(|n| cg.contains(n)) => {
                residual_cgroups.push(pid_dir.display().to_string())
            }
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound && !pid_dir.exists() => {}
            Err(e) => errs.push(format!("cgroup {}: {e}", pid_dir.display())),
        }
    }
    (residual_pids, residual_cgroups)
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

fn is_root_owned(meta: &std::fs::Metadata) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        meta.uid() == 0 && (meta.mode() & 0o077) == 0
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = meta;
        true
    }
}

pub(super) fn read_launcher_metadata_at(
    launcher_root: &Path,
    attempt: Uuid,
) -> Result<Option<LauncherSessionMeta>, String> {
    let dir = launcher_root.join(attempt.simple().to_string());
    let sess_path = dir.join("session.json");
    let meta = match std::fs::symlink_metadata(&sess_path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(m) => m,
    };
    if !meta.is_file() || meta.file_type().is_symlink() || !is_root_owned(&meta) {
        return Err(format!(
            "unsafe session.json custody: {}",
            sess_path.display()
        ));
    }
    let file = std::fs::File::open(&sess_path).map_err(|e| e.to_string())?;
    let parsed: RawSessionJson =
        serde_json::from_reader(std::io::Read::take(file, 16_385)).map_err(|e| e.to_string())?;
    let started_marker_present = match std::fs::symlink_metadata(dir.join("started")) {
        Ok(m) => m.is_file() && !m.file_type().is_symlink() && is_root_owned(&m),
        Err(e) if e.kind() == ErrorKind::NotFound => false,
        Err(e) => return Err(e.to_string()),
    };
    Ok(Some(LauncherSessionMeta {
        run_id: parsed.run_id,
        attempt: parsed.request.attempt,
        session_generation: parsed.request.session_generation,
        granted_phase: parsed.granted_phase,
        started_marker_present,
    }))
}

#[cfg(target_os = "linux")]
pub(super) mod linux {
    use super::super::active_effect_evidence::{
        ActiveWindowObservation, ArmedFaultWindow, DisposalEvidence, EXPECTED_PRE_FAULT_SDK_CALLS,
        LauncherSessionEvidence, MIN_INJECTION_MARGIN_MS, ResponderCounterEvidence, SqlActiveState,
        arm_fault_window, sql_head_candidate_ready, sql_window_missed,
    };
    use super::super::support::linux_host::NativeHost;
    use super::super::topology::{CmdRunner, LAUNCHER_ROOT};
    use super::super::topology_linux::OsCmdRunner;
    use super::*;
    use crate::sdk_fixture::{FixtureChild, Server};
    use postgres::Client;
    use serde_json::Value;
    use std::time::{Duration, Instant};

    pub(in super::super) fn read_sql_active_state(
        admin: &mut Client,
        run_id: Uuid,
    ) -> Result<Option<SqlActiveState>, String> {
        let row = admin
            .query_opt(
                "SELECT ac.status, s.status, s.resolved_endpoint_ref,
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
                               WHERE e.attempt = s.head_attempt_id AND e.phase = 'result_recorded'),
                        s.head_result_id IS NOT NULL
                            OR EXISTS(SELECT 1 FROM execution.live_https_results lhr
                                      WHERE lhr.run_id = $1 AND lhr.step_index = 2),
                        EXISTS(SELECT 1 FROM execution.autonomous_phases ap
                               WHERE ap.run_id = $1 AND ap.phase = 'decision_three')
                            OR EXISTS(SELECT 1 FROM execution.https_loop_steps s3
                                      WHERE s3.run_id = $1 AND s3.step_index = 3
                                        AND (s3.status NOT IN ('pending', 'stopped')
                                             OR EXISTS(SELECT 1 FROM cognition.reasoning_decisions rd
                                                       WHERE (rd.e, rd.c, rd.d) = (s3.e, s3.c, s3.decision_id)))),
                        (les.request->>'deadline_at_ms')::bigint,
                        floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
                 FROM execution.https_loop_steps s
                 JOIN execution.autonomous_control ac ON ac.run_id = s.run_id
                 LEFT JOIN execution.live_effect_sessions les ON les.attempt = s.head_attempt_id
                 LEFT JOIN execution.live_effect_control lec ON lec.attempt = s.head_attempt_id
                 WHERE s.run_id = $1 AND s.step_index = 2",
                &[&run_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(row.map(|r| SqlActiveState {
            run_id,
            run_status: r.get(0),
            step_index: 2,
            step_status: r.get(1),
            resolved_endpoint_ref: r.get(2),
            dns_result_present: r.get(3),
            head_attempt_id: r.get(4),
            attempt_kind: r.get(5),
            session_generation: r.get(6),
            control_phase: r.get(7),
            granted_phase: r.get(8),
            possibly_started: r.get(9),
            disposal_confirmed: r.get(10),
            has_may_start_event: r.get(11),
            has_phase_head_event: r.get(12),
            has_result_recorded_event: r.get(13),
            head_result_present: r.get(14),
            d3_executed: r.get(15),
            attempt_deadline_at_ms: r.get(16),
            db_now_ms: r.get(17),
        }))
    }

    fn candidate_window_ready(
        host: &mut NativeHost,
        sdk: &Server,
        run_id: Uuid,
        scenario: &str,
        sql: &SqlActiveState,
    ) -> Result<Option<(Uuid, LauncherSessionMeta)>, String> {
        if !sql_head_candidate_ready(sql, run_id) {
            return Ok(None);
        }
        let Some(attempt) = sql.head_attempt_id else {
            return Ok(None);
        };
        let Some(meta) = read_launcher_metadata_at(Path::new(LAUNCHER_ROOT), attempt)? else {
            return Ok(None);
        };
        let meta_ok = meta.run_id == run_id
            && meta.attempt == attempt
            && sql.session_generation == Some(meta.session_generation as i64)
            && meta.granted_phase.as_deref() == Some("head")
            && meta.started_marker_present;
        let rsp = ResponderCounterEvidence::from_json(&host.https_counters());
        let rsp_ok = rsp.run_id == run_id.to_string()
            && rsp.scenario == scenario
            && rsp.dns_answered >= 1
            && rsp.connections >= 1
            && rsp.tls_handshakes >= 1
            && rsp.head_requests >= 1;
        Ok(
            (meta_ok && rsp_ok && sdk.count() == EXPECTED_PRE_FAULT_SDK_CALLS)
                .then_some((attempt, meta)),
        )
    }

    fn wait_for_head_candidate(
        host: &mut NativeHost,
        parent: &mut FixtureChild,
        sdk: &Server,
        run_id: Uuid,
        scenario: &str,
        timeout: Duration,
    ) -> Result<(Uuid, LauncherSessionMeta), String> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match parent.try_wait() {
                Ok(Some(status)) => {
                    return Err(format!("m1-run exited before active HEAD ({status})"));
                }
                Ok(None) => {}
                Err(e) => return Err(format!("try_wait polling m1-run failed: {e}")),
            }
            if let Some(sql) = read_sql_active_state(&mut host.admin, run_id)? {
                if sql_window_missed(&sql) {
                    return Err(format!(
                        "active HEAD window missed (run={}, step={})",
                        sql.run_status, sql.step_status
                    ));
                }
                if let Some(found) = candidate_window_ready(host, sdk, run_id, scenario, &sql)? {
                    return Ok(found);
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err("timed out waiting for active HEAD candidate window".into())
    }

    pub(in super::super) fn prepare_armed_head_window(
        host: &mut NativeHost,
        parent: &mut FixtureChild,
        sdk: &Server,
        run_id: Uuid,
        scenario: &str,
        timeout: Duration,
    ) -> Result<(ArmedFaultWindow, Value, Value), String> {
        let (attempt, meta) =
            wait_for_head_candidate(host, parent, sdk, run_id, scenario, timeout)?;
        let workers = inspect_worker_pair_with(&mut OsCmdRunner, attempt)?;
        let before_sql = host.sql_snapshot(run_id)?;
        let before_counters = host.https_counters();
        let sdk_calls = sdk.count();

        let anchor = Instant::now();
        let final_sql = read_sql_active_state(&mut host.admin, run_id)?
            .ok_or_else(|| "step 2 missing during final SQL window check".to_string())?;
        let after_sql = Instant::now();

        let obs = ActiveWindowObservation {
            sql: final_sql,
            launcher: LauncherSessionEvidence { meta, workers },
            responder: ResponderCounterEvidence::from_json(&before_counters),
            sdk_calls,
        };
        let armed = arm_fault_window(
            run_id,
            scenario,
            &obs,
            anchor,
            after_sql,
            MIN_INJECTION_MARGIN_MS,
        )?;
        Ok((armed, before_sql, before_counters))
    }

    pub(in super::super) fn probe_attempt_disposal(
        attempt: Uuid,
        baseline: &WorkerPairBaseline,
    ) -> DisposalEvidence {
        let mut ev = DisposalEvidence {
            attempt_id: attempt,
            baseline: Some(baseline.clone()),
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
        ev.containers_present = inspect_attempt_containers_with(
            &mut OsCmdRunner,
            attempt,
            baseline,
            &mut ev.probe_errors,
        );
        ev.netns_present = path_exists_checked(
            Path::new(&format!("/run/netns/dw{short}")),
            &mut ev.probe_errors,
        );
        ev.veth_present = path_exists_checked(
            Path::new(&format!("/sys/class/net/dvh{short}")),
            &mut ev.probe_errors,
        );
        probe_nft(short, &mut ev);
        let (pids, cgroups) =
            scan_proc_disposal_at(Path::new("/proc"), baseline, &mut ev.probe_errors);
        ev.residual_pids = pids;
        ev.residual_cgroups = cgroups;
        ev
    }

    fn probe_nft(short: &str, ev: &mut DisposalEvidence) {
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
    }
}

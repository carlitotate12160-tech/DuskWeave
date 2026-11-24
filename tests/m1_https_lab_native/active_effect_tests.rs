#![cfg(test)]
//! Unprivileged unit, Docker/proc seam, monotonic-clock, and outcome regressions for active-HEAD fault evidence.

use super::active_effect::{
    ExpectedFaultOutcome, validate_post_fault_state, validate_post_recover_sql,
    validate_recovery_receipts,
};
use super::active_effect_evidence::{
    ActiveHeadBinding, ActiveWindowObservation, DisposalEvidence, EXPECTED_PRE_FAULT_SDK_CALLS,
    LauncherSessionEvidence, LauncherSessionMeta, MIN_INJECTION_MARGIN_MS,
    ResponderCounterEvidence, SqlActiveState, WORKER_OWNER_LABEL, WorkerIdentity,
    WorkerPairBaseline, WorkerRole, arm_fault_window, execute_armed_fault,
    validate_active_head_window, validate_disposal_evidence,
};
use super::active_effect_probe::{
    inspect_attempt_containers_with, inspect_worker_pair_with, scan_proc_disposal_at,
};
use super::topology::{CmdOutput, CmdRunner};
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

const UNK: ExpectedFaultOutcome = ExpectedFaultOutcome::ControlRootLossUnknown;
type Mut<T> = fn(&mut T);

fn sample_workers(attempt: Uuid) -> WorkerPairBaseline {
    let mk = |role: WorkerRole, c: &str, pid: u32| WorkerIdentity {
        role,
        name: role.expected_name(attempt),
        container_id: c.repeat(64),
        pid,
        running: true,
        attempt_label: attempt.to_string(),
        owner_label: WORKER_OWNER_LABEL.into(),
    };
    WorkerPairBaseline {
        capture: mk(WorkerRole::Capture, "a", 1001),
        exporter: mk(WorkerRole::Exporter, "b", 1002),
    }
}

fn sample_sql(run_id: Uuid, attempt: Uuid) -> SqlActiveState {
    SqlActiveState {
        run_id,
        run_status: "running".into(),
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
        head_result_present: false,
        d3_executed: false,
        attempt_deadline_at_ms: Some(1_004_500),
        db_now_ms: 1_000_000,
    }
}

fn sample_obs(run_id: Uuid, attempt: Uuid) -> ActiveWindowObservation {
    let meta = LauncherSessionMeta {
        run_id,
        attempt,
        session_generation: 2,
        granted_phase: Some("head".into()),
        started_marker_present: true,
    };
    ActiveWindowObservation {
        sql: sample_sql(run_id, attempt),
        launcher: LauncherSessionEvidence {
            meta,
            workers: sample_workers(attempt),
        },
        responder: ResponderCounterEvidence {
            run_id: run_id.to_string(),
            scenario: "timeout".into(),
            dns_answered: 1,
            connections: 1,
            tls_handshakes: 1,
            head_requests: 1,
        },
        sdk_calls: EXPECTED_PRE_FAULT_SDK_CALLS,
    }
}

fn sample_snapshot(run_id: Uuid, attempt: Uuid, status: &str, step3: &str) -> Value {
    json!({
        "control": { "run_id": run_id, "status": status },
        "steps": [
            { "step_index": 1, "status": "completed" },
            { "step_index": 2, "status": status, "head_attempt_id": attempt, "head_result_id": null, "head_outcome": null },
            { "step_index": 3, "status": step3 }
        ],
        "sessions": [{
            "attempt": attempt, "step_index": 2, "attempt_kind": "head", "generation": 2,
            "control_phase": "may_start", "granted_phase": "head", "possibly_started": true, "disposal_confirmed": false
        }],
        "events": [
            { "attempt": attempt, "generation": 2, "phase": "reserved", "occurred_at_ms": 100 },
            { "attempt": attempt, "generation": 2, "phase": "may_start", "occurred_at_ms": 101 },
            { "attempt": attempt, "generation": 2, "phase": "phase_head", "occurred_at_ms": 102 }
        ]
    })
}

fn sample_receipt(run_id: Uuid, status: &str) -> Value {
    json!({
        "run_id": run_id, "status": status, "live_qualified": true, "decision_count": 2,
        "dns_effect_count": 1, "head_effect_count": 0, "decision_history_count": 2,
        "terrain_history_count": 1, "release_sha256": "a".repeat(64)
    })
}

struct StubRunner(BTreeMap<String, Result<CmdOutput, String>>);
impl CmdRunner for StubRunner {
    fn run(&mut self, bin: &str, args: &[&str]) -> Result<CmdOutput, String> {
        let key = format!("{bin} {}", args.join(" "));
        self.0
            .get(&key)
            .cloned()
            .unwrap_or_else(|| Err(format!("unexpected: {key}")))
    }
}

fn ok_cmd(stdout: String) -> Result<CmdOutput, String> {
    Ok(CmdOutput {
        success: true,
        stdout,
        stderr: String::new(),
    })
}

#[test]
fn active_head_validator_rejects_pre_head_recorded_d3_and_bad_workers() {
    let (run_id, attempt) = (Uuid::new_v4(), Uuid::new_v4());
    let ok = sample_obs(run_id, attempt);
    let b = validate_active_head_window(run_id, "timeout", &ok).unwrap();
    assert_eq!(
        (b.run_id, b.attempt_id, b.session_generation),
        (run_id, attempt, 2)
    );
    let cases: [(&str, Mut<ActiveWindowObservation>); 21] = [
        ("pre_head_tls", |o| o.sql.granted_phase = Some("tls".into())),
        ("reservation_only", |o| o.sql.possibly_started = false),
        ("missing_phase_head", |o| o.sql.has_phase_head_event = false),
        ("already_completed", |o| {
            o.sql.step_status = "completed".into()
        }),
        ("run_not_running", |o| o.sql.run_status = "unknown".into()),
        ("result_recorded", |o| {
            o.sql.has_result_recorded_event = true
        }),
        ("head_result_present", |o| o.sql.head_result_present = true),
        ("d3_executed", |o| o.sql.d3_executed = true),
        ("third_sdk_call", |o| o.sdk_calls = 3),
        ("missing_started", |o| {
            o.launcher.meta.started_marker_present = false
        }),
        ("attempt_mismatch", |o| {
            o.launcher.meta.attempt = Uuid::new_v4()
        }),
        ("gen_mismatch", |o| o.launcher.meta.session_generation = 9),
        ("run_mismatch", |o| {
            o.responder.run_id = Uuid::new_v4().to_string()
        }),
        ("zero_head", |o| o.responder.head_requests = 0),
        ("zero_pid", |o| o.launcher.workers.exporter.pid = 0),
        ("not_running", |o| {
            o.launcher.workers.capture.running = false
        }),
        ("dup_cid", |o| {
            o.launcher.workers.exporter.container_id = "a".repeat(64)
        }),
        ("dup_pid", |o| o.launcher.workers.exporter.pid = 1001),
        ("wrong_owner", |o| {
            o.launcher.workers.capture.owner_label = "wrong".into()
        }),
        ("wrong_attempt_lbl", |o| {
            o.launcher.workers.exporter.attempt_label = Uuid::new_v4().to_string()
        }),
        ("wrong_name", |o| {
            o.launcher.workers.capture.name = "dwexp0000".into()
        }),
    ];
    for (name, f) in cases {
        let mut bad = ok.clone();
        f(&mut bad);
        assert!(
            validate_active_head_window(run_id, "timeout", &bad).is_err(),
            "{name}"
        );
    }
}

#[test]
fn fault_window_arming_and_injected_clock_reject_expired_slow_and_late_windows() {
    let (run_id, attempt) = (Uuid::new_v4(), Uuid::new_v4());
    let ok = sample_obs(run_id, attempt);
    let anchor = Instant::now();
    let ms = |n| Duration::from_millis(n);
    let arm = |obs: &ActiveWindowObservation, d| {
        arm_fault_window(
            run_id,
            "timeout",
            obs,
            anchor,
            anchor + ms(d),
            MIN_INJECTION_MARGIN_MS,
        )
    };
    let armed = arm(&ok, 50).unwrap();
    assert_eq!(armed.remaining_at_db_sample_ms, 4_500);

    let mut expired = ok.clone();
    expired.sql.db_now_ms = 1_004_500;
    assert!(arm(&expired, 0).is_err());

    let mut tight = ok.clone();
    tight.sql.attempt_deadline_at_ms = Some(1_002_499);
    assert!(arm(&tight, 0).is_err());
    assert!(arm(&ok, 2_100).is_err());

    let invoked = Cell::new(false);
    let late = execute_armed_fault(
        &armed,
        MIN_INJECTION_MARGIN_MS,
        || anchor + ms(2_100),
        || {
            invoked.set(true);
            Ok(())
        },
    );
    assert!(late.is_err() && !invoked.get());

    let run_seq = |t1, t2| {
        let i = Cell::new(0);
        let ts = [anchor + ms(t1), anchor + ms(t2)];
        execute_armed_fault(
            &armed,
            MIN_INJECTION_MARGIN_MS,
            || {
                let v = ts[i.get()];
                i.set(i.get() + 1);
                v
            },
            || Ok(()),
        )
    };
    assert!(run_seq(500, 4_500).is_err());
    let (_, timing) = run_seq(100, 300).unwrap();
    assert!(timing.completed_before_cutoff && timing.remaining_at_complete_ms == 4_200);
}

#[test]
fn disposal_validator_requires_valid_baseline_and_rejects_residuals() {
    let attempt = Uuid::new_v4();
    assert!(
        validate_disposal_evidence(&DisposalEvidence {
            attempt_id: attempt,
            ..Default::default()
        })
        .is_err()
    );
    let clean = DisposalEvidence {
        attempt_id: attempt,
        baseline: Some(sample_workers(attempt)),
        ..Default::default()
    };
    assert!(validate_disposal_evidence(&clean).is_ok());
    let cases: [(&str, Mut<DisposalEvidence>); 11] = [
        ("bad_baseline", |e| {
            e.baseline.as_mut().unwrap().capture.pid = 0
        }),
        ("probe_error", |e| e.probe_errors.push("nft err".into())),
        ("launcher_dir", |e| e.launcher_dir_present = true),
        ("pending", |e| {
            e.pending_markers.push("dwcap.pending".into())
        }),
        ("network_slot", |e| e.network_slot_held = true),
        ("container", |e| e.containers_present.push("dwcap1".into())),
        ("netns", |e| e.netns_present = true),
        ("veth", |e| e.veth_present = true),
        ("nft", |e| e.nft_table_present = true),
        ("pid", |e| e.residual_pids.push(4321)),
        ("cgroup", |e| e.residual_cgroups.push("/proc/4321".into())),
    ];
    for (name, f) in cases {
        let mut bad = clean.clone();
        f(&mut bad);
        assert!(validate_disposal_evidence(&bad).is_err(), "{name}");
    }
}

#[test]
fn docker_inspect_parser_and_pair_validator_enforce_complete_worker_identities() {
    let attempt = Uuid::new_v4();
    let (cap, exp) = (
        WorkerRole::Capture.expected_name(attempt),
        WorkerRole::Exporter.expected_name(attempt),
    );
    let fmt = super::active_effect_evidence::DOCKER_INSPECT_FORMAT;
    let key = |n: &str| format!("/usr/bin/docker inspect --format {fmt} {n}");
    let line = |c: &str, n: &str, p: &str, r: &str, a: &str, o: &str| {
        format!("{c}\t/{n}\t{p}\t{r}\t{a}\t{o}\n")
    };
    let (cid1, cid2, att) = ("a".repeat(64), "b".repeat(64), attempt.to_string());
    let (ok_cap, ok_exp) = (
        ok_cmd(line(&cid1, &cap, "201", "true", &att, WORKER_OWNER_LABEL)),
        ok_cmd(line(&cid2, &exp, "202", "true", &att, WORKER_OWNER_LABEL)),
    );

    let mut r = StubRunner(BTreeMap::from([
        (key(&cap), ok_cap.clone()),
        (key(&exp), ok_exp.clone()),
    ]));
    let pair = inspect_worker_pair_with(&mut r, attempt).unwrap();
    assert_eq!((pair.capture.pid, pair.exporter.pid), (201, 202));

    let bad_exp = |c, p, r, a, o| ok_cmd(line(c, &exp, p, r, a, o));
    let other_att = Uuid::new_v4().to_string();
    let ow = WORKER_OWNER_LABEL;
    let fail_cmd = Ok(CmdOutput {
        success: false,
        stdout: String::new(),
        stderr: "no such".into(),
    });
    let bad = [
        ("cmd_err", Err("docker down".into())),
        ("exit_nonzero", fail_cmd),
        ("malformed_cols", ok_cmd(format!("{cid2}\t/{exp}\t202\n"))),
        ("zero_pid", bad_exp(&cid2, "0", "true", &att, ow)),
        ("not_running", bad_exp(&cid2, "202", "false", &att, ow)),
        ("dup_cid", bad_exp(&cid1, "202", "true", &att, ow)),
        ("dup_pid", bad_exp(&cid2, "201", "true", &att, ow)),
        (
            "wrong_attempt",
            bad_exp(&cid2, "202", "true", &other_att, ow),
        ),
        ("wrong_owner", bad_exp(&cid2, "202", "true", &att, "other")),
    ];
    for (name, r_exp) in bad {
        let mut r = StubRunner(BTreeMap::from([
            (key(&cap), ok_cap.clone()),
            (key(&exp), r_exp),
        ]));
        assert!(inspect_worker_pair_with(&mut r, attempt).is_err(), "{name}");
    }
}

#[test]
fn disposal_probes_detect_orphan_id_cgroup_pid_and_unreadable_proc_errors() {
    let attempt = Uuid::new_v4();
    let baseline = sample_workers(attempt);
    let root = std::env::temp_dir().join(format!("dw-probe-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(root.join("1001")).unwrap();
    std::fs::create_dir_all(root.join("9999")).unwrap();
    std::fs::write(
        root.join("9999/cgroup"),
        format!("0::/docker/{}\n", baseline.exporter.container_id),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("8888/cgroup")).unwrap();

    let mut errs = Vec::new();
    let (pids, cgroups) = scan_proc_disposal_at(&root, &baseline, &mut errs);
    assert!(
        pids == vec![1001] && cgroups.len() == 1 && cgroups[0].contains("9999") && !errs.is_empty()
    );

    let ps_key = |f: String| {
        format!("/usr/bin/docker ps -a --filter {f} --format {{{{.ID}}}} {{{{.Names}}}}")
    };
    let mut ps_errs = Vec::new();
    let mut ps_runner = StubRunner(BTreeMap::from([
        (
            ps_key(format!("label=org.duskweave.attempt={attempt}")),
            ok_cmd(String::new()),
        ),
        (
            ps_key(format!("id={}", baseline.capture.container_id)),
            ok_cmd(String::new()),
        ),
        (
            ps_key(format!("id={}", baseline.exporter.container_id)),
            ok_cmd(format!(
                "{} {}",
                baseline.exporter.container_id, baseline.exporter.name
            )),
        ),
    ]));
    let found = inspect_attempt_containers_with(&mut ps_runner, attempt, &baseline, &mut ps_errs);
    assert!(ps_errs.is_empty() && found.len() == 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn post_fault_and_recovery_validators_enforce_unknown_vs_stopped_without_replay() {
    let (run_id, attempt) = (Uuid::new_v4(), Uuid::new_v4());
    let b = ActiveHeadBinding {
        run_id,
        step_index: 2,
        attempt_id: attempt,
        session_generation: 2,
    };
    let mut sql_unk = sample_sql(run_id, attempt);
    (sql_unk.run_status, sql_unk.step_status) = ("unknown".into(), "unknown".into());
    let snap_unk = sample_snapshot(run_id, attempt, "unknown", "pending");
    assert!(validate_post_fault_state(UNK, &b, &sql_unk, &snap_unk, 2).is_ok());

    let r_unk = sample_receipt(run_id, "unknown");
    assert!(validate_recovery_receipts(UNK, run_id, &r_unk, &r_unk, &r_unk).is_ok());
    assert!(validate_post_recover_sql(UNK, &b, &sql_unk, &sql_unk).is_ok());

    let mut sql_stop = sample_sql(run_id, attempt);
    (sql_stop.run_status, sql_stop.step_status) = ("stopped".into(), "stopped".into());
    let snap_stop = sample_snapshot(run_id, attempt, "stopped", "stopped");
    assert!(
        validate_post_fault_state(ExpectedFaultOutcome::Stopped, &b, &sql_stop, &snap_stop, 2)
            .is_ok()
    );
    assert!(validate_post_fault_state(UNK, &b, &sql_stop, &snap_stop, 2).is_err());

    let cases: [(&str, Mut<SqlActiveState>, Mut<Value>); 5] = [
        (
            "completed_retained",
            |s| {
                s.run_status = "retained".into();
                s.step_status = "completed".into();
            },
            |v| {
                v["control"]["status"] = json!("retained");
                v["steps"][1]["status"] = json!("completed");
            },
        ),
        (
            "timeout_result",
            |s| s.head_result_present = true,
            |v| {
                v["steps"][1]["head_result_id"] = json!(Uuid::new_v4());
                v["steps"][1]["head_outcome"] = json!("timed_out");
            },
        ),
        (
            "result_recorded",
            |s| s.has_result_recorded_event = true,
            |v| {
                let att = v["sessions"][0]["attempt"].clone();
                v["events"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"attempt": att, "phase": "result_recorded"}));
            },
        ),
        (
            "substituted_attempt",
            |s| s.head_attempt_id = Some(Uuid::new_v4()),
            |v| v["steps"][1]["head_attempt_id"] = json!(Uuid::new_v4()),
        ),
        (
            "d3_executed",
            |s| s.d3_executed = true,
            |v| v["steps"][2]["status"] = json!("decision_running"),
        ),
    ];
    for (name, ms, mv) in cases {
        let (mut s, mut v) = (sql_unk.clone(), snap_unk.clone());
        ms(&mut s);
        mv(&mut v);
        assert!(
            validate_post_fault_state(UNK, &b, &s, &snap_unk, 2).is_err(),
            "{name} sql"
        );
        assert!(
            validate_post_fault_state(UNK, &b, &sql_unk, &v, 2).is_err(),
            "{name} snap"
        );
    }
    let mut r_bad = r_unk.clone();
    r_bad["head_effect_count"] = json!(1);
    assert!(validate_recovery_receipts(UNK, run_id, &r_unk, &r_unk, &r_bad).is_err());
    let mut sql_fake = sql_unk.clone();
    sql_fake.disposal_confirmed = true;
    assert!(validate_post_recover_sql(UNK, &b, &sql_unk, &sql_fake).is_err());
}

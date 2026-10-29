//! SDK termination qualification through the existing native fixture.
use super::*;

pub(super) fn sdk_stop_before_release_keeps_zero_calls_and_effects() {
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    refresh();
    let server = fixture::Server::start("127.0.0.1:18083", |_| fixture::Reply::Reset);
    let q = registered(0x2a400);
    let record = run_support::start_only(&q);
    let mut store = run_support::broker_store();
    store
        .start_phase(record.request.run_id, RunPhase::DecisionOne)
        .unwrap();
    let (ok, _) = run_support::cli("m1-run-stop", "--run", q["run_id"].as_str().unwrap());
    assert!(ok);
    assert!(
        store
            .claim_phase(
                record.request.run_id,
                RunPhase::DecisionOne,
                &run_support::release()
            )
            .is_err()
    );
    for _ in 0..2 {
        let (_, r) = run_support::cli("m1-run-recover", "--run", q["run_id"].as_str().unwrap());
        assert_eq!(r["decision_count"], 0);
        assert_eq!(r["effect_count"], 0);
        assert_eq!(server.count(), 0);
    }
    assert_eq!(owner_counts(record.decision_one), [0, 0, 0]);
}
pub(super) fn sdk_observed_send_stop_lease_loss_and_parent_loss_terminate_without_replay() {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    let _guard = native::db();
    refresh();
    let selected = std::env::var("DW_TEST_SDK_FAULT_MODE").ok();
    assert!(
        selected
            .as_deref()
            .is_none_or(|m| ["stop", "lease", "parent"].contains(&m)),
        "closed test mode"
    );
    for (index, mode) in ["stop", "lease", "parent"].into_iter().enumerate() {
        if selected.as_deref().is_some_and(|m| m != mode) {
            continue;
        }
        let held = Arc::new(AtomicBool::new(false));
        let gate = held.clone();
        let server = fixture::Server::start("127.0.0.1:18083", move |_| {
            fixture::Reply::Hold(gate.clone())
        });
        let q = registered(0x2a410 + index as u128);
        let run = run_support::id(&q);
        let mut observer = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
        observer
            .batch_execute("SET statement_timeout='200ms'; SET lock_timeout='100ms'")
            .unwrap();
        let (mut parent, log, launched) = launch_sdk_run(&q);
        await_current_run_request(
            &server,
            &mut parent,
            &mut observer,
            &q,
            &log,
            launched,
            mode,
        );
        let (child, _) = parent.phase().expect("owned phase child before fault");
        assert!(
            parent.try_wait().unwrap().is_none()
                && !run_support::observe_child_exit(child, Instant::now()),
            "{mode}: intended post-send fault requires a live product child and parent"
        );
        let injected = Instant::now();
        match mode {
            "stop" => {
                let (ok, _) = run_support::cli("m1-run-stop", "--run", &run.to_string());
                assert!(ok);
            }
            "lease" => {
                native::admin()
                    .execute(
                        "UPDATE execution.autonomous_control SET lease_until_ms=0 WHERE run_id=$1",
                        &[&run],
                    )
                    .unwrap();
            }
            _ => {
                parent.kill().unwrap();
                parent.wait().unwrap();
            }
        }
        let until = Instant::now() + Duration::from_secs(8);
        let child_terminated = run_support::observe_child_exit(child, until);
        while parent.try_wait().unwrap().is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let parent_terminated = parent.try_wait().unwrap().is_some();
        eprintln!(
            "safe_sdk_fault mode={mode} child_terminated={child_terminated} parent_terminated={parent_terminated} elapsed_ms={}",
            injected.elapsed().as_millis()
        );
        if !parent_terminated {
            let _ = parent.kill();
            let _ = parent.wait();
        }
        held.store(true, Ordering::SeqCst);
        assert!(
            child_terminated && parent_terminated,
            "{mode}: bounded product termination required"
        );
        if mode == "parent" {
            std::thread::sleep(Duration::from_millis(2100));
        }
        let (_, before) = run_support::cli("m1-run-inspect", "--run", &run.to_string());
        assert_eq!(before["effect_count"], 0);
        assert_eq!(server.count(), 1);
        assert_eq!(
            before["inference"].as_array().unwrap().len(),
            1,
            "possible send remains charged"
        );
        for _ in 0..2 {
            let (_, after) = run_support::cli("m1-run-recover", "--run", &run.to_string());
            assert_eq!(after["effect_count"], 0);
            assert_eq!(server.count(), 1, "{mode} cannot replay");
            assert_eq!(after["decision_one"], before["decision_one"]);
            assert_eq!(
                after["inference"], before["inference"],
                "{mode} retains original charge/usage uncertainty"
            );
        }
    }
}

fn safe_parent_terminal(log: &std::path::Path, mode: &str) {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let receipt = text
        .lines()
        .find_map(|l| serde_json::from_str::<serde_json::Value>(l).ok());
    let status = match receipt.as_ref().and_then(|v| v["status"].as_str()) {
        Some("unknown") => "unknown",
        Some("stopped") => "stopped",
        Some("completed") => "completed",
        _ => "unavailable",
    };
    let diagnostic = receipt.as_ref().map(|v| &v["diagnostics"][0]);
    let ready = diagnostic
        .and_then(|d| d["readiness_observed"].as_bool())
        .unwrap_or(false);
    let cause = match diagnostic.and_then(|d| d["cause"].as_str()) {
        Some("readiness_deadline") => "readiness_deadline",
        Some("child_error") => "child_error",
        Some("phase_deadline") => "phase_deadline",
        _ => "unavailable",
    };
    let error = match diagnostic.and_then(|d| d["child_error"].as_str()) {
        Some("run_inference_pending") => "run_inference_pending",
        Some("inference_control_unavailable") => "inference_control_unavailable",
        _ => "unavailable",
    };
    let disposition = match receipt
        .as_ref()
        .and_then(|v| v["recorded_decisions"][0]["disposition"].as_str())
    {
        Some("inference_deadline") => "inference_deadline",
        Some("provider_cancelled_uncertain") => "provider_cancelled_uncertain",
        Some("provider_transport_unknown") => "provider_transport_unknown",
        _ => "unavailable",
    };
    eprintln!(
        "safe_sdk_parent mode={mode} status={status} ready={ready} cause={cause} error={error} disposition={disposition}"
    );
}
fn await_current_run_request(
    server: &fixture::Server,
    parent: &mut run_support::OwnedRun,
    observer: &mut postgres::Client,
    q: &serde_json::Value,
    log: &std::path::Path,
    launched: std::time::Instant,
    mode: &str,
) {
    use std::time::{Duration, Instant};
    let run = run_support::id(q);
    // The request still belongs to the original absolute run and phase bounds.
    // Observing readiness or a reservation never creates a new clock origin.
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let remaining = q["deadline_at_ms"].as_i64().unwrap() - wall;
    assert!((1..=400000).contains(&remaining), "original run bound");
    let mut end = Instant::now() + Duration::from_millis(remaining as u64);
    let mut seen = [false; 7];
    loop {
        if let Some(bounds) = run_support::phase_bounds(
            observer,
            run,
            duskweave::m1_inference_profile::REASONING_BOUNDED.phase_ms,
        ) {
            end = end.min(bounds.earliest_stop);
            for (index, at) in [(0, Some(bounds.started)), (1, bounds.claimed)] {
                if let Some(at) = at
                    && !seen[index]
                {
                    seen[index] = true;
                    eprintln!(
                        "safe_sdk_startup mode={mode} stage={} elapsed_ms={} uncertainty_ms={}",
                        ["phase_start", "phase_claim"][index],
                        at.saturating_duration_since(launched).as_millis(),
                        bounds.uncertainty.as_millis()
                    );
                }
            }
            if bounds.claimed.is_some() {
                let _ = parent.phase();
            }
        }
        let row=observer.query_opt("SELECT x.decision_one, (SELECT count(*) FROM execution.inference_reservations a WHERE a.operation_id=x.decision_one), (SELECT count(*) FROM execution.inference_calls a WHERE a.d=x.decision_one AND a.state='may_send_uncertain'), EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename=session_user AND a.pid<>pg_backend_pid() AND a.state='idle' AND a.query='SELECT execution.inference_window_ms($1,$2,$3)'), EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename=session_user AND a.pid<>pg_backend_pid() AND position('mission.current_inference_publication' in a.query)>0), EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename=session_user AND a.pid<>pg_backend_pid() AND position('inference_reservation(' in a.query)>0) FROM execution.autonomous_runs x WHERE x.run_id=$1",&[&run]).unwrap_or_else(|_|panic!("restricted startup observation unavailable"));
        if let Some(row) = row {
            let flags = [
                row.get::<_, bool>(3),
                row.get::<_, bool>(4),
                row.get::<_, bool>(5),
                row.get::<_, i64>(1) > 0,
                row.get::<_, i64>(2) > 0,
            ];
            for (i, flag) in flags.into_iter().enumerate() {
                if flag && !seen[i + 2] {
                    seen[i + 2] = true;
                    if i >= 2 {
                        eprintln!(
                            "safe_sdk_startup mode={mode} stage=context_preflight_and_readiness_completed upper_bound_ms={}",
                            launched.elapsed().as_millis()
                        );
                    }
                    eprintln!(
                        "safe_sdk_startup mode={mode} stage={} observed_ms={}",
                        [
                            "window_acquired",
                            "context_read",
                            "ready_before_reservation",
                            "reservation_committed",
                            "may_send"
                        ][i],
                        launched.elapsed().as_millis()
                    );
                }
            }
            if server.count() > 0 {
                assert_eq!(
                    server.count(),
                    1,
                    "exactly one complete request before fault: {mode}"
                );
                let requests = server.requests.lock().unwrap();
                let wire: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
                let p = fixture::pack_in(&wire).expect("immutable current-run pack");
                assert!(
                    p["decision_id"] == row.get::<_, uuid::Uuid>(0).to_string(),
                    "complete request must belong to the current decision"
                );
                for key in ["engagement_id", "campaign_id", "permission_id"] {
                    assert!(
                        p[key] == q["context"]["mission"][key],
                        "complete request must belong to the current run scope"
                    );
                }
                eprintln!(
                    "safe_sdk_startup mode={mode} stage=actual_send elapsed_ms={} requests=1",
                    launched.elapsed().as_millis()
                );
                return;
            }
        }
        if parent.try_wait().unwrap().is_some() {
            safe_parent_terminal(log, mode);
            panic!("parent terminated before intended send: {mode}");
        }
        if Instant::now() >= end {
            safe_parent_terminal(log, mode);
            eprintln!(
                "safe_sdk_startup mode={mode} stage=send_barrier elapsed_ms={} requests={}",
                launched.elapsed().as_millis(),
                server.count()
            );
            panic!("intended send barrier not reached: {mode}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn launch_sdk_run(
    q: &serde_json::Value,
) -> (
    run_support::OwnedRun,
    std::path::PathBuf,
    std::time::Instant,
) {
    use std::process::Stdio;
    let run = run_support::id(q);
    let input = run_support::path(q);
    let root = run_support::control_root();
    let log = std::env::temp_dir().join(format!("dw-sdk-parent-{run}.log"));
    let output = std::fs::File::create(&log).unwrap();
    let launched = std::time::Instant::now();
    let child = run_support::command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args([
            "m1-run",
            "--input",
            input.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ])
        .stderr(Stdio::from(output.try_clone().unwrap()))
        .stdout(Stdio::from(output))
        .spawn()
        .unwrap();
    let mut owned = run_support::OwnedRun::new(child, run);
    owned.own_file(&input);
    owned.own_file(&log);
    owned.own_control_root(&root);
    (owned, log, launched)
}
pub(super) fn sdk_post_send_assertion_cleanup_settles_owned_processes_response_and_files() {
    let _guard = native::db();
    refresh();
    let held = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let gate = held.clone();
    let server = fixture::Server::start("127.0.0.1:18083", move |_| {
        fixture::Reply::Hold(gate.clone())
    });
    let q = registered(0x2a420);
    let mut observer = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let (mut parent, log, launched) = launch_sdk_run(&q);
    await_current_run_request(
        &server,
        &mut parent,
        &mut observer,
        &q,
        &log,
        launched,
        "cleanup",
    );
    let child = parent
        .phase()
        .expect("owned phase before cleanup injection")
        .0;
    let verdict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _server = server;
        let _parent = parent;
        panic!("synthetic post-send assertion failure");
    }));
    assert!(verdict.is_err());
    // A custody verdict, deliberately distinct from product termination.
    assert!(run_support::observe_child_exit(
        child,
        std::time::Instant::now() + std::time::Duration::from_secs(2)
    ));
    assert!(!log.exists());
    assert!(
        std::net::TcpListener::bind("127.0.0.1:18083").is_ok(),
        "held response port must be reusable after failure"
    );
    let idle:i64=observer.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND usename=session_user AND state='idle in transaction'",&[]).unwrap().get(0);
    assert_eq!(
        idle, 0,
        "owned SQL transaction must be settled before next case"
    );
}

//! Restricted diagnostic publication and real-entrypoint configuration boundaries.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_terrain_context.rs"]
mod terrain;
use terrain::terrain_support::native;
#[path = "support/m1_run.rs"]
mod run_support;
use duskweave::m1_run::{CauseSource, DiagnosticCause, PhaseDiagnostic, RunPhase};
use run_support::*;
use serde_json::{Value, json};
#[path = "support/database_wait_proxy.rs"]
mod proxy;

fn diagnostic(run: uuid::Uuid) -> PhaseDiagnostic {
    PhaseDiagnostic {
        version: "m1_run_phase_diagnostic_v1".into(),
        run_id: run,
        phase: RunPhase::DecisionOne,
        cause: DiagnosticCause::ChildExit,
        cause_source: CauseSource::ParentObserved,
        child_error: None,
        exit_code: Some(124),
        readiness_observed: false,
        elapsed_ms: Some(20),
        reported_guard: None,
    }
}
fn counts() -> Value {
    let row = native::admin()
        .query_one(
            "SELECT (SELECT count(*) FROM execution.autonomous_runs), \
        (SELECT count(*) FROM execution.inference_reservations), \
        (SELECT count(*) FROM execution.inference_calls), \
        (SELECT count(*) FROM execution.fixture_dns_attempts)",
            &[],
        )
        .unwrap();
    json!([
        row.get::<_, i64>(0),
        row.get::<_, i64>(1),
        row.get::<_, i64>(2),
        row.get::<_, i64>(3)
    ])
}
#[test]
fn start_preflight_reports_closed_owner_and_category_before_any_run_write() {
    let _guard = native::db();
    refresh();
    let q = registered(0xda01);
    let input = path(&q);
    let before = counts();
    for (owner, key) in [
        ("core", "DW_DATABASE_URL"),
        ("broker", "DW_INFERENCE_BROKER_DATABASE_URL"),
        ("cognition", "DW_COGNITION_DATABASE_URL"),
        ("trajectory", "DW_TRAJECTORY_DATABASE_URL"),
        ("terrain", "DW_TERRAIN_DATABASE_URL"),
    ] {
        for (case, expected) in [
            (0, "missing_env"),
            (1, "invalid_dsn"),
            (2, "database_config_conflict"),
        ] {
            let mut child = command();
            match case {
                0 => {
                    child.env_remove(key);
                }
                1 => {
                    child.env(key, "synthetic-sensitive-invalid-dsn");
                }
                _ => {
                    child.env(format!("{key}_FILE"), "synthetic-sensitive-file");
                }
            }
            let output = child
                .args(["m1-run", "--input", input.to_str().unwrap()])
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                format!("config_owner={owner}\nerror={expected}")
            );
            assert_eq!(counts(), before);
        }
    }
    #[cfg(windows)]
    {
        let out = command()
            .env_remove("SystemRoot")
            .args(["m1-run", "--input", input.to_str().unwrap()])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "config_owner=system\nerror=missing_env"
        );
        assert_eq!(counts(), before);
    }
    std::fs::remove_file(input).unwrap();
}

#[test]
fn restricted_sql_validates_closed_payload_owner_phase_and_immutable_duplicate() {
    let _guard = native::db();
    refresh();
    let q = registered(0xda02);
    let rec = start_only(&q);
    let r = id(&q);
    let d = diagnostic(r);
    let raw = serde_json::to_value(&d).unwrap();
    let mut c = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    assert!(
        c.execute("SELECT execution.run_phase_diagnostic($1)", &[&raw])
            .is_err()
    );
    broker_store()
        .start_phase(r, RunPhase::DecisionOne)
        .unwrap();
    let mut foreign = native::client("DW_NATIVE_OTHER_BROKER_DATABASE_URL");
    assert!(
        foreign
            .execute("SELECT execution.run_phase_diagnostic($1)", &[&raw])
            .is_err()
    );
    let mut invalid = Vec::new();
    for (key, value) in [
        ("run_id", json!(consent_id())),
        ("phase", json!("decision_two")),
        ("child_error", json!("synthetic-sensitive-sentinel")),
        ("unexpected", json!(0)),
        ("cause", Value::Null),
        ("cause_source", Value::Null),
        ("readiness_observed", json!(0)),
        ("exit_code", json!(2147483648_i64)),
        ("exit_code", json!(-2147483649_i64)),
        ("exit_code", json!(1.5)),
        ("elapsed_ms", json!(150001)),
        ("elapsed_ms", json!(-1)),
        ("elapsed_ms", json!("secret")),
        ("child_error", json!("x".repeat(1100))),
    ] {
        let mut v = raw.clone();
        v[key] = value;
        invalid.push(v);
    }
    let mut missing = raw.clone();
    missing.as_object_mut().unwrap().remove("elapsed_ms");
    invalid.push(missing);
    for v in invalid {
        assert!(
            c.execute("SELECT execution.run_phase_diagnostic($1)", &[&v])
                .is_err()
        );
    }
    let before = broker_store().summary(r).unwrap();
    assert_eq!(before["diagnostic_publication"], "unavailable");
    for code in [i32::MIN, i32::MAX] {
        let mut trial = raw.clone();
        trial["exit_code"] = json!(code);
        let mut tx = c.transaction().unwrap();
        tx.execute("SELECT execution.run_phase_diagnostic($1)", &[&trial])
            .unwrap();
        tx.rollback().unwrap();
    }
    broker_store().stop(r).unwrap();
    // Expiry/STOP does not revoke permission to account for an already-started phase.
    let stopped = broker_store().summary(r).unwrap();
    let mut store = broker_store();
    store.publish_diagnostic(&d).unwrap();
    store.publish_diagnostic(&d).unwrap();
    let mut conflict = raw.clone();
    conflict["exit_code"] = json!(1);
    assert!(
        c.execute("SELECT execution.run_phase_diagnostic($1)", &[&conflict])
            .is_err()
    );
    assert!(
        c.execute(
            "DELETE FROM execution.run_phase_diagnostics WHERE run_id=$1",
            &[&r]
        )
        .is_err()
    );
    assert!(
        c.execute(
            "UPDATE execution.run_phase_diagnostics SET payload=$2 WHERE run_id=$1",
            &[&r, &raw]
        )
        .is_err()
    );
    assert!(
        c.execute(
            "INSERT INTO execution.run_phase_diagnostics VALUES($1,'acquisition',$2)",
            &[&r, &raw]
        )
        .is_err()
    );
    let (ok, after) = cli("m1-run-inspect", "--run", &r.to_string());
    assert!(ok);
    assert_eq!(after["diagnostics"], json!([raw]));
    for key in [
        "status",
        "phases",
        "inference",
        "decision_count",
        "effect_count",
        "acquisition",
        "terrain",
    ] {
        assert_eq!(after[key], stopped[key], "{key}");
    }
    assert!(
        !serde_json::to_string(&after)
            .unwrap()
            .contains("preparation_timeout")
    );
    assert_eq!(rec.request.run_id, r);
}

#[test]
fn publication_connection_loss_retains_honest_local_receipt_without_replay() {
    let _guard = native::db();
    refresh();
    let q = registered(0xda03);
    let rec = start_only(&q);
    let r = rec.request.run_id;
    broker_store()
        .start_phase(r, RunPhase::DecisionOne)
        .unwrap();
    broker_store().stop(r).unwrap();
    let mut client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    // The restricted login can terminate its own connection; no admin privilege is added.
    assert!(
        client
            .batch_execute("SELECT pg_terminate_backend(pg_backend_pid())")
            .is_err()
    );
    let mut store = duskweave::postgres_m1_run::PgRunStore::new(client);
    assert!(store.publish_diagnostic(&diagnostic(r)).is_err());
    let local = store.summary(r).unwrap();
    assert_eq!(local["diagnostic_publication"], "pending_or_unknown");
    assert_eq!(local["pending_diagnostics"][0]["exit_code"], 124);
    let (_, after) = cli("m1-run-recover", "--run", &r.to_string());
    assert_eq!(after["diagnostic_publication"], "unavailable");
    assert_eq!(after["diagnostics"], json!([]));
    assert_eq!(after["effect_count"], 0);
    assert_eq!(after["decision_count"], 0);
}

#[test]
fn real_parent_publication_lock_failure_reports_pending_and_fresh_recovery_never_replays() {
    let _guard = native::db();
    refresh();
    let q = registered_with(0xda04, |raw| {
        let p = &mut raw["m1_inference_permission"];
        p["run_profile"] = json!("autonomous-dns-provider-fixture-v1");
        p["profile"] = json!("deepseek-responses-autonomous-v1");
        p["recipient"] = json!("https://api.deepseek.com/responses");
    });
    let input = path(&q);
    let root = control_root();
    let mut admin = native::admin();
    let mut lock = admin.transaction().unwrap();
    lock.batch_execute("LOCK TABLE execution.run_phase_diagnostics IN ACCESS EXCLUSIVE MODE")
        .unwrap();
    let launched = std::time::Instant::now();
    let output = command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args(["m1-run", "--input", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let receipt: Value = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .unwrap();
    assert_eq!(receipt["diagnostic_publication"], "pending_or_unknown");
    assert_eq!(
        receipt["pending_diagnostics"][0]["child_error"],
        "run_inference_pending"
    );
    assert_eq!(receipt["pending_diagnostics"][0]["exit_code"], 1);
    println!(
        "publication_lock_parent_total_ms={} diagnostic={}",
        launched.elapsed().as_millis(),
        receipt["pending_diagnostics"][0]
    );
    lock.rollback().unwrap();
    std::fs::remove_file(input).unwrap();
    let (ok, before) = cli("m1-run-inspect", "--run", &id(&q).to_string());
    assert!(ok);
    assert_eq!(before["diagnostic_publication"], "unavailable");
    assert_eq!(before["diagnostics"], json!([]));
    let (ok, after) = cli("m1-run-recover", "--run", &id(&q).to_string());
    assert!(ok);
    for key in ["diagnostics", "inference", "effect_count", "decision_count"] {
        assert_eq!(before[key], after[key], "{key}");
    }
    assert_eq!(after["effect_count"], 0);
    assert_eq!(after["inference"], json!([]));
}

#[test]
fn expired_acquisition_phase_publishes_diagnosis_within_remaining_run_budget() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let _guard = native::db();
    refresh();
    let q = registered(0xda05);
    let r = id(&q);
    let input = path(&q);
    let log = std::env::temp_dir().join(format!("dw-phase-expiry-{r}.log"));
    let relay = proxy::Relay::start(
        "127.0.0.1:5432".parse().unwrap(),
        b"SELECT execution.reserve_fixture_dns",
    );
    let routed =
        terrain::terrain_support::through_dsn("DW_INFERENCE_BROKER_DATABASE_URL", relay.port());
    let launched = Instant::now();
    let out = std::fs::File::create(&log).unwrap();
    let child = command()
        .env("DW_INFERENCE_BROKER_DATABASE_URL", routed)
        .args(["m1-run", "--input", input.to_str().unwrap()])
        .stdout(Stdio::from(out.try_clone().unwrap()))
        .stderr(Stdio::from(out))
        .spawn()
        .unwrap();
    // Owned custody is installed before the first panicking barrier: the
    // parent, its phase children, the input and the log are all settled by
    // the custodian even on a failed witness.
    let mut parent = OwnedRun::new(child, r);
    parent.own_file(&input);
    parent.own_file(&log);
    // Stage reached: the parked reserve call must be observed inside the
    // original acquisition/run envelope while the owned parent still runs.
    // A dead parent or an exhausted bound is a failed witness, never a
    // restarted clock.
    let mut observer = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut bounds: Option<PhaseBounds> = None;
    let mut bounds_at = Instant::now();
    let mut bound_now: Option<Instant> = None;
    let outcome = original_barrier(
        |within| relay.observed(within),
        || parent.try_wait().unwrap().is_none(),
        || {
            if Instant::now() >= bounds_at {
                if let Some(b) = phase_bounds_for(&mut observer, r, RunPhase::Acquisition, 60000) {
                    bounds = Some(b);
                }
                bound_now = match &bounds {
                    Some(b) => Some(b.expiry_upper(60000)),
                    None => run_stop(&mut observer, r),
                };
                bounds_at = Instant::now() + Duration::from_millis(200);
            }
            bound_now
        },
        Instant::now,
        std::thread::sleep,
    );
    let observed = Instant::now();
    match outcome {
        Barrier::Reached => {}
        Barrier::ParentExited => panic!(
            "reached stage failed: parent exited before the reserve barrier: {}",
            parent_log(&log)
        ),
        Barrier::BoundPassed(end) => panic!(
            "reached stage failed: reserve was never withheld inside the \
             original acquisition envelope ({}ms over bound): {}",
            observed.saturating_duration_since(end).as_millis(),
            parent_log(&log)
        ),
    }
    // The forwarded reserve call implies the same-run, same-acquisition claim
    // already committed; the started/claimed records bracket the local child
    // launch with measured projection uncertainty.
    let brackets = phase_bounds_for(&mut observer, r, RunPhase::Acquisition, 60000)
        .filter(|b| b.claimed.is_some())
        .expect("reached stage failed: same-acquisition claim must precede the reserve call");
    assert!(
        brackets.uncertainty <= Duration::from_millis(100),
        "acquisition bracket observation round trip unqualified"
    );
    assert!(
        observer
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM execution.autonomous_phases \
                 WHERE run_id=$1 AND phase='decision_one' AND status='completed')",
                &[&r],
            )
            .unwrap()
            .get::<_, bool>(0),
        "acquisition began without a completed first decision"
    );
    // The acquisition child, not a cached decision-one PID, is the owned
    // expiry witness: product termination must precede the upper expiry
    // bracket, and a custodian kill never satisfies this witness.
    assert!(
        parent.phase_for(RunPhase::Acquisition).is_some(),
        "owned same-acquisition phase child must be discoverable"
    );
    let expiry_upper = brackets.expiry_upper(60000);
    let child_done = parent.observe_phase_exit(expiry_upper).unwrap_or_else(|| {
        panic!(
            "finished stage failed: acquisition child survived its original \
             phase expiry: {}",
            parent_log(&log)
        )
    });
    // Remaining publication allowance after the expiry upper bracket. The
    // admitted run deadline already bounds expiry itself and no clock is
    // restarted here.
    let publication_end = expiry_upper + Duration::from_millis(5000);
    let done = wait_parent_exit(&mut parent, publication_end, "acquisition");
    let parent_ms = launched.elapsed().as_millis();
    drop(relay);
    let output = std::fs::read_to_string(&log).unwrap();
    let receipt: Value = output
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .unwrap();
    let (ok, fresh) = cli("m1-run-inspect", "--run", &r.to_string());
    assert!(ok);
    assert_eq!(fresh["effect_count"], 0);
    assert_eq!(fresh["decision_count"], 1);
    assert_eq!(
        fresh["diagnostic_publication"], "committed",
        "publication stage failed: {receipt}"
    );
    let diagnostic = fresh["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["phase"] == "acquisition")
        .expect("expired phase must be diagnosable from a fresh process");
    assert!(diagnostic["elapsed_ms"].as_u64().unwrap() >= 59000);
    assert_eq!(diagnostic["cause"], "phase_deadline");
    assert_eq!(diagnostic["cause_source"], "parent_observed");
    assert_eq!(receipt["diagnostics"], fresh["diagnostics"]);
    let (ok, recovered) = cli("m1-run-recover", "--run", &r.to_string());
    assert!(ok);
    for key in ["diagnostics", "effect_count", "decision_count"] {
        assert_eq!(recovered[key], fresh[key], "{key}");
    }
    println!(
        "expired_phase parent_ms={parent_ms} started={}ms claimed={}ms \
         expiry_slack={}ms child_done={}ms parent_done={}ms diagnostic={diagnostic}",
        brackets
            .started
            .saturating_duration_since(launched)
            .as_millis(),
        brackets
            .claimed
            .unwrap()
            .saturating_duration_since(launched)
            .as_millis(),
        expiry_upper
            .saturating_duration_since(child_done)
            .as_millis(),
        child_done.saturating_duration_since(launched).as_millis(),
        done.saturating_duration_since(launched).as_millis()
    );
}

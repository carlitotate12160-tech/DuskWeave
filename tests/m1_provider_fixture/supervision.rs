//! Provider startup, local STOP, and independently supervised deadlines.
use super::{native, provider_permission, proxy, run_support, terrain, withdrawal};
use duskweave::m1_run::RunPhase;
use duskweave::mission::OperationId;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_mission::PgMissionStore;
use run_support::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) fn normal_binary_missing_control_refuses_provider_phase_without_deterministic_fallback()
{
    let _guard = native::db();
    refresh();
    let q = registered_with(0xc903, provider_permission);
    let path = path(&q);
    let output = command()
        .args(["m1-run", "--input", path.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("run_control_unqualified"));
    assert!(
        broker_store().inspect(id(&q)).is_err(),
        "missing start configuration must create no run"
    );
}

pub(super) fn qualified_private_control_reaches_selected_provider_and_missing_key_is_durable_not_fallback()
 {
    let _guard = native::db();
    refresh();
    let q = registered_with(0xc905, provider_permission);
    let path = path(&q);
    let root = control_root();
    let launched = Instant::now();
    let output = command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args(["m1-run", "--input", path.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    let rec = broker_store().inspect(id(&q)).unwrap();
    let row = native::admin()
        .query_opt(
            "SELECT payload FROM cognition.reasoning_decisions WHERE d=$1",
            &[&rec.decision_one],
        )
        .unwrap()
        .unwrap_or_else(|| {
            panic!(
                "provider phase failed before publication: {}; diagnostic phase: {}",
                String::from_utf8_lossy(&output.stdout),
                diagnose_phase(&root)
            )
        });
    let decision: Value = row.get(0);
    assert_eq!(
        decision["disposition"],
        "live_key_missing",
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let before = broker_store().summary(id(&q)).unwrap();
    let receipt: Value = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .unwrap();
    assert_eq!(receipt["diagnostics"], before["diagnostics"]);
    assert_eq!(before["diagnostic_publication"], "committed");
    assert_eq!(
        before["diagnostics"][0]["child_error"],
        "run_inference_pending"
    );
    assert_eq!(before["diagnostics"][0]["cause_source"], "child_report");
    assert_eq!(before["diagnostics"][0]["readiness_observed"], true);
    println!(
        "offline_parent_total_ms={} phase_diagnostic={}",
        launched.elapsed().as_millis(),
        before["diagnostics"][0]
    );
    assert_eq!(before["effect_count"], 0);
    assert_eq!(before["inference"], json!([]));
    let (ok, after) = cli("m1-run-recover", "--run", &id(&q).to_string());
    assert!(ok, "{after}");
    assert_eq!(after["effect_count"], 0);
    assert_eq!(after["inference"], json!([]));
    assert_eq!(after["diagnostics"], before["diagnostics"]);
}

fn diagnose_phase(root: &std::path::Path) -> String {
    use std::io::Read;
    use std::process::Stdio;
    let q = registered_with(0xc906, provider_permission);
    let rec = start_only(&q);
    let custodian = LeaseCustodian::start(&rec, broker_store());
    broker_store()
        .start_phase(rec.request.run_id, RunPhase::DecisionOne)
        .unwrap();
    let mut child = command()
        .env("DW_M1_RUN_CONTROL_ROOT", root)
        .env("DW_M1_RUN_PROVIDER_DECISION", "1")
        .args([
            "m1-run-phase",
            "--run",
            &rec.request.run_id.to_string(),
            "--phase",
            "decision_one",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let end = Instant::now() + Duration::from_secs(8);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= end {
            child.kill().unwrap();
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = child.wait().unwrap();
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .take(16384)
        .read_to_end(&mut bytes)
        .unwrap();
    let _ = custodian.finish();
    format!("status={status} {}", String::from_utf8_lossy(&bytes))
}

pub(super) fn production_stop_withdraw_and_manual_terrain_signal_before_blocked_authority_sql() {
    let _guard = native::db();
    refresh();
    for (slot, writer) in [(0xc940, "stop"), (0xc942, "terrain")] {
        signal_before_sql(slot, writer);
    }
    withdrawal::signal_before_sql(0xc941, false);
}
fn signal_before_sql(slot: u128, writer: &str) {
    use std::process::Stdio;
    let q = registered_with(slot, provider_permission);
    let rec = start_only(&q);
    let custodian = LeaseCustodian::start(&rec, broker_store());
    let root = control_root();
    let m = &rec.request.context.mission;
    let mut c = command();
    c.env("DW_M1_RUN_CONTROL_ROOT", &root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut input = None;
    let marker = if writer == "stop" {
        c.args(["m1-run-stop", "--run", &rec.request.run_id.to_string()]);
        root.join(format!("{}.run-stop", rec.request.run_id))
    } else {
        let now: i64 = native::admin()
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
                &[],
            )
            .unwrap()
            .get(0);
        let doc = terrain::terrain_support::dns_request_json(m.engagement_id, m.campaign_id, now);
        let path = std::env::temp_dir().join(format!("dw-pre-sql-{}.json", rec.request.run_id));
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        c.args([
            "m1-terrain-record",
            "--input",
            path.to_str().unwrap(),
            "--operation",
            &consent_id().to_string(),
        ]);
        input = Some(path);
        root.join(format!(
            "{}.{}.campaign-stop",
            m.engagement_id.0, m.campaign_id.0
        ))
    };
    let mut locker = native::admin();
    let key = scope_key(&rec);
    lock_scope(&mut locker, &key);
    let mut child = c.spawn().unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(Instant::now() < end, "missing pre-SQL signal for {writer}");
        assert!(
            child.try_wait().unwrap().is_none(),
            "writer exited before signal: {writer}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(std::fs::metadata(&marker).unwrap().len() <= 5);
    loop {
        let blocked: bool = locker
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_locks l JOIN pg_stat_activity a ON a.pid=l.pid \
             WHERE l.locktype='advisory' AND NOT l.granted AND a.datname=current_database())",
                &[],
            )
            .unwrap()
            .get(0);
        if blocked {
            break;
        }
        assert!(
            Instant::now() < end,
            "writer never reached the held SQL fence: {writer}"
        );
        assert!(
            child.try_wait().unwrap().is_none(),
            "writer refused before held fence: {writer}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let status: String = locker
        .query_one(
            "SELECT status FROM execution.autonomous_control WHERE run_id=$1",
            &[&rec.request.run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        status, "running",
        "a local intent is not a committed SQL withdrawal"
    );
    unlock_scope(&mut locker, &key);
    let end = Instant::now() + Duration::from_secs(8);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= end {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("writer not reaped: {writer}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        child.wait().unwrap().success(),
        "authoritative writer failed: {writer}"
    );
    assert!(
        marker.exists(),
        "a terminal marker must not be cleared to resume"
    );
    assert_eq!(std::fs::read(&marker).unwrap(), b"STOP\n");
    let summary = broker_store().summary(rec.request.run_id).unwrap();
    assert_eq!(summary["effect_count"], 0);
    assert_eq!(summary["inference"], json!([]));
    if writer == "terrain" {
        broker_store().stop(rec.request.run_id).unwrap();
    }
    let _ = custodian.finish();
    if let Some(path) = input {
        std::fs::remove_file(path).unwrap();
    }
}

fn wait_parent_exit(parent: &mut std::process::Child, end: Instant, mode: &str) -> Instant {
    while parent.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < end,
            "parent survived a parked {mode} session"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let done = Instant::now();
    assert!(
        !parent.wait().unwrap().success(),
        "{mode} must fail honestly"
    );
    done
}

fn deadline_parent(
    input: &std::path::Path,
    routed: &str,
    root: &std::path::Path,
    preparation_ms: Option<&str>,
) -> (std::process::Child, std::path::PathBuf) {
    use std::process::Stdio;
    let log = std::env::temp_dir().join(format!(
        "dw-deadline-parent-{}-{}.log",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let out = std::fs::File::create(&log).unwrap();
    let err = out.try_clone().unwrap();
    let mut parent = command();
    parent
        .env("DW_INFERENCE_BROKER_DATABASE_URL", routed)
        .env("DW_M1_RUN_CONTROL_ROOT", root)
        .env("DEEPSEEK_API_KEY", "dw-synthetic-provider-key")
        .args(["m1-run", "--input", input.to_str().unwrap()])
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    if let Some(ms) = preparation_ms {
        parent.env("DW_M1_RUN_PREPARATION_MS", ms);
    }
    (parent.spawn().unwrap(), log)
}
fn parent_log(log: &std::path::Path) -> String {
    String::from_utf8_lossy(&std::fs::read(log).unwrap_or_default()).into_owned()
}

pub(super) fn stalled_claim_reply_exits_provider_child_from_the_launch_clock() {
    let _guard = native::db();
    refresh();
    let q = registered_with(0xc960, provider_permission);
    let root = control_root();
    let relay = proxy::Relay::start(
        "127.0.0.1:5432".parse().unwrap(),
        b"SELECT execution.run_phase_claim",
    );
    let routed =
        terrain::terrain_support::through_dsn("DW_INFERENCE_BROKER_DATABASE_URL", relay.port());
    let input = path(&q);
    let launched = Instant::now();
    let (mut parent, log) = deadline_parent(&input, &routed, &root, Some("120000"));
    assert!(
        relay.observed(Duration::from_secs(10)),
        "claim query was never withheld: {}",
        parent_log(&log)
    );
    let observed = Instant::now();
    println!(
        "claim relay launch_to_barrier_ms={}",
        observed.duration_since(launched).as_millis()
    );
    let (child, child_launch) = phase_child(parent.id(), id(&q));
    // Observe the actual launch, with at most 500ms OS observation margin.
    let end = child_launch + Duration::from_millis(5500);
    let terminated = observe_child_exit(child, end);
    let assertion = std::panic::catch_unwind(|| {
        assert!(
            terminated,
            "child survived fixed preparation policy with stale override"
        )
    });
    if let Err(failure) = assertion {
        // Custodian cleanup occurs only after recording the failed assertion.
        parent.kill().unwrap();
        parent.wait().unwrap();
        std::panic::resume_unwind(failure);
    }
    // The launch-clock preparation bound is 5000ms, well inside the 50s phase
    // ceiling the old post-claim guard allowed a stalled claim to survive.
    let done = wait_parent_exit(&mut parent, observed + Duration::from_secs(14), "claim");
    std::fs::remove_file(&input).unwrap();
    let summary = broker_store().summary(id(&q)).unwrap();
    println!(
        "stalled-claim exit launch={}ms observed={}ms status={} out={}",
        launched.elapsed().as_millis(),
        done.duration_since(observed).as_millis(),
        summary["status"],
        parent_log(&log)
    );
    assert_eq!(summary["effect_count"], 0);
    assert_eq!(summary["decision_count"], 0);
    assert_eq!(summary["inference"], json!([]));
}

pub(super) fn stalled_reservation_reply_hard_stops_at_the_original_decision_envelope() {
    let _guard = native::db();
    refresh();
    let q = registered_with(0xc961, provider_permission);
    let root = control_root();
    // The needle names the function call, not the table: claim, identity,
    // window and context statements (which read inference_reservations) flow
    // unimpaired so readiness arms before only the reservation reply is held.
    let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"inference_reservation(");
    let routed =
        terrain::terrain_support::through_dsn("DW_INFERENCE_BROKER_DATABASE_URL", relay.port());
    let input = path(&q);
    let launched = Instant::now();
    let (mut parent, log) = deadline_parent(&input, &routed, &root, None);
    assert!(
        relay.observed(Duration::from_secs(8)),
        "reservation query was never withheld: {}",
        parent_log(&log)
    );
    let observed = Instant::now();
    println!(
        "reservation relay launch_to_barrier_ms={}",
        observed.duration_since(launched).as_millis()
    );
    let (child, child_launch) = phase_child(parent.id(), id(&q));
    // The original Broker connection remains idle on inference_window_ms
    // while the separate reservation connection is parked. Read its original
    // query-completion timestamp through a fresh restricted connection; do not
    // start a new window or infer a decision anchor from reservation arrival.
    let mut observer = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let sampled = Instant::now();
    let row = observer
        .query_one(
            "SELECT floor(extract(epoch FROM (clock_timestamp()-state_change))*1000)::bigint, \
         floor(extract(epoch FROM (state_change-query_start))*1000)::bigint \
         FROM pg_stat_activity WHERE usename=current_user AND datname=current_database() \
         AND state='idle' AND query='SELECT execution.inference_window_ms($1,$2,$3)'",
            &[],
        )
        .expect("original decision anchor must be observable");
    let uncertainty = sampled.elapsed();
    assert!(
        uncertainty <= Duration::from_millis(100),
        "anchor observation round trip unqualified"
    );
    // Project server-relative age onto the local monotonic clock. Docker's
    // wall clock need not agree with the Windows host's clock.
    let anchor = sampled - Duration::from_millis(u64::try_from(row.get::<_, i64>(0)).unwrap());
    let query_ms: i64 = row.get(1);
    assert!(
        (0..=100).contains(&query_ms),
        "anchor SQL duration {query_ms}ms is unqualified"
    );
    // 500ms total OS observation margin. The server completion observation
    // precedes the client's original monotonic deadline assignment by local
    // reply delivery only; it is never shifted to the reservation/readiness.
    let end = anchor + Duration::from_millis(45500);
    assert!(
        end < launched + Duration::from_millis(50000),
        "phase reaper must be later than this assertion"
    );
    println!(
        "decision anchor from child launch={}ms query={}ms remaining={}ms phase_slack={}ms pid={child}",
        anchor.duration_since(child_launch).as_millis(),
        query_ms,
        end.saturating_duration_since(Instant::now()).as_millis(),
        (launched + Duration::from_millis(50000))
            .duration_since(end)
            .as_millis()
    );
    // This relay parks ParseComplete, before the child's reservation can
    // commit. Seed an existing charge through the actual restricted owner
    // for this original identity to challenge no-refund recovery as well.
    // This is explicit test injection, not proof of a lost COMMIT ACK.
    let rec = broker_store().inspect(id(&q)).unwrap();
    let reserved = duskweave::m1_inference_budget::reservation(
        &mut PgInferenceReservationStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL")),
        Some(&mut PgMissionStore::new(native::client(
            "DW_TEST_DATABASE_URL",
        ))),
        duskweave::m1_inference_budget::ReservationAction::Reserve,
        &rec.request.context.mission,
        OperationId(rec.decision_one),
    )
    .unwrap()
    .unwrap();
    assert_eq!(reserved.capacity.calls, 2);
    assert_eq!(reserved.capacity.wall_time_ms, 45000);
    // Readiness was already sent: a ready inference child must survive past
    // the 5s preparation bound while its decision clock permits work.
    std::thread::sleep(Duration::from_millis(6000));
    assert!(
        parent.try_wait().unwrap().is_none(),
        "an overbroad 5s kill terminated a ready inference child: {}",
        parent_log(&log)
    );
    let terminated = observe_child_exit(child, end);
    let child_done = Instant::now();
    println!(
        "child exit observed={terminated} after anchor={}ms anchor observation uncertainty={}ms",
        child_done.duration_since(anchor).as_millis(),
        uncertainty.as_millis()
    );
    let assertion = std::panic::catch_unwind(|| {
        assert!(
            terminated && child_done <= end,
            "decision child survived original envelope; phase protection cannot satisfy this assertion"
        )
    });
    if let Err(failure) = assertion {
        // The fault relay remains parked until after the failed assertion.
        parent.kill().unwrap();
        parent.wait().unwrap();
        std::panic::resume_unwind(failure);
    }
    println!(
        "decision child terminated after anchor={}ms lifetime={}ms; parent accounting observed separately",
        child_done.duration_since(anchor).as_millis(),
        child_done.duration_since(child_launch).as_millis()
    );
    let done = wait_parent_exit(
        &mut parent,
        child_done + Duration::from_millis(4000),
        "reservation",
    );
    assert!(
        done.duration_since(observed) > Duration::from_secs(30),
        "termination must come from the parked reservation, not an early refusal"
    );
    std::fs::remove_file(&input).unwrap();
    let summary = broker_store().summary(id(&q)).unwrap();
    println!(
        "stalled-reservation exit stalled={}ms launch={}ms status={} out={}",
        done.duration_since(observed).as_millis(),
        done.duration_since(launched).as_millis(),
        summary["status"],
        parent_log(&log)
    );
    assert_eq!(summary["effect_count"], 0);
    assert_eq!(summary["decision_count"], 0);
    assert_eq!(summary["inference"], json!([]));
    assert_eq!(summary["status"], "unknown");
    let before = native::client("DW_INFERENCE_BROKER_DATABASE_URL").query_one(
        "SELECT operation_id,generation_call_id,repair_call_id,capacity FROM execution.inference_reservations \
         WHERE operation_id=$1 AND record_revision=1",
        &[&Uuid::parse_str(summary["decision_one"].as_str().unwrap()).unwrap()],
    ).expect("charged reservation survives the lost reply");
    assert_eq!(before.get::<_, Uuid>(1), reserved.generation_call_id);
    assert_eq!(before.get::<_, Uuid>(2), reserved.repair_call_id);
    for command_name in ["m1-run-recover", "m1-run-recover"] {
        let (ok, after) = cli(command_name, "--run", &id(&q).to_string());
        assert!(ok, "{after}");
        assert_eq!(after["effect_count"], 0);
        assert_eq!(after["inference"], json!([]));
        for field in ["decision_one", "decision_two", "attempt", "result"] {
            assert_eq!(after[field], summary[field]);
        }
    }
    let after = native::client("DW_INFERENCE_BROKER_DATABASE_URL").query_one(
        "SELECT operation_id,generation_call_id,repair_call_id,capacity FROM execution.inference_reservations \
         WHERE operation_id=$1 AND record_revision=1",
        &[&before.get::<_, Uuid>(0)],
    ).unwrap();
    for column in 0..3 {
        assert_eq!(after.get::<_, Uuid>(column), before.get::<_, Uuid>(column));
    }
    assert_eq!(after.get::<_, Value>(3), before.get::<_, Value>(3));
}

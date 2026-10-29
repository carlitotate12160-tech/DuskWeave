//! Original-window reservation witness and immutable-upgrade compatibility.
use super::*;

pub(crate) fn stalled_reservation_reply_hard_stops_at_the_original_decision_envelope() {
    let _guard = native::db();
    let upgraded = std::env::var("DW_TEST_GENAI_UPGRADE_COMPAT").ok();
    assert!(
        upgraded.as_deref().is_none_or(|v| v == "1"),
        "closed upgrade compatibility mode"
    );
    if upgraded.is_some() {
        // The native harness requalifies 0012 once per process. Reapply the
        // same immutable 0022 setup after that, preserving prior SDK records.
        let mut setup = native::admin();
        let prior:i64=setup.query_one("SELECT count(*) FROM execution.inference_calls WHERE configuration LIKE '%dw-inference-intent-v1%'",&[]).unwrap().get(0);
        assert!(
            prior > 0,
            "compatibility requires retained SDK call history"
        );
        setup
            .batch_execute(include_str!(
                "../../../migrations/0022_genai_inference_intent.sql"
            ))
            .unwrap_or_else(|_| panic!("owned 0022 fixture refresh failed"));
        let retained:i64=setup.query_one("SELECT count(*) FROM execution.inference_calls WHERE configuration LIKE '%dw-inference-intent-v1%'",&[]).unwrap().get(0);
        assert_eq!(
            retained, prior,
            "fixture refresh preserves SDK call history"
        );
        assert_sdk_sql_still_installed();
    } else {
        refresh();
    }
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
    let (parent, log) = deadline_parent(&input, &routed, &root, None);
    let mut parent = OwnedRun::new(parent, id(&q));
    parent.own_file(&input);
    parent.own_file(&log);
    parent.own_control_root(&root);
    let _ = parent.phase();
    // Absolute state-aware barrier: the withheld reservation reply must be
    // observed inside the original decision-one envelope. The bound is the
    // phase-start/claim bracket's upper expiry, or the admitted run deadline
    // while no phase record is observable; a dead parent or an exhausted
    // bound is a failed witness, never a restarted clock.
    let mut barrier_observer = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut bounds: Option<PhaseBounds> = None;
    let mut bounds_at = Instant::now();
    let mut bound_now: Option<Instant> = None;
    let reached = run_support::original_barrier(
        |within| relay.observed(within),
        || parent.try_wait().unwrap().is_none(),
        || {
            if Instant::now() >= bounds_at {
                if let Some(b) = run_support::phase_bounds(&mut barrier_observer, id(&q), 50000) {
                    bounds = Some(b);
                }
                bound_now = match &bounds {
                    Some(b) => Some(b.expiry_upper(50000)),
                    None => run_support::run_stop(&mut barrier_observer, id(&q)),
                };
                bounds_at = Instant::now() + Duration::from_millis(200);
            }
            bound_now
        },
        Instant::now,
        std::thread::sleep,
    );
    let observed = Instant::now();
    match reached {
        Barrier::Reached => {}
        Barrier::ParentExited => panic!(
            "parent exited before the reservation barrier: {}",
            parent_log(&log)
        ),
        Barrier::BoundPassed(end) => panic!(
            "reservation query was never withheld inside its original envelope ({}ms over bound): {}",
            observed.saturating_duration_since(end).as_millis(),
            parent_log(&log)
        ),
    }
    println!(
        "reservation relay launch_to_barrier_ms={}",
        observed.duration_since(launched).as_millis()
    );
    let (child, child_launch) = parent.phase().expect("ready owned phase child");
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
    let phase_bounds =
        run_support::phase_bounds(&mut observer, id(&q), 50000).expect("original phase start");
    assert!(
        phase_bounds.uncertainty <= Duration::from_millis(100),
        "phase observation round trip unqualified"
    );
    assert!(
        end < phase_bounds.earliest_stop,
        "phase reaper must be later than this assertion"
    );
    println!(
        "decision anchor from child launch={}ms query={}ms remaining={}ms phase_slack={}ms pid={child}",
        anchor.duration_since(child_launch).as_millis(),
        query_ms,
        end.saturating_duration_since(Instant::now()).as_millis(),
        phase_bounds.earliest_stop.duration_since(end).as_millis()
    );
    // This relay parks ParseComplete, before the child's reservation can
    // commit. Seed an existing charge through the actual restricted owner
    // for this original identity to challenge no-refund recovery as well.
    // This is explicit test injection, not proof of a lost COMMIT ACK.
    let rec = broker_store().inspect(id(&q)).unwrap();
    assert_eq!(rec.request.version, "m1_autonomous_run_request_v1");
    assert!(!rec.request.successor());
    assert_eq!(
        rec.request.run_profile,
        "autonomous-dns-provider-fixture-v1"
    );
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
    let exit = parent.observe_phase_exit(end);
    let observed_at = Instant::now();
    let terminated = exit.is_some();
    let child_done = exit.unwrap_or(observed_at);
    println!(
        "child exit observed={terminated} after anchor={}ms anchor observation uncertainty={}ms observer_settlement_ms={}",
        child_done.duration_since(anchor).as_millis(),
        uncertainty.as_millis(),
        observed_at
            .saturating_duration_since(child_done)
            .as_millis()
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
    if upgraded.is_some() {
        assert_sdk_sql_still_installed();
    }
}

fn assert_sdk_sql_still_installed() {
    let installed:bool=native::client("DW_INFERENCE_BROKER_DATABASE_URL").query_one("SELECT position('genai_binding_valid' in pg_get_functiondef('execution.inference_may_send(uuid,uuid,uuid,uuid,text,text,text,text)'::regprocedure))>0",&[]).unwrap_or_else(|_|panic!("SQL source identity observation unavailable")).get(0);
    assert!(
        installed,
        "upgrade compatibility must exercise installed 0022 admission without fixture replacement"
    );
}

#[path = "support/registration_db.rs"]
mod db_support;
use terrain::terrain_support::native;
#[path = "support/m1_run.rs"]
mod run_support;
#[path = "support/m1_terrain_context.rs"]
mod terrain;
use duskweave::m1_run::RunPhase;
use duskweave::postgres_m1_run::PgRunStore;

#[test]
fn hard_parent_loss_during_child_publication_leaves_unknown_and_zero_effect_replay() {
    use std::{process::Stdio, time::Duration};
    let _g = native::db();
    refresh();
    let q = registered(0xa727);
    let input = path(&q);
    let r = id(&q);
    let relay = proxy::Relay::start(
        "127.0.0.1:5432".parse().unwrap(),
        b"SELECT cognition.publish_reasoning",
    );
    let routed = terrain::terrain_support::through_dsn("DW_COGNITION_DATABASE_URL", relay.port());
    let mut parent = command()
        .env("DW_COGNITION_DATABASE_URL", routed)
        .args(["m1-run", "--input", input.to_str().unwrap()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let observed = relay.observed(Duration::from_secs(10));
    // Kill only this test's parent. No graceful Drop/final receipt can be assumed.
    let _ = parent.kill();
    parent.wait().unwrap();
    // Interrupt this test's child connection so the fixture does not leave an orphaned wait.
    drop(relay);
    std::fs::remove_file(input).unwrap();
    assert!(observed);
    std::thread::sleep(Duration::from_millis(2100));
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    assert!(store.renew(r).is_err());
    let before = store.summary(r).unwrap();
    assert_eq!(before["status"], "unknown_supervisor_lost");
    assert_eq!(before["effect_count"], 0);
    for command in ["m1-run-recover", "m1-run-recover"] {
        let (ok, v) = cli(command, "--run", &r.to_string());
        assert!(ok, "{v}");
        assert_eq!(v["effect_count"], 0);
        assert_eq!(v["decision_two"], before["decision_two"]);
    }
}
use run_support::*;
use serde_json::json;
#[path = "support/database_wait_proxy.rs"]
mod proxy;

#[test]
fn real_parent_records_child_role_failure_as_unknown_and_recovery_never_restarts_it() {
    let _g = native::db();
    refresh();
    let q = registered(0xa725);
    let input = path(&q);
    // A child receives a real but unqualified principal for Cognition, so construction fails closed.
    let output = command()
        .env(
            "DW_COGNITION_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        )
        .args(["m1-run", "--input", input.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_file(input).unwrap();
    assert!(!output.status.success());
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    let summary = store.summary(id(&q)).unwrap();
    assert_eq!(summary["status"], "unknown");
    assert_eq!(summary["effect_count"], 0);
    assert_eq!(summary["decision_count"], 0);
    for _ in 0..2 {
        let (ok, v) = cli("m1-run-recover", "--run", &id(&q).to_string());
        assert!(ok, "{v}");
        assert_eq!(v["effect_count"], 0);
        assert_eq!(v["decision_count"], 0);
    }
}

#[test]
fn result_commit_ack_loss_recovers_original_terrain_without_replaying_effect_or_decision() {
    use std::time::Duration;
    let _g = native::db();
    refresh();
    let rel = release();
    let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
    let mut store = broker_store();
    let custodian_store = broker_store();
    let mut relayed = broker_store_through(relay.port());
    let unsupervised_q = registered(0xa728);
    let q = registered(0xa722);
    let record = start_with(&mut store, &q);
    let custodian = LeaseCustodian::start(&record, custodian_store);
    complete_first(&record);
    let r = id(&q);
    store.start_phase(r, RunPhase::Acquisition).unwrap();
    store.claim_phase(r, RunPhase::Acquisition, &rel).unwrap();
    let request = store.reserve_effect(r, 0).unwrap();
    let result = duskweave::m1_fixture_dns::acquire(request).unwrap();
    let unsupervised = start_with(&mut store, &unsupervised_q);
    std::thread::sleep(Duration::from_millis(PRODUCTION_LEASE_MS + 250));
    assert_eq!(
        store.renew(unsupervised.request.run_id),
        Err(duskweave::Fail::State("run_stopped"))
    );
    store.check(r).unwrap();
    let outcome = std::thread::scope(|s| {
        let worker = s.spawn(|| relayed.commit_result(r, &result));
        assert!(relay.observed(Duration::from_secs(10)));
        assert!(
            relay.consumed(Duration::from_secs(10)),
            "server must confirm durable COMMIT before ACK loss"
        );
        let summary = store.summary(r).unwrap();
        assert_eq!(summary["effect_count"], 1);
        assert_eq!(summary["terrain_history_count"], 0);
        assert_eq!(
            native::admin()
                .query_one(
                    "SELECT count(*) FROM execution.fixture_dns_results WHERE result=$1",
                    &[&record.result]
                )
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        drop(relay);
        worker.join().unwrap()
    });
    let renewals = custodian.finish().unwrap();
    assert!(
        renewals >= 5,
        "custodian must renew at <=400ms cadence across >2000ms setup barrier: {renewals}"
    );
    assert_eq!(
        outcome,
        Err(duskweave::Fail::Unresolved("fixture_result_unknown"))
    );
    for _ in 0..2 {
        let (ok, v) = cli("m1-run-recover", "--run", &r.to_string());
        assert!(ok, "{v}");
        assert_eq!(v["run_id"], json!(r));
        assert_eq!(v["effect_count"], 1);
        assert_eq!(v["decision_count"], 1);
        assert_eq!(v["terrain_history_count"], 1);
        assert_eq!(v["result"], json!(record.result));
        assert_eq!(v["attempt"], json!(record.attempt));
    }
    assert!(store.reserve_effect(r, 0).is_err());
}

#[test]
fn withdrawal_after_first_decision_blocks_effect_and_preserves_historical_decision() {
    use duskweave::{
        mission::{OperationId, OperatorRef},
        withdrawal::{WithdrawalReason, WithdrawalRequest, WithdrawalStore},
    };
    let _g = native::db();
    refresh();
    let q = registered(0xa723);
    let record = start_only(&q);
    complete_first(&record);
    let scope = &record.request.context.mission;
    let request = WithdrawalRequest {
        engagement_id: scope.engagement_id,
        campaign_id: scope.campaign_id,
        operator_ref: OperatorRef(uuid::Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    };
    let mut allocator =
        duskweave::postgres_mission::PgAllocator::new(native::client("DW_TEST_DATABASE_URL"));
    let mut mission =
        duskweave::postgres_mission::PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    mission
        .withdraw(&request, OperationId(consent_id()), false, &mut allocator)
        .unwrap()
        .unwrap();
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    assert!(store.start_phase(id(&q), RunPhase::Acquisition).is_err());
    assert!(store.reserve_effect(id(&q), 0).is_err());
    let (ok, v) = cli("m1-run-recover", "--run", &id(&q).to_string());
    assert!(ok, "{v}");
    assert_eq!(v["status"], "stopped");
    assert_eq!(v["effect_count"], 0);
    assert_eq!(v["decision_history_count"], 1);
}

#[test]
fn lost_supervisor_lease_cannot_be_renewed_or_resumed_by_recovery() {
    let _g = native::db();
    refresh();
    let q = registered(0xa724);
    let record = start_only(&q);
    let r = id(&q);
    // Administrative fixture fault only: expire the persisted lease, without waiting on wall time.
    native::admin()
        .execute(
            "UPDATE execution.autonomous_control SET lease_until_ms=0 WHERE run_id=$1",
            &[&r],
        )
        .unwrap();
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    assert!(store.renew(r).is_err());
    assert!(store.start_phase(r, RunPhase::DecisionOne).is_err());
    let (ok, v) = cli("m1-run-recover", "--run", &r.to_string());
    assert!(ok, "{v}");
    assert_eq!(v["status"], "unknown_supervisor_lost");
    assert_eq!(v["effect_count"], 0);
    assert_eq!(v["decision_one"], json!(record.decision_one));
}

#[test]
fn stop_before_release_blocks_children_and_fresh_recovery_has_zero_effects() {
    let _g = native::db();
    refresh();
    let q = registered(0xa720);
    let record = start_only(&q);
    let r = id(&q);
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    store.start_phase(r, RunPhase::DecisionOne).unwrap();
    let (ok, receipt) = cli("m1-run-stop", "--run", &r.to_string());
    assert!(ok, "{receipt}");
    assert!(
        store
            .claim_phase(r, RunPhase::DecisionOne, &release())
            .is_err()
    );
    assert!(store.reserve_effect(r, 0).is_err());
    for _ in 0..2 {
        let (ok, v) = cli("m1-run-recover", "--run", &r.to_string());
        assert!(ok, "{v}");
        assert_eq!(v["effect_count"], 0);
        assert_eq!(v["decision_count"], 0);
        assert_eq!(v["attempt"], json!(record.attempt));
    }
    assert_eq!(store.summary(r).unwrap()["status"], "stopped");
}
#[test]
fn forged_source_and_missing_issued_result_are_rejected_by_restricted_terrain_owner() {
    let _g = native::db();
    refresh();
    let q = registered(0xa721);
    let record = start_only(&q);
    let mut t = duskweave::postgres_m1_terrain::PgTerrainStore::new(native::client(
        "DW_TERRAIN_DATABASE_URL",
    ));
    let mut m =
        duskweave::postgres_mission::PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let reference = duskweave::m1_terrain_issued::IssuedDnsReference::fixture(
        record.request.context.mission.engagement_id,
        record.request.context.mission.campaign_id,
        record.result,
    )
    .unwrap();
    assert!(t.record_issued(&mut m, &reference).is_err());
    let raw = terrain::terrain_support::dns_request_json(
        record.request.context.mission.engagement_id,
        record.request.context.mission.campaign_id,
        record.request.deadline_at_ms / 1000 - 149,
    );
    let mut forged = raw;
    forged["version"] = json!("m1_terrain_dns_issued_observation_v1");
    forged["source_kind"] = json!("fixture_dns_acquisition");
    assert!(
        duskweave::m1_terrain_input::parse_terrain_dns_request(
            &serde_json::to_vec(&forged).unwrap()
        )
        .is_err()
    );
    let req: duskweave::m1_terrain_input::TerrainDnsRequest =
        serde_json::from_value(forged).unwrap();
    assert!(
        duskweave::m1_terrain::verify_mission_permission(&mut m, &req)
            .unwrap()
            .is_ok()
    );
    assert!(
        t.record_dns(&mut m, &req, duskweave::mission::OperationId(consent_id()))
            .is_err()
    );
    assert_eq!(
        PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
            .summary(id(&q))
            .unwrap()["effect_count"],
        0
    );
}
#[test]
fn both_decision_budgets_are_required_before_any_phase_or_fixture_effect() {
    let _g = native::db();
    refresh();
    for (i, (field, value)) in [
        ("calls", 3),
        ("request_bytes", 65535),
        ("response_bytes", 262143),
        ("output_token_capacity", 16383),
        ("wall_time_ms", 89999),
    ]
    .into_iter()
    .enumerate()
    {
        let q = registered_with(0xa730 + i as u128, |r| {
            r["m1_inference_permission"]["campaign_limits"][field] = json!(value)
        });
        let path = path(&q);
        let (ok, v) = cli("m1-run", "--input", path.to_str().unwrap());
        assert!(!ok, "{field} {v}");
        let r = id(&q);
        assert!(
            PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
                .inspect(r)
                .is_err()
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn eight_seed_broker_commit_preserves_the_bound_selected_input_identity() {
    let _g = native::db();
    refresh();
    let q = registered_with(0xa726, |raw| {
        let rule = raw["m1_permission"]["contact_rules"][0].clone();
        let assets: Vec<_> = (0..8)
            .map(|n| uuid::Uuid::from_u128(if n == 0 { 0x21 } else { 0x100 + n }))
            .collect();
        raw["included_assets"] = json!(assets);
        raw["m1_permission"]["contact_rules"] = json!(
            assets
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let mut rule = rule.clone();
                    rule["asset_ref"] = json!(id);
                    rule["priority"] = json!(i + 1);
                    if i > 0 {
                        rule["rule"] = json!({"exact":format!("seed{i}.example.invalid")});
                    }
                    rule
                })
                .collect::<Vec<_>>()
        );
    });
    let rel = release();
    let mut store = broker_store();
    let custodian_store = broker_store();
    let mut b = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let record = start_with(&mut store, &q);
    let custodian = LeaseCustodian::start(&record, custodian_store);
    complete_first(&record);
    let r = id(&q);
    store.start_phase(r, RunPhase::Acquisition).unwrap();
    store.claim_phase(r, RunPhase::Acquisition, &rel).unwrap();
    let request = store.reserve_effect(r, 0).unwrap();
    let result = duskweave::m1_fixture_dns::acquire(request).unwrap();
    let raw = serde_json::to_value(&result).unwrap();
    let mut tx = b.transaction().unwrap();
    let check = tx.execute("SELECT execution.commit_fixture_dns($1,$2)", &[&r, &raw]);
    assert!(
        check.is_ok(),
        "commit result SQL: {}; synthetic request {raw}",
        check
            .err()
            .and_then(|e| e.as_db_error().map(|e| e.message().to_owned()))
            .unwrap_or_default()
    );
    tx.rollback().unwrap();
    store.commit_result(r, &result).unwrap();
    assert!(custodian.finish().unwrap() >= 1);
}

fn assert_release_race(kind: &str, serializable: bool, slot: u128) {
    let (rec, _store, custodian) = ready_acquisition(slot);
    let r = rec.request.run_id;
    let mut a = native::admin();
    let mut holder = timed_conn("DW_TEST_DATABASE_URL");
    let key = scope_key(&rec);
    let writer_var = if kind == "withdrawal" {
        "DW_TEST_DATABASE_URL"
    } else {
        "DW_INFERENCE_BROKER_DATABASE_URL"
    };
    let mut writer = timed_conn(writer_var);
    let writer_pid: i32 = writer
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let (mut worker, worker_pid) = timed_broker(serializable);
    assert_eq!(fence_state(&mut a, &rec)["status"], "running");
    custodian.finish().unwrap();
    lock_scope(&mut holder, &key);
    let (writer_ok, released) = std::thread::scope(|s| {
        let reference = &rec;
        let w = s.spawn(move || match kind {
            "stop" => PgRunStore::new(writer).stop(r),
            "unknown" => PgRunStore::new(writer).unknown(r, RunPhase::Acquisition),
            _ => withdraw_scope(reference, writer),
        });
        wait_advisory_blocked(&mut a, writer_pid);
        let candidate = s.spawn(move || worker.reserve_effect(r, 0));
        wait_advisory_blocked(&mut a, worker_pid);
        unlock_scope(&mut holder, &key);
        (w.join().unwrap().is_ok(), candidate.join().unwrap())
    });
    assert!(writer_ok, "{kind} writer must commit");
    assert!(
        released.is_err(),
        "{kind} before release must reject: {serializable}"
    );
    let after = fence_state(&mut a, &rec);
    let expected_status = if kind == "unknown" {
        "unknown"
    } else {
        "stopped"
    };
    assert_eq!(after["status"], expected_status);
    assert_eq!(after["attempts"], 0);
    assert_eq!(after["results"], 0);
    assert_eq!(after["session_phase"], "idle");
    assert_eq!(after["session_attempt_match"], json!(null));
}

fn assert_commit_race(kind: &str, serializable: bool, slot: u128) {
    let (rec, mut store, custodian) = ready_acquisition(slot);
    let r = rec.request.run_id;
    let issued = store.reserve_effect(r, 0).unwrap();
    let result = duskweave::m1_fixture_dns::acquire(issued).unwrap();
    let mut a = native::admin();
    let mut holder = timed_conn("DW_TEST_DATABASE_URL");
    let key = scope_key(&rec);
    let writer_var = if kind == "withdrawal" {
        "DW_TEST_DATABASE_URL"
    } else {
        "DW_INFERENCE_BROKER_DATABASE_URL"
    };
    let mut writer = timed_conn(writer_var);
    let writer_pid: i32 = writer
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let (mut worker, worker_pid) = timed_broker(serializable);
    custodian.finish().unwrap();
    lock_scope(&mut holder, &key);
    let (writer_ok, committed) = std::thread::scope(|s| {
        let reference = &rec;
        let w = s.spawn(move || match kind {
            "stop" => PgRunStore::new(writer).stop(r),
            "unknown" => PgRunStore::new(writer).unknown(r, RunPhase::Acquisition),
            _ => withdraw_scope(reference, writer),
        });
        wait_advisory_blocked(&mut a, writer_pid);
        let commit = s.spawn(move || worker.commit_result(r, &result));
        wait_advisory_blocked(&mut a, worker_pid);
        unlock_scope(&mut holder, &key);
        (w.join().unwrap().is_ok(), commit.join().unwrap())
    });
    assert!(writer_ok, "{kind} writer must commit");
    assert!(
        committed.is_err(),
        "{kind} before result commit must reject: {serializable}"
    );
    let after = fence_state(&mut a, &rec);
    let expected_status = if kind == "unknown" {
        "unknown"
    } else {
        "stopped"
    };
    assert_eq!(after["status"], expected_status);
    assert_eq!(after["attempts"], 1);
    assert_eq!(after["results"], 0);
    assert_eq!(after["session_phase"], "idle");
    assert_eq!(after["session_attempt_match"], json!(null));
    let (ok, recovered) = cli("m1-run-recover", "--run", &r.to_string());
    assert!(ok, "{recovered}");
    assert_eq!(recovered["effect_count"], 0);
}

fn assert_lease_wait_expiry(serializable: bool, slot: u128) {
    use std::time::{Duration, Instant};
    let (rec, _store, custodian) = ready_acquisition(slot);
    let r = rec.request.run_id;
    let mut a = native::admin();
    let mut holder = timed_conn("DW_TEST_DATABASE_URL");
    let key = scope_key(&rec);
    let (mut worker, pid) = timed_broker(serializable);
    custodian.finish().unwrap();
    lock_scope(&mut holder, &key);
    loop {
        let remaining: i64 = a
            .query_one(
                "SELECT lease_until_ms-floor(extract(epoch FROM clock_timestamp())*1000)::bigint \
                 FROM execution.autonomous_control WHERE run_id=$1",
                &[&r],
            )
            .unwrap()
            .get(0);
        assert!(remaining > 0, "missed active lease window");
        if remaining <= 350 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fence_state(&mut a, &rec)["lease_current"], true);
    let released = std::thread::scope(|s| {
        let w = s.spawn(move || worker.reserve_effect(r, 0));
        wait_advisory_blocked(&mut a, pid);
        let limit = Instant::now() + Duration::from_millis(700);
        while fence_state(&mut a, &rec)["lease_current"] == true {
            assert!(
                Instant::now() < limit,
                "lease did not expire inside wait window"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        unlock_scope(&mut holder, &key);
        w.join().unwrap()
    });
    assert!(
        released.is_err(),
        "expired lease during lock wait must reject"
    );
    let after = fence_state(&mut a, &rec);
    assert_eq!(after["attempts"], 0);
    assert_eq!(after["results"], 0);
    assert_eq!(after["session_phase"], "idle");
}

#[test]
fn fenced_withdrawal_and_stop_races_reject_release_and_late_result_commit() {
    let _g = native::db();
    refresh();
    let (pos_rec, mut pos_store, pos_custodian) = ready_acquisition(0xa740);
    let pos_r = pos_rec.request.run_id;
    let issued = pos_store.reserve_effect(pos_r, 0).unwrap();
    let result = duskweave::m1_fixture_dns::acquire(issued).unwrap();
    pos_store.commit_result(pos_r, &result).unwrap();
    pos_custodian.finish().unwrap();
    pos_store.stop(pos_r).unwrap();
    let pos_after = fence_state(&mut native::admin(), &pos_rec);
    assert_eq!(pos_after["status"], "stopped");
    assert_eq!(pos_after["attempts"], 1);
    assert_eq!(pos_after["results"], 1);
    assert_eq!(pos_after["session_phase"], "idle");
    let (ok, recovered) = cli("m1-run-recover", "--run", &pos_r.to_string());
    assert!(ok, "{recovered}");
    assert_eq!(recovered["result"], json!(pos_rec.result));
    assert_eq!(recovered["effect_count"], 1);
    for (i, (kind, serializable)) in [
        ("withdrawal", false),
        ("stop", false),
        ("unknown", false),
        ("withdrawal", true),
        ("stop", true),
        ("unknown", true),
    ]
    .into_iter()
    .enumerate()
    {
        assert_release_race(kind, serializable, 0xa741 + i as u128);
    }
    for (i, (kind, serializable)) in [
        ("withdrawal", false),
        ("stop", false),
        ("unknown", false),
        ("withdrawal", true),
        ("stop", true),
        ("unknown", true),
    ]
    .into_iter()
    .enumerate()
    {
        assert_commit_race(kind, serializable, 0xa750 + i as u128);
    }
    assert_lease_wait_expiry(false, 0xa760);
    assert_lease_wait_expiry(true, 0xa761);
}

fn assert_terminal_fence_race(slot: u128, phase_complete: bool) {
    let q = registered(slot);
    let (mut store, _) = timed_broker(false);
    let (custodian_store, _) = timed_broker(false);
    let rec = start_with(&mut store, &q);
    let custodian = LeaseCustodian::start(&rec, custodian_store);
    if phase_complete {
        publish_first(&rec);
    } else {
        complete_first(&rec);
    }
    let r = rec.request.run_id;
    let mut a = native::admin();
    let mut holder = timed_conn("DW_TEST_DATABASE_URL");
    let key = scope_key(&rec);
    let (mut stopper, stopper_pid) = timed_broker(false);
    let (mut worker, worker_pid) = timed_broker(true);
    let mut positive = timed_conn("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut tx = positive.transaction().unwrap();
    let valid_without_stop = if phase_complete {
        tx.execute(
            "SELECT execution.run_phase_complete($1,'decision_one')",
            &[&r],
        )
    } else {
        tx.execute("SELECT execution.run_terminal($1,'capability_gap')", &[&r])
    };
    assert!(
        valid_without_stop.is_ok(),
        "the competing writer must be valid before STOP: {valid_without_stop:?}"
    );
    tx.rollback().unwrap();
    custodian.finish().unwrap();
    lock_scope(&mut holder, &key);
    let (stop_ok, competing_res) = std::thread::scope(|s| {
        let w = s.spawn(move || stopper.stop(r));
        wait_advisory_blocked(&mut a, stopper_pid);
        let t = s.spawn(move || {
            if phase_complete {
                worker.complete_phase(r, RunPhase::DecisionOne)
            } else {
                worker.terminal(r, "capability_gap")
            }
        });
        wait_advisory_blocked(&mut a, worker_pid);
        unlock_scope(&mut holder, &key);
        (w.join().unwrap().is_ok(), t.join().unwrap())
    });
    assert!(stop_ok);
    assert!(competing_res.is_err());
    assert_eq!(fence_state(&mut a, &rec)["status"], "stopped");
}

#[test]
fn fenced_stop_blocks_competing_terminal_and_phase_completion_writers() {
    let _g = native::db();
    refresh();
    assert_terminal_fence_race(0xa765, false);
    assert_terminal_fence_race(0xa766, true);
}

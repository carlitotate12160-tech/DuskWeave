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
    let q = registered(0xa722);
    let record = start_only(&q);
    complete_first(&record);
    let r = id(&q);
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    store.start_phase(r, RunPhase::Acquisition).unwrap();
    store
        .claim_phase(r, RunPhase::Acquisition, &release())
        .unwrap();
    let request = store.reserve_effect(r, 0).unwrap();
    let result = duskweave::m1_fixture_dns::acquire(request).unwrap();
    let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
    let mut relayed = PgRunStore::new(native::through(
        "DW_INFERENCE_BROKER_DATABASE_URL",
        relay.port(),
    ));
    store.renew(r).unwrap();
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
    assert_eq!(
        outcome,
        Err(duskweave::Fail::Unresolved("fixture_result_unknown"))
    );
    for _ in 0..2 {
        let (ok, v) = cli("m1-run-recover", "--run", &r.to_string());
        assert!(ok, "{v}");
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
    let record = start_only(&q);
    complete_first(&record);
    let r = id(&q);
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    store.start_phase(r, RunPhase::Acquisition).unwrap();
    store
        .claim_phase(r, RunPhase::Acquisition, &release())
        .unwrap();
    let request = store.reserve_effect(r, 0).unwrap();
    let result = duskweave::m1_fixture_dns::acquire(request).unwrap();
    let raw = serde_json::to_value(&result).unwrap();
    let mut b = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
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
}

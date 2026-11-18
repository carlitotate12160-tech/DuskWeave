use crate::https_support;
use crate::terrain::terrain_support::native;
use duskweave::Fail;
use duskweave::m1_https::{
    HttpsHeadResult, HttpsOutcomeKind, HttpsSourceKind, RedirectDisposition, VERSION_HEAD_RESULT,
};
use duskweave::m1_https_terrain::TerrainIds;
use duskweave::mission::OperationId;
use duskweave::postgres_m1_run::PgRunStore;
use serde_json::{Value, json};
use uuid::Uuid;

#[ignore]
#[test]
fn result_ingestion_and_terrain_scope_reject_mutations_conflicts_and_extra_fields() {
    let _guard = native::qualified_existing_db();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut terrain = native::client("DW_TERRAIN_DATABASE_URL");
    let mut admin = native::admin();

    let q = https_support::registered_with(0xd101, |_| {});
    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    let m = &q["context"]["mission"];
    let eng = Uuid::parse_str(m["engagement_id"].as_str().unwrap()).unwrap();
    let cmp = Uuid::parse_str(m["campaign_id"].as_str().unwrap()).unwrap();
    let (_server, _) = https_support::start_server(https_support::principal_witness_proposal);
    let (ok, receipt) = https_support::run_cli(&q);
    assert!(ok, "{receipt}");

    let existing_hr: Value = admin
        .query_one(
            "SELECT result_payload FROM execution.fixture_https_results WHERE run_id=$1 AND step_index=2",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    // Exact duplicate HEAD commit succeeds idempotently.
    broker
        .execute(
            "SELECT execution.commit_https_loop_head($1,2,$2)",
            &[&run_id, &existing_hr],
        )
        .unwrap();

    // Extra raw field or mutated immutable tuple field is rejected without overwriting stored result.
    let mut with_extra = existing_hr.clone();
    with_extra["raw_body"] = json!("secret");
    assert!(
        broker
            .execute(
                "SELECT execution.commit_https_loop_head($1,2,$2)",
                &[&run_id, &with_extra]
            )
            .is_err()
    );

    for (field, bad_val) in [
        ("hostname", json!("evil.example.invalid")),
        ("ipv4", json!("198.51.100.99")),
        ("approved_path", json!("/admin")),
        ("port", json!(8443)),
        ("parent_claim_revision", json!(99)),
        ("asset_ref", json!(Uuid::new_v4())),
    ] {
        let mut mutated = existing_hr.clone();
        mutated[field] = bad_val;
        assert!(
            broker
                .execute(
                    "SELECT execution.commit_https_loop_head($1,2,$2)",
                    &[&run_id, &mutated]
                )
                .is_err(),
            "mutated field {field} must fail"
        );
    }

    // Conflicting outcome / result_id on committed row fails instead of silently succeeding.
    let mut conflicting = existing_hr.clone();
    conflicting["result_id"] = json!(Uuid::new_v4());
    conflicting["outcome"] = json!("head_response");
    conflicting["status_code"] = json!(200);
    assert!(
        broker
            .execute(
                "SELECT execution.commit_https_loop_head($1,2,$2)",
                &[&run_id, &conflicting]
            )
            .is_err()
    );
    let stored_after: Value = admin
        .query_one(
            "SELECT result_payload FROM execution.fixture_https_results WHERE run_id=$1 AND step_index=2",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(stored_after, existing_hr);

    // DNS conflicting duplicate or duplicate IPv4s must fail.
    let existing_dns: Value = admin
        .query_one(
            "SELECT request FROM execution.fixture_dns_results WHERE run_id=$1 AND step_index=1 AND attempt_kind='initial'",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    let mut conflict_dns = existing_dns.clone();
    conflict_dns["answers"] = json!([{"ipv4": "198.51.100.99", "ttl_seconds": 60}]);
    assert!(
        broker
            .execute(
                "SELECT execution.commit_https_loop_dns($1,1,'initial',$2)",
                &[&run_id, &conflict_dns]
            )
            .is_err()
    );
    let mut dup_ip_dns = existing_dns.clone();
    dup_ip_dns["answers"] = json!([
        {"ipv4": "198.51.100.10", "ttl_seconds": 60},
        {"ipv4": "198.51.100.10", "ttl_seconds": 60}
    ]);
    assert!(
        broker
            .execute(
                "SELECT execution.commit_https_loop_dns($1,1,'initial',$2)",
                &[&run_id, &dup_ip_dns]
            )
            .is_err()
    );

    // Terrain accept_https_observation rejects forged hash, forged proof flags, forged IDs, and cross-campaign registration.
    let outbox_row = admin
        .query_one(
            "SELECT operation_id, request_hash, contract, registration_operation_id FROM terrain.https_outbox WHERE engagement_id=$1 AND campaign_id=$2 LIMIT 1",
            &[&eng, &cmp],
        )
        .unwrap();
    let real_contract: Value = outbox_row.get(2);
    let real_hash: String = outbox_row.get(1);
    let reg_op: Uuid = outbox_row.get(3);
    let new_op = Uuid::new_v4();
    let new_ids = TerrainIds::derive(
        duskweave::mission::EngagementId(eng),
        duskweave::mission::CampaignId(cmp),
        OperationId(new_op),
    )
    .unwrap();
    let mut forged_flags = real_contract.clone();
    forged_flags["operation_id"] = json!(new_op);
    forged_flags["event_id"] = json!(new_ids.event_id.0);
    forged_flags["evaluation"]["observation_id"] = json!(new_ids.observation_id);
    forged_flags["evaluation"]["material_id"] = json!(new_ids.material_id);
    forged_flags["evaluation"]["evaluation_id"] = json!(new_ids.evaluation_id);
    forged_flags["evaluation"]["claim_id"] = json!(new_ids.claim_id);
    forged_flags["claim"]["claim_id"] = json!(new_ids.claim_id);
    forged_flags["claim"]["evaluation_id"] = json!(new_ids.evaluation_id);
    forged_flags["claim"]["observation_id"] = json!(new_ids.observation_id);
    forged_flags["claim"]["material_id"] = json!(new_ids.material_id);
    forged_flags["claim"]["whole_host_proven"] = json!(true);
    let mut tx = terrain
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    assert!(
        tx.query_one(
            "SELECT terrain.accept_https_observation($1,$2,$3,$4)",
            &[&existing_hr, &"f".repeat(64), &forged_flags, &reg_op],
        )
        .is_err()
    );
    tx.rollback().unwrap();
    let mut tx2 = terrain
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    assert!(
        tx2.query_one(
            "SELECT terrain.accept_https_observation($1,$2,$3,$4)",
            &[&existing_hr, &real_hash, &forged_flags, &reg_op],
        )
        .is_err()
    );
    tx2.rollback().unwrap();

    let parsed: HttpsHeadResult = serde_json::from_value(existing_hr).unwrap();
    assert_eq!(parsed.engagement_id.0, eng);
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    assert!(
        store
            .read_committed_https_result(Uuid::new_v4(), 1)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .read_committed_https_result(run_id, 2)
            .unwrap()
            .is_some()
    );
}

#[ignore]
#[test]
fn committed_head_interrupted_publication_recovers_cleanly_twice() {
    let _guard = native::qualified_existing_db();
    let mut q = https_support::registered_with(0xd102, |_| {});
    https_support::seed_dns_claim(0xd102, &q, vec![("198.51.100.10", 60)]);
    q["run_id"] = json!(Uuid::new_v4());
    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    let m = &q["context"]["mission"];
    let eng = Uuid::parse_str(m["engagement_id"].as_str().unwrap()).unwrap();
    let cmp = Uuid::parse_str(m["campaign_id"].as_str().unwrap()).unwrap();
    let asset = Uuid::parse_str(q["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut admin = native::admin();
    let cid: Uuid = admin
        .query_one(
            "SELECT claim_id FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2",
            &[&eng, &cmp],
        )
        .unwrap()
        .get(0);
    broker
        .execute("SELECT execution.run_start($1,$2)", &[&q, &"a".repeat(64)])
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_start($1,'decision_one')",
            &[&run_id],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_claim($1,'decision_one',$2)",
            &[&run_id, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .query_one(
            "SELECT execution.protect_next_decision_capacity($1,1)",
            &[&run_id],
        )
        .unwrap();
    admin.execute("INSERT INTO execution.autonomous_phases VALUES ($1,'decision_one','completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint)", &[&run_id]).unwrap();
    admin.execute(
        "UPDATE execution.https_loop_steps SET next_intent_kind='https-head-v1', resolved_asset_ref=$2, \
         resolved_hostname='www.example.invalid', resolved_ipv4='198.51.100.10', resolved_endpoint_ref='endpoint_0', \
         resolved_path='/', parent_claim_id=$3, parent_claim_revision=1, dependency_refresh='none' WHERE run_id=$1 AND step_index=1",
        &[&run_id, &asset, &cid],
    ).unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_start($1,'acquisition_one')",
            &[&run_id],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_claim($1,'acquisition_one',$2)",
            &[&run_id, &"a".repeat(64)],
        )
        .unwrap();
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    let req = store.reserve_https_head(run_id, 1).unwrap();
    let head_res = HttpsHeadResult {
        version: VERSION_HEAD_RESULT.into(),
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        run_id: req.run_id,
        decision_id: req.decision_id,
        attempt_id: req.attempt_id,
        result_id: Uuid::new_v4(),
        asset_ref: req.asset_ref,
        endpoint_ref: req.endpoint_ref,
        hostname: req.hostname,
        ipv4: req.ipv4,
        port: 443,
        approved_path: req.approved_path,
        method: "HEAD".into(),
        parent_claim_id: req.parent_claim_id,
        parent_claim_revision: req.parent_claim_revision,
        perceived_at: admin
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
                &[],
            )
            .unwrap()
            .get(0),
        uncertainty_seconds: 1,
        outcome: HttpsOutcomeKind::HeadResponse,
        status_code: Some(200),
        redirect: RedirectDisposition::None,
        source_kind: HttpsSourceKind::FixtureHttpsAcquisition,
        limitations: vec!["single_endpoint_only".into()],
    };
    store.commit_https_head(run_id, 1, &head_res).unwrap();
    // Simulate crash before Terrain delivery and recovery via m1-run-recover twice: both succeed, deliver Terrain & history once.
    broker
        .execute("SELECT execution.run_stop($1)", &[&run_id])
        .unwrap();
    let (ok_r1, rec1) = https_support::cli(&["m1-run-recover", "--run", &run_id.to_string()]);
    let (ok_r2, rec2) = https_support::cli(&["m1-run-recover", "--run", &run_id.to_string()]);
    assert!(ok_r1 && ok_r2, "recover1={rec1} recover2={rec2}");
    assert_eq!(rec1, rec2);
    assert_eq!(rec1["head_effect_count"], 1);
    assert_eq!(rec1["terrain_history_count"], 1);
    assert_eq!(rec1["decision_count"], 0);
    let _ = Fail::State("");
}

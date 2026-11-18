use crate::terrain::terrain_support::native;
use crate::{https_support, run_support};
use duskweave::m1_autonomous_context::{AutonomousContext, RunContextBinding};
use duskweave::m1_context;
use duskweave::m1_context_input::ContextRequest;
use duskweave::m1_reasoning::inference::Scope;
use duskweave::m1_reasoning::inference_intent;
use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_mission::PgContextReader;
use serde_json::json;
use uuid::Uuid;

#[ignore]
#[test]
fn endpoint_disclosure_rebind_and_cognition_privileges_hold() {
    let _guard = native::qualified_existing_db();
    // Cognition role cannot read private_endpoint_map or broker binding columns.
    let err = native::client("DW_COGNITION_DATABASE_URL")
        .query_one(
            "SELECT private_endpoint_map FROM execution.native_input_bindings LIMIT 1",
            &[],
        )
        .unwrap_err();
    assert_eq!(err.as_db_error().unwrap().code().code(), "42501");

    // Seed DNS and prepare real D1 pack + endpoint map via PgContextReader.
    let q = https_support::registered_with(0xb201, |_| {});
    https_support::seed_dns_claim(
        0xb201,
        &q,
        vec![("198.51.100.10", 60), ("198.51.100.11", 60)],
    );
    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    let m = &q["context"]["mission"];
    let eng = Uuid::parse_str(m["engagement_id"].as_str().unwrap()).unwrap();
    let cmp = Uuid::parse_str(m["campaign_id"].as_str().unwrap()).unwrap();
    let mut broker = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    broker
        .start(&serde_json::from_value(q.clone()).unwrap(), &"a".repeat(64))
        .unwrap();
    let step1 = broker.https_step(run_id, 1).unwrap();
    let ctx_req: ContextRequest = serde_json::from_value(q["context"].clone()).unwrap();
    let auto = AutonomousContext {
        binding: RunContextBinding {
            engagement_id: EngagementId(eng),
            campaign_id: CampaignId(cmp),
            run_profile: https_support::RUN_PROFILE.into(),
            execution_mode: "controlled_fixture".into(),
            inference_profile: Some(https_support::PROFILE.into()),
            release_sha256: "a".repeat(64),
            qualification_ref: Uuid::new_v4(),
        },
        predecessor: None,
    };
    let mut reader = PgContextReader::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    let prep = m1_context::https_loop::prepare(
        &mut reader,
        &ctx_req,
        OperationId(step1.decision_id),
        run_id,
        &auto,
    )
    .unwrap();
    let mut map = prep.endpoint_map.clone();
    let pack_bytes = prep.prepared.utf8().as_bytes();
    broker
        .bind_https_input(
            eng,
            cmp,
            step1.decision_id,
            run_id,
            &"b".repeat(64),
            &map,
            pack_bytes,
        )
        .unwrap();
    // Conflicting rebind fails while persisted map survives in DB across new broker connections.
    map.pack_digest = "c".repeat(64);
    assert!(
        broker
            .bind_https_input(
                eng,
                cmp,
                step1.decision_id,
                run_id,
                &"b".repeat(64),
                &map,
                pack_bytes,
            )
            .is_err()
    );
    let mut fresh_broker = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    let reloaded = fresh_broker
        .read_https_endpoint_map(eng, cmp, step1.decision_id)
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.bindings.len(), 2);

    // Wrong proposal references rejected cleanly end to end; server sees literal IP attempt.
    let q2 = https_support::registered_with(0xb202, |_| {});
    https_support::seed_dns_claim(0xb202, &q2, vec![("198.51.100.10", 60)]);
    let (_server, _reqs) = https_support::start_server(|_pack| {
        https_support::proposal_json(
            "Attempt literal IP selection.",
            json!(["asset_0", "endpoint_0"]),
            "validate",
            json!(["asset_0", "198.51.100.10"]),
            Some("https-head-v1"),
            "none",
        )
    });
    let (ok, receipt) = https_support::run_cli(&q2);
    assert!(!ok);
    assert_eq!(receipt["head_effect_count"], 0);
    assert!(matches!(
        receipt["recorded_decisions"][0]["disposition"].as_str(),
        Some("schema_repair_exhausted" | "reference_ineligible")
    ));
}

#[ignore]
#[test]
fn withdrawal_integrity_hold_and_superseded_claim_block_head() {
    let _guard = native::qualified_existing_db();
    // Integrity hold on seeded DNS claim prevents HEAD from executing.
    let q = https_support::registered_with(0xb203, |_| {});
    https_support::seed_dns_claim(0xb203, &q, vec![("198.51.100.10", 60)]);
    let eng = Uuid::parse_str(q["context"]["mission"]["engagement_id"].as_str().unwrap()).unwrap();
    let cmp = Uuid::parse_str(q["context"]["mission"]["campaign_id"].as_str().unwrap()).unwrap();
    native::admin()
        .execute(
            "INSERT INTO terrain.integrity_conflicts \
             SELECT engagement_id,campaign_id,operation_id,claim_id,perceived_at,true,'input_mismatch' \
             FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2",
            &[&eng, &cmp],
        )
        .unwrap();
    let (_server, reqs) = https_support::start_server(|_pack| {
        https_support::proposal_json(
            "Try HEAD despite integrity hold.",
            json!(["asset_0"]),
            "validate",
            json!(["asset_0", "endpoint_0"]),
            Some("https-head-v1"),
            "none",
        )
    });
    let (_ok, receipt) = https_support::run_cli(&q);
    assert_eq!(receipt["head_effect_count"], 0);
    assert!(reqs.lock().unwrap().is_empty() || receipt["head_effect_count"] == 0);

    // Withdrawal before start blocks run completely.
    let q_w = https_support::registered_with(0xb204, |_| {});
    let rec: duskweave::m1_run::RunRecord = serde_json::from_value(json!({
        "request": q_w,
        "decision_one": Uuid::new_v4(),
        "decision_two": Uuid::new_v4(),
        "attempt": Uuid::new_v4(),
        "result": Uuid::new_v4(),
        "terrain_operation": Uuid::new_v4(),
        "release_sha256": "a".repeat(64),
        "qualification_ref": Uuid::new_v4(),
        "status": "running",
        "deadline_at_ms": q_w["deadline_at_ms"]
    }))
    .unwrap();
    run_support::withdraw_scope(&rec, native::client("DW_TEST_DATABASE_URL")).unwrap();
    let (ok_w, _) = https_support::run_cli(&q_w);
    assert!(!ok_w);
}

#[ignore]
#[test]
fn utf8_byte_measurements_and_oversize_pack_refusal_verified() {
    let _guard = native::qualified_existing_db();
    let q = https_support::registered_with(0xb205, |raw| {
        raw["m1_permission"]["contact_rules"] = json!((0..8).map(|i| {
            let mut r = raw["m1_permission"]["contact_rules"][0].clone();
            let asset = if i == 0 {
                Uuid::from_u128(0x21)
            } else {
                Uuid::from_u128(0x300 + i)
            };
            r["asset_ref"] = json!(asset);
            r["priority"] = json!(i + 1);
            r["rule"] = json!({"exact": if i == 0 { "fiveips.example.invalid".into() } else { format!("s{i}.example.invalid") }});
            r
        }).collect::<Vec<_>>());
        raw["included_assets"] = json!(
            (0..8)
                .map(|i| if i == 0 {
                    Uuid::from_u128(0x21)
                } else {
                    Uuid::from_u128(0x300 + i)
                })
                .collect::<Vec<_>>()
        );
    });
    https_support::seed_dns_claim(
        0xb205,
        &q,
        (10..14)
            .map(|n| {
                (
                    Box::leak(format!("198.51.100.{n}").into_boxed_str()) as &str,
                    60,
                )
            })
            .collect(),
    );
    let q_min = https_support::registered_with(0xb206, |_| {});
    let min_id = {
        let (_s_min, _) = https_support::start_server(|_| {
            https_support::proposal_json(
                "Minimum pack measurement defer.",
                json!(["asset_0"]),
                "defer",
                json!(["asset_0"]),
                None,
                "none",
            )
        });
        let (ok_min, _) = https_support::run_cli(&q_min);
        assert!(ok_min);
        Uuid::parse_str(q_min["run_id"].as_str().unwrap()).unwrap()
    };
    let (_server, _reqs) = https_support::start_server(https_support::principal_witness_proposal);
    let (_ok, receipt) = https_support::run_cli(&q);
    assert_eq!(receipt["status"], "completed");

    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    let mut admin = native::admin();
    for (label, id) in [("min", min_id), ("max", run_id)] {
        let rows = admin.query(
            "SELECT s.step_index, ic.p, ic.configuration \
             FROM execution.inference_calls ic JOIN execution.https_loop_steps s ON s.decision_id=ic.d \
             WHERE s.run_id=$1 ORDER BY s.step_index, length(ic.configuration) LIMIT 8",
            &[&id],
        ).unwrap();
        for r in &rows {
            let step_idx: i32 = r.get(0);
            let p_str: String = r.get(1);
            let cfg_str: String = r.get(2);
            assert!(p_str.len() <= 8192);
            assert!(cfg_str.len() <= 16384);
            let repair_bound = inference_intent::preflight(p_str.as_bytes(), 8192, 16384).unwrap();
            assert!(repair_bound <= 16384);
            println!(
                "MEASUREMENTS profile={} step={} pack_bytes={} intent_bytes={} preflight_8repair_bound={}",
                label,
                step_idx,
                p_str.len(),
                cfg_str.len(),
                repair_bound
            );
        }
    }

    // Oversize pack refuses before provider send.
    let huge = vec![b' '; 8193];
    assert_eq!(
        inference_intent::preflight(&huge, 8192, 16384),
        Err("request_size")
    );

    // G: Semantic template and digest persisted write-once, unreadable to Cognition, reject tampering & legacy hash-only row.
    let mut admin = native::admin();
    let mut cog = native::client("DW_COGNITION_DATABASE_URL");
    let tpl_row = admin
        .query_one(
            "SELECT nb.semantic_template, nb.template_digest, \
                    encode(sha256(convert_to(execution.genai_canonical(nb.semantic_template), 'UTF8')), 'hex') \
             FROM execution.native_input_bindings nb JOIN execution.https_loop_steps s ON s.decision_id=nb.d \
             WHERE s.run_id=$1 AND s.step_index=1",
            &[&run_id],
        )
        .unwrap();
    let tpl_val: serde_json::Value = tpl_row.get(0);
    let tpl_dig: String = tpl_row.get(1);
    let sql_dig: String = tpl_row.get(2);
    assert_eq!(tpl_dig, sql_dig);
    assert_eq!(tpl_val["template_version"], "m1-https-loop-v1");
    assert_eq!(tpl_val["schema_version"], "m1_https_loop_v1");
    assert!(
        cog.query_one(
            "SELECT semantic_template FROM execution.native_input_bindings LIMIT 1",
            &[]
        )
        .is_err(),
        "Cognition must not SELECT semantic_template directly"
    );
    let d1_row = admin
        .query_one(
            "SELECT nb.e, nb.c, nb.d, ic.call, ic.p, ic.configuration, ic.body, ic.binding \
             FROM execution.https_loop_steps s \
             JOIN execution.native_input_bindings nb ON (nb.e, nb.c, nb.d) = (s.e, s.c, s.decision_id) \
             JOIN execution.inference_calls ic ON (ic.e, ic.c, ic.d) = (nb.e, nb.c, nb.d) \
             WHERE s.run_id=$1 AND s.step_index=1 LIMIT 1",
            &[&run_id],
        )
        .unwrap();
    let (de, dc, dd, dc_id): (Uuid, Uuid, Uuid, Uuid) =
        (d1_row.get(0), d1_row.get(1), d1_row.get(2), d1_row.get(3));
    let (dp, dcfg, db, dh): (String, String, String, String) =
        (d1_row.get(4), d1_row.get(5), d1_row.get(6), d1_row.get(7));
    assert!(
        admin
            .query_one(
                "SELECT execution.genai_binding_valid($1,$2,$3,$4,$5,$6,$7,$8)",
                &[&de, &dc, &dd, &dc_id, &dp, &dcfg, &db, &dh]
            )
            .unwrap()
            .get::<_, bool>(0)
    );
    admin.execute(
        "UPDATE execution.native_input_bindings SET template_digest=$4 WHERE e=$1 AND c=$2 AND d=$3",
        &[&de, &dc, &dd, &"f".repeat(64)],
    ).unwrap();
    assert!(
        !admin
            .query_one(
                "SELECT execution.genai_binding_valid($1,$2,$3,$4,$5,$6,$7,$8)",
                &[&de, &dc, &dd, &dc_id, &dp, &dcfg, &db, &dh]
            )
            .unwrap()
            .get::<_, bool>(0)
    );
    // Legacy hash-only native_input_bindings row refuses m1_https_loop_v1 reservation.
    let mut q_leg_row = https_support::registered_with(0xb207, |_| {});
    let r_lr = Uuid::parse_str(q_leg_row["run_id"].as_str().unwrap()).unwrap();
    let lm = &q_leg_row["context"]["mission"];
    let (le, lc, lp): (Uuid, Uuid, Uuid) = (
        Uuid::parse_str(lm["engagement_id"].as_str().unwrap()).unwrap(),
        Uuid::parse_str(lm["campaign_id"].as_str().unwrap()).unwrap(),
        Uuid::parse_str(lm["permission_id"].as_str().unwrap()).unwrap(),
    );
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    broker
        .execute(
            "SELECT execution.run_start($1,$2)",
            &[&q_leg_row, &"a".repeat(64)],
        )
        .unwrap();
    let ld1: Uuid = admin
        .query_one(
            "SELECT decision_one FROM execution.autonomous_runs WHERE run_id=$1",
            &[&r_lr],
        )
        .unwrap()
        .get(0);
    broker
        .execute(
            "SELECT execution.bind_inference_input($1,$2,$3,$4)",
            &[&le, &lc, &ld1, &"b".repeat(64)],
        )
        .unwrap();
    let lreq = json!({
        "version": "m1_inference_reservation_request_v1",
        "engagement_id": le, "campaign_id": lc,
        "operator_ref": lm["operator_ref"], "expected_mission_revision": 1,
        "permission_id": lp, "goal_ref": lm["goal_ref"], "exercise_mode": lm["exercise_mode"]
    });
    assert!(
        broker.query_one(
            "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
            &[&ld1, &lreq, &le, &lc],
        ).is_err(),
        "legacy hash-only input binding row must be rejected for m1_https_loop_v1"
    );
    let _ = (&mut q_leg_row, std::mem::size_of::<Scope>());
}

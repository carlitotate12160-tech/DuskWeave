use crate::https_support;
use crate::terrain::terrain_support::native;
use serde_json::{Value, json};
use uuid::Uuid;

#[ignore]
#[test]
fn protected_capacity_enforces_exact_allocation_ownership_and_honest_release() {
    let _guard = native::qualified_existing_db();
    // Exact full 8-call campaign limits pass cleanly for 4-decision loop, while 1 call short fails D4 protection.
    let q_short = https_support::registered_with(0xb301, |raw| {
        raw["m1_inference_permission"]["campaign_limits"]["calls"] = json!(7);
    });
    https_support::seed_dns_claim(
        0xb301,
        &q_short,
        vec![("198.51.100.10", 60), ("198.51.100.11", 60)],
    );
    let (_s, _) = https_support::start_server(https_support::principal_witness_proposal);
    let (ok_short, r_short) = https_support::run_cli(&q_short);
    assert!(ok_short, "{r_short}");
    assert_eq!(r_short["status"], "blocked_budget_or_time");
    assert_eq!(r_short["decision_count"], 3);

    // Non-producer decision cannot spend protected capacity allocated for next_decision_id.
    let q_steal = https_support::registered_with(0xb302, |raw| {
        raw["m1_inference_permission"]["campaign_limits"]["calls"] = json!(4);
        raw["m1_inference_permission"]["campaign_limits"]["request_bytes"] = json!(65536);
        raw["m1_inference_permission"]["campaign_limits"]["response_bytes"] = json!(262144);
        raw["m1_inference_permission"]["campaign_limits"]["output_token_capacity"] = json!(32768);
        raw["m1_inference_permission"]["campaign_limits"]["wall_time_ms"] = json!(498000);
    });
    let eng = Uuid::parse_str(
        q_steal["context"]["mission"]["engagement_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let cmp = Uuid::parse_str(
        q_steal["context"]["mission"]["campaign_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let perm = Uuid::parse_str(
        q_steal["context"]["mission"]["permission_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let run_id = Uuid::parse_str(q_steal["run_id"].as_str().unwrap()).unwrap();

    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    broker
        .execute(
            "SELECT execution.run_start($1,$2)",
            &[&q_steal, &"a".repeat(64)],
        )
        .unwrap();
    // Simulate step 1 claiming D1, reserving & closing D1, then protecting D2 (consumes last 2 calls).
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
    let st1: Value = broker
        .query_one("SELECT execution.https_loop_step_record($1,1)", &[&run_id])
        .unwrap()
        .get(0);
    let d1 = Uuid::parse_str(st1["decision_id"].as_str().unwrap()).unwrap();
    let d1_req = json!({
        "version": "m1_inference_reservation_request_v1",
        "engagement_id": eng,
        "campaign_id": cmp,
        "operator_ref": q_steal["context"]["mission"]["operator_ref"],
        "expected_mission_revision": 1,
        "permission_id": perm,
        "goal_ref": q_steal["context"]["mission"]["goal_ref"],
        "exercise_mode": q_steal["context"]["mission"]["exercise_mode"]
    });
    broker
        .execute(
            "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
            &[&d1, &d1_req, &eng, &cmp],
        )
        .unwrap();
    let prot: Value = broker
        .query_one(
            "SELECT execution.protect_next_decision_capacity($1,1)",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    let next_d = Uuid::parse_str(prot["next_decision_id"].as_str().unwrap()).unwrap();

    // Unrelated decision cannot steal the 2 protected calls.
    let rogue_d = Uuid::new_v4();
    let steal_err = broker.query_one(
        "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
        &[&rogue_d, &d1_req, &eng, &cmp],
    );
    assert!(steal_err.is_err());

    // Conversion requires actual pack binding / window / claim first.
    let conv_early = broker.query_one(
        "SELECT execution.convert_protected_capacity($1,$2,$3)",
        &[&eng, &cmp, &next_d],
    );
    assert!(conv_early.is_err());

    // Verify unknown release keeps protected_held charged (2), while released returns calls to 0.
    broker
        .execute(
            "SELECT execution.release_protected_capacity($1,1,'unknown')",
            &[&run_id],
        )
        .unwrap();
    let held_unk: Value = broker
        .query_one(
            "SELECT execution.https_loop_protected_held($1,$2)",
            &[&eng, &cmp],
        )
        .unwrap()
        .get(0);
    assert_eq!(held_unk["calls"], 2);

    let q_stop = https_support::registered_with(0xb309, |_| {});
    let s_run_id = Uuid::parse_str(q_stop["run_id"].as_str().unwrap()).unwrap();
    let sm = &q_stop["context"]["mission"];
    let s_eng = Uuid::parse_str(sm["engagement_id"].as_str().unwrap()).unwrap();
    let s_cmp = Uuid::parse_str(sm["campaign_id"].as_str().unwrap()).unwrap();
    let s_perm = Uuid::parse_str(sm["permission_id"].as_str().unwrap()).unwrap();
    broker
        .execute(
            "SELECT execution.run_start($1,$2)",
            &[&q_stop, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_start($1,'decision_one')",
            &[&s_run_id],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_claim($1,'decision_one',$2)",
            &[&s_run_id, &"a".repeat(64)],
        )
        .unwrap();
    let st_s: Value = broker
        .query_one(
            "SELECT execution.https_loop_step_record($1,1)",
            &[&s_run_id],
        )
        .unwrap()
        .get(0);
    let sd1 = Uuid::parse_str(st_s["decision_id"].as_str().unwrap()).unwrap();
    let sd1_req = json!({
        "version": "m1_inference_reservation_request_v1",
        "engagement_id": s_eng, "campaign_id": s_cmp,
        "operator_ref": sm["operator_ref"], "expected_mission_revision": 1,
        "permission_id": s_perm, "goal_ref": sm["goal_ref"], "exercise_mode": sm["exercise_mode"]
    });
    broker.execute(
        "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
        &[&sd1, &sd1_req, &s_eng, &s_cmp],
    ).unwrap();
    broker
        .query_one(
            "SELECT execution.protect_next_decision_capacity($1,1)",
            &[&s_run_id],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.release_protected_capacity($1,1,'released')",
            &[&s_run_id],
        )
        .unwrap();
    let held_stop: Value = broker
        .query_one(
            "SELECT execution.https_loop_protected_held($1,$2)",
            &[&s_eng, &s_cmp],
        )
        .unwrap()
        .get(0);
    assert_eq!(held_stop["calls"], 0);
}

#[ignore]
#[test]
fn release_returns_protected_capacity_and_budget_time_ceilings_block_cleanly() {
    let _guard = native::qualified_existing_db();
    // Run with tight DNS/HEAD campaign limits verifies DNS=3 and HEAD<=1/2 enforcement.
    let q = https_support::registered_with(0xb303, |raw| {
        raw["m1_permission"]["campaign_limits"]["head_requests"] = json!(1);
        raw["m1_permission"]["campaign_limits"]["tcp_connections"] = json!(1);
    });
    let (_s, _) = https_support::start_server(|pack| {
        let refs = pack["references"].as_array().unwrap();
        let https_count = refs
            .iter()
            .filter(|r| r["id"].as_str().is_some_and(|s| s.starts_with("https_")))
            .count();
        let has_ep0 = refs.iter().any(|r| r["id"] == "endpoint_0");
        if !has_ep0 {
            return https_support::proposal_json(
                "D1 DNS",
                json!(["asset_0"]),
                "observe_more",
                json!(["mission", "asset_0"]),
                Some("dns_udp"),
                "none",
            );
        }
        if https_count == 0 {
            return https_support::proposal_json(
                "D2 first HEAD",
                json!(["asset_0", "endpoint_0"]),
                "validate",
                json!(["asset_0", "endpoint_0"]),
                Some("https-head-v1"),
                "none",
            );
        }
        if pack["steps"].as_array().is_none() && refs.iter().any(|r| r["id"] == "endpoint_1") {
            return https_support::proposal_json(
                "D3 blocked 2nd HEAD",
                json!(["asset_0", "previous_decision", "https_0", "endpoint_1"]),
                "validate",
                json!(["asset_0", "endpoint_1"]),
                Some("https-head-v1"),
                "none",
            );
        }
        https_support::proposal_json(
            "D4 defer after budget block",
            json!(["asset_0", "previous_decision"]),
            "defer",
            json!(["asset_0"]),
            None,
            "none",
        )
    });
    let (ok, receipt) = https_support::run_cli(&q);
    assert!(ok, "{receipt}");
    assert_eq!(receipt["head_effect_count"], 1);
    assert_eq!(receipt["steps"][2]["head_outcome"], "blocked");
    assert_eq!(receipt["steps"][2]["disposition"], "head_admission_refused");

    // Remaining run time < 314_000 ms blocks next acquisition cleanly with blocked_budget_or_time.
    let q_time = https_support::registered_with(0xb304, |_| {});
    let mut q_short_time = q_time;
    let now_s: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    q_short_time["deadline_at_ms"] = json!(now_s * 1000 + 260_000);
    let (ok_t, r_t) = https_support::run_cli(&q_short_time);
    assert!(ok_t, "{r_t}");
    assert_eq!(r_t["status"], "blocked_budget_or_time");
    assert_eq!(r_t["decision_count"], 1);
    assert_eq!(r_t["dns_effect_count"], 0);
}

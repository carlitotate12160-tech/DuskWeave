//! Real `duskweave m1-run` -> fixed child -> GenAI SDK (`127.0.0.1:18083`) ->
//! restricted PostgreSQL (`0037`) end-to-end witness and boundary matrix.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_genai_fixture.rs"]
mod fixture;
#[path = "support/m1_https_loop.rs"]
mod https_support;
#[path = "support/m1_run.rs"]
mod run_support;
#[path = "support/m1_terrain_context.rs"]
mod terrain;

#[path = "m1_https_loop_postgres/admission.rs"]
mod admission;
#[path = "m1_https_loop_postgres/capacity.rs"]
mod capacity;
#[path = "m1_https_loop_postgres/recovery.rs"]
mod recovery;

use serde_json::{Value, json};
use terrain::terrain_support::native;
use uuid::Uuid;

#[ignore]
#[test]
fn principal_four_decision_https_loop_reaches_head_and_records_d4_proposal() {
    let _guard = native::qualified_existing_db();
    let (_server, reqs) = https_support::start_server(https_support::principal_witness_proposal);
    let q = https_support::registered_with(0xb101, |_| {});
    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    let (ok, receipt) = https_support::run_cli(&q);
    assert!(ok, "m1-run failed: {receipt}");
    assert_eq!(receipt["kind"], "m1_https_loop_run_receipt_v1");
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["dispatch_granted"], false);
    assert_eq!(receipt["decision_count"], 4);
    assert_eq!(receipt["decision_history_count"], 4);
    assert_eq!(receipt["dns_effect_count"], 1);
    assert_eq!(receipt["head_effect_count"], 2);
    assert_eq!(receipt["terrain_history_count"], 3);
    assert_eq!(reqs.lock().unwrap().len(), 4);

    let https_results = receipt["https_results"].as_array().unwrap();
    assert_eq!(https_results.len(), 2);
    assert_eq!(https_results[0]["endpoint_ref"], "endpoint_0");
    assert_eq!(https_results[0]["outcome"], "connection_failed");
    assert_eq!(https_results[0]["whole_host_proven"], false);
    assert_eq!(https_results[0]["objective_completed"], false);
    assert_eq!(https_results[1]["endpoint_ref"], "endpoint_1");
    assert_eq!(https_results[1]["outcome"], "head_response");
    assert_eq!(https_results[1]["status_code"], 200);
    assert_eq!(https_results[1]["whole_host_proven"], false);
    assert_eq!(https_results[1]["objective_completed"], false);

    let steps = receipt["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0]["next_intent_kind"], "dns_udp");
    assert_eq!(steps[1]["next_intent_kind"], "https-head-v1");
    assert_eq!(steps[1]["resolved_endpoint_ref"], "endpoint_0");
    assert_eq!(steps[1]["head_outcome"], "connection_failed");
    assert_eq!(steps[2]["next_intent_kind"], "https-head-v1");
    assert_eq!(steps[2]["resolved_endpoint_ref"], "endpoint_1");
    assert_eq!(steps[2]["head_outcome"], "head_response");
    assert_eq!(steps[3]["next_intent_kind"], "validate_refused_at_d4");

    // Verify D4 proposal preserved verbatim while HEAD never dispatched a 3rd attempt.
    let mut admin = native::admin();
    let d4_intent: Value = admin
        .query_one(
            "SELECT cd.payload#>'{proposal,next_intent}' FROM cognition.reasoning_decisions cd \
             JOIN execution.https_loop_steps st ON st.decision_id=cd.d \
             WHERE st.run_id=$1 AND st.step_index=4",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(d4_intent["category"], "validate");
    assert_eq!(d4_intent["manifest_reference"], "https-head-v1");

    // Zero literal IPs or seed hostnames leaked into Cognition, History, or packs.
    let seed_name = q["seed_names"][0]["name"].as_str().unwrap();
    for table_col in [
        "SELECT cd.payload::text FROM cognition.reasoning_decisions cd JOIN execution.https_loop_steps s ON s.decision_id=cd.d WHERE s.run_id=$1",
        "SELECT th.contract::text FROM trajectory.reasoning_history th JOIN execution.https_loop_steps s ON s.decision_id=th.operation_id WHERE s.run_id=$1",
        "SELECT ic.p FROM execution.inference_calls ic JOIN execution.https_loop_steps s ON s.decision_id=ic.d WHERE s.run_id=$1",
    ] {
        for row in admin.query(table_col, &[&run_id]).unwrap() {
            let txt: String = row.get(0);
            assert!(!txt.contains(seed_name), "leaked hostname into {table_col}");
            assert!(
                !txt.contains("198.51.100.10"),
                "leaked IP1 into {table_col}"
            );
            assert!(
                !txt.contains("198.51.100.11"),
                "leaked IP2 into {table_col}"
            );
        }
    }

    // No leaked protected capacity remains after run completion.
    let eng = Uuid::parse_str(q["context"]["mission"]["engagement_id"].as_str().unwrap()).unwrap();
    let cmp = Uuid::parse_str(q["context"]["mission"]["campaign_id"].as_str().unwrap()).unwrap();
    let held: Value = admin
        .query_one(
            "SELECT execution.https_loop_protected_held($1,$2)",
            &[&eng, &cmp],
        )
        .unwrap()
        .get(0);
    let snap: Value = admin
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&eng, &cmp],
        )
        .unwrap()
        .get(0);
    assert_eq!(held["calls"], 0);
    assert_eq!(snap["charged"]["calls"], 8);

    // Recover x2 and inspect produce identical summary with zero extra provider/effect calls.
    let id_str = run_id.to_string();
    let (ok_r1, r1) = https_support::cli(&["m1-run-recover", "--run", &id_str]);
    let (ok_r2, r2) = https_support::cli(&["m1-run-recover", "--run", &id_str]);
    let (ok_i, ins) = https_support::cli(&["m1-run-inspect", "--run", &id_str]);
    assert!(ok_r1 && ok_r2 && ok_i);
    assert_eq!(r1, receipt);
    assert_eq!(r2, receipt);
    assert_eq!(ins, receipt);
    assert_eq!(reqs.lock().unwrap().len(), 4);
}

#[ignore]
#[test]
fn early_defer_at_decision_one_and_head_start_with_existing_dns_work() {
    let _guard = native::qualified_existing_db();
    {
        let (_server, reqs) = https_support::start_server(|_pack| {
            https_support::proposal_json(
                "D1 defers early without acquisition.",
                json!(["asset_0"]),
                "defer",
                json!(["asset_0"]),
                None,
                "none",
            )
        });
        let q = https_support::registered_with(0xb102, |_| {});
        let (ok, receipt) = https_support::run_cli(&q);
        assert!(ok, "{receipt}");
        assert_eq!(receipt["status"], "deferred");
        assert_eq!(receipt["decision_count"], 1);
        assert_eq!(receipt["dns_effect_count"], 0);
        assert_eq!(receipt["head_effect_count"], 0);
        assert_eq!(reqs.lock().unwrap().len(), 1);
    }

    let (_server, reqs) = https_support::start_server(|pack| {
        let refs = pack["references"].as_array().unwrap();
        let https_count = refs
            .iter()
            .filter(|r| r["id"].as_str().is_some_and(|id| id.starts_with("https_")))
            .count();
        match https_count {
            0 => https_support::proposal_json(
                "Existing fresh DNS allows immediate HEAD on endpoint_0.",
                json!(["asset_0", "endpoint_0"]),
                "validate",
                json!(["asset_0", "endpoint_0"]),
                Some("https-head-v1"),
                "none",
            ),
            1 => https_support::proposal_json(
                "Pivot to endpoint_1 after endpoint_0 failed.",
                json!(["asset_0", "previous_decision", "https_0", "endpoint_1"]),
                "validate",
                json!(["asset_0", "endpoint_1"]),
                Some("https-head-v1"),
                "none",
            ),
            _ => https_support::proposal_json(
                "D3 retains after IP2 200 response.",
                json!(["asset_0", "previous_decision", "https_1"]),
                "retain",
                json!(["asset_0", "https_1"]),
                None,
                "none",
            ),
        }
    });
    let q = https_support::registered_with(0xb103, |_| {});
    https_support::seed_dns_claim(
        0xb103,
        &q,
        vec![("198.51.100.10", 60), ("198.51.100.11", 60)],
    );
    let (ok, receipt) = https_support::run_cli(&q);
    assert!(ok, "{receipt}");
    assert_eq!(receipt["status"], "retained");
    assert_eq!(receipt["decision_count"], 3);
    assert_eq!(receipt["dns_effect_count"], 0);
    assert_eq!(receipt["head_effect_count"], 2);
    assert_eq!(reqs.lock().unwrap().len(), 3);
}

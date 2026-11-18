use crate::https_support;
use crate::terrain::terrain_support::native;
use serde_json::{Value, json};
use uuid::Uuid;

#[ignore]
#[test]
fn ttl30_and_60s_cap_expiry_trigger_refresh_and_head_while_bad_refreshes_block() {
    let _guard = native::qualified_existing_db();
    for (slot, host, ttl, age, expect_head) in [
        (0xb401u128, "ttl30.example.invalid", 30u32, 31i64, true),
        (0xb402u128, "ttl300.example.invalid", 300u32, 61i64, true),
        (
            0xb403u128,
            "refreshdrop.example.invalid",
            30u32,
            31i64,
            false,
        ),
        (
            0xb404u128,
            "refreshzero.example.invalid",
            30u32,
            31i64,
            false,
        ),
    ] {
        let q = https_support::registered_with(slot, |raw| {
            raw["m1_permission"]["contact_rules"][0]["rule"] = json!({"exact": host});
        });
        https_support::seed_dns_claim_age(slot, &q, vec![("198.51.100.10", ttl)], age);
        let (_s, _) = https_support::start_server(|pack| {
            let refs = pack["references"].as_array().unwrap();
            let https_count = refs
                .iter()
                .filter(|r| r["id"].as_str().is_some_and(|s| s.starts_with("https_")))
                .count();
            if https_count == 0 && pack["predecessor"]["availability"] != "committed" {
                return https_support::proposal_json(
                    "Refresh stale DNS and run HEAD.",
                    json!(["asset_0", "endpoint_0"]),
                    "validate",
                    json!(["asset_0", "endpoint_0"]),
                    Some("https-head-v1"),
                    "dns_once_if_stale",
                );
            }
            https_support::proposal_json(
                "Terminate cleanly after refresh chain outcome.",
                json!(["asset_0", "previous_decision"]),
                "defer",
                json!(["asset_0"]),
                None,
                "none",
            )
        });
        let (ok, receipt) = https_support::run_cli(&q);
        assert!(ok, "host={host} receipt={receipt}");
        assert_eq!(receipt["recorded_decisions"][0]["version"], 2);
        assert_eq!(
            receipt["recorded_decisions"][0]["disposition"],
            "validated_proposal"
        );
        if expect_head {
            assert_eq!(receipt["head_effect_count"], 1, "host={host}");
            assert!(receipt["steps"][0]["refresh_result_id"].is_string());
        } else {
            assert_eq!(receipt["head_effect_count"], 0, "host={host}");
            assert_eq!(receipt["steps"][0]["head_outcome"], "blocked");
        }
    }

    // Unknown refresh failure keeps protection_state = 'unknown' and status = 'unknown',
    // and m1-run-recover preserves unknown cleanly without replay.
    let q_unk = https_support::registered_with(0xb405, |raw| {
        raw["m1_permission"]["contact_rules"][0]["rule"] =
            json!({"exact": "refreshfail.example.invalid"});
    });
    https_support::seed_dns_claim_age(0xb405, &q_unk, vec![("198.51.100.10", 30)], 31);
    let (_s, _) = https_support::start_server(|_| {
        https_support::proposal_json(
            "Refresh triggers unknown effect.",
            json!(["asset_0", "endpoint_0"]),
            "validate",
            json!(["asset_0", "endpoint_0"]),
            Some("https-head-v1"),
            "dns_once_if_stale",
        )
    });
    let (_ok, r_unk) = https_support::run_cli(&q_unk);
    assert_eq!(r_unk["status"], "unknown");
    assert_eq!(r_unk["head_effect_count"], 0);
    assert_eq!(r_unk["steps"][0]["protection_state"], "unknown");
    let (ok_rec_unk, r_rec_unk) =
        https_support::cli(&["m1-run-recover", "--run", r_unk["run_id"].as_str().unwrap()]);
    assert!(ok_rec_unk, "{r_rec_unk}");
    assert_eq!(r_rec_unk["status"], "unknown");
}

#[ignore]
#[test]
fn five_ips_omission_failboth_honesty_and_recover_zero_replay() {
    let _guard = native::qualified_existing_db();
    // Legal 4-IP cap on wire (5-IP projection omission checked in unit test above) + recover zero-replay + failboth honesty.
    let q = https_support::registered_with(0xb406, |raw| {
        raw["m1_permission"]["contact_rules"][0]["rule"] =
            json!({"exact": "fiveips.example.invalid"});
    });
    {
        let (_s, reqs) = https_support::start_server(https_support::principal_witness_proposal);
        let (ok, receipt) = https_support::run_cli(&q);
        assert!(ok, "{receipt}");
        let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
        let d2_pack: Value = native::admin()
            .query_one(
                "SELECT ic.p::jsonb FROM execution.inference_calls ic \
                 JOIN execution.https_loop_steps s ON s.decision_id=ic.d \
                 WHERE s.run_id=$1 AND s.step_index=2 LIMIT 1",
                &[&run_id],
            )
            .unwrap()
            .get(0);
        assert_eq!(d2_pack["omissions"]["endpoints"]["dns_answers_total"], 4);
        assert_eq!(d2_pack["omissions"]["endpoints"]["shown_endpoints"], 4);
        assert_eq!(d2_pack["omissions"]["endpoints"]["omitted_endpoints"], 0);
        assert_eq!(d2_pack["omissions"]["endpoints"]["partial_coverage"], true);
        let req_count = reqs.lock().unwrap().len();
        let (_ok_r1, _) = https_support::cli(&["m1-run-recover", "--run", &run_id.to_string()]);
        let (_ok_r2, _) = https_support::cli(&["m1-run-recover", "--run", &run_id.to_string()]);
        assert_eq!(reqs.lock().unwrap().len(), req_count);
    }

    // Both endpoints fail on failboth.example.invalid -> neither sets whole_host_proven or objective_completed.
    let q_fail = https_support::registered_with(0xb407, |raw| {
        raw["m1_permission"]["contact_rules"][0]["rule"] =
            json!({"exact": "failboth.example.invalid"});
    });
    let (_s2, _) = https_support::start_server(https_support::principal_witness_proposal);
    let (ok_f, r_f) = https_support::run_cli(&q_fail);
    assert!(ok_f, "{r_f}");
    assert_eq!(r_f["head_effect_count"], 2);
    for hr in r_f["https_results"].as_array().unwrap() {
        assert_eq!(hr["outcome"], "connection_failed");
        assert_eq!(hr["whole_host_proven"], false);
        assert_eq!(hr["objective_completed"], false);
    }
}

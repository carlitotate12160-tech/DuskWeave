use crate::https_support;
use crate::terrain::terrain_support::native;
use duskweave::Fail;
use duskweave::postgres_m1_run::PgRunStore;
use serde_json::{Value, json};
use uuid::Uuid;

fn setup_step1_acq1(
    slot: u128,
    protect: bool,
    change: impl FnOnce(&mut Value),
) -> (Value, Uuid, Uuid, Uuid) {
    let q = https_support::registered_with(slot, change);
    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    let m = &q["context"]["mission"];
    let eng = Uuid::parse_str(m["engagement_id"].as_str().unwrap()).unwrap();
    let cmp = Uuid::parse_str(m["campaign_id"].as_str().unwrap()).unwrap();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut admin = native::admin();
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
    if protect {
        broker
            .query_one(
                "SELECT execution.protect_next_decision_capacity($1,1)",
                &[&run_id],
            )
            .unwrap();
    }
    admin
        .execute(
            "INSERT INTO execution.autonomous_phases VALUES ($1,'decision_one','completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint) ON CONFLICT DO NOTHING",
            &[&run_id],
        )
        .unwrap();
    (q, run_id, eng, cmp)
}

fn claim_acq1(broker: &mut postgres::Client, run_id: Uuid) {
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
}

#[ignore]
#[test]
fn direct_sql_enforces_protection_policy_latch_refresh_and_budget_gates() {
    let _guard = native::qualified_existing_db();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut admin = native::admin();

    // 1. Unprotected step cannot reserve DNS and leaves 0 attempts.
    let (q1, r1, _, _) = setup_step1_acq1(0xc101, false, |_| {});
    let a1 = Uuid::parse_str(q1["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    broker
        .execute(
            "SELECT execution.record_https_loop_dns_intent($1,1,$2,'www.example.invalid')",
            &[&r1, &a1],
        )
        .unwrap();
    claim_acq1(&mut broker, r1);
    assert!(
        broker
            .query_one(
                "SELECT execution.reserve_https_loop_dns($1,1,'initial',0)",
                &[&r1]
            )
            .is_err()
    );
    let dns_cnt: i64 = admin
        .query_one(
            "SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1",
            &[&r1],
        )
        .unwrap()
        .get(0);
    assert_eq!(dns_cnt, 0);

    // 2. Discovery-only hostname (present in discovery_rules, absent in contact_rules) refuses via SQL AND classifies as Fail::State("fixture_policy_denied").
    let mut q2 = https_support::registered_with(0xc102, |raw| {
        raw["m1_permission"]["discovery_rules"] = json!([
            {"exact": "example.invalid"},
            {"exact": "only-discovery.example.invalid"}
        ]);
        raw["m1_permission"]["contact_rules"][0]["rule"] =
            json!({"exact": "target.example.invalid"});
    });
    q2["seed_names"][0]["name"] = json!("only-discovery.example.invalid");
    let r2 = Uuid::parse_str(q2["run_id"].as_str().unwrap()).unwrap();
    broker
        .execute("SELECT execution.run_start($1,$2)", &[&q2, &"a".repeat(64)])
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_start($1,'decision_one')",
            &[&r2],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_claim($1,'decision_one',$2)",
            &[&r2, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .query_one(
            "SELECT execution.protect_next_decision_capacity($1,1)",
            &[&r2],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO execution.autonomous_phases VALUES ($1,'decision_one','completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint) ON CONFLICT DO NOTHING",
            &[&r2],
        )
        .unwrap();
    let a2 = Uuid::parse_str(q2["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    broker
        .execute(
            "SELECT execution.record_https_loop_dns_intent($1,1,$2,'only-discovery.example.invalid')",
            &[&r2, &a2],
        )
        .unwrap();
    claim_acq1(&mut broker, r2);
    let err2 = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
        .reserve_https_dns(r2, 1, "initial", 0)
        .unwrap_err();
    assert_eq!(err2, Fail::State("fixture_policy_denied"));

    // 3. Non-P0001 DB failure (read-only SQLSTATE 25006) classifies as Fail::Store("fixture_reserve_unknown").
    let mut ro_client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    ro_client
        .batch_execute("SET default_transaction_read_only = on")
        .unwrap();
    let err_ro = PgRunStore::new(ro_client)
        .reserve_https_dns(r2, 1, "initial", 0)
        .unwrap_err();
    assert_eq!(err_ro, Fail::Store("fixture_reserve_unknown"));

    // 4. Expired m1_permission refuses while m1_inference_permission remains active.
    let (q3, r3, _e3, _c3) = setup_step1_acq1(0xc103, true, |raw| {
        let now_reg: i64 = native::admin()
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
                &[],
            )
            .unwrap()
            .get(0);
        raw["m1_permission"]["ends_at"] = json!(now_reg + 2);
    });
    let a3 = Uuid::parse_str(q3["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    broker
        .execute(
            "SELECT execution.record_https_loop_dns_intent($1,1,$2,'www.example.invalid')",
            &[&r3, &a3],
        )
        .unwrap();
    claim_acq1(&mut broker, r3);
    for _ in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(900));
        broker
            .execute("SELECT execution.run_renew($1)", &[&r3])
            .unwrap();
    }
    let err3 = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
        .reserve_https_dns(r3, 1, "initial", 0)
        .unwrap_err();
    assert_eq!(err3, Fail::State("fixture_policy_denied"));

    // 5. Campaign episodes = 1 allows step 1 DNS and stops cleanly before step 2 episode.
    let q_ep1 = https_support::registered_with(0xc104, |raw| {
        raw["m1_permission"]["campaign_limits"]["episodes"] = json!(1);
    });
    let (_s, _) = https_support::start_server(https_support::principal_witness_proposal);
    let (ok_ep1, r_ep1) = https_support::run_cli(&q_ep1);
    assert!(ok_ep1, "{r_ep1}");
    assert_eq!(r_ep1["dns_effect_count"], 1);
    assert_eq!(r_ep1["head_effect_count"], 0);
}

#[ignore]
#[test]
fn direct_sql_enforces_hostname_latch_ttl_refresh_lineage_and_314s_254s_window() {
    let _guard = native::qualified_existing_db();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut admin = native::admin();

    // 1. Fresh DNS claim refuses direct SQL 'refresh' even if dependency_refresh='dns_once_if_stale'.
    let mut q_fr = https_support::registered_with(0xc105, |_| {});
    https_support::seed_dns_claim_age(0xc105, &q_fr, vec![("198.51.100.10", 60)], 5);
    q_fr["run_id"] = json!(Uuid::new_v4());
    let r_new = Uuid::parse_str(q_fr["run_id"].as_str().unwrap()).unwrap();
    let a_new = Uuid::parse_str(q_fr["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    let m_new = &q_fr["context"]["mission"];
    let e_new = Uuid::parse_str(m_new["engagement_id"].as_str().unwrap()).unwrap();
    let c_new = Uuid::parse_str(m_new["campaign_id"].as_str().unwrap()).unwrap();
    let cid: Uuid = admin
        .query_one(
            "SELECT claim_id FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e_new, &c_new],
        )
        .unwrap()
        .get(0);
    broker
        .execute(
            "SELECT execution.run_start($1,$2)",
            &[&q_fr, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_start($1,'decision_one')",
            &[&r_new],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_claim($1,'decision_one',$2)",
            &[&r_new, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .query_one(
            "SELECT execution.protect_next_decision_capacity($1,1)",
            &[&r_new],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO execution.autonomous_phases VALUES ($1,'decision_one','completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint)",
            &[&r_new],
        )
        .unwrap();
    admin
        .execute(
            "UPDATE execution.https_loop_steps SET next_intent_kind='https-head-v1', resolved_asset_ref=$2, \
             resolved_hostname='www.example.invalid', resolved_ipv4='198.51.100.10', resolved_endpoint_ref='endpoint_0', \
             resolved_path='/', parent_claim_id=$3, parent_claim_revision=1, dependency_refresh='dns_once_if_stale' \
             WHERE run_id=$1 AND step_index=1",
            &[&r_new, &a_new, &cid],
        )
        .unwrap();
    claim_acq1(&mut broker, r_new);
    let fresh_ref_err = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
        .reserve_https_dns(r_new, 1, "refresh", 0)
        .unwrap_err();
    assert_eq!(fresh_ref_err, Fail::State("fixture_premise_ineligible"));

    // 2. Hostname switch refuses with 'fixture_hostname_mismatch', and 2nd initial DNS refuses with 'fixture_dns_exhausted'.
    let a2_uuid = Uuid::from_u128(0xc206);
    let mut q_h2 = https_support::registered_with(0xc106, |raw| {
        raw["included_assets"] = json!([Uuid::from_u128(0x21), a2_uuid]);
        raw["m1_permission"]["contact_rules"] = json!([
            {"asset_ref": Uuid::from_u128(0x21), "rule": {"exact": "target.example.invalid"}, "priority": 1},
            {"asset_ref": a2_uuid, "rule": {"exact": "other.example.invalid"}, "priority": 2}
        ]);
    });
    q_h2["seed_names"] = json!([
        {"asset_ref": Uuid::from_u128(0x21), "name": "target.example.invalid"},
        {"asset_ref": a2_uuid, "name": "other.example.invalid"}
    ]);
    q_h2["context"]["candidates"] = json!([
        {"asset_ref": Uuid::from_u128(0x21), "priority": "high"},
        {"asset_ref": a2_uuid, "priority": "medium"}
    ]);
    let r_sw = Uuid::parse_str(q_h2["run_id"].as_str().unwrap()).unwrap();
    broker
        .execute(
            "SELECT execution.run_start($1,$2)",
            &[&q_h2, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_start($1,'decision_one')",
            &[&r_sw],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_claim($1,'decision_one',$2)",
            &[&r_sw, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .query_one(
            "SELECT execution.protect_next_decision_capacity($1,1)",
            &[&r_sw],
        )
        .unwrap();
    let a1_uuid = Uuid::from_u128(0x21);
    broker
        .execute(
            "SELECT execution.record_https_loop_dns_intent($1,1,$2,'target.example.invalid')",
            &[&r_sw, &a1_uuid],
        )
        .unwrap();
    admin.execute("INSERT INTO execution.autonomous_phases VALUES ($1,'decision_one','completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint)", &[&r_sw]).unwrap();
    claim_acq1(&mut broker, r_sw);
    broker
        .query_one(
            "SELECT execution.reserve_https_loop_dns($1,1,'initial',0)",
            &[&r_sw],
        )
        .unwrap();
    admin.execute(
        "UPDATE execution.https_loop_steps SET next_intent_kind='dns_udp', resolved_asset_ref=$2, resolved_hostname='target.example.invalid', \
         protection_state='protected', next_decision_id=gen_random_uuid(), protected_capacity='{}'::jsonb WHERE run_id=$1 AND step_index=2",
        &[&r_sw, &a1_uuid],
    ).unwrap();
    admin.execute("UPDATE execution.autonomous_control SET phase='acquisition_two', claimed=true WHERE run_id=$1", &[&r_sw]).unwrap();
    assert_eq!(
        PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
            .reserve_https_dns(r_sw, 2, "initial", 0)
            .unwrap_err(),
        Fail::State("fixture_dns_exhausted")
    );
    admin.execute(
        "UPDATE execution.https_loop_steps SET resolved_asset_ref=$2, resolved_hostname='other.example.invalid' WHERE run_id=$1 AND step_index=2",
        &[&r_sw, &a2_uuid],
    ).unwrap();
    assert_eq!(
        PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
            .reserve_https_dns(r_sw, 2, "initial", 1)
            .unwrap_err(),
        Fail::State("fixture_hostname_mismatch")
    );

    // 3. New chain requires 314000ms; once refresh starts chain, < 314000ms (>= 254000ms) succeeds for HEAD,
    //    and missing refresh_result_id Terrain lineage refuses HEAD admission.
    let (q_low, r_low, _, _) = setup_step1_acq1(0xc108, true, |_| {});
    let a_low = Uuid::parse_str(q_low["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    admin.execute("UPDATE execution.autonomous_runs SET deadline_at_ms = floor(extract(epoch FROM clock_timestamp())*1000)::bigint + 310000 WHERE run_id=$1", &[&r_low]).unwrap();
    broker
        .execute(
            "SELECT execution.record_https_loop_dns_intent($1,1,$2,'www.example.invalid')",
            &[&r_low, &a_low],
        )
        .unwrap();
    claim_acq1(&mut broker, r_low);
    assert_eq!(
        PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"))
            .reserve_https_dns(r_low, 1, "initial", 0)
            .unwrap_err(),
        Fail::State("fixture_time_insufficient")
    );

    let mut q_ch = https_support::registered_with(0xc109, |_| {});
    https_support::seed_dns_claim_age(0xc109, &q_ch, vec![("198.51.100.10", 30)], 31);
    q_ch["run_id"] = json!(Uuid::new_v4());
    let r_ch = Uuid::parse_str(q_ch["run_id"].as_str().unwrap()).unwrap();
    let a_ch = Uuid::parse_str(q_ch["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    let m_ch = &q_ch["context"]["mission"];
    let e_ch = Uuid::parse_str(m_ch["engagement_id"].as_str().unwrap()).unwrap();
    let c_ch = Uuid::parse_str(m_ch["campaign_id"].as_str().unwrap()).unwrap();
    let cid_ch: Uuid = admin
        .query_one(
            "SELECT claim_id FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e_ch, &c_ch],
        )
        .unwrap()
        .get(0);
    broker
        .execute(
            "SELECT execution.run_start($1,$2)",
            &[&q_ch, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_start($1,'decision_one')",
            &[&r_ch],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.run_phase_claim($1,'decision_one',$2)",
            &[&r_ch, &"a".repeat(64)],
        )
        .unwrap();
    broker
        .query_one(
            "SELECT execution.protect_next_decision_capacity($1,1)",
            &[&r_ch],
        )
        .unwrap();
    admin.execute("INSERT INTO execution.autonomous_phases VALUES ($1,'decision_one','completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint)", &[&r_ch]).unwrap();
    admin.execute(
        "UPDATE execution.https_loop_steps SET next_intent_kind='https-head-v1', resolved_asset_ref=$2, \
         resolved_hostname='www.example.invalid', resolved_ipv4='198.51.100.10', resolved_endpoint_ref='endpoint_0', \
         resolved_path='/', parent_claim_id=$3, parent_claim_revision=1, dependency_refresh='dns_once_if_stale' WHERE run_id=$1 AND step_index=1",
        &[&r_ch, &a_ch, &cid_ch],
    ).unwrap();
    admin.execute("UPDATE execution.autonomous_runs SET deadline_at_ms = floor(extract(epoch FROM clock_timestamp())*1000)::bigint + 314800 WHERE run_id=$1", &[&r_ch]).unwrap();
    claim_acq1(&mut broker, r_ch);
    let mut store_ch = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    let ref_req = store_ch.reserve_https_dns(r_ch, 1, "refresh", 0).unwrap();
    // Before refresh result is committed into Terrain, reserve_https_head refuses lineage.
    std::thread::sleep(std::time::Duration::from_millis(1200));
    broker
        .execute("SELECT execution.run_renew($1)", &[&r_ch])
        .unwrap();
    // Now < 314000ms (> 254000ms) remains AND without Terrain refresh record reserve_https_head fails fixture_premise_ineligible.
    assert_eq!(
        store_ch.reserve_https_head(r_ch, 1).unwrap_err(),
        Fail::State("fixture_premise_ineligible")
    );
    let mut dns_acq = ref_req;
    dns_acq.answers = vec![duskweave::m1_terrain_input::DnsAnswer {
        ipv4: "198.51.100.10".into(),
        ttl_seconds: 60,
    }];
    store_ch
        .commit_https_dns(r_ch, 1, "refresh", &dns_acq)
        .unwrap();
    let st_ref = store_ch.https_step(r_ch, 1).unwrap();
    let reference = duskweave::m1_terrain_issued::IssuedDnsReference::controlled_fixture(
        duskweave::mission::EngagementId(e_ch),
        duskweave::mission::CampaignId(c_ch),
        st_ref.refresh_result_id.unwrap(),
        https_support::RUN_PROFILE,
    )
    .unwrap();
    let mut tr = duskweave::postgres_m1_terrain::PgTerrainStore::new(native::client(
        "DW_TERRAIN_DATABASE_URL",
    ));
    let mut ms =
        duskweave::postgres_mission::PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    tr.record_issued(&mut ms, &reference).unwrap().unwrap();
    // After refresh observation lands with < 314000ms (> 254000ms) left, preserved episode deadline permits HEAD!
    let head_req = store_ch.reserve_https_head(r_ch, 1).unwrap();
    assert_eq!(head_req.ipv4, "198.51.100.10");
}

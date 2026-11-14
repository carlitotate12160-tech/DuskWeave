//! Production Pro-v2 1-unit capacity bounds, concurrency barrier and incomplete-charge recovery.
use super::super::fixture::*;
use super::super::run_support::*;
use super::super::successor_episode::{cli, command};
use super::accounting::{
    SyntheticRepairProProvider, assert_counts, capacity_snapshot, run_prepared_inference, start_d1,
};
use super::{PRO_ENDPOINT, PRO_MODEL, register_pro_v2, set_pro_v2_limits, sql_matches};
use duskweave::Fail;
use duskweave::m1_inference_profile as profiles;
use duskweave::m1_reasoning::inference::{
    Broker, IncompleteReason, Provider, ProviderReport, Received, ResponseMetadata, ResponseStatus,
    Usage, UsageAvailability,
};
use duskweave::m1_reasoning::{inference_intent, proposal::ProposalBasis, request::BoundCall};
use duskweave::mission::{CampaignId, EngagementId};
use duskweave::postgres_m1_run::PgRunStore;
use serde_json::{Value, json};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
use uuid::Uuid;

struct UncertainIncompleteProvider {
    digest: Option<String>,
    report: Option<ProviderReport>,
}
impl Provider for UncertainIncompleteProvider {
    fn terminal_report(&self, d: &str) -> Option<&ProviderReport> {
        self.report
            .as_ref()
            .filter(|_| self.digest.as_deref() == Some(d))
    }
    fn prepare(&mut self, call: &BoundCall, _: Instant) -> Result<(), &'static str> {
        inference_intent::verify(call).map(|_| ())
    }
    fn call(
        &mut self,
        call: &BoundCall,
        _: Instant,
        _: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        self.digest = Some(call.digest.clone());
        self.report = Some(ProviderReport {
            usage: Usage {
                input_tokens: Some(400),
                output_tokens: Some(8192),
                reasoning_tokens: Some(1500),
                response_bytes: 900,
                wall_time_ms: 120000,
                requested_model: PRO_MODEL,
                reported_model: Some("probe-fixture".into()),
            },
            provider_response: ResponseMetadata {
                status: ResponseStatus::Incomplete,
                incomplete_reason: Some(IncompleteReason::MaxOutputTokens),
                usage_availability: UsageAvailability::Reported,
            },
        });
        Err("provider_stream_incomplete")
    }
}

fn shape_pro_v2_custom(raw: &mut Value, out_cap: i64, wall_ms: i64) {
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v5");
    p["profile"] = json!(profiles::PRO_V2_PROFILE);
    p["recipient"] = json!(PRO_ENDPOINT);
    p["run_profile"] = json!(profiles::RUN_PROFILE);
    p["execution_mode"] = json!("controlled_fixture");
    set_pro_v2_limits(p);
    p["campaign_limits"] = json!({"calls": 6, "request_bytes": 98304, "response_bytes": 393216, "output_token_capacity": out_cap, "wall_time_ms": wall_ms});
}

fn inference_and_effect_identities(e: Uuid, c: Uuid, run: Uuid) -> Value {
    let calls: Value = admin().query_one("SELECT coalesce(jsonb_agg(jsonb_build_array(call, binding, state) ORDER BY call),'[]') FROM execution.inference_calls WHERE e=$1 AND c=$2", &[&e, &c]).unwrap().get(0);
    let res: Value = admin().query_one("SELECT coalesce(jsonb_agg(jsonb_build_array(generation, generation_call_id, repair_call_id, record_revision) ORDER BY generation, record_revision),'[]') FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2", &[&e, &c]).unwrap().get(0);
    let effects: i64 = admin().query_one("SELECT (SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1)+(SELECT count(*) FROM execution.live_effect_sessions WHERE run_id=$1)", &[&run]).unwrap().get(0);
    json!({"calls": calls, "reservations": res, "effects": effects})
}

fn verify_one_unit_shortfalls() {
    let now = now_ms() / 1000;
    for (field, val) in [
        ("output_tokens_per_call", 8191_i64),
        ("decision_wall_time_ms", 248999_i64),
    ] {
        let mut doc = consent::registration(
            EngagementId(Uuid::from_u128(0xab10)),
            CampaignId(Uuid::from_u128(0xab10)),
        );
        doc["starts_at"] = json!(now - 3600);
        doc["ends_at"] = json!(now + 3600);
        doc["m1_permission"] = json!({"policy_version": 1, "ct_base_domain": "example.invalid", "provider_disclosure": "crt_sh", "vantage_ref": Uuid::from_u128(0x31), "resolver_ipv4": "192.0.2.53", "discovery_rules": [{"label_suffix": "example.invalid"}], "contact_rules": [{"asset_ref": Uuid::from_u128(0x21), "rule": {"exact": "api.example.invalid"}, "priority": 1}], "excluded_names": [{"exact": "excluded.example.invalid"}], "approved_path": "/", "starts_at": now - 3500, "ends_at": now + 3500, "campaign_limits": {"episodes": 2, "provider_calls": 3, "dns_questions": 10, "dns_followups": 2, "tcp_connections": 5, "head_requests": 2}, "concurrency": 1});
        shape_pro_v2_custom(&mut doc, 32768, 498000);
        doc["m1_inference_permission"]["starts_at"] = json!(now - 3500);
        doc["m1_inference_permission"]["ends_at"] = json!(now + 3500);
        doc["m1_inference_permission"]["operation_limits"][field] = json!(val);
        assert!(
            duskweave::input::parse_register(&serde_json::to_vec(&doc).unwrap()).is_err(),
            "{field}={val}"
        );
        assert!(
            !sql_matches(&doc["m1_inference_permission"]),
            "{field}={val}"
        );
    }
    for (slot, out_cap, wall_ms) in [
        (0xab11_u128, 32767_i64, 498000_i64),
        (0xab12, 32768, 497999),
    ] {
        let (e, c, _) = register_shaped(slot, |raw| shape_pro_v2_custom(raw, out_cap, wall_ms));
        let run = Uuid::new_v4();
        let pub_val = live_publication(e, c, run);
        assert_eq!(
            PgRunStore::new(broker()).start(
                &successor_request(&pub_val, run, "m1_autonomous_run_request_v3"),
                RELEASE
            ),
            Err(Fail::State("run_start_refused"))
        );
        assert_counts(e, c, 0, 0);
        assert_eq!(capacity_snapshot(e, c)["charged"]["calls"], 0);
    }
    for (slot, out_cap, wall_ms) in [
        (0xab13_u128, 49151_i64, 747000_i64),
        (0xab14, 49152, 746999),
    ] {
        let (e, c, _) = register_shaped(slot, |raw| shape_pro_v2_custom(raw, out_cap, wall_ms));
        let (_pub, run1, mut runs, record1, prep1) = start_d1(e, c);
        let mut provider = SyntheticRepairProProvider::default();
        let rec1 = run_prepared_inference(&record1, &prep1, &mut provider);
        assert_eq!(
            (rec1.proposal_disposition, rec1.publication, provider.calls),
            ("validated_proposal", "committed", 2)
        );
        runs.stop(run1).unwrap();
        let run2 = Uuid::new_v4();
        let pub2 = live_publication(e, c, run2);
        assert_eq!(
            PgRunStore::new(broker()).start(
                &successor_request(&pub2, run2, "m1_autonomous_run_request_v3"),
                RELEASE
            ),
            Err(Fail::State("run_start_refused"))
        );
        assert_counts(e, c, 1, 2);
        assert_eq!(
            (
                capacity_snapshot(e, c)["charged"]["output_token_capacity"].as_i64(),
                capacity_snapshot(e, c)["charged"]["wall_time_ms"].as_i64()
            ),
            (Some(16384), Some(249000))
        );
    }
}

fn verify_barrier_concurrency() {
    let (e, c, _) = register_pro_v2(0xab20, profiles::PRO_V2_PROFILE);
    let (_pub, run, mut runs, record, prep) = start_d1(e, c);
    let dl = Instant::now() + Duration::from_millis(prep.window_ms as u64);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (b, sc, pk) = (
                barrier.clone(),
                prep.scope.clone(),
                prep.pack.utf8().as_bytes().to_vec(),
            );
            std::thread::spawn(move || {
                let rec = PgRunStore::new(broker()).inspect(run).unwrap();
                let mut port = broker_port(&rec, dl);
                b.wait();
                port.reserve(&sc, &pk)
                    .map(|r| (r.generation, r.generation_call, r.repair_call))
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        results[0], results[1],
        "same-op barrier contenders observe idempotent hold"
    );
    assert!(results[0].is_ok());
    assert_counts(e, c, 1, 0);
    let snap = capacity_snapshot(e, c);
    assert_eq!(
        (
            snap["active_holds"].as_i64(),
            snap["charged"]["output_token_capacity"].as_i64(),
            snap["charged"]["wall_time_ms"].as_i64()
        ),
        (Some(1), Some(16384), Some(249000))
    );

    broker()
        .execute(
            "SELECT execution.bind_inference_input($1,$2,$3,$4)",
            &[&e, &c, &record.decision_two, &prep.input_hash],
        )
        .unwrap();
    let mut comp_scope = prep.scope.clone();
    comp_scope.decision = record.decision_two;
    assert!(
        broker_port(&record, dl)
            .reserve(&comp_scope, prep.pack.utf8().as_bytes())
            .is_err(),
        "competing op refused"
    );
    assert_counts(e, c, 1, 0);
    assert_eq!(capacity_snapshot(e, c)["charged"]["calls"], 2);
    runs.stop(run).unwrap();
}

pub(super) fn run_capacity_and_unknown_suite() {
    verify_one_unit_shortfalls();
    verify_barrier_concurrency();

    let (e, c, _) = register_pro_v2(0xab22, profiles::PRO_V2_PROFILE);
    let (_pub, run, mut runs, record, prep) = start_d1(e, c);
    let mut provider = UncertainIncompleteProvider {
        digest: None,
        report: None,
    };
    let receipt = run_prepared_inference(&record, &prep, &mut provider);
    assert_eq!(receipt.consumption, "uncertain_charged");

    let (state, usage): (String, Value) = admin()
        .query_one(
            "SELECT state, usage FROM execution.inference_calls WHERE e=$1 AND c=$2 AND d=$3",
            &[&e, &c, &prep.scope.decision],
        )
        .map(|r| (r.get(0), r.get(1)))
        .unwrap();
    assert_eq!(
        (
            state.as_str(),
            usage["provider_response"]["status"].as_str(),
            usage["provider_response"]["incomplete_reason"].as_str()
        ),
        (
            "usage_unknown",
            Some("incomplete"),
            Some("max_output_tokens")
        )
    );
    let before_snap = capacity_snapshot(e, c);
    assert_eq!(
        (
            before_snap["active_holds"].as_i64(),
            before_snap["charged"]["output_token_capacity"].as_i64(),
            before_snap["charged"]["wall_time_ms"].as_i64()
        ),
        (Some(1), Some(16384), Some(249000))
    );
    runs.stop(run).unwrap();

    let before_ids = inference_and_effect_identities(e, c, run);
    let (ok_i1, r_i1) = cli("m1-run-inspect", run);
    let (_ok_rec, r_rec) = cli("m1-run-recover", run);
    let (ok_i2, r_i2) = cli("m1-run-inspect", run);
    assert!(
        ok_i1
            && ok_i2
            && r_i1["inference"][0]["state"] == "usage_unknown"
            && r_rec["inference"][0]["state"] == "usage_unknown"
            && r_i2["inference"][0]["state"] == "usage_unknown"
    );
    assert_eq!(
        inference_and_effect_identities(e, c, run),
        before_ids,
        "key-free inspect/recover never mutates calls/effects"
    );
    let after_snap = capacity_snapshot(e, c);
    assert_eq!(
        (after_snap["active_holds"].as_i64(), &after_snap["charged"]),
        (
            before_snap["active_holds"].as_i64(),
            &before_snap["charged"]
        ),
        "held capacity retained with zero refill"
    );
    assert!(
        command()
            .env_remove("DEEPSEEK_API_KEY")
            .args(["m1-run-inspect", "--run", &run.to_string()])
            .output()
            .unwrap()
            .status
            .success()
    );
}

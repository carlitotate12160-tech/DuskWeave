use duskweave::m1_reasoning::proposal::Proposal;
use duskweave::m1_run::{NextStep, RunPhase, next_step};
use serde_json::json;

fn proposal(category: &str) -> Proposal {
    serde_json::from_value(json!({"version":"m1_inference_proposal_v1","hypothesis":"bounded hypothesis",
        "considered_alternative":"bounded alternative","references":["asset_0"],"expected_observation":"routing evidence",
        "falsifying_observation":"missing routing","uncertainty":"unknown","rationale":"bounded reason",
        "next_intent":{"category":category,"references":["asset_0"],"manifest_reference":null}})).unwrap()
}
#[test]
fn genuine_non_effect_intents_remain_terminal_and_unknown_effects_are_capability_gaps() {
    for (c, result) in [
        ("retain", "retained"),
        ("defer", "deferred"),
        ("stop", "stopped"),
        ("validate", "capability_gap"),
        ("synthesize_candidate_implementation", "capability_gap"),
    ] {
        assert_eq!(
            next_step(&proposal(c), 1).unwrap(),
            NextStep::Terminal(result)
        );
    }
    assert_eq!(
        next_step(&proposal("observe_more"), 1).unwrap(),
        NextStep::Terminal("capability_gap")
    );
    let mut p = proposal("observe_more");
    p.next_intent.manifest_reference = Some("dns_udp".into());
    assert_eq!(next_step(&p, 1).unwrap(), NextStep::Dns { seed_index: 0 });
    for refs in [
        vec!["asset_1".into()],
        vec!["asset_00".into()],
        vec!["arbitrary endpoint".into()],
    ] {
        p.next_intent.references = refs;
        assert!(next_step(&p, 1).is_err());
    }
    p.next_intent.references = vec!["asset_0".into(), "asset_1".into()];
    assert_eq!(
        next_step(&p, 2).unwrap(),
        NextStep::Terminal("capability_gap")
    );
}
#[test]
fn view_read_is_not_dns_and_mixed_effect_and_view_is_rejected() {
    let mut p = proposal("observe_more");
    p.next_intent.view_request =
        serde_json::from_value(json!({"owner":"cyber_terrain","selectors":["asset_0"]})).unwrap();
    assert_eq!(
        next_step(&p, 1).unwrap(),
        NextStep::Terminal("owner_view_requested")
    );
    p.next_intent.manifest_reference = Some("dns_udp".into());
    assert!(next_step(&p, 1).is_err());
    for p in [
        RunPhase::DecisionOne,
        RunPhase::Acquisition,
        RunPhase::DecisionTwo,
    ] {
        assert_eq!(RunPhase::parse(p.name()).unwrap(), p);
    }
    assert!(RunPhase::parse("arbitrary executable").is_err());
}

#[test]
fn deterministic_fixture_preserves_issued_snapshot_clock_and_rejects_manual_source() {
    let id = "00000000-0000-0000-0000-000000000021";
    let mut raw = json!({"version":"m1_terrain_dns_issued_observation_v1","engagement_id":id,"campaign_id":id,
        "operator_ref":id,"expected_mission_revision":1,"goal_ref":id,"exercise_mode":"blind","correlation_id":id,
        "source_kind":"fixture_dns_acquisition","vantage_ref":id,"resolver_ipv4":"192.0.2.53",
        "subject_name":"www.example.invalid","asset_ref":id,"expected_claim_revision":0,"perceived_at":1700000000,
        "uncertainty_seconds":0,"transport":"udp","qtype":"A","qclass":"IN","rcode":"noerror","truncated":false,
        "completeness":"complete","limitations":["single_vantage_only"],"answers":[]});
    let result =
        duskweave::m1_fixture_dns::acquire(serde_json::from_value(raw.clone()).unwrap()).unwrap();
    assert_eq!(
        result.perceived_at, 1700000000,
        "deterministic snapshot retains the issuing Broker clock"
    );
    assert_eq!(result.answers.len(), 1);
    assert_eq!(result.answers[0].ttl_seconds, 60);
    raw["source_kind"] = json!("operator_reviewed_dns");
    raw["version"] = json!("m1_terrain_dns_observation_v1");
    assert!(duskweave::m1_fixture_dns::acquire(serde_json::from_value(raw).unwrap()).is_err());
}

#[test]
fn issued_reference_accepts_both_fixture_profiles_but_never_live_or_unknown_modes() {
    let id = "00000000-0000-0000-0000-000000000021";
    let mut raw = json!({"version":"m1_terrain_dns_issued_observation_v1",
        "engagement_id":id,"campaign_id":id,"result_ref":id,
        "run_profile":"autonomous-dns-provider-fixture-v1","execution_mode":"controlled_fixture"});
    let reference: duskweave::m1_terrain_issued::IssuedDnsReference =
        serde_json::from_value(raw.clone()).unwrap();
    assert!(reference.validate().is_ok());
    for (profile, mode) in [
        ("autonomous-dns-v1", "live"),
        ("autonomous-dns-provider-fixture-v1", "live"),
        ("unknown", "controlled_fixture"),
    ] {
        raw["run_profile"] = json!(profile);
        raw["execution_mode"] = json!(mode);
        let reference: duskweave::m1_terrain_issued::IssuedDnsReference =
            serde_json::from_value(raw.clone()).unwrap();
        assert!(reference.validate().is_err());
    }
}

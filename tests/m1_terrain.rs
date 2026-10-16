//! Pure domain and strict intake tests for M1 Terrain DNS observations and claims.
use duskweave::Fail;
use duskweave::m1_terrain::{
    CLAIM_KIND_DNS_IPV4, ClaimFreshness, DnsAnswer, EVENT_KIND_TERRAIN_CHANGED, EvaluationOutcome,
    OBLIGATION_TERRAIN_HISTORY, TerrainAcceptedEvent, TerrainClaimStatus, TerrainIds,
    TerrainReceipt, TerrainSourceKind, TrustTier, evaluate_dns_observation,
};
use duskweave::m1_terrain_input::{
    MAX_INPUT_BYTES, TerrainDnsRequest, canonical_ipv4, parse_terrain_dns_request,
    read_terrain_dns_file, validate_dns_hostname,
};
use duskweave::mission::{CampaignId, EngagementId, EventId, OperationId};
use serde_json::{Value, json};
use uuid::Uuid;

fn base_json() -> Value {
    json!({
        "version": "m1_terrain_dns_observation_v1",
        "engagement_id": Uuid::from_u128(0x101),
        "campaign_id": Uuid::from_u128(0x102),
        "operator_ref": Uuid::from_u128(0x11),
        "expected_mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "exercise_mode": "blind",
        "correlation_id": Uuid::from_u128(0x701),
        "source_kind": "operator_reviewed_dns",
        "vantage_ref": Uuid::from_u128(0x31),
        "resolver_ipv4": "192.0.2.53",
        "subject_name": "www.example.invalid",
        "asset_ref": Uuid::from_u128(0x21),
        "expected_claim_revision": 0,
        "perceived_at": 1_700_000_000_i64,
        "uncertainty_seconds": 1,
        "transport": "udp",
        "qtype": "A",
        "qclass": "IN",
        "rcode": "noerror",
        "truncated": false,
        "completeness": "complete",
        "limitations": ["single_vantage_only"],
        "answers": [
            {"ipv4": "198.51.100.10", "ttl_seconds": 120},
            {"ipv4": "198.51.100.11", "ttl_seconds": 45}
        ]
    })
}

fn lineage() -> (OperationId, EventId, i64) {
    (
        OperationId(Uuid::from_u128(0x801)),
        EventId(Uuid::from_u128(0x802)),
        1_700_000_005,
    )
}

#[test]
fn intake_accepts_canonical_dns_request_and_derives_stable_hash() {
    let raw = serde_json::to_vec(&base_json()).unwrap();
    let req = parse_terrain_dns_request(&raw).unwrap();
    assert_eq!(req.source_kind, TerrainSourceKind::OperatorReviewedDns);
    assert_eq!(req.answers.len(), 2);
    let h1 = req.request_hash().unwrap();
    let h2 = parse_terrain_dns_request(&raw)
        .unwrap()
        .request_hash()
        .unwrap();
    assert_eq!(h1.len(), 64);
    assert_eq!(h1, h2);

    let path =
        std::env::temp_dir().join(format!("dw-terrain-unit-{}.json", Uuid::from_u128(0x991)));
    std::fs::write(&path, &raw).unwrap();
    let from_file = read_terrain_dns_file(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(from_file, req);
    assert!(read_terrain_dns_file(std::path::Path::new("nonexistent-terrain.json")).is_err());
}

#[test]
fn intake_rejects_malformed_extra_field_forged_source_and_caller_status_flags() {
    assert_eq!(
        parse_terrain_dns_request(b""),
        Err(Fail::Input("size_limit"))
    );
    assert_eq!(
        parse_terrain_dns_request(b"   \n\t "),
        Err(Fail::Input("size_limit"))
    );
    let oversized = vec![b' '; MAX_INPUT_BYTES + 1];
    assert_eq!(
        parse_terrain_dns_request(&oversized),
        Err(Fail::Input("size_limit"))
    );

    let deep = b"[[[[[[[[[1]]]]]]]]]";
    assert_eq!(
        parse_terrain_dns_request(deep),
        Err(Fail::Input("depth_limit"))
    );
    assert_eq!(
        parse_terrain_dns_request(b"{not_json"),
        Err(Fail::Input("invalid_json"))
    );

    for forbidden_field in [
        "accepted",
        "fact",
        "status",
        "trust_tier",
        "corroborated",
        "raw_packet",
    ] {
        let mut extra = base_json();
        extra[forbidden_field] = json!(true);
        assert_eq!(
            parse_terrain_dns_request(&serde_json::to_vec(&extra).unwrap()),
            Err(Fail::Input("invalid_json")),
            "must reject unknown field {forbidden_field}"
        );
    }

    for forged_source in [
        "duskweave_acquired_dns",
        "provider_qualified",
        "defender_oracle",
        "ct_log",
    ] {
        let mut forged = base_json();
        forged["source_kind"] = json!(forged_source);
        assert_eq!(
            parse_terrain_dns_request(&serde_json::to_vec(&forged).unwrap()),
            Err(Fail::Input("invalid_json")),
            "must reject forged source {forged_source}"
        );
    }
}

#[test]
fn intake_validates_hostnames_ipv4_nil_ids_answers_and_bounds() {
    assert!(canonical_ipv4("192.0.2.53").is_ok());
    assert!(canonical_ipv4("192.0.2.053").is_err());
    assert!(canonical_ipv4("2001:db8::1").is_err());

    assert!(validate_dns_hostname("api.example.invalid").is_ok());
    for bad_host in [
        "localhost",
        "*.example.invalid",
        "www.example.invalid.",
        "WWW.example.invalid",
        "xn--bcher-kva.example.invalid",
        "192.0.2.10",
        "-bad.example.invalid",
    ] {
        assert!(
            validate_dns_hostname(bad_host).is_err(),
            "must reject hostname {bad_host}"
        );
    }

    let mut nil_case = base_json();
    nil_case["vantage_ref"] = json!(Uuid::nil());
    assert_eq!(
        parse_terrain_dns_request(&serde_json::to_vec(&nil_case).unwrap()),
        Err(Fail::Input("nil_reference"))
    );

    let mut bad_bounds = base_json();
    bad_bounds["uncertainty_seconds"] = json!(301);
    assert_eq!(
        parse_terrain_dns_request(&serde_json::to_vec(&bad_bounds).unwrap()),
        Err(Fail::Input("invalid_bounds"))
    );

    let mut five_answers = base_json();
    five_answers["answers"] = json!([
        {"ipv4": "198.51.100.1", "ttl_seconds": 60},
        {"ipv4": "198.51.100.2", "ttl_seconds": 60},
        {"ipv4": "198.51.100.3", "ttl_seconds": 60},
        {"ipv4": "198.51.100.4", "ttl_seconds": 60},
        {"ipv4": "198.51.100.5", "ttl_seconds": 60}
    ]);
    assert_eq!(
        parse_terrain_dns_request(&serde_json::to_vec(&five_answers).unwrap()),
        Err(Fail::Input("answer_limit"))
    );

    let mut dup_answer = base_json();
    dup_answer["answers"] = json!([
        {"ipv4": "198.51.100.1", "ttl_seconds": 60},
        {"ipv4": "198.51.100.1", "ttl_seconds": 30}
    ]);
    assert_eq!(
        parse_terrain_dns_request(&serde_json::to_vec(&dup_answer).unwrap()),
        Err(Fail::Input("invalid_answer"))
    );

    let mut dup_lim = base_json();
    dup_lim["limitations"] = json!(["single_vantage_only", "single_vantage_only"]);
    assert_eq!(
        parse_terrain_dns_request(&serde_json::to_vec(&dup_lim).unwrap()),
        Err(Fail::Input("duplicate_limitation"))
    );
}

#[test]
fn terrain_ids_are_distinct_and_deterministic() {
    let e = EngagementId(Uuid::from_u128(0x101));
    let c = CampaignId(Uuid::from_u128(0x102));
    let op = OperationId(Uuid::from_u128(0x103));
    let ids = TerrainIds::derive(e, c, op).unwrap();
    assert_eq!(ids, TerrainIds::derive(e, c, op).unwrap());
    let mut bad = ids;
    bad.claim_id = bad.observation_id;
    assert_eq!(bad.validate(), Err(Fail::State("terrain_identity_invalid")));
}

#[test]
fn evaluate_complete_dns_produces_tier1_observed_and_enforces_freshness_window() {
    let req = parse_terrain_dns_request(&serde_json::to_vec(&base_json()).unwrap()).unwrap();
    let ids = TerrainIds::derive(
        req.engagement_id,
        req.campaign_id,
        OperationId(Uuid::from_u128(0x501)),
    )
    .unwrap();
    let ev = evaluate_dns_observation(&req, None, ids, lineage()).unwrap();

    assert_eq!(ev.kind, EVENT_KIND_TERRAIN_CHANGED);
    assert_eq!(ev.publication_obligation, OBLIGATION_TERRAIN_HISTORY);
    assert_eq!(ev.evaluation.outcome, EvaluationOutcome::QualifiedDnsClaim);
    assert!(ev.evaluation.usable_routing_premise);
    assert_eq!(ev.evaluation.lowest_ttl_seconds, Some(45));
    // min(perceived_at + 45, perceived_at + 60) = perceived_at + 45
    assert_eq!(ev.evaluation.valid_until, Some(1_700_000_045));
    assert_eq!(ev.claim.claim_kind, CLAIM_KIND_DNS_IPV4);
    assert_eq!(ev.claim.trust_tier, TrustTier::Tier1);
    assert_eq!(ev.claim.claim_status, TerrainClaimStatus::Observed);
    assert!(!ev.claim.corroborated);
    assert!(!ev.claim.conflicted);
    assert_eq!(ev.claim.claim_revision, 1);
    assert_eq!(ev.claim.previous_claim_revision, 0);

    let fresh_view = ev.claim.view_at(1_700_000_044);
    assert_eq!(fresh_view.freshness, ClaimFreshness::Fresh);
    assert!(fresh_view.routing_usable);

    let expired_view = ev.claim.view_at(1_700_000_045);
    assert_eq!(expired_view.freshness, ClaimFreshness::Expired);
    assert!(!expired_view.routing_usable);

    let withdrawn_view = ev.claim.view_with_authority(1_700_000_010, false);
    assert_eq!(withdrawn_view.freshness, ClaimFreshness::Fresh);
    assert!(!withdrawn_view.authority_active);
    assert!(!withdrawn_view.routing_usable);

    let receipt = TerrainReceipt::from_event(&ev, &ev.claim, "completed", 1_700_000_010, true);
    assert_eq!(receipt.result, "accepted");
    assert_eq!(receipt.publication, "committed");
    assert!(!receipt.dispatch_granted);
    assert!(!receipt.acquisition_qualified);
    assert!(!receipt.corroborated);
}

#[test]
fn evaluate_withholds_incomplete_truncated_nxdomain_empty_and_ttl_zero_material() {
    let cases = [
        (
            "truncated",
            json!({"truncated": true}),
            EvaluationOutcome::TruncatedMaterial,
        ),
        (
            "incomplete",
            json!({"completeness": "incomplete"}),
            EvaluationOutcome::IncompleteMaterial,
        ),
        (
            "nxdomain",
            json!({"rcode": "nxdomain", "answers": []}),
            EvaluationOutcome::NegativeOrErrorRcode,
        ),
        (
            "empty",
            json!({"answers": []}),
            EvaluationOutcome::InsufficientEmptyAnswers,
        ),
        (
            "ttl_zero",
            json!({"answers": [{"ipv4": "198.51.100.10", "ttl_seconds": 0}]}),
            EvaluationOutcome::TtlZeroNotUsable,
        ),
    ];

    for (idx, (label, patch, expected_outcome)) in cases.into_iter().enumerate() {
        let mut raw = base_json();
        for (k, v) in patch.as_object().unwrap() {
            raw[k] = v.clone();
        }
        let req = parse_terrain_dns_request(&serde_json::to_vec(&raw).unwrap()).unwrap();
        let ids = TerrainIds::derive(
            req.engagement_id,
            req.campaign_id,
            OperationId(Uuid::from_u128(0x600 + idx as u128)),
        )
        .unwrap();
        let ev = evaluate_dns_observation(&req, None, ids, lineage()).unwrap();
        assert_eq!(ev.evaluation.outcome, expected_outcome, "case {label}");
        assert!(!ev.evaluation.usable_routing_premise, "case {label}");
        assert_eq!(
            ev.claim.claim_status,
            TerrainClaimStatus::Withheld,
            "case {label}"
        );
        let view = ev.claim.view_at(req.perceived_at);
        assert!(
            !view.routing_usable,
            "case {label} must not be usable for routing"
        );
        if label == "ttl_zero" {
            assert_eq!(view.freshness, ClaimFreshness::Expired);
        } else {
            assert_eq!(view.freshness, ClaimFreshness::NotApplicable);
        }
    }
}

fn initial_eval(op_byte: u128) -> (TerrainAcceptedEvent, TerrainDnsRequest) {
    let req = parse_terrain_dns_request(&serde_json::to_vec(&base_json()).unwrap()).unwrap();
    let ids = TerrainIds::derive(
        req.engagement_id,
        req.campaign_id,
        OperationId(Uuid::from_u128(op_byte)),
    )
    .unwrap();
    let ev = evaluate_dns_observation(&req, None, ids, lineage()).unwrap();
    (ev, req)
}

#[test]
fn evaluate_rejects_stale_expected_claim_revision() {
    let (ev1, req1) = initial_eval(0x701);
    let mut stale_raw = base_json();
    stale_raw["perceived_at"] = json!(1_700_000_010_i64);
    stale_raw["expected_claim_revision"] = json!(0);
    let stale_req = parse_terrain_dns_request(&serde_json::to_vec(&stale_raw).unwrap()).unwrap();
    let ids = TerrainIds::derive(
        req1.engagement_id,
        req1.campaign_id,
        OperationId(Uuid::from_u128(0x702)),
    )
    .unwrap();
    assert_eq!(
        evaluate_dns_observation(&stale_req, Some(&ev1.claim), ids, lineage()),
        Err(Fail::Conflict("claim_revision_mismatch"))
    );
}

#[test]
fn evaluate_accepts_rrset_rotation_at_later_perception_time() {
    let (ev1, req1) = initial_eval(0x703);
    let mut rot_raw = base_json();
    rot_raw["perceived_at"] = json!(1_700_000_020_i64);
    rot_raw["expected_claim_revision"] = json!(1);
    rot_raw["answers"] = json!([{"ipv4": "198.51.100.99", "ttl_seconds": 300}]);
    let rot_req = parse_terrain_dns_request(&serde_json::to_vec(&rot_raw).unwrap()).unwrap();
    let ids = TerrainIds::derive(
        req1.engagement_id,
        req1.campaign_id,
        OperationId(Uuid::from_u128(0x704)),
    )
    .unwrap();
    let ev2 = evaluate_dns_observation(&rot_req, Some(&ev1.claim), ids, lineage()).unwrap();
    assert_eq!(ev2.claim.claim_revision, 2);
    assert_eq!(ev2.claim.previous_claim_revision, 1);
    assert_eq!(ev2.claim.claim_status, TerrainClaimStatus::Observed);
    assert!(!ev2.claim.conflicted);
    assert_eq!(
        ev2.claim.answers,
        vec![DnsAnswer {
            ipv4: "198.51.100.99".to_string(),
            ttl_seconds: 300
        }]
    );
    assert_eq!(ev2.claim.valid_until, Some(1_700_000_080));
}

#[test]
fn evaluate_marks_conflicted_on_different_answers_at_same_perception_time() {
    let (ev1, req1) = initial_eval(0x705);
    let mut conf_raw = base_json();
    conf_raw["expected_claim_revision"] = json!(1);
    conf_raw["answers"] = json!([{"ipv4": "198.51.100.200", "ttl_seconds": 60}]);
    let conf_req = parse_terrain_dns_request(&serde_json::to_vec(&conf_raw).unwrap()).unwrap();
    let ids = TerrainIds::derive(
        req1.engagement_id,
        req1.campaign_id,
        OperationId(Uuid::from_u128(0x706)),
    )
    .unwrap();
    let ev2 = evaluate_dns_observation(&conf_req, Some(&ev1.claim), ids, lineage()).unwrap();
    assert_eq!(
        ev2.evaluation.outcome,
        EvaluationOutcome::ApplicabilityConflict
    );
    assert_eq!(ev2.claim.claim_status, TerrainClaimStatus::Conflicted);
    assert!(ev2.claim.conflicted);
    assert!(!ev2.evaluation.usable_routing_premise);
    assert!(!ev2.claim.view_at(1_700_000_025).routing_usable);
}

#[test]
fn evaluate_duplicate_answers_at_same_perception_time_remain_observed() {
    let (ev1, req1) = initial_eval(0x707);
    let mut dup_raw = base_json();
    dup_raw["expected_claim_revision"] = json!(1);
    let dup_req = parse_terrain_dns_request(&serde_json::to_vec(&dup_raw).unwrap()).unwrap();
    let ids = TerrainIds::derive(
        req1.engagement_id,
        req1.campaign_id,
        OperationId(Uuid::from_u128(0x708)),
    )
    .unwrap();
    let ev2 = evaluate_dns_observation(&dup_req, Some(&ev1.claim), ids, lineage()).unwrap();
    assert_eq!(ev2.claim.claim_status, TerrainClaimStatus::Observed);
    assert!(!ev2.claim.conflicted);
}

#[test]
fn evaluate_clean_observation_recovers_from_prior_conflict() {
    let (ev1, req1) = initial_eval(0x709);
    let mut conf_raw = base_json();
    conf_raw["expected_claim_revision"] = json!(1);
    conf_raw["answers"] = json!([{"ipv4": "198.51.100.200", "ttl_seconds": 60}]);
    let conf_req = parse_terrain_dns_request(&serde_json::to_vec(&conf_raw).unwrap()).unwrap();
    let ids_conf = TerrainIds::derive(
        req1.engagement_id,
        req1.campaign_id,
        OperationId(Uuid::from_u128(0x70a)),
    )
    .unwrap();
    let ev_conf =
        evaluate_dns_observation(&conf_req, Some(&ev1.claim), ids_conf, lineage()).unwrap();
    assert!(ev_conf.claim.conflicted);

    let mut rec_raw = base_json();
    rec_raw["perceived_at"] = json!(1_700_000_030_i64);
    rec_raw["expected_claim_revision"] = json!(2);
    rec_raw["answers"] = json!([{"ipv4": "198.51.100.222", "ttl_seconds": 60}]);
    let rec_req = parse_terrain_dns_request(&serde_json::to_vec(&rec_raw).unwrap()).unwrap();
    let ids_rec = TerrainIds::derive(
        req1.engagement_id,
        req1.campaign_id,
        OperationId(Uuid::from_u128(0x70b)),
    )
    .unwrap();
    let ev_rec =
        evaluate_dns_observation(&rec_req, Some(&ev_conf.claim), ids_rec, lineage()).unwrap();
    assert_eq!(
        ev_rec.evaluation.outcome,
        EvaluationOutcome::QualifiedDnsClaim
    );
    assert_eq!(ev_rec.claim.claim_status, TerrainClaimStatus::Observed);
    assert!(!ev_rec.claim.conflicted);
    assert!(ev_rec.claim.view_at(1_700_000_035).routing_usable);
}

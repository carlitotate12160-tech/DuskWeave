use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn sample_binding(n: usize, ip: &str, asset: u128) -> EndpointBinding {
    EndpointBinding {
        local_id: format!("endpoint_{n}"),
        asset_ref: AssetRef(id(asset)),
        hostname: "target.example.invalid".into(),
        ipv4: ip.into(),
        port: 443,
        approved_path: "/health".into(),
        method: "HEAD".into(),
        parent_claim_id: id(0x20),
        parent_claim_revision: 1,
        resolver_ipv4: "192.0.2.53".into(),
        vantage_ref: id(0x30),
        perceived_at: 1_700_000_000,
        lowest_ttl_seconds: Some(300),
        valid_until: 1_700_000_300,
    }
}

fn sample_map() -> DecisionEndpointMap {
    DecisionEndpointMap {
        version: VERSION_ENDPOINT_MAP.into(),
        engagement_id: EngagementId(id(1)),
        campaign_id: CampaignId(id(2)),
        decision_id: OperationId(id(3)),
        request_id: id(4),
        pack_digest: sha256_hex(b"sample-pack"),
        pack_revision: 1,
        assets: vec![
            IssuedAssetBinding {
                local_id: "asset_0".into(),
                asset_ref: AssetRef(id(10)),
            },
            IssuedAssetBinding {
                local_id: "asset_1".into(),
                asset_ref: AssetRef(id(11)),
            },
        ],
        bindings: vec![
            sample_binding(0, "198.51.100.10", 10),
            sample_binding(1, "198.51.100.11", 10),
        ],
    }
}

#[test]
fn resolve_head_requires_single_matching_asset_and_endpoint() {
    let map = sample_map();
    let (ep, asset) = map
        .resolve_head(&["mission".into(), "asset_0".into(), "endpoint_1".into()])
        .unwrap();
    assert_eq!(ep.ipv4, "198.51.100.11");
    assert_eq!(asset.local_id, "asset_0");

    assert!(map.resolve_head(&["asset_0".into()]).is_err());
    assert!(map.resolve_head(&["endpoint_0".into()]).is_err());
    assert!(
        map.resolve_head(&["asset_0".into(), "endpoint_0".into(), "endpoint_1".into()])
            .is_err()
    );
    assert!(
        map.resolve_head(&["asset_1".into(), "endpoint_0".into()])
            .is_err()
    );
    assert!(
        map.resolve_head(&["asset_0".into(), "endpoint_9".into()])
            .is_err()
    );
    assert!(
        map.resolve_head(&[
            "asset_0".into(),
            "endpoint_0".into(),
            "198.51.100.10".into()
        ])
        .is_err()
    );
}

#[test]
fn ttl_cap_limits_freshness_to_sixty_seconds() {
    let ep = sample_binding(0, "198.51.100.10", 10);
    assert_eq!(ep.effective_valid_until(), Some(1_700_000_060));
    assert!(!ep.is_fresh_at(1_699_999_999));
    assert!(ep.is_fresh_at(1_700_000_000));
    assert!(ep.is_fresh_at(1_700_000_059));
    assert!(!ep.is_fresh_at(1_700_000_060));
    assert_eq!(
        ttl_capped_valid_until(1000, Some(0), Some(2000)),
        Some(1000)
    );
}

#[test]
fn semantic_template_varies_only_with_immutable_context_pack() {
    let pack = serde_json::json!({
        "version": VERSION_HTTPS_LOOP,
        "run_profile": VERSION_HTTPS_LOOP,
        "execution_mode": "controlled_fixture",
        "inference_profile": "fixture-genai-v4-pro-low-v2",
        "mission_revision": 1,
        "engagement_id": id(1),
        "campaign_id": id(2),
        "decision_id": id(3),
        "permission_id": id(4),
        "question": "Select next action.",
        "references": [{"id": "asset_0", "owner": "mission"}, {"id": "endpoint_0", "owner": "cyber_terrain"}],
        "manifests": [
            {"reference": "dns_udp", "availability": "qualified"},
            {"reference": "https-head-v1", "availability": "qualified"}
        ]
    });
    let bytes = serde_json::to_vec(&pack).unwrap();
    let (t1, v1, d1) = crate::m1_reasoning::inference_intent::semantic_template(&bytes).unwrap();
    let (t2, v2, d2) = crate::m1_reasoning::inference_intent::semantic_template(&bytes).unwrap();
    assert_eq!(t1, t2);
    assert_eq!(v1, v2);
    assert_eq!(d1, d2);
    assert_eq!(t1.template_version, "m1-https-loop-v1");
    assert_eq!(t1.schema_version, VERSION_HTTPS_LOOP);
}

#[test]
fn head_result_enforces_status_only_for_head_response() {
    let mut r = HttpsHeadResult {
        version: VERSION_HEAD_RESULT.into(),
        engagement_id: EngagementId(id(1)),
        campaign_id: CampaignId(id(2)),
        run_id: id(3),
        decision_id: OperationId(id(4)),
        attempt_id: id(5),
        result_id: id(6),
        asset_ref: AssetRef(id(10)),
        endpoint_ref: "endpoint_0".into(),
        hostname: "target.example.invalid".into(),
        ipv4: "198.51.100.10".into(),
        port: 443,
        approved_path: "/health".into(),
        method: "HEAD".into(),
        parent_claim_id: id(7),
        parent_claim_revision: 1,
        perceived_at: 1_700_000_010,
        uncertainty_seconds: 1,
        outcome: HttpsOutcomeKind::ConnectionFailed,
        status_code: None,
        redirect: RedirectDisposition::None,
        source_kind: HttpsSourceKind::FixtureHttpsAcquisition,
        limitations: vec!["single_endpoint_only".into()],
    };
    r.validate().unwrap();
    r.status_code = Some(200);
    assert!(r.validate().is_err());
    r.outcome = HttpsOutcomeKind::HeadResponse;
    r.validate().unwrap();
}

#[test]
fn https_lab_context_pack_admitted_in_inference_intent_and_aliasing_rejected() {
    use crate::m1_reasoning::inference::{Reservation, Scope};
    use crate::m1_reasoning::inference_intent;

    let (eng, cmp, dec, perm, qref) = (id(1), id(2), id(3), id(4), id(0x99));
    let (gen_call, rep_call) = (id(0x101), id(0x102));
    let scope = Scope {
        engagement: eng,
        campaign: cmp,
        decision: dec,
        permission: perm,
        mission_revision: 1,
    };
    let reservation = Reservation {
        generation: 1,
        generation_call: gen_call,
        repair_call: rep_call,
        shared_deadline: std::time::Instant::now(),
    };
    let live_binding = serde_json::json!({
        "release_sha256": "a".repeat(64),
        "qualification_ref": qref,
        "resolver_ipv4": "192.0.2.53",
    });
    let lab_pack = serde_json::json!({
        "version": VERSION_HTTPS_LOOP,
        "run_profile": "autonomous-https-genai-lab-v1",
        "execution_mode": "live",
        "inference_profile": "fixture-genai-v4-pro-low-v2",
        "qualification_purpose": "HTTPS_LOOP_LAB",
        "live_binding": live_binding,
        "mission_revision": 1,
        "engagement_id": eng,
        "campaign_id": cmp,
        "decision_id": dec,
        "permission_id": perm,
        "question": "Select next action.",
        "references": [{"id": "asset_0", "owner": "mission"}, {"id": "endpoint_0", "owner": "cyber_terrain"}],
        "manifests": [
            {"reference": "dns_udp", "availability": "qualified"},
            {"reference": "https-head-v1", "availability": "qualified"}
        ]
    });
    let pack_bytes = serde_json::to_vec(&lab_pack).unwrap();
    assert!(pack_bytes.len() <= 8192);
    let upper = inference_intent::preflight(&pack_bytes, 8192, 16384).unwrap();
    assert!(upper <= 16384);

    let bound_gen =
        inference_intent::bind(&scope, &reservation, gen_call, &pack_bytes, None).unwrap();
    assert!(bound_gen.configuration.len() <= upper);
    let verified_gen = inference_intent::verify(&bound_gen).unwrap();
    inference_intent::verify_owned(&bound_gen, &scope, &reservation, gen_call).unwrap();
    assert_eq!(verified_gen["qualification_purpose"], "HTTPS_LOOP_LAB");
    assert_eq!(verified_gen["live_binding"], live_binding);
    assert_eq!(verified_gen["call_role"], "generation");

    let codes = ["unexpected_view_request"];
    let bound_rep =
        inference_intent::bind(&scope, &reservation, rep_call, &pack_bytes, Some(&codes)).unwrap();
    assert!(bound_rep.configuration.len() <= upper);
    let verified_rep = inference_intent::verify(&bound_rep).unwrap();
    inference_intent::verify_owned(&bound_rep, &scope, &reservation, rep_call).unwrap();
    assert_eq!(verified_rep["qualification_purpose"], "HTTPS_LOOP_LAB");
    assert_eq!(verified_rep["live_binding"], live_binding);
    assert_eq!(verified_rep["call_role"], "repair");

    // Fixture pack rejects explicit null or injected live properties.
    let mut fixture_pack = lab_pack.clone();
    fixture_pack["run_profile"] = serde_json::json!(VERSION_HTTPS_LOOP);
    fixture_pack["execution_mode"] = serde_json::json!("controlled_fixture");
    fixture_pack.as_object_mut().unwrap().remove("live_binding");
    fixture_pack
        .as_object_mut()
        .unwrap()
        .remove("qualification_purpose");
    let fix_bytes = serde_json::to_vec(&fixture_pack).unwrap();
    assert!(inference_intent::preflight(&fix_bytes, 8192, 16384).is_ok());

    for bad_mut in [
        {
            let mut p = fixture_pack.clone();
            p["live_binding"] = serde_json::Value::Null;
            p
        },
        {
            let mut p = fixture_pack.clone();
            p["qualification_purpose"] = serde_json::Value::Null;
            p
        },
        {
            let mut p = fixture_pack.clone();
            p["live_binding"] = live_binding.clone();
            p
        },
        {
            let mut p = lab_pack.clone();
            p["version"] = serde_json::json!("m1_context_pack_autonomous_v4");
            p
        },
        {
            let mut p = lab_pack.clone();
            p["qualification_purpose"] = serde_json::json!("LAB");
            p
        },
        {
            let mut p = lab_pack.clone();
            p["execution_mode"] = serde_json::json!("controlled_fixture");
            p
        },
    ] {
        let b = serde_json::to_vec(&bad_mut).unwrap();
        assert!(inference_intent::preflight(&b, 8192, 16384).is_err());
    }
}

#[test]
fn https_lab_dns_request_enforces_singleton_seed_hostname() {
    use crate::m1_dns::{DnsRequest, PROFILE, SCHEMA};
    let now = 1_700_000_000_000i64;
    let base = DnsRequest {
        schema: SCHEMA.into(),
        kind: "REQUEST".into(),
        attempt: id(1),
        session_generation: 1,
        sequence: 0,
        engagement_id: id(2),
        campaign_id: id(3),
        decision_id: id(4),
        profile: PROFILE.into(),
        execution_mode: "live".into(),
        run_profile: "autonomous-https-genai-lab-v1".into(),
        release_sha256: "a".repeat(64),
        qualification_ref: id(5),
        name: "head.duskweave.test".into(),
        resolver_ipv4: "192.0.2.53".into(),
        transport: "udp".into(),
        query_id: 7,
        allowed_names: vec!["head.duskweave.test".into()],
        deadline_at_ms: now + 2000,
        max_duration_ms: 2000,
    };
    base.validate(now).unwrap();

    for mut bad in [
        {
            let mut r = base.clone();
            r.name = "other.duskweave.test".into();
            r.allowed_names = vec!["other.duskweave.test".into()];
            r
        },
        {
            let mut r = base.clone();
            r.name = "target.example.invalid".into();
            r.allowed_names = vec!["target.example.invalid".into()];
            r
        },
        {
            let mut r = base.clone();
            r.allowed_names = vec!["head.duskweave.test".into(), "extra.duskweave.test".into()];
            r
        },
        {
            let mut r = base.clone();
            r.allowed_names = vec!["head.duskweave.test".into(), "head.duskweave.test".into()];
            r
        },
        {
            let mut r = base.clone();
            r.resolver_ipv4 = "1.1.1.1".into();
            r
        },
    ] {
        assert!(bad.validate(now).is_err());
        bad.run_profile = "autonomous-dns-genai-lab-v1".into();
    }
}

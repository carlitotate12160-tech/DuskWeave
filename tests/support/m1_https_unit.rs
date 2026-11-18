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
    assert!(ep.is_fresh_at(1_700_000_059));
    assert!(!ep.is_fresh_at(1_700_000_060));
    assert_eq!(
        ttl_capped_valid_until(1000, Some(0), Some(2000)),
        Some(1000)
    );
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

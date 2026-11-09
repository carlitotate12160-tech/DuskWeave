//! Source labels cannot substitute for issued provenance or dependency identity.
use duskweave::m1_terrain_input::{TerrainDnsRequest, parse_terrain_dns_request};
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::m1_terrain_view::{TerrainClaimSummary, TerrainDependency};
use duskweave::mission::{CampaignId, EngagementId};
use serde_json::{Value, json};
use uuid::Uuid;

fn id() -> Uuid {
    Uuid::from_u128(1)
}

#[test]
fn live_issued_material_requires_the_owner_reference_path() {
    let q = json!({"version":"m1_terrain_dns_issued_observation_v2","source_kind":"issued_dns_acquisition",
 "engagement_id":id(),"campaign_id":id(),"operator_ref":id(),"goal_ref":id(),"correlation_id":id(),
 "expected_mission_revision":1,"exercise_mode":"blind","vantage_ref":id(),"resolver_ipv4":"192.0.2.53",
 "subject_name":"seed.duskweave.test","asset_ref":id(),"perceived_at":1000,"transport":"udp",
 "qtype":"A","qclass":"IN","rcode":"noerror","truncated":false,"completeness":"complete",
 "limitations":["single_vantage_only"],"answers":[{"ipv4":"198.51.100.7","ttl_seconds":30}]});
    let admitted: TerrainDnsRequest = serde_json::from_value(q.clone()).unwrap();
    assert!(admitted.validate().is_ok());
    assert!(
        parse_terrain_dns_request(&serde_json::to_vec(&q).unwrap()).is_err(),
        "manual input is not issuance"
    );
    for (version, source) in [
        (
            "m1_terrain_dns_issued_observation_v1",
            "issued_dns_acquisition",
        ),
        (
            "m1_terrain_dns_issued_observation_v2",
            "fixture_dns_acquisition",
        ),
        ("m1_terrain_dns_observation_v1", "issued_dns_acquisition"),
    ] {
        let mut forged = q.clone();
        forged["version"] = json!(version);
        forged["source_kind"] = json!(source);
        assert!(
            serde_json::from_value::<TerrainDnsRequest>(forged)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let reference = IssuedDnsReference::live(
        EngagementId(id()),
        CampaignId(id()),
        id(),
        "autonomous-dns-genai-lab-v1",
    )
    .unwrap();
    let raw = serde_json::to_value(reference).unwrap();
    assert_eq!(raw["execution_mode"], "live");
    for (field, value) in [
        ("execution_mode", "controlled_fixture"),
        ("run_profile", "autonomous-dns-fixture-v2"),
        ("version", "m1_terrain_dns_issued_observation_v1"),
    ] {
        let mut forged = raw.clone();
        forged[field] = json!(value);
        assert!(
            serde_json::from_value::<IssuedDnsReference>(forged)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}

fn claim() -> TerrainClaimSummary {
    serde_json::from_value(
        json!({"claim_id":id(),"claim_revision":1,"previous_claim_revision":0,
 "operation_id":id(),"event_id":id(),"observation_id":id(),"material_id":id(),"evaluation_id":id(),
 "asset_ref":id(),"vantage_ref":id(),"source_kind":"issued_dns_acquisition",
 "claim_kind":"dns_hostname_ipv4_resolution","trust_tier":"tier_1","claim_status":"observed",
 "effective_status":"observed","corroborated":false,"conflicted":false,"integrity_hold":false,
 "hold_frontier":null,"temporal_frontier":1000,"perceived_at":1000,"uncertainty_seconds":0,
 "evaluated_at":1001,"valid_until":1030,"lowest_ttl_seconds":30,"answer_count":1,
 "completeness":"complete","limitations":["single_vantage_only"],"outcome":"qualified_dns_claim",
 "freshness":"fresh","routing_usable":true}),
    )
    .unwrap()
}

#[test]
fn current_dependency_cannot_change_source_or_original_observation_time() {
    let original = claim();
    let dependency = TerrainDependency::from_claim("terrain_0".into(), &original).unwrap();
    assert!(dependency.matches_current(&original));
    for (field, value) in [
        ("source_kind", json!("fixture_dns_acquisition")),
        ("perceived_at", json!(1001)),
    ] {
        let mut changed: Value = serde_json::to_value(&original).unwrap();
        changed[field] = value;
        let changed = serde_json::from_value(changed).unwrap();
        assert!(
            !dependency.matches_current(&changed),
            "dependency changed {field}"
        );
    }
}

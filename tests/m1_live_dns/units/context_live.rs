use super::*;
use crate::m1_autonomous_context::{AutonomousContext, RunContextBinding};
use crate::m1_context::ContextBasis;
use crate::mission::{CampaignId, EngagementId, MissionRegistered};
use serde_json::json;
use uuid::Uuid;

fn sample_basis(live: bool, e: Uuid, c: Uuid, qual: Uuid) -> ContextBasis {
    let now = 1000i64;
    let perm = json!({
        "policy_version": 1,
        "ct_base_domain": "duskweave.test",
        "provider_disclosure": "crt_sh",
        "vantage_ref": "00000000-0000-0000-0000-000000000001",
        "resolver_ipv4": "192.0.2.53",
        "discovery_rules": [{"label_suffix": "duskweave.test"}],
        "contact_rules": [],
        "excluded_names": [],
        "approved_path": "/",
        "starts_at": now,
        "ends_at": now + 3600,
        "campaign_limits": {
            "episodes": 1, "provider_calls": 1, "dns_questions": 1,
            "dns_followups": 1, "tcp_connections": 1, "head_requests": 1
        },
        "concurrency": 1
    });

    let mut raw = json!({
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21)],
        "excluded_assets": [],
        "exercise_mode": "blind",
        "starts_at": now,
        "ends_at": now + 3600,
        "m1_permission": perm,
    });

    if live {
        raw["m1_inference_permission"] = json!({
            "version": "m1_inference_permission_v6",
            "permission_id": Uuid::from_u128(0x31),
            "engagement_id": e,
            "campaign_id": c,
            "operator_ref": Uuid::from_u128(0x11),
            "authority_ref": Uuid::from_u128(0x12),
            "authority_revision": 1,
            "mission_revision": 1,
            "goal_ref": Uuid::from_u128(0x13),
            "exercise_mode": "blind",
            "recipient": "http://127.0.0.1:18083/responses",
            "profile": "fixture-genai-responses-v1",
            "execution_mode": "live",
            "run_profile": "autonomous-dns-genai-lab-v1",
            "field_classes": [
                "mission_question_constraints",
                "scoped_opaque_references",
                "omissions_unknown",
                "capacity"
            ],
            "starts_at": now,
            "ends_at": now + 3600,
            "provider_terms_ref": Uuid::from_u128(0x32),
            "provider_terms_approved": true,
            "campaign_limits": {
                "calls": 10, "request_bytes": 100000, "response_bytes": 500000,
                "output_token_capacity": 20000, "wall_time_ms": 100000
            },
            "operation_limits": {
                "context_bytes": 4096, "candidates": 8, "request_bytes": 16384,
                "configuration_bytes": 16384, "response_bytes": 65536, "calls_per_decision": 2,
                "output_tokens_per_call": 4096, "decision_wall_time_ms": 20000,
                "connect_time_ms": 5000, "proposal_bytes": 8192, "repair_bytes": 2048,
                "text_field_bytes": 1024, "repair_error_codes": 8, "json_depth": 8,
                "collection_entries": 64, "concurrency": 1
            },
            "live_binding": {
                "qualification_ref": qual,
                "release_sha256": "a".repeat(64),
                "resolver_ipv4": "192.0.2.53"
            }
        });
    }

    let mission: MissionRegistered = serde_json::from_value(json!({
        "event_id": Uuid::from_u128(0xa001),
        "operation_id": Uuid::from_u128(0xa002),
        "engagement_id": e,
        "campaign_id": c,
        "producer": "mission",
        "affected_entity": c,
        "owner_revision": 1,
        "kind": "mission_registered",
        "version": 3,
        "causation_id": Uuid::from_u128(0xa002),
        "correlation_id": Uuid::from_u128(0xa002),
        "occurred_at": now,
        "recorded_at": now,
        "fields": raw
    }))
    .unwrap();

    let capacity = crate::m1_context_capacity::ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 1,
        session_claimed: false,
        active_holds: 0,
        charged: crate::m1_inference_budget::ReservedCapacity {
            calls: 0,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    };

    ContextBasis {
        mission,
        capacity,
        evaluated_at: now,
        terrain: crate::m1_context::TerrainContextState::NotLoaded,
    }
}

fn sample_autonomous(
    live: bool,
    e: Uuid,
    c: Uuid,
    qual: Uuid,
    profile: &str,
    sha: &str,
) -> AutonomousContext {
    let mode = if live { "live" } else { "controlled_fixture" };
    AutonomousContext {
        binding: RunContextBinding {
            engagement_id: EngagementId(e),
            campaign_id: CampaignId(c),
            run_profile: profile.into(),
            execution_mode: mode.into(),
            inference_profile: Some("fixture-genai-responses-v1".into()),
            release_sha256: sha.into(),
            qualification_ref: qual,
        },
        predecessor: None,
    }
}

#[test]
fn bind_handles_none_and_non_live_autonomous() {
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let qual = Uuid::new_v4();
    let basis = sample_basis(true, e, c, qual);
    let mut pack = json!({});

    // None autonomous returns Ok(())
    assert!(bind(&mut pack, &basis, None).is_ok());

    // Non-live autonomous returns Ok(()) without injecting live_binding
    let fixture_auto = sample_autonomous(
        false,
        e,
        c,
        qual,
        "autonomous-dns-fixture-v1",
        &"a".repeat(64),
    );
    assert!(bind(&mut pack, &basis, Some(&fixture_auto)).is_ok());
    assert!(pack.get("live_binding").is_none());

    // Live autonomous injects live_binding and qualification_purpose
    let live_auto = sample_autonomous(
        true,
        e,
        c,
        qual,
        "autonomous-dns-genai-lab-v1",
        &"a".repeat(64),
    );
    assert!(bind(&mut pack, &basis, Some(&live_auto)).is_ok());
    assert!(pack.get("live_binding").is_some());
    assert_eq!(pack["qualification_purpose"], "LAB");
}

#[test]
fn check_autonomous_basis_enforces_engagement_campaign_and_profile() {
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let qual = Uuid::new_v4();
    let basis = sample_basis(true, e, c, qual);
    let auto = sample_autonomous(
        true,
        e,
        c,
        qual,
        "autonomous-dns-genai-lab-v1",
        &"a".repeat(64),
    );

    // Matching succeeds
    assert!(check_autonomous_basis(&basis, &auto).is_ok());

    // Engagement mismatch fails
    let bad_e = sample_autonomous(
        true,
        Uuid::new_v4(),
        c,
        qual,
        "autonomous-dns-genai-lab-v1",
        &"a".repeat(64),
    );
    assert_eq!(
        check_autonomous_basis(&basis, &bad_e).unwrap_err(),
        Fail::State("context_basis_mismatch")
    );

    // Campaign mismatch fails
    let bad_c = sample_autonomous(
        true,
        e,
        Uuid::new_v4(),
        qual,
        "autonomous-dns-genai-lab-v1",
        &"a".repeat(64),
    );
    assert_eq!(
        check_autonomous_basis(&basis, &bad_c).unwrap_err(),
        Fail::State("context_basis_mismatch")
    );

    // Profile mismatch fails
    let bad_prof = sample_autonomous(true, e, c, qual, "unknown-profile", &"a".repeat(64));
    assert_eq!(
        check_autonomous_basis(&basis, &bad_prof).unwrap_err(),
        Fail::State("context_basis_mismatch")
    );
}

#[test]
fn qualified_binding_rejects_missing_permission_or_binding_mismatches() {
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let qual = Uuid::new_v4();
    let auto = sample_autonomous(
        true,
        e,
        c,
        qual,
        "autonomous-dns-genai-lab-v1",
        &"a".repeat(64),
    );

    // Missing m1_inference_permission in basis
    let non_live_basis = sample_basis(false, e, c, qual);
    assert_eq!(
        qualified_binding(&non_live_basis, &auto.binding).unwrap_err(),
        Fail::State("context_basis_mismatch")
    );

    // Qualification ref mismatch
    let basis = sample_basis(true, e, c, qual);
    let bad_ref = sample_autonomous(
        true,
        e,
        c,
        Uuid::new_v4(),
        "autonomous-dns-genai-lab-v1",
        &"a".repeat(64),
    );
    assert_eq!(
        qualified_binding(&basis, &bad_ref.binding).unwrap_err(),
        Fail::State("context_basis_mismatch")
    );

    // Release sha mismatch
    let bad_sha = sample_autonomous(
        true,
        e,
        c,
        qual,
        "autonomous-dns-genai-lab-v1",
        &"0".repeat(64),
    );
    assert_eq!(
        qualified_binding(&basis, &bad_sha.binding).unwrap_err(),
        Fail::State("context_basis_mismatch")
    );
}

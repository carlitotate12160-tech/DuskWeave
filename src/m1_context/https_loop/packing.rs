//! Bounded ContextPack fitting and selected-seed protection for `m1_https_loop_v1`.
use super::{
    AutonomousContext, Candidate, ContextDisclosure, ContextRequest, EndpointBinding, Fail,
    HttpsContextBasis, OperationId, PreparedContext, PreparedHttpsLoop, Res, TerrainClaimSummary,
    TerrainContextState, TerrainDependency, TerrainHttpsClaimView, append_https_dns_references,
    apply_https_extensions, build_endpoint_map, endpoint_parent_dependencies, filter_https_claims,
    issued_assets, loop_dns_claims, pack, select_endpoints, wire_fits,
};
use serde_json::Value;

pub(super) fn fit_https_pack(
    ctx: &HttpsContextBasis,
    d: &ContextDisclosure,
    q: &ContextRequest,
    op: OperationId,
    terrain: &TerrainContextState,
    a: &AutonomousContext,
) -> Res<PreparedHttpsLoop> {
    let mut cands = q.candidates[..q.candidates.len().min(d.candidates)].to_vec();
    let protected = a
        .selected_seed()
        .or_else(|| cands.first().map(|c| c.asset_ref));
    let total_cands = cands.len();
    loop {
        match try_https_pack(ctx, d, q, op, terrain, a, &cands, total_cands - cands.len()) {
            Ok(built) => return Ok(built),
            Err(Fail::State("context_bytes_insufficient")) => {
                drop_optional_candidate(&mut cands, ctx, terrain, protected)
                    .ok_or(Fail::State("context_bytes_insufficient"))?;
            }
            Err(other) => return Err(other),
        }
    }
}

fn drop_optional_candidate(
    cands: &mut Vec<Candidate>,
    ctx: &HttpsContextBasis,
    terrain: &TerrainContextState,
    protected: Option<crate::mission::AssetRef>,
) -> Option<()> {
    let idx = cands
        .iter()
        .enumerate()
        .rev()
        .find(|(_, c)| {
            Some(c.asset_ref) != protected
                && !ctx.dns_material.iter().any(|m| m.asset_ref == c.asset_ref)
                && !ctx.https_claims.iter().any(|h| h.asset_ref == c.asset_ref)
                && loop_dns_claims(terrain, std::slice::from_ref(c)).is_empty()
        })?
        .0;
    cands.remove(idx);
    Some(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "local assembly helper keeps fit_https_pack shallow"
)]
fn try_https_pack(
    ctx: &HttpsContextBasis,
    d: &ContextDisclosure,
    q: &ContextRequest,
    op: OperationId,
    terrain: &TerrainContextState,
    a: &AutonomousContext,
    cands: &[Candidate],
    omitted_cands: usize,
) -> Res<PreparedHttpsLoop> {
    let dns_claims = loop_dns_claims(terrain, cands);
    let (endpoints, omitted_eps, total_eps) = select_endpoints(ctx, cands)?;
    let (utf8, deps) = build_https_utf8(
        ctx,
        d,
        q,
        op,
        terrain,
        a,
        cands,
        &dns_claims,
        &endpoints,
        &filter_https_claims(&ctx.https_claims, cands),
        omitted_eps,
        total_eps,
        omitted_cands,
    )?;
    let endpoint_map =
        build_endpoint_map(ctx, op, issued_assets(cands), endpoints, utf8.as_bytes())?;
    Ok(PreparedHttpsLoop {
        prepared: PreparedContext::prepared(op, utf8, deps),
        endpoint_map,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "local projection writer keeps try_https_pack under McCabe cap"
)]
fn build_https_utf8(
    ctx: &HttpsContextBasis,
    d: &ContextDisclosure,
    q: &ContextRequest,
    op: OperationId,
    terrain: &TerrainContextState,
    a: &AutonomousContext,
    cands: &[Candidate],
    dns_claims: &[TerrainClaimSummary],
    endpoints: &[EndpointBinding],
    https_shown: &[&TerrainHttpsClaimView],
    omitted_eps: usize,
    total_eps: usize,
    omitted_cands: usize,
) -> Res<(String, Vec<TerrainDependency>)> {
    let (mut raw, mut deps) = pack(&ctx.basis, d, q, op, cands, terrain, &[], 0)?;
    let terrain_deps = append_https_dns_references(&mut raw, dns_claims)?;
    let endpoint_deps = endpoint_parent_dependencies(terrain, endpoints)?;
    apply_https_extensions(
        &mut raw,
        ctx,
        a,
        endpoints,
        &terrain_deps,
        https_shown,
        omitted_eps,
        total_eps,
        omitted_cands,
    )?;
    a.extend_https(&mut raw, d.context_bytes, d.configuration_bytes)?;
    deps.extend(terrain_deps);
    deps.extend(endpoint_deps);
    Ok((encode_bounded_pack(&raw, d)?, deps))
}

fn encode_bounded_pack(raw: &Value, d: &ContextDisclosure) -> Res<String> {
    let utf8 = serde_json::to_string(raw).map_err(|_| Fail::Store("context_encode"))?;
    wire_fits(&utf8, d, true)
        .then_some(utf8)
        .ok_or(Fail::State("context_bytes_insufficient"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_autonomous_context::RunContextBinding;
    use crate::m1_context::ContextBasis;
    use crate::m1_context_capacity::ContextCapacity;
    use crate::m1_context_input::Priority;
    use crate::m1_inference_budget::ReservedCapacity;
    use crate::m1_terrain_input::{
        ClaimFreshness, DnsCompleteness, DnsLimitation, EvaluationOutcome, TerrainClaimStatus,
        TerrainSourceKind, TrustTier,
    };
    use crate::m1_terrain_view::{EffectiveClaimStatus, TerrainDependency, TerrainPublicView};
    use crate::mission::{
        AssetRef, CampaignId, EngagementId, EventId, Mission, RegistrationFields, RegistrationInput,
    };
    use uuid::Uuid;

    fn sample_mission(a1: Uuid, a2: Uuid) -> crate::mission::MissionRegistered {
        let fields_v = serde_json::json!({
            "operator_ref": Uuid::from_u128(3), "authority_ref": Uuid::from_u128(4),
            "authority_revision": 1, "goal_ref": Uuid::from_u128(5), "exercise_mode": "blind",
            "included_assets": [a1, a2], "excluded_assets": [],
            "starts_at": 1_700_000_000i64, "ends_at": 1_700_005_000i64,
            "m1_permission": {
                "policy_version": 1, "ct_base_domain": "example.invalid", "provider_disclosure": "crt_sh",
                "vantage_ref": Uuid::from_u128(7), "resolver_ipv4": "192.0.2.53",
                "discovery_rules": [{"exact": "example.invalid"}],
                "contact_rules": [{"asset_ref": a1, "rule": {"exact": "target.example.invalid"}, "priority": 1}],
                "excluded_names": [], "approved_path": "/", "starts_at": 1_700_000_000, "ends_at": 1_700_005_000,
                "campaign_limits": {"episodes": 3, "provider_calls": 8, "dns_questions": 3, "dns_followups": 3, "tcp_connections": 2, "head_requests": 2},
                "concurrency": 1
            }
        });
        let fields: RegistrationFields = serde_json::from_value(fields_v).unwrap();
        let input = RegistrationInput {
            engagement_id: EngagementId(Uuid::from_u128(1)),
            campaign_id: CampaignId(Uuid::from_u128(2)),
            fields,
        };
        Mission::register(
            &input,
            OperationId(Uuid::from_u128(8)),
            EventId(Uuid::from_u128(9)),
            1_700_000_000,
        )
        .unwrap()
        .1
    }

    #[test]
    fn drop_optional_candidate_protects_non_zero_selected_seed() {
        let a1 = Uuid::from_u128(0x21);
        let a2 = Uuid::from_u128(0x22);
        let ctx = HttpsContextBasis {
            basis: ContextBasis {
                mission: sample_mission(a1, a2),
                capacity: ContextCapacity {
                    version: "m1_inference_capacity_snapshot_v2".into(),
                    epoch: 1,
                    session_claimed: false,
                    active_holds: 0,
                    charged: ReservedCapacity {
                        calls: 0,
                        request_bytes: 0,
                        response_bytes: 0,
                        output_token_capacity: 0,
                        wall_time_ms: 0,
                    },
                },
                evaluated_at: 1_700_000_100,
                terrain: TerrainContextState::NotLoaded,
            },
            request_id: Uuid::from_u128(0x99),
            dns_material: Vec::new(),
            https_claims: Vec::new(),
        };
        let mut cands = vec![
            Candidate {
                asset_ref: AssetRef(a1),
                priority: Priority::High,
            },
            Candidate {
                asset_ref: AssetRef(a2),
                priority: Priority::Medium,
            },
        ];
        let dropped = drop_optional_candidate(
            &mut cands,
            &ctx,
            &TerrainContextState::NotLoaded,
            Some(AssetRef(a2)),
        );
        assert!(dropped.is_some());
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].asset_ref, AssetRef(a2));
    }

    #[test]
    fn endpoint_parent_dependencies_retains_stale_ttl_parent_and_rejects_missing_parent() {
        let a1 = AssetRef(Uuid::from_u128(0x21));
        let claim_id = Uuid::from_u128(0x55);
        let ep = EndpointBinding {
            local_id: "endpoint_0".into(),
            asset_ref: a1,
            hostname: "target.example.invalid".into(),
            ipv4: "198.51.100.10".into(),
            port: 443,
            approved_path: "/".into(),
            method: "HEAD".into(),
            parent_claim_id: claim_id,
            parent_claim_revision: 1,
            resolver_ipv4: "192.0.2.53".into(),
            vantage_ref: Uuid::from_u128(7),
            perceived_at: 1_700_000_000,
            lowest_ttl_seconds: Some(30),
            valid_until: 1_700_000_030,
        };
        assert_eq!(
            endpoint_parent_dependencies(
                &TerrainContextState::NotLoaded,
                std::slice::from_ref(&ep)
            )
            .unwrap_err(),
            Fail::State("context_basis_mismatch")
        );
        let stale_claim = TerrainClaimSummary {
            claim_id,
            claim_revision: 1,
            previous_claim_revision: 0,
            operation_id: OperationId(Uuid::from_u128(0x50)),
            event_id: EventId(Uuid::from_u128(0x51)),
            observation_id: Uuid::from_u128(0x52),
            material_id: Uuid::from_u128(0x53),
            evaluation_id: Uuid::from_u128(0x54),
            asset_ref: a1,
            vantage_ref: Uuid::from_u128(7),
            source_kind: TerrainSourceKind::FixtureDnsAcquisition,
            claim_kind: "dns_hostname_ipv4_resolution".into(),
            trust_tier: TrustTier::Tier1,
            claim_status: TerrainClaimStatus::Observed,
            effective_status: EffectiveClaimStatus::Stale,
            corroborated: false,
            conflicted: false,
            integrity_hold: false,
            hold_frontier: None,
            temporal_frontier: 1_700_000_000,
            perceived_at: 1_700_000_000,
            uncertainty_seconds: 0,
            evaluated_at: 1_700_000_050,
            valid_until: Some(1_700_000_030),
            lowest_ttl_seconds: Some(30),
            answer_count: 1,
            completeness: DnsCompleteness::Complete,
            limitations: vec![DnsLimitation::SingleVantageOnly],
            outcome: EvaluationOutcome::QualifiedDnsClaim,
            freshness: ClaimFreshness::Expired,
            routing_usable: false,
        };
        let view = TerrainPublicView {
            engagement_id: EngagementId(Uuid::from_u128(1)),
            campaign_id: CampaignId(Uuid::from_u128(2)),
            mission_revision: 1,
            registration_operation_id: OperationId(Uuid::from_u128(8)),
            registration_event_id: EventId(Uuid::from_u128(9)),
            vantage_ref: Uuid::from_u128(7),
            evaluated_at: 1_700_000_050,
            selected_assets: vec![a1],
            claims: vec![stale_claim.clone()],
            omitted_by_claim_limit: 0,
        };
        let deps =
            endpoint_parent_dependencies(&TerrainContextState::Available(view), &[ep]).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].local_id, "endpoint_0");
        assert!(deps[0].matches_integrity_current(&stale_claim));
        let _ = TerrainDependency::from_endpoint_parent;
        let _ = RunContextBinding {
            engagement_id: EngagementId(Uuid::from_u128(1)),
            campaign_id: CampaignId(Uuid::from_u128(2)),
            run_profile: "m1_https_loop_v1".into(),
            execution_mode: "controlled_fixture".into(),
            inference_profile: None,
            release_sha256: "a".repeat(64),
            qualification_ref: Uuid::from_u128(1),
        };
    }
}

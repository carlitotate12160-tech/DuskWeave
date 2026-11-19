//! Closed successor phase/intent selection for `m1_https_loop_v1`.
use super::{Fail, Res, RunPhase};
use crate::m1_https::{
    DecisionEndpointMap, DependencyRefresh, INTENT_DNS_UDP, INTENT_HTTPS_HEAD_V1,
    VERSION_HTTPS_LOOP,
};
use crate::m1_reasoning::proposal::{Category, Proposal};
use crate::mission::AssetRef;

pub const MAX_DECISIONS: usize = 4;
pub const MAX_EPISODES: usize = 3;
pub const MAX_HEAD_ATTEMPTS: usize = 2;
pub const MAX_DNS_QUERIES: usize = 3;
pub const MIN_REMAINING_BEFORE_EPISODE_MS: i64 = 314_000;

#[derive(Debug, PartialEq, Eq)]
pub enum HttpsNextStep {
    Dns {
        seed_index: usize,
    },
    Head {
        asset_ref: AssetRef,
        endpoint_ref: String,
        dependency_refresh: DependencyRefresh,
    },
    Terminal(&'static str),
}

pub fn decision_phase_for_step(step: usize) -> Res<RunPhase> {
    match step {
        1 => Ok(RunPhase::DecisionOne),
        2 => Ok(RunPhase::DecisionTwo),
        3 => Ok(RunPhase::DecisionThree),
        4 => Ok(RunPhase::DecisionFour),
        _ => Err(Fail::State("https_loop_step_invalid")),
    }
}

pub fn acquisition_phase_for_step(step: usize) -> Res<RunPhase> {
    match step {
        1 => Ok(RunPhase::AcquisitionOne),
        2 => Ok(RunPhase::AcquisitionTwo),
        3 => Ok(RunPhase::AcquisitionThree),
        _ => Err(Fail::State("https_loop_step_invalid")),
    }
}

pub fn classify_next_step(
    p: &Proposal,
    candidate_count: usize,
    map: Option<&DecisionEndpointMap>,
) -> Res<HttpsNextStep> {
    let refresh = p
        .next_intent
        .dependency_refresh
        .filter(|_| p.version == VERSION_HTTPS_LOOP)
        .ok_or(Fail::State("run_proposal_ineligible"))?;
    let cat = &p.next_intent.category;
    if *cat != Category::Validate && refresh != DependencyRefresh::None {
        return Err(Fail::State("run_proposal_ineligible"));
    }
    classify_category(p, cat, candidate_count, map, refresh)
}

fn classify_category(
    p: &Proposal,
    cat: &Category,
    candidate_count: usize,
    map: Option<&DecisionEndpointMap>,
    refresh: DependencyRefresh,
) -> Res<HttpsNextStep> {
    match cat {
        Category::ObserveMore => classify_observe_more(p, candidate_count),
        Category::Validate => classify_validate(p, map, refresh),
        Category::Retain => Ok(HttpsNextStep::Terminal("retained")),
        Category::Defer => Ok(HttpsNextStep::Terminal("deferred")),
        Category::Stop => Ok(HttpsNextStep::Terminal("stopped")),
        Category::SynthesizeCandidateImplementation => {
            Ok(HttpsNextStep::Terminal("capability_gap"))
        }
    }
}

pub(super) fn is_context_only_ref(r: &str) -> bool {
    matches!(r, "mission" | "previous_decision")
        || canonical_index(r, "terrain_").is_some()
        || canonical_index(r, "endpoint_").is_some()
        || canonical_index(r, "https_").is_some()
}

pub(super) fn canonical_index(r: &str, prefix: &str) -> Option<usize> {
    let index = r.strip_prefix(prefix)?.parse::<usize>().ok()?;
    (format!("{prefix}{index}") == r).then_some(index)
}

fn collect_dns_seeds(refs: &[String], candidate_count: usize) -> Res<Vec<usize>> {
    let mut seeds = Vec::new();
    for r in refs.iter().filter(|r| !is_context_only_ref(r)) {
        let idx = canonical_index(r, "asset_")
            .filter(|i| *i < candidate_count)
            .ok_or(Fail::State("effect_seed_ineligible"))?;
        if !seeds.contains(&idx) {
            seeds.push(idx);
        }
    }
    Ok(seeds)
}

fn classify_observe_more(p: &Proposal, candidate_count: usize) -> Res<HttpsNextStep> {
    if p.next_intent.view_request.is_some() {
        return p
            .next_intent
            .manifest_reference
            .is_none()
            .then_some(HttpsNextStep::Terminal("owner_view_requested"))
            .ok_or(Fail::State("ambiguous_effect_intent"));
    }
    if p.next_intent.manifest_reference.as_deref() != Some(INTENT_DNS_UDP) {
        return Ok(HttpsNextStep::Terminal("capability_gap"));
    }
    match collect_dns_seeds(&p.next_intent.references, candidate_count)?.as_slice() {
        [seed_index] => Ok(HttpsNextStep::Dns {
            seed_index: *seed_index,
        }),
        _ => Ok(HttpsNextStep::Terminal("capability_gap")),
    }
}

fn classify_validate(
    p: &Proposal,
    map: Option<&DecisionEndpointMap>,
    dependency_refresh: DependencyRefresh,
) -> Res<HttpsNextStep> {
    if p.next_intent.view_request.is_some()
        || p.next_intent.manifest_reference.as_deref() != Some(INTENT_HTTPS_HEAD_V1)
    {
        return Ok(HttpsNextStep::Terminal("capability_gap"));
    }
    let map = map.ok_or(Fail::State("endpoint_map_unavailable"))?;
    let (ep, _) = map.resolve_head(&p.next_intent.references)?;
    Ok(HttpsNextStep::Head {
        asset_ref: ep.asset_ref,
        endpoint_ref: ep.local_id.clone(),
        dependency_refresh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_https::{EndpointBinding, IssuedAssetBinding, sha256_hex};
    use crate::mission::{CampaignId, EngagementId, OperationId};
    use uuid::Uuid;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn base_proposal(
        category: &str,
        manifest: Option<&str>,
        refs: &[&str],
        refresh: &str,
    ) -> Proposal {
        serde_json::from_value(serde_json::json!({
            "version": VERSION_HTTPS_LOOP,
            "hypothesis": "h",
            "considered_alternative": "a",
            "references": ["asset_0"],
            "expected_observation": "e",
            "falsifying_observation": "f",
            "uncertainty": "u",
            "rationale": "r",
            "next_intent": {
                "category": category,
                "references": refs,
                "manifest_reference": manifest,
                "dependency_refresh": refresh
            }
        }))
        .unwrap()
    }

    #[test]
    fn classify_selects_head_only_with_valid_asset_and_endpoint_pair() {
        let map = DecisionEndpointMap {
            version: crate::m1_https::VERSION_ENDPOINT_MAP.into(),
            engagement_id: EngagementId(id(1)),
            campaign_id: CampaignId(id(2)),
            decision_id: OperationId(id(3)),
            request_id: id(4),
            pack_digest: sha256_hex(b"pack"),
            pack_revision: 1,
            assets: vec![IssuedAssetBinding {
                local_id: "asset_0".into(),
                asset_ref: AssetRef(id(10)),
            }],
            bindings: vec![EndpointBinding {
                local_id: "endpoint_0".into(),
                asset_ref: AssetRef(id(10)),
                hostname: "target.example.invalid".into(),
                ipv4: "198.51.100.10".into(),
                port: 443,
                approved_path: "/".into(),
                method: "HEAD".into(),
                parent_claim_id: id(20),
                parent_claim_revision: 1,
                resolver_ipv4: "192.0.2.53".into(),
                vantage_ref: id(30),
                perceived_at: 1000,
                lowest_ttl_seconds: Some(60),
                valid_until: 1060,
            }],
        };
        let p = base_proposal(
            "validate",
            Some(INTENT_HTTPS_HEAD_V1),
            &["asset_0", "endpoint_0"],
            "dns_once_if_stale",
        );
        assert_eq!(
            classify_next_step(&p, 1, Some(&map)).unwrap(),
            HttpsNextStep::Head {
                asset_ref: AssetRef(id(10)),
                endpoint_ref: "endpoint_0".into(),
                dependency_refresh: DependencyRefresh::DnsOnceIfStale,
            }
        );
        let bad = base_proposal(
            "validate",
            Some(INTENT_HTTPS_HEAD_V1),
            &["asset_0", "endpoint_9"],
            "none",
        );
        assert!(classify_next_step(&bad, 1, Some(&map)).is_err());
    }

    #[test]
    fn legacy_next_step_rejects_https_loop_endpoint_and_https_refs() {
        for bad_ref in ["endpoint_0", "https_0"] {
            let mut p: Proposal = serde_json::from_value(serde_json::json!({
                "version": "m1_inference_proposal_v1",
                "hypothesis": "h",
                "considered_alternative": "a",
                "references": ["asset_0"],
                "expected_observation": "e",
                "falsifying_observation": "f",
                "uncertainty": "u",
                "rationale": "r",
                "next_intent": {
                    "category": "observe_more",
                    "references": ["asset_0", bad_ref],
                    "manifest_reference": "dns_udp"
                }
            }))
            .unwrap();
            assert_eq!(
                super::super::next_step(&p, 1),
                Err(Fail::State("effect_seed_ineligible")),
                "legacy selector must reject {bad_ref}"
            );
            p.references.push(bad_ref.into());
        }
    }
}

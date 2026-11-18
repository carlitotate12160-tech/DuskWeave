//! Safe Terrain reference and owner-view projection for ContextPack assembly.
use super::*;

pub(super) fn terrain_reference_json(local_id: &str, c: &TerrainClaimSummary) -> Value {
    json!({
        "id": local_id,
        "owner": "cyber_terrain",
        "source": c.source_kind,
        "entity": c.claim_id,
        "asset_ref": c.asset_ref,
        "vantage_ref": c.vantage_ref,
        "revision": c.claim_revision,
        "lineage": [c.event_id, c.operation_id, c.observation_id, c.material_id, c.evaluation_id],
        "perceived_at": c.perceived_at,
        "time_uncertainty": c.uncertainty_seconds,
        "evaluated_at": c.evaluated_at,
        "temporal_frontier": c.temporal_frontier,
        "valid_until": c.valid_until,
        "ttl_seconds": c.lowest_ttl_seconds,
        "status": c.effective_status,
        "tier": "tier1",
        "freshness": c.freshness,
        "completeness": c.completeness,
        "limitations": c.limitations,
        "corroborated": c.corroborated,
        "conflicted": c.conflicted,
        "integrity_hold": c.integrity_hold
    })
}

fn available_terrain_view(
    b: &ContextBasis,
    v: &TerrainPublicView,
    shown: usize,
    omitted_by_byte_bound: u32,
) -> Value {
    let rev = v.claims.iter().map(|c| c.claim_revision).max();
    let frontier = v.claims.iter().map(|c| c.temporal_frontier).max();
    let mut view = json!({
        "owner": "cyber_terrain",
        "availability": "available",
        "claim_kind": "dns_hostname_ipv4_resolution",
        "revision": rev,
        "frontier": frontier,
        "hold_frontier": v.hold_frontier(),
        "evaluated_at": b.evaluated_at,
        "shown_claims": shown,
        "omissions": v.omissions(omitted_by_byte_bound)
    });
    if let Some(claim) = v.claims.first() {
        view["source"] = json!(claim.source_kind);
    }
    view
}

pub(super) fn terrain_owner_view(
    b: &ContextBasis,
    terrain: &TerrainContextState,
    shown: usize,
    omitted_by_byte_bound: u32,
) -> Value {
    let reason = match terrain {
        TerrainContextState::Available(v) => {
            return available_terrain_view(b, v, shown, omitted_by_byte_bound);
        }
        TerrainContextState::NotLoaded => "not_loaded_by_this_profile",
        TerrainContextState::NotRequested => "not_requested",
        TerrainContextState::DisclosureNotPermitted => "disclosure_not_permitted",
        TerrainContextState::Unavailable(r) => r,
    };
    json!({
        "owner": "cyber_terrain",
        "availability": "unavailable",
        "revision": null,
        "frontier": null,
        "evaluated_at": b.evaluated_at,
        "reason": reason
    })
}

pub(super) fn build_references(
    b: &ContextBasis,
    cands: &[Candidate],
    kept: &[TerrainClaimSummary],
) -> Res<(Vec<Value>, Vec<TerrainDependency>)> {
    let m = &b.mission;
    let mut refs = vec![json!({
        "id": "mission",
        "owner": "mission",
        "entity": m.campaign_id,
        "revision": m.owner_revision,
        "lineage": [m.event_id, m.operation_id],
        "perceived_at": m.occurred_at,
        "time_uncertainty": "unspecified_by_owner",
        "status": "owner_published",
        "tier": null,
        "freshness": "as_of_snapshot",
        "completeness": "scope_reference"
    })];
    refs.extend(cands.iter().enumerate().map(|(i, c)| {
        json!({
            "id": format!("asset_{i}"),
            "owner": "operator",
            "entity": c.asset_ref,
            "revision": null,
            "lineage": [],
            "source": "caller_declared_premise",
            "perceived_at": null,
            "time_uncertainty": "unknown",
            "status": "premise",
            "tier": null,
            "freshness": "unknown",
            "completeness": "declared_only"
        })
    }));
    let mut deps = Vec::with_capacity(kept.len());
    for (i, c) in kept.iter().enumerate() {
        let local_id = format!("terrain_{i}");
        deps.push(TerrainDependency::from_claim(local_id.clone(), c)?);
        refs.push(terrain_reference_json(&local_id, c));
    }
    Ok((refs, deps))
}

pub(super) fn build_candidates(cands: &[Candidate], deps: &[TerrainDependency]) -> Vec<Value> {
    cands
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let t_refs: Vec<&str> = deps
                .iter()
                .filter(|d| d.asset_ref == c.asset_ref)
                .map(|d| d.local_id.as_str())
                .collect();
            let mut obj = json!({
                "reference": format!("asset_{i}"),
                "priority": c.priority,
                "status": "operator_premise"
            });
            if !t_refs.is_empty() {
                obj["terrain_references"] = json!(t_refs);
            }
            obj
        })
        .collect()
}

pub(super) fn stale_tolerant_dependency(
    local_id: String,
    c: &TerrainClaimSummary,
) -> Res<TerrainDependency> {
    if let Ok(dep) = TerrainDependency::from_claim(local_id.clone(), c) {
        return Ok(dep);
    }
    let valid_until = c
        .valid_until
        .filter(|_| {
            c.effective_status == EffectiveClaimStatus::Observed
                && !c.conflicted
                && !c.integrity_hold
                && !c.corroborated
        })
        .ok_or(Fail::Input("forged_terrain_summary"))?;
    Ok(TerrainDependency {
        local_id,
        asset_ref: c.asset_ref,
        claim_id: c.claim_id,
        claim_revision: c.claim_revision,
        material_id: c.material_id,
        evaluation_id: c.evaluation_id,
        observation_id: c.observation_id,
        event_id: c.event_id,
        operation_id: c.operation_id,
        temporal_frontier: c.temporal_frontier,
        valid_until,
        source_kind: c.source_kind,
        perceived_at: c.perceived_at,
    })
}

pub(super) fn append_https_dns_references(
    raw: &mut Value,
    claims: &[TerrainClaimSummary],
) -> Res<Vec<TerrainDependency>> {
    let mut deps = Vec::with_capacity(claims.len());
    let refs = raw["references"]
        .as_array_mut()
        .ok_or(Fail::Store("context_encode"))?;
    for (i, c) in claims.iter().enumerate() {
        let local_id = format!("terrain_{i}");
        deps.push(stale_tolerant_dependency(local_id.clone(), c)?);
        refs.push(terrain_reference_json(&local_id, c));
    }
    if let Some(view) = raw["owner_views"]
        .as_array_mut()
        .and_then(|v| v.iter_mut().find(|o| o["owner"] == "cyber_terrain"))
        && view["availability"] == "available"
    {
        view["shown_claims"] = json!(deps.len());
    }
    Ok(deps)
}

pub(super) fn endpoint_ref_json(ep: &crate::m1_https::EndpointBinding, evaluated_at: i64) -> Value {
    let cap_until = crate::m1_https::ttl_capped_valid_until(
        ep.perceived_at,
        ep.lowest_ttl_seconds,
        Some(ep.valid_until),
    );
    let fresh = cap_until.is_some_and(|u| evaluated_at < u);
    json!({
        "id": ep.local_id, "owner": "cyber_terrain", "source": "issued_endpoint_binding",
        "entity": ep.parent_claim_id, "asset_ref": ep.asset_ref, "parent_revision": ep.parent_claim_revision,
        "port": ep.port, "method": ep.method, "approved_path": ep.approved_path,
        "valid_until": cap_until, "ttl_seconds": ep.lowest_ttl_seconds,
        "freshness": if fresh { "fresh" } else { "expired" }, "status": "observed"
    })
}

pub(super) fn https_ref_json(
    i: usize,
    h: &crate::m1_https_terrain::TerrainHttpsClaimView,
    endpoints: &[crate::m1_https::EndpointBinding],
    evaluated_at: i64,
) -> Value {
    let ep_id = endpoints
        .iter()
        .find(|e| e.asset_ref == h.asset_ref && e.ipv4 == h.ipv4)
        .map(|e| e.local_id.as_str());
    json!({
        "id": format!("https_{i}"), "owner": "cyber_terrain", "source": h.source_kind,
        "entity": h.claim_id, "asset_ref": h.asset_ref, "revision": h.claim_revision,
        "parent_revision": h.parent_claim_revision, "endpoint_ref": ep_id, "outcome": h.outcome,
        "status_code": h.status_code, "redirect": h.redirect, "endpoint_reachable": h.endpoint_reachable,
        "whole_host_proven": h.whole_host_proven, "objective_completed": h.objective_completed,
        "perceived_at": h.perceived_at, "evaluated_at": evaluated_at, "limitations": h.limitations,
        "status": h.claim_status
    })
}

pub(super) fn attach_candidate_refs(
    raw: &mut Value,
    endpoints: &[crate::m1_https::EndpointBinding],
    deps: &[TerrainDependency],
    https_claims: &[&crate::m1_https_terrain::TerrainHttpsClaimView],
) -> Res<()> {
    let refs = raw["references"].clone();
    let cands = raw["candidates"]
        .as_array_mut()
        .ok_or(Fail::Store("context_encode"))?;
    for cand in cands {
        let Some(asset_str) = cand["reference"].as_str().and_then(|id| {
            refs.as_array()?
                .iter()
                .find(|r| r["id"] == id)
                .and_then(|r| r["entity"].as_str())
        }) else {
            continue;
        };
        let t_ids: Vec<&str> = deps
            .iter()
            .filter(|d| d.asset_ref.0.to_string() == asset_str)
            .map(|d| d.local_id.as_str())
            .collect();
        if !t_ids.is_empty() {
            cand["terrain_references"] = json!(t_ids);
        }
        let ep_ids: Vec<&str> = endpoints
            .iter()
            .filter(|e| e.asset_ref.0.to_string() == asset_str)
            .map(|e| e.local_id.as_str())
            .collect();
        if !ep_ids.is_empty() {
            cand["endpoint_references"] = json!(ep_ids);
        }
        let https_ids: Vec<String> = https_claims
            .iter()
            .enumerate()
            .filter(|(_, h)| h.asset_ref.0.to_string() == asset_str)
            .map(|(i, _)| format!("https_{i}"))
            .collect();
        if !https_ids.is_empty() {
            cand["https_references"] = json!(https_ids);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::https_loop::{DnsEndpointMaterial, append_material_bindings};
    use crate::m1_https::MAX_ENDPOINTS_PER_HOST;
    use crate::mission::AssetRef;
    use uuid::Uuid;

    #[test]
    fn five_dns_ips_bind_four_endpoints_with_explicit_omission() {
        let mut bindings = Vec::new();
        let mat = DnsEndpointMaterial {
            asset_ref: AssetRef(Uuid::from_u128(0x21)),
            hostname: "target.example.invalid".into(),
            claim_id: Uuid::from_u128(0x22),
            claim_revision: 1,
            vantage_ref: Uuid::from_u128(0x23),
            resolver_ipv4: "192.0.2.53".into(),
            perceived_at: 1_700_000_000,
            lowest_ttl_seconds: Some(60),
            valid_until: 1_700_000_060,
            effective_status: "observed".into(),
            conflicted: false,
            integrity_hold: false,
            answers: (10..15).map(|n| format!("198.51.100.{n}")).collect(),
        };
        append_material_bindings(&mut bindings, &mat, "/health");
        assert_eq!(bindings.len(), MAX_ENDPOINTS_PER_HOST);
        assert_eq!(mat.answers.len() - bindings.len(), 1);
        assert_eq!(bindings[0].local_id, "endpoint_0");
        assert_eq!(bindings[3].local_id, "endpoint_3");
    }
}

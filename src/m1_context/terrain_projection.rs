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

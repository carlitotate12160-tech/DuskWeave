//! Cognition-owned, read-only ContextPack assembly. No accepted decision or transport.
use crate::m1_context_capacity::ContextCapacity;
use crate::m1_context_input::ContextRequest;
use crate::m1_inference_permission::ContextDisclosure;
use crate::m1_terrain_view::{
    EffectiveClaimStatus, TerrainClaimSummary, TerrainDependency, TerrainPublicView,
};
use crate::mission::{AssetRef, MissionRegistered, OperationId};
use crate::{Fail, Res};
use serde_json::{Value, json};

pub const QUESTION: &str = "Which already-authorized, mission-relevant HTTPS candidate is the best next subject for access-validation planning within the permitted contact envelope, and which routing or response premise still needs observation, a different candidate, or deferral?";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TerrainContextState {
    #[default]
    NotLoaded,
    NotRequested,
    DisclosureNotPermitted,
    Unavailable(&'static str),
    Available(TerrainPublicView),
}

pub struct ContextBasis {
    pub mission: MissionRegistered,
    pub capacity: ContextCapacity,
    pub evaluated_at: i64,
    pub terrain: TerrainContextState,
}
pub trait ContextReader {
    fn read(&mut self, q: &ContextRequest, op: OperationId) -> Res<ContextBasis>;
}
pub struct PreparedContext {
    pub decision_id: OperationId,
    utf8: String,
    terrain_dependencies: Vec<TerrainDependency>,
}
impl PreparedContext {
    pub fn utf8(&self) -> &str {
        &self.utf8
    }
    pub fn terrain_dependencies(&self) -> &[TerrainDependency] {
        &self.terrain_dependencies
    }
}

pub fn prepare(
    reader: &mut impl ContextReader,
    q: &ContextRequest,
    op: OperationId,
) -> Res<PreparedContext> {
    q.validate(op)?;
    let basis = reader.read(q, op)?;
    let disclosure = check_basis(&basis, q)?;
    let terrain = resolve_terrain(&basis, &disclosure, q)?;
    fit_pack(&basis, &disclosure, q, op, &terrain)
}

fn resolve_terrain(
    b: &ContextBasis,
    d: &ContextDisclosure,
    q: &ContextRequest,
) -> Res<TerrainContextState> {
    if !d.owner_qualified_summaries {
        return Ok(TerrainContextState::DisclosureNotPermitted);
    }
    if q.candidates.is_empty()
        && matches!(
            b.terrain,
            TerrainContextState::NotLoaded | TerrainContextState::NotRequested
        )
    {
        return Ok(TerrainContextState::NotRequested);
    }
    let TerrainContextState::Available(view) = &b.terrain else {
        return Ok(b.terrain.clone());
    };
    let selected = selected_assets(q, q.candidates.len().min(d.candidates));
    let checked = view.validate_for_mission(&b.mission, &selected, b.evaluated_at)?;
    Ok(TerrainContextState::Available(checked))
}

fn selected_assets(q: &ContextRequest, count: usize) -> Vec<AssetRef> {
    let mut assets = Vec::with_capacity(count);
    for c in &q.candidates[..count] {
        if !assets.contains(&c.asset_ref) {
            assets.push(c.asset_ref);
        }
    }
    assets
}

fn eligible_claims(terrain: &TerrainContextState) -> Vec<TerrainClaimSummary> {
    let TerrainContextState::Available(view) = terrain else {
        return Vec::new();
    };
    view.claims
        .iter()
        .filter(|c| c.routing_usable && c.effective_status == EffectiveClaimStatus::Observed)
        .cloned()
        .collect()
}

fn kept_claims(
    all_claims: &[TerrainClaimSummary],
    q: &ContextRequest,
    cand_count: usize,
    claim_count: usize,
) -> Vec<TerrainClaimSummary> {
    let allowed = selected_assets(q, cand_count);
    all_claims
        .iter()
        .filter(|c| allowed.contains(&c.asset_ref))
        .take(claim_count)
        .cloned()
        .collect()
}

fn shrink_step(
    q: &ContextRequest,
    kept: &[TerrainClaimSummary],
    cand_count: &mut usize,
    claim_count: &mut usize,
) -> bool {
    if *cand_count > 0
        && !kept
            .iter()
            .any(|c| c.asset_ref == q.candidates[*cand_count - 1].asset_ref)
    {
        *cand_count -= 1;
        return true;
    }
    if *claim_count > 0 {
        *claim_count -= 1;
        return true;
    }
    if *cand_count > 0 {
        *cand_count -= 1;
        return true;
    }
    false
}

// Bounded optional-candidate and Terrain-claim omission: preserve required metadata even at zero.
fn fit_pack(
    basis: &ContextBasis,
    disclosure: &ContextDisclosure,
    q: &ContextRequest,
    op: OperationId,
    terrain: &TerrainContextState,
) -> Res<PreparedContext> {
    let all_claims = eligible_claims(terrain);
    let mut cand_count = q.candidates.len().min(disclosure.candidates);
    let mut claim_count = all_claims.len();
    loop {
        let kept = kept_claims(&all_claims, q, cand_count, claim_count);
        let omitted_bytes = (all_claims.len() - kept.len()) as u32;
        let (raw, deps) = pack(
            basis,
            disclosure,
            q,
            op,
            cand_count,
            terrain,
            &kept,
            omitted_bytes,
        )?;
        let utf8 = serde_json::to_string(&raw).map_err(|_| Fail::Store("context_encode"))?;
        if utf8.len() <= disclosure.context_bytes {
            return Ok(PreparedContext {
                decision_id: op,
                utf8,
                terrain_dependencies: deps,
            });
        }
        if !shrink_step(q, &kept, &mut cand_count, &mut claim_count) {
            return Err(Fail::State("context_size_deferred"));
        }
    }
}

fn check_basis(b: &ContextBasis, q: &ContextRequest) -> Res<ContextDisclosure> {
    let m = &b.mission;
    let f = &m.fields;
    let p = f
        .m1_inference_permission()
        .ok_or(Fail::State("inference_consent_absent"))?;
    let d = p.context_disclosure();
    let wanted = crate::m1_inference_budget::ReservationRequest {
        version: "m1_inference_reservation_request_v1".into(),
        engagement_id: m.engagement_id,
        campaign_id: m.campaign_id,
        operator_ref: f.operator_ref,
        expected_mission_revision: 1,
        permission_id: d.permission_id,
        goal_ref: f.goal_ref,
        exercise_mode: f.exercise_mode,
    };
    if q.mission != wanted || (m.version, m.owner_revision) != (3, 1) {
        return Err(Fail::State("context_basis_mismatch"));
    }
    check_window(b, &d)?;
    check_candidates(q, m, d.operator_premises)?;
    Ok(d)
}

// Mission and consent intervals use the same owner-qualified database clock.
fn check_window(b: &ContextBasis, d: &ContextDisclosure) -> Res<()> {
    let f = &b.mission.fields;
    if !d.required_classes
        || b.evaluated_at < d.starts_at
        || b.evaluated_at >= d.ends_at
        || b.evaluated_at < f.starts_at()
        || b.evaluated_at >= f.ends_at()
    {
        return Err(Fail::State("context_consent_deferred"));
    }
    Ok(())
}

fn check_candidates(q: &ContextRequest, m: &MissionRegistered, allowed: bool) -> Res<()> {
    if !allowed && !q.candidates.is_empty() {
        return Err(Fail::State("context_disclosure_deferred"));
    }
    if q.candidates.iter().any(|c| {
        !m.fields.included_assets.contains(&c.asset_ref)
            || m.fields.excluded_assets.contains(&c.asset_ref)
    }) {
        return Err(Fail::Input("context_scope_refused"));
    }
    Ok(())
}

fn build_references(
    b: &ContextBasis,
    q: &ContextRequest,
    count: usize,
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
    refs.extend(q.candidates[..count].iter().enumerate().map(|(i, c)| {
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

fn terrain_reference_json(local_id: &str, c: &TerrainClaimSummary) -> Value {
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

fn terrain_owner_view(
    b: &ContextBasis,
    terrain: &TerrainContextState,
    shown: usize,
    omitted_by_byte_bound: u32,
) -> Value {
    if let TerrainContextState::Available(v) = terrain {
        let rev = v.claims.iter().map(|c| c.claim_revision).max();
        let frontier = v.claims.iter().map(|c| c.temporal_frontier).max();
        return json!({
            "owner": "cyber_terrain",
            "availability": "available",
            "source": "operator_reviewed_dns",
            "claim_kind": "dns_hostname_ipv4_resolution",
            "revision": rev,
            "frontier": frontier,
            "hold_frontier": v.hold_frontier(),
            "evaluated_at": b.evaluated_at,
            "shown_claims": shown,
            "omissions": v.omissions(omitted_by_byte_bound)
        });
    }
    let reason = match terrain {
        TerrainContextState::NotLoaded => "not_loaded_by_this_profile",
        TerrainContextState::NotRequested => "not_requested",
        TerrainContextState::DisclosureNotPermitted => "disclosure_not_permitted",
        TerrainContextState::Unavailable(r) => r,
        TerrainContextState::Available(_) => unreachable!(),
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

fn build_candidates(q: &ContextRequest, count: usize, deps: &[TerrainDependency]) -> Vec<Value> {
    q.candidates[..count]
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

#[allow(clippy::too_many_arguments)]
fn pack(
    b: &ContextBasis,
    d: &ContextDisclosure,
    q: &ContextRequest,
    op: OperationId,
    count: usize,
    terrain: &TerrainContextState,
    kept: &[TerrainClaimSummary],
    omitted_by_byte_bound: u32,
) -> Res<(Value, Vec<TerrainDependency>)> {
    let m = &b.mission;
    let remaining = b.capacity.remaining(d.totals)?;
    let required = json!({
        "calls": d.required[0],
        "request_bytes": d.required[1],
        "response_bytes": d.required[2],
        "output_token_capacity": d.required[3],
        "wall_time_ms": d.required[4]
    });
    let mut views = vec![
        json!({
            "owner": "mission",
            "availability": "available",
            "revision": 1,
            "frontier": m.event_id,
            "evaluated_at": b.evaluated_at
        }),
        terrain_owner_view(b, terrain, kept.len(), omitted_by_byte_bound),
    ];
    views.extend(
        [
            "foothold_graph",
            "attack_path_view",
            "objective_state",
            "campaign_trajectory",
        ]
        .iter()
        .map(|owner| {
            json!({
                "owner": owner,
                "availability": "unavailable",
                "revision": null,
                "frontier": null,
                "evaluated_at": b.evaluated_at,
                "reason": "not_loaded_by_this_profile"
            })
        }),
    );
    let (refs, deps) = build_references(b, q, count, kept)?;
    let candidates = build_candidates(q, count, &deps);
    let val = json!({
        "version": "m1_context_pack_v1",
        "engagement_id": m.engagement_id,
        "campaign_id": m.campaign_id,
        "decision_id": op,
        "permission_id": d.permission_id,
        "mission_revision": 1,
        "goal_ref": m.fields.goal_ref,
        "exercise_mode": m.fields.exercise_mode,
        "scope": {"owner": "mission", "publication_ref": m.event_id},
        "question": QUESTION,
        "evaluated_at": b.evaluated_at,
        "owner_views": views,
        "references": refs,
        "candidates": candidates,
        "omissions": {
            "supplied_candidates": q.candidates.len(),
            "shown_candidates": count,
            "omitted_candidates": q.candidates.len() - count,
            "selection": "input_order_prefix_with_count_and_byte_bounds",
            "manifests": "catalog_unavailable"
        },
        "pending_unknown": [
            "environmental_views_unavailable",
            "operator_premises_unverified",
            "send_admission_pending"
        ],
        "manifests": [],
        "remaining_capacity": {
            "snapshot_epoch": b.capacity.epoch,
            "campaign": remaining,
            "required_for_full_decision": required,
            "exclusive_owner_busy": b.capacity.session_claimed || b.capacity.active_holds > 0,
            "reservation_granted": false,
            "shared_call_clock": "not_started"
        }
    });
    Ok((val, deps))
}

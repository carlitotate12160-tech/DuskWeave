//! Current-use and recording-time dependencies retain owner-qualified lineage.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainDependency {
    pub local_id: String,
    pub asset_ref: AssetRef,
    pub claim_id: Uuid,
    pub claim_revision: u32,
    pub material_id: Uuid,
    pub evaluation_id: Uuid,
    pub observation_id: Uuid,
    pub event_id: EventId,
    pub operation_id: OperationId,
    pub temporal_frontier: i64,
    pub valid_until: i64,
    pub source_kind: TerrainSourceKind,
    pub perceived_at: i64,
}

impl TerrainDependency {
    pub fn from_claim(local_id: String, claim: &TerrainClaimSummary) -> Res<Self> {
        let valid_until = claim
            .valid_until
            .filter(|_| claim.routing_usable)
            .ok_or(Fail::Input("forged_terrain_summary"))?;
        Ok(Self {
            local_id,
            asset_ref: claim.asset_ref,
            claim_id: claim.claim_id,
            claim_revision: claim.claim_revision,
            material_id: claim.material_id,
            evaluation_id: claim.evaluation_id,
            observation_id: claim.observation_id,
            event_id: claim.event_id,
            operation_id: claim.operation_id,
            temporal_frontier: claim.temporal_frontier,
            valid_until,
            source_kind: claim.source_kind,
            perceived_at: claim.perceived_at,
        })
    }

    pub fn matches_current(&self, claim: &TerrainClaimSummary) -> bool {
        [
            claim.routing_usable,
            claim.effective_status == EffectiveClaimStatus::Observed,
            claim.freshness == ClaimFreshness::Fresh,
            !claim.conflicted,
            !claim.integrity_hold,
            claim.hold_frontier.is_none(),
            claim.asset_ref == self.asset_ref,
            claim.claim_id == self.claim_id,
            claim.claim_revision == self.claim_revision,
            claim.material_id == self.material_id,
            claim.evaluation_id == self.evaluation_id,
            claim.observation_id == self.observation_id,
            claim.event_id == self.event_id,
            claim.operation_id == self.operation_id,
            claim.temporal_frontier == self.temporal_frontier,
            claim.valid_until == Some(self.valid_until),
            claim.source_kind == self.source_kind,
            claim.perceived_at == self.perceived_at,
        ]
        .into_iter()
        .all(|b| b)
    }

    /// TTL may expire after release; identity, source and original validity may not change.
    pub fn matches_integrity_current(&self, claim: &TerrainClaimSummary) -> bool {
        let intact = [
            claim.claim_status == TerrainClaimStatus::Observed,
            observed_coherence(claim),
            !claim.conflicted,
            !claim.integrity_hold,
            claim.hold_frontier.is_none(),
            claim.asset_ref == self.asset_ref,
            claim.claim_id == self.claim_id,
            claim.claim_revision == self.claim_revision,
            claim.material_id == self.material_id,
            claim.evaluation_id == self.evaluation_id,
            claim.observation_id == self.observation_id,
            claim.event_id == self.event_id,
            claim.operation_id == self.operation_id,
            claim.temporal_frontier == self.temporal_frontier,
            claim.valid_until == Some(self.valid_until),
            claim.source_kind == self.source_kind,
            claim.perceived_at == self.perceived_at,
        ];
        intact.into_iter().all(|v| v)
    }
}

pub fn verify_current_dependencies(
    deps: &[TerrainDependency],
    current: &TerrainPublicView,
    evaluated_at: i64,
) -> Result<(), &'static str> {
    let evaluated = current.evaluate_at(evaluated_at, true);
    for dep in deps {
        let ok = evaluated
            .claims
            .iter()
            .find(|c| c.claim_id == dep.claim_id)
            .is_some_and(|c| dep.matches_current(c));
        if !ok {
            return Err("reference_ineligible");
        }
    }
    Ok(())
}

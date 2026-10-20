//! Terrain-owned bounded public DNS context view and current-use dependency contracts.
use crate::m1_terrain::{
    CLAIM_KIND_DNS_IPV4, ClaimFreshness, DnsCompleteness, EvaluationOutcome, TerrainClaimRevision,
    TerrainClaimStatus, TerrainIds, TerrainSourceKind, TrustTier,
};
use crate::m1_terrain_input::DnsLimitation;
use crate::mission::{
    AssetRef, CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, MissionRegistered,
    OperationId, OperatorRef,
};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const VIEW_QUERY_VERSION: &str = "m1_terrain_context_view_query_v1";
pub const VIEW_VERSION: &str = "m1_terrain_dns_context_view_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveClaimStatus {
    Observed,
    Stale,
    Withheld,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainViewQuery {
    pub version: String,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: u32,
    pub goal_ref: GoalRef,
    pub exercise_mode: ExerciseMode,
    pub selected_assets: Vec<AssetRef>,
    pub claim_ids: Option<Vec<Uuid>>,
}

impl TerrainViewQuery {
    pub fn from_mission(
        mission: &MissionRegistered,
        selected_assets: Vec<AssetRef>,
        claim_ids: Option<Vec<Uuid>>,
    ) -> Res<Self> {
        let q = Self {
            version: VIEW_QUERY_VERSION.to_string(),
            engagement_id: mission.engagement_id,
            campaign_id: mission.campaign_id,
            operator_ref: mission.fields.operator_ref,
            expected_mission_revision: 1,
            goal_ref: mission.fields.goal_ref,
            exercise_mode: mission.fields.exercise_mode,
            selected_assets,
            claim_ids,
        };
        q.validate()?;
        Ok(q)
    }

    pub fn validate(&self) -> Res<()> {
        let ids_ok = [
            self.version == VIEW_QUERY_VERSION,
            self.expected_mission_revision == 1,
            !self.engagement_id.0.is_nil(),
            !self.campaign_id.0.is_nil(),
            !self.operator_ref.0.is_nil(),
            !self.goal_ref.0.is_nil(),
            valid_asset_slice(&self.selected_assets),
            self.claim_ids.as_deref().is_none_or(valid_uuid_slice),
        ];
        ids_ok
            .into_iter()
            .all(|b| b)
            .then_some(())
            .ok_or(Fail::Input("invalid_terrain_view_query"))
    }
}

fn valid_asset_slice(items: &[AssetRef]) -> bool {
    !items.is_empty()
        && items.len() <= 8
        && items
            .iter()
            .enumerate()
            .all(|(i, a)| !a.0.is_nil() && !items[..i].contains(a))
}

fn valid_uuid_slice(items: &[Uuid]) -> bool {
    !items.is_empty()
        && items.len() <= 8
        && items
            .iter()
            .enumerate()
            .all(|(i, id)| !id.is_nil() && !items[..i].contains(id))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTerrainClaim {
    pub claim_id: Uuid,
    pub claim_revision: u32,
    pub previous_claim_revision: u32,
    pub operation_id: OperationId,
    pub event_id: EventId,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub evaluation_id: Uuid,
    pub asset_ref: AssetRef,
    pub vantage_ref: Uuid,
    pub source_kind: TerrainSourceKind,
    pub claim_kind: String,
    pub trust_tier: TrustTier,
    pub claim_status: TerrainClaimStatus,
    pub corroborated: bool,
    pub conflicted: bool,
    pub integrity_hold: bool,
    pub hold_frontier: Option<i64>,
    pub temporal_frontier: i64,
    pub perceived_at: i64,
    pub uncertainty_seconds: u32,
    pub valid_until: Option<i64>,
    pub lowest_ttl_seconds: Option<u32>,
    pub answer_count: u32,
    pub completeness: DnsCompleteness,
    pub limitations: Vec<DnsLimitation>,
    pub outcome: EvaluationOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTerrainView {
    pub version: String,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub mission_revision: u32,
    pub registration_operation_id: OperationId,
    pub registration_event_id: EventId,
    pub vantage_ref: Uuid,
    pub evaluated_at: i64,
    pub selected_assets: Vec<AssetRef>,
    pub claims: Vec<RawTerrainClaim>,
    pub omitted_by_claim_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainClaimSummary {
    pub claim_id: Uuid,
    pub claim_revision: u32,
    pub previous_claim_revision: u32,
    pub operation_id: OperationId,
    pub event_id: EventId,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub evaluation_id: Uuid,
    pub asset_ref: AssetRef,
    pub vantage_ref: Uuid,
    pub source_kind: TerrainSourceKind,
    pub claim_kind: String,
    pub trust_tier: TrustTier,
    pub claim_status: TerrainClaimStatus,
    pub effective_status: EffectiveClaimStatus,
    pub corroborated: bool,
    pub conflicted: bool,
    pub integrity_hold: bool,
    pub hold_frontier: Option<i64>,
    pub temporal_frontier: i64,
    pub perceived_at: i64,
    pub uncertainty_seconds: u32,
    pub evaluated_at: i64,
    pub valid_until: Option<i64>,
    pub lowest_ttl_seconds: Option<u32>,
    pub answer_count: u32,
    pub completeness: DnsCompleteness,
    pub limitations: Vec<DnsLimitation>,
    pub outcome: EvaluationOutcome,
    pub freshness: ClaimFreshness,
    pub routing_usable: bool,
}

impl TerrainClaimSummary {
    pub fn from_raw(raw: RawTerrainClaim, evaluated_at: i64, authority_active: bool) -> Self {
        let base = Self {
            claim_id: raw.claim_id,
            claim_revision: raw.claim_revision,
            previous_claim_revision: raw.previous_claim_revision,
            operation_id: raw.operation_id,
            event_id: raw.event_id,
            observation_id: raw.observation_id,
            material_id: raw.material_id,
            evaluation_id: raw.evaluation_id,
            asset_ref: raw.asset_ref,
            vantage_ref: raw.vantage_ref,
            source_kind: raw.source_kind,
            claim_kind: raw.claim_kind,
            trust_tier: raw.trust_tier,
            claim_status: raw.claim_status,
            effective_status: EffectiveClaimStatus::Withheld,
            corroborated: raw.corroborated,
            conflicted: raw.conflicted,
            integrity_hold: raw.integrity_hold,
            hold_frontier: raw.hold_frontier,
            temporal_frontier: raw.temporal_frontier,
            perceived_at: raw.perceived_at,
            uncertainty_seconds: raw.uncertainty_seconds,
            evaluated_at,
            valid_until: raw.valid_until,
            lowest_ttl_seconds: raw.lowest_ttl_seconds,
            answer_count: raw.answer_count,
            completeness: raw.completeness,
            limitations: raw.limitations,
            outcome: raw.outcome,
            freshness: ClaimFreshness::NotApplicable,
            routing_usable: false,
        };
        base.evaluate_at(evaluated_at, authority_active)
    }

    pub fn evaluate_at(&self, evaluated_at: i64, authority_active: bool) -> Self {
        let rev = TerrainClaimRevision {
            claim_id: self.claim_id,
            claim_revision: self.claim_revision,
            previous_claim_revision: self.previous_claim_revision,
            evaluation_id: self.evaluation_id,
            observation_id: self.observation_id,
            material_id: self.material_id,
            subject_name: "basis.invalid".to_string(),
            claim_kind: self.claim_kind.clone(),
            vantage_ref: self.vantage_ref,
            resolver_ipv4: "192.0.2.1".to_string(),
            asset_ref: Some(self.asset_ref),
            trust_tier: self.trust_tier,
            claim_status: self.claim_status,
            corroborated: self.corroborated,
            conflicted: self.conflicted,
            temporal_frontier: self.temporal_frontier,
            perceived_at: self.perceived_at,
            valid_until: self.valid_until,
            answers: Vec::new(),
        };
        let view = rev.view_with_authority(evaluated_at, authority_active && !self.integrity_hold);
        let effective_status = classify_effective(self, view.freshness, view.routing_usable);
        let mut out = self.clone();
        out.evaluated_at = evaluated_at;
        out.freshness = view.freshness;
        out.effective_status = effective_status;
        out.routing_usable = view.routing_usable;
        out
    }

    pub fn validate_basis(
        &self,
        e: EngagementId,
        c: CampaignId,
        vantage_ref: Uuid,
        selected_assets: &[AssetRef],
    ) -> Res<()> {
        let ids = TerrainIds::derive(e, c, self.operation_id)
            .map_err(|_| Fail::Input("forged_terrain_summary"))?;
        let actual_ids = (
            self.observation_id,
            self.material_id,
            self.evaluation_id,
            self.claim_id,
            self.event_id,
        );
        let expected_ids = (
            ids.observation_id,
            ids.material_id,
            ids.evaluation_id,
            ids.claim_id,
            ids.event_id,
        );
        let base_ok = [
            actual_ids == expected_ids,
            selected_assets.contains(&self.asset_ref),
            self.vantage_ref == vantage_ref,
            self.claim_kind == CLAIM_KIND_DNS_IPV4,
            !self.corroborated,
            self.claim_revision > 0,
            self.claim_revision > self.previous_claim_revision,
            self.perceived_at > 0,
            self.temporal_frontier >= self.perceived_at,
            self.uncertainty_seconds <= 300,
            self.answer_count <= 4,
            self.limitations.len() <= 4,
            self.integrity_hold == self.hold_frontier.is_some(),
            self.hold_frontier
                .is_none_or(|h| self.temporal_frontier >= h),
            valid_status_coherence(self),
        ];
        base_ok
            .into_iter()
            .all(|b| b)
            .then_some(())
            .ok_or(Fail::Input("forged_terrain_summary"))
    }
}

fn classify_effective(
    c: &TerrainClaimSummary,
    freshness: ClaimFreshness,
    routing_usable: bool,
) -> EffectiveClaimStatus {
    if [
        c.conflicted,
        c.integrity_hold,
        c.claim_status == TerrainClaimStatus::Conflicted,
    ]
    .into_iter()
    .any(|b| b)
    {
        return EffectiveClaimStatus::Conflicted;
    }
    if c.claim_status == TerrainClaimStatus::Withheld {
        return EffectiveClaimStatus::Withheld;
    }
    if freshness == ClaimFreshness::Fresh && routing_usable {
        EffectiveClaimStatus::Observed
    } else {
        EffectiveClaimStatus::Stale
    }
}

fn valid_status_coherence(c: &TerrainClaimSummary) -> bool {
    let unique_lims =
        (0..c.limitations.len()).all(|i| !c.limitations[..i].contains(&c.limitations[i]));
    let status_ok = match c.claim_status {
        TerrainClaimStatus::Observed => observed_coherence(c),
        TerrainClaimStatus::Conflicted => {
            (c.conflicted, c.outcome, c.valid_until)
                == (true, EvaluationOutcome::ApplicabilityConflict, None)
        }
        TerrainClaimStatus::Withheld => {
            !c.conflicted
                && c.outcome != EvaluationOutcome::QualifiedDnsClaim
                && c.outcome != EvaluationOutcome::ApplicabilityConflict
        }
    };
    unique_lims && status_ok
}

fn observed_coherence(c: &TerrainClaimSummary) -> bool {
    let expected_until = c
        .lowest_ttl_seconds
        .filter(|ttl| (1..=86_400).contains(ttl))
        .map(|ttl| c.perceived_at.saturating_add(i64::from(ttl.min(60))));
    [
        !c.conflicted,
        c.outcome == EvaluationOutcome::QualifiedDnsClaim,
        c.completeness == DnsCompleteness::Complete,
        !c.limitations.contains(&DnsLimitation::PartialAliasChain),
        c.answer_count > 0,
        expected_until.is_some(),
        c.valid_until == expected_until,
    ]
    .into_iter()
    .all(|b| b)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainViewOmissions {
    pub stale_expired: u32,
    pub withheld: u32,
    pub held_or_conflicted: u32,
    pub omitted_by_claim_limit: u32,
    pub omitted_by_byte_bound: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainPublicView {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub mission_revision: u32,
    pub registration_operation_id: OperationId,
    pub registration_event_id: EventId,
    pub vantage_ref: Uuid,
    pub evaluated_at: i64,
    pub selected_assets: Vec<AssetRef>,
    pub claims: Vec<TerrainClaimSummary>,
    pub omitted_by_claim_limit: u32,
}

impl TerrainPublicView {
    pub fn from_raw(
        raw: RawTerrainView,
        mission: &MissionRegistered,
        selected_assets: &[AssetRef],
    ) -> Res<Self> {
        if raw.version != VIEW_VERSION {
            return Err(Fail::State("incompatible_lineage"));
        }
        let claims = raw
            .claims
            .into_iter()
            .map(|c| TerrainClaimSummary::from_raw(c, raw.evaluated_at, true))
            .collect();
        let view = Self {
            engagement_id: raw.engagement_id,
            campaign_id: raw.campaign_id,
            mission_revision: raw.mission_revision,
            registration_operation_id: raw.registration_operation_id,
            registration_event_id: raw.registration_event_id,
            vantage_ref: raw.vantage_ref,
            evaluated_at: raw.evaluated_at,
            selected_assets: raw.selected_assets,
            claims,
            omitted_by_claim_limit: raw.omitted_by_claim_limit,
        };
        view.validate_for_mission(mission, selected_assets, view.evaluated_at)
    }

    pub fn validate_for_mission(
        &self,
        mission: &MissionRegistered,
        selected_assets: &[AssetRef],
        evaluated_at: i64,
    ) -> Res<Self> {
        let perm = mission
            .fields
            .m1_permission()
            .ok_or(Fail::State("incompatible_lineage"))?;
        let (vantage, _) = perm.dns_vantage();
        let lineage_ok = [
            self.engagement_id == mission.engagement_id,
            self.campaign_id == mission.campaign_id,
            self.mission_revision == 1,
            self.registration_operation_id == mission.operation_id,
            self.registration_event_id == mission.event_id,
            self.vantage_ref == vantage,
            valid_asset_slice(&self.selected_assets),
            selected_assets
                .iter()
                .all(|a| self.selected_assets.contains(a)),
        ];
        if !lineage_ok.into_iter().all(|b| b) {
            return Err(Fail::State("incompatible_lineage"));
        }
        self.validate_claims()?;
        self.validate_source_mode(mission)?;
        Ok(self.evaluate_at(evaluated_at, true))
    }

    fn validate_claims(&self) -> Res<()> {
        if self.claims.len() > 8 {
            return Err(Fail::Input("forged_terrain_summary"));
        }
        for (idx, claim) in self.claims.iter().enumerate() {
            if idx > 0 && self.claims[idx - 1].claim_id >= claim.claim_id {
                return Err(Fail::Input("forged_terrain_summary"));
            }
            claim.validate_basis(
                self.engagement_id,
                self.campaign_id,
                self.vantage_ref,
                &self.selected_assets,
            )?;
        }
        Ok(())
    }

    pub fn evaluate_at(&self, evaluated_at: i64, authority_active: bool) -> Self {
        let mut out = self.clone();
        out.evaluated_at = evaluated_at;
        out.claims = self
            .claims
            .iter()
            .map(|c| c.evaluate_at(evaluated_at, authority_active))
            .collect();
        out
    }

    pub fn hold_frontier(&self) -> Option<i64> {
        self.claims.iter().filter_map(|c| c.hold_frontier).max()
    }

    pub fn omissions(&self, omitted_by_byte_bound: u32) -> TerrainViewOmissions {
        let count_status = |target: EffectiveClaimStatus| {
            self.claims
                .iter()
                .filter(|c| !c.routing_usable && c.effective_status == target)
                .count() as u32
        };
        TerrainViewOmissions {
            stale_expired: count_status(EffectiveClaimStatus::Stale),
            withheld: count_status(EffectiveClaimStatus::Withheld),
            held_or_conflicted: count_status(EffectiveClaimStatus::Conflicted),
            omitted_by_claim_limit: self.omitted_by_claim_limit,
            omitted_by_byte_bound,
        }
    }
}

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
        ]
        .into_iter()
        .all(|b| b)
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

impl TerrainPublicView {
    fn validate_source_mode(&self, mission: &MissionRegistered) -> Res<()> {
        let fixture = mission
            .fields
            .m1_inference_permission()
            .and_then(|p| p.autonomous_binding())
            .is_some_and(|(_, m)| m == "controlled_fixture");
        let source = if fixture {
            TerrainSourceKind::FixtureDnsAcquisition
        } else {
            TerrainSourceKind::OperatorReviewedDns
        };
        if self.claims.iter().any(|c| c.source_kind != source) {
            return Err(Fail::Input("terrain_source_mode_mismatch"));
        }
        Ok(())
    }
}

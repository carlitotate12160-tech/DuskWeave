//! Terrain DNS observation, canonical material, narrow claim evaluation, and view models.
pub use crate::m1_terrain_input::{
    ClaimFreshness, DnsAnswer, DnsCompleteness, DnsRcode, EvaluationOutcome, TerrainClaimStatus,
    TerrainDnsMaterial, TerrainDnsRequest, TerrainObservation, TerrainSourceKind, TrustTier,
};
use crate::mission::{AssetRef, CampaignId, EngagementId, EventId, OperationId};
pub use crate::postgres_m1_terrain::verify_mission_permission;
pub use crate::postgres_terrain_history::{TerrainIds, TerrainReceipt};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CLAIM_KIND_DNS_IPV4: &str = "dns_hostname_ipv4_resolution";
pub const EVENT_KIND_TERRAIN_CHANGED: &str = "terrain_changed";
pub const OBLIGATION_TERRAIN_HISTORY: &str = "trajectory.terrain_history.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainEvaluation {
    pub evaluation_id: Uuid,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub claim_id: Uuid,
    pub outcome: EvaluationOutcome,
    pub usable_routing_premise: bool,
    pub lowest_ttl_seconds: Option<u32>,
    pub valid_until: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainClaimRevision {
    pub claim_id: Uuid,
    pub claim_revision: u32,
    pub previous_claim_revision: u32,
    pub evaluation_id: Uuid,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub subject_name: String,
    pub claim_kind: String,
    pub vantage_ref: Uuid,
    pub resolver_ipv4: String,
    pub asset_ref: Option<AssetRef>,
    pub trust_tier: TrustTier,
    pub claim_status: TerrainClaimStatus,
    pub corroborated: bool,
    pub conflicted: bool,
    pub perceived_at: i64,
    pub valid_until: Option<i64>,
    pub answers: Vec<DnsAnswer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainClaimView {
    #[serde(flatten)]
    pub revision: TerrainClaimRevision,
    pub evaluated_at: i64,
    pub freshness: ClaimFreshness,
    pub authority_active: bool,
    pub routing_usable: bool,
}

impl TerrainClaimRevision {
    pub fn view_at(&self, evaluated_at: i64) -> TerrainClaimView {
        self.view_with_authority(evaluated_at, true)
    }

    pub fn view_with_authority(
        &self,
        evaluated_at: i64,
        authority_active: bool,
    ) -> TerrainClaimView {
        let ok = [
            self.claim_status == TerrainClaimStatus::Observed,
            !self.conflicted,
            !self.corroborated,
        ]
        .into_iter()
        .all(|b| b);
        let in_win = self
            .valid_until
            .is_some_and(|u| (self.perceived_at..u).contains(&evaluated_at));
        let fresh_claim = [ok, in_win].into_iter().all(|b| b);
        let exp = [ok, self.valid_until == Some(self.perceived_at)]
            .into_iter()
            .any(|b| b);
        let freshness = match (fresh_claim, exp) {
            (true, _) => ClaimFreshness::Fresh,
            (false, true) => ClaimFreshness::Expired,
            (false, false) => ClaimFreshness::NotApplicable,
        };
        let routing_usable = [authority_active, fresh_claim].into_iter().all(|b| b);
        TerrainClaimView {
            revision: self.clone(),
            evaluated_at,
            freshness,
            authority_active,
            routing_usable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainAcceptedEvent {
    pub kind: String,
    pub version: u32,
    pub producer: String,
    pub publication_obligation: String,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub event_id: EventId,
    pub correlation_id: Uuid,
    pub registration_operation_id: OperationId,
    pub registration_event_id: EventId,
    pub mission_revision: u32,
    pub request_hash: String,
    pub observation: TerrainObservation,
    pub material: TerrainDnsMaterial,
    pub evaluation: TerrainEvaluation,
    pub claim: TerrainClaimRevision,
}

impl TerrainAcceptedEvent {
    pub fn validate(&self) -> Res<()> {
        TerrainIds {
            operation_id: self.operation_id,
            observation_id: self.observation.observation_id,
            material_id: self.material.material_id,
            evaluation_id: self.evaluation.evaluation_id,
            claim_id: self.claim.claim_id,
            event_id: self.event_id,
        }
        .validate()?;
        let valid = [
            self.kind == EVENT_KIND_TERRAIN_CHANGED,
            self.version == 1,
            self.producer == "terrain",
            self.publication_obligation == OBLIGATION_TERRAIN_HISTORY,
            self.mission_revision == 1,
            !self.correlation_id.is_nil(),
            !self.registration_operation_id.0.is_nil(),
            !self.registration_event_id.0.is_nil(),
            self.request_hash.len() == 64,
            self.observation.operation_id == self.operation_id,
            self.observation.engagement_id == self.engagement_id,
            self.observation.campaign_id == self.campaign_id,
            self.material.observation_id == self.observation.observation_id,
            self.evaluation.observation_id == self.observation.observation_id,
            self.evaluation.material_id == self.material.material_id,
            self.evaluation.claim_id == self.claim.claim_id,
            self.claim.evaluation_id == self.evaluation.evaluation_id,
            self.claim.observation_id == self.observation.observation_id,
            self.claim.material_id == self.material.material_id,
            self.claim.claim_kind == CLAIM_KIND_DNS_IPV4,
            !self.claim.corroborated,
            self.claim.claim_revision > self.claim.previous_claim_revision,
        ];
        valid
            .into_iter()
            .all(|b| b)
            .then_some(())
            .ok_or(Fail::Store("contract_decode"))
    }
}

fn resolve_revision(
    req: &TerrainDnsRequest,
    prior: Option<&TerrainClaimRevision>,
) -> Res<(u32, u32, bool)> {
    let Some(cur) = prior else {
        return (req.expected_claim_revision == 0)
            .then_some((0, 1, false))
            .ok_or(Fail::Conflict("claim_revision_mismatch"));
    };
    if req.expected_claim_revision != cur.claim_revision {
        return Err(Fail::Conflict("claim_revision_mismatch"));
    }
    let normal = [!cur.conflicted, req.perceived_at > cur.perceived_at]
        .into_iter()
        .all(|b| b);
    Ok((cur.claim_revision, cur.claim_revision + 1, !normal))
}

fn classify_material(
    req: &TerrainDnsRequest,
    conflict: bool,
) -> (
    EvaluationOutcome,
    TerrainClaimStatus,
    Option<u32>,
    Option<i64>,
) {
    let withhold = |o| (o, TerrainClaimStatus::Withheld, None, None);
    let trunc = [
        req.truncated,
        req.completeness == DnsCompleteness::Truncated,
    ]
    .into_iter()
    .any(|b| b);
    if conflict {
        return (
            EvaluationOutcome::ApplicabilityConflict,
            TerrainClaimStatus::Conflicted,
            None,
            None,
        );
    }
    if trunc {
        return withhold(EvaluationOutcome::TruncatedMaterial);
    }
    if req.completeness != DnsCompleteness::Complete {
        return withhold(EvaluationOutcome::IncompleteMaterial);
    }
    if req.rcode != DnsRcode::Noerror {
        return withhold(EvaluationOutcome::NegativeOrErrorRcode);
    }
    let Some(ttl) = req.answers.iter().map(|a| a.ttl_seconds).min() else {
        return withhold(EvaluationOutcome::InsufficientEmptyAnswers);
    };
    if ttl == 0 {
        return (
            EvaluationOutcome::TtlZeroNotUsable,
            TerrainClaimStatus::Withheld,
            Some(0),
            Some(req.perceived_at),
        );
    }
    let until = req.perceived_at.saturating_add(i64::from(ttl.min(60)));
    (
        EvaluationOutcome::QualifiedDnsClaim,
        TerrainClaimStatus::Observed,
        Some(ttl),
        Some(until),
    )
}

fn build_obs_mat(
    req: &TerrainDnsRequest,
    ids: TerrainIds,
    recorded_at: i64,
) -> (TerrainObservation, TerrainDnsMaterial) {
    (
        TerrainObservation {
            observation_id: ids.observation_id,
            operation_id: ids.operation_id,
            engagement_id: req.engagement_id,
            campaign_id: req.campaign_id,
            correlation_id: req.correlation_id,
            source_kind: req.source_kind,
            vantage_ref: req.vantage_ref,
            resolver_ipv4: req.resolver_ipv4.clone(),
            subject_name: req.subject_name.clone(),
            asset_ref: req.asset_ref,
            perceived_at: req.perceived_at,
            uncertainty_seconds: req.uncertainty_seconds,
            recorded_at,
        },
        TerrainDnsMaterial {
            material_id: ids.material_id,
            observation_id: ids.observation_id,
            transport: req.transport,
            qtype: req.qtype,
            qclass: req.qclass,
            rcode: req.rcode,
            truncated: req.truncated,
            completeness: req.completeness,
            limitations: req.limitations.clone(),
            answers: req.answers.clone(),
        },
    )
}

pub fn evaluate_dns_observation(
    req: &TerrainDnsRequest,
    prior: Option<&TerrainClaimRevision>,
    ids: TerrainIds,
    reg_lineage: (OperationId, EventId, i64),
) -> Res<TerrainAcceptedEvent> {
    req.validate()?;
    ids.validate()?;
    let request_hash = req.request_hash()?;
    let (prev_rev, next_rev, conflict) = resolve_revision(req, prior)?;
    let (outcome, claim_status, lowest_ttl, valid_until) = classify_material(req, conflict);
    let usable = outcome == EvaluationOutcome::QualifiedDnsClaim;
    let (observation, material) = build_obs_mat(req, ids, reg_lineage.2);
    let event = TerrainAcceptedEvent {
        kind: EVENT_KIND_TERRAIN_CHANGED.to_string(),
        version: 1,
        producer: "terrain".to_string(),
        publication_obligation: OBLIGATION_TERRAIN_HISTORY.to_string(),
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        operation_id: ids.operation_id,
        event_id: ids.event_id,
        correlation_id: req.correlation_id,
        registration_operation_id: reg_lineage.0,
        registration_event_id: reg_lineage.1,
        mission_revision: 1,
        request_hash,
        observation,
        material,
        evaluation: TerrainEvaluation {
            evaluation_id: ids.evaluation_id,
            observation_id: ids.observation_id,
            material_id: ids.material_id,
            claim_id: ids.claim_id,
            outcome,
            usable_routing_premise: usable,
            lowest_ttl_seconds: lowest_ttl,
            valid_until,
        },
        claim: TerrainClaimRevision {
            claim_id: ids.claim_id,
            claim_revision: next_rev,
            previous_claim_revision: prev_rev,
            evaluation_id: ids.evaluation_id,
            observation_id: ids.observation_id,
            material_id: ids.material_id,
            subject_name: req.subject_name.clone(),
            claim_kind: CLAIM_KIND_DNS_IPV4.to_string(),
            vantage_ref: req.vantage_ref,
            resolver_ipv4: req.resolver_ipv4.clone(),
            asset_ref: req.asset_ref,
            trust_tier: TrustTier::Tier1,
            claim_status,
            corroborated: false,
            conflicted: conflict,
            perceived_at: req.perceived_at,
            valid_until,
            answers: req.answers.clone(),
        },
    };
    event.validate()?;
    Ok(event)
}

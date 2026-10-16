//! Terrain DNS domain contracts, read-port admission, evaluation and current-use receipts.
use crate::m1_policy::{
    M1PolicyQuery, M1PolicyReader, NamePurpose, PolicyReason, check_name_policy,
};
pub use crate::m1_terrain_input::{
    ClaimFreshness, DnsAnswer, DnsCompleteness, DnsRcode, EvaluationOutcome, TerrainClaimStatus,
    TerrainDnsMaterial, TerrainDnsRequest, TerrainIds, TerrainObservation, TerrainSourceKind,
    TrustTier,
};
use crate::mission::{AssetRef, CampaignId, EngagementId, EventId, MissionRegistered, OperationId};
use crate::registration::MissionStore;
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
    /// Highest accepted perception frontier; late material never moves this backwards.
    pub temporal_frontier: i64,
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
        self.observation
            .validate_basis()
            .map_err(|_| Fail::Store("contract_decode"))?;
        self.material
            .validate_basis()
            .map_err(|_| Fail::Store("contract_decode"))?;
        crate::m1_terrain_input::validate_dns_hostname(&self.claim.subject_name)
            .map_err(|_| Fail::Store("contract_decode"))?;
        crate::m1_terrain_input::canonical_ipv4(&self.claim.resolver_ipv4)
            .map_err(|_| Fail::Store("contract_decode"))?;
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
            self.request_hash.bytes().all(|b| b.is_ascii_hexdigit()),
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
            self.claim.subject_name == self.observation.subject_name,
            self.claim.vantage_ref == self.observation.vantage_ref,
            self.claim.resolver_ipv4 == self.observation.resolver_ipv4,
            self.claim.asset_ref == self.observation.asset_ref,
            self.claim.answers == self.material.answers,
            self.claim.perceived_at == self.observation.perceived_at,
            self.claim.temporal_frontier >= self.claim.perceived_at,
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
    prior: Option<&TerrainAcceptedEvent>,
) -> Res<(u32, u32, bool)> {
    let Some(prior) = prior else {
        return (req.expected_claim_revision == 0)
            .then_some((0, 1, false))
            .ok_or(Fail::Conflict("claim_revision_mismatch"));
    };
    let cur = &prior.claim;
    if req.expected_claim_revision != cur.claim_revision {
        return Err(Fail::Conflict("claim_revision_mismatch"));
    }
    let duplicate = [
        !cur.conflicted,
        req.perceived_at == cur.temporal_frontier,
        req.perceived_at == cur.perceived_at,
        req.uncertainty_seconds == prior.observation.uncertainty_seconds,
        req.asset_ref == cur.asset_ref,
        prior.material.matches_request(req),
    ]
    .into_iter()
    .all(|b| b);
    let normal = req.perceived_at > cur.temporal_frontier || duplicate;
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
    if req.completeness != DnsCompleteness::Complete
        || req
            .limitations
            .contains(&crate::m1_terrain_input::DnsLimitation::PartialAliasChain)
    {
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

/// Only an admitted, eligible variant of the affected applicability can hold its use.
pub(crate) fn integrity_conflict_applies(
    req: &TerrainDnsRequest,
    ev: &TerrainAcceptedEvent,
) -> bool {
    [
        req.subject_name == ev.observation.subject_name,
        req.vantage_ref == ev.observation.vantage_ref,
        req.resolver_ipv4 == ev.observation.resolver_ipv4,
        req.asset_ref == ev.observation.asset_ref,
        classify_material(req, false).0 == EvaluationOutcome::QualifiedDnsClaim,
    ]
    .into_iter()
    .all(|b| b)
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
    prior: Option<&TerrainAcceptedEvent>,
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
            temporal_frontier: prior.map_or(req.perceived_at, |p| {
                p.claim.temporal_frontier.max(req.perceived_at)
            }),
            perceived_at: req.perceived_at,
            valid_until,
            answers: req.answers.clone(),
        },
    };
    event.validate()?;
    Ok(event)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainReceipt {
    pub kind: String,
    pub result: String,
    pub reason: String,
    pub publication: String,
    pub history: String,
    pub dispatch_granted: bool,
    pub acquisition_qualified: bool,
    pub corroborated: bool,
    pub engagement_id: Option<EngagementId>,
    pub campaign_id: Option<CampaignId>,
    pub operation_id: Option<OperationId>,
    pub event_id: Option<EventId>,
    pub observation_id: Option<Uuid>,
    pub material_id: Option<Uuid>,
    pub evaluation_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub source_kind: Option<TerrainSourceKind>,
    pub evaluation: Option<TerrainEvaluation>,
    pub claim: Option<TerrainClaimView>,
    pub current_claim: Option<TerrainClaimView>,
}

impl TerrainReceipt {
    pub fn failure(
        result: &str,
        reason: &str,
        req: Option<&TerrainDnsRequest>,
        op: Option<OperationId>,
    ) -> Self {
        Self {
            kind: "m1_terrain_receipt_v1".to_string(),
            result: result.to_string(),
            reason: reason.to_string(),
            publication: if reason == "input_mismatch" {
                "committed"
            } else if result == "unresolved" {
                "unknown"
            } else {
                "not_committed"
            }
            .to_string(),
            history: if result == "unresolved" || reason == "input_mismatch" {
                "unknown"
            } else {
                "not_recorded"
            }
            .to_string(),
            dispatch_granted: false,
            acquisition_qualified: false,
            corroborated: false,
            engagement_id: req.map(|r| r.engagement_id),
            campaign_id: req.map(|r| r.campaign_id),
            operation_id: op,
            event_id: None,
            observation_id: None,
            material_id: None,
            evaluation_id: None,
            claim_id: None,
            source_kind: req.map(|r| r.source_kind),
            evaluation: None,
            claim: None,
            current_claim: None,
        }
    }

    pub fn from_event(
        ev: &TerrainAcceptedEvent,
        current: &TerrainClaimRevision,
        history: &str,
        now: i64,
        authority_active: bool,
    ) -> Self {
        let (result, reason) = match (history, ev.claim.conflicted) {
            ("anomaly", _) => ("conflict", "history_anomaly"),
            ("pending", _) => ("pending", "history_pending"),
            (_, true) => ("conflict", ev.evaluation.outcome.as_str()),
            _ => ("accepted", ev.evaluation.outcome.as_str()),
        };
        let mut r = Self::failure(result, reason, None, Some(ev.operation_id));
        r.publication = "committed".to_string();
        r.history = history.to_string();
        r.engagement_id = Some(ev.engagement_id);
        r.campaign_id = Some(ev.campaign_id);
        r.event_id = Some(ev.event_id);
        r.observation_id = Some(ev.observation.observation_id);
        r.material_id = Some(ev.material.material_id);
        r.evaluation_id = Some(ev.evaluation.evaluation_id);
        r.claim_id = Some(ev.claim.claim_id);
        r.source_kind = Some(ev.observation.source_kind);
        r.evaluation = Some(ev.evaluation.clone());
        r.claim = Some(ev.claim.view_with_authority(now, authority_active));
        r.current_claim = Some(current.view_with_authority(now, authority_active));
        if let Some(claim) = &mut r.claim {
            claim.routing_usable &= history != "anomaly"
                && !current.conflicted
                && current.claim_id == ev.claim.claim_id;
        }
        if let Some(claim) = &mut r.current_claim {
            claim.routing_usable &=
                history != "anomaly" || current.temporal_frontier > ev.claim.temporal_frontier;
        }
        r
    }
}

fn map_policy_reason(reason: PolicyReason) -> Result<(), &'static str> {
    match reason {
        PolicyReason::MatchesNamePolicy | PolicyReason::NameNotPermitted => Ok(()),
        PolicyReason::MissionMissing => Err("mission_missing"),
        PolicyReason::AuthorityWithdrawn => Err("authority_withdrawn"),
        PolicyReason::StaleRevision => Err("stale_revision"),
        PolicyReason::PurposeMismatch => Err("purpose_mismatch"),
        PolicyReason::ModeMismatch => Err("mode_mismatch"),
        PolicyReason::M1PolicyAbsent => Err("m1_policy_absent"),
        PolicyReason::OutsideOperatingWindow => Err("outside_operating_window"),
        _ => Err("name_not_permitted"),
    }
}

fn check_m1_bindings(
    reg: &MissionRegistered,
    purpose: &NamePurpose,
    req: &TerrainDnsRequest,
    now: i64,
    matched_name: bool,
) -> Option<&'static str> {
    if reg.fields.operator_ref != req.operator_ref {
        return Some("purpose_mismatch");
    }
    let Some(perm) = reg.fields.m1_permission() else {
        return Some("m1_policy_absent");
    };
    let (vantage, resolver) = perm.dns_vantage();
    if vantage != req.vantage_ref {
        return Some("vantage_mismatch");
    }
    if resolver.to_string() != req.resolver_ipv4 {
        return Some("resolver_mismatch");
    }
    let asset_ok = req.asset_ref.is_none_or(|a| {
        [
            reg.fields.included_assets.contains(&a),
            !reg.fields.excluded_assets.contains(&a),
        ]
        .into_iter()
        .all(|b| b)
    });
    if !asset_ok {
        return Some("asset_not_permitted");
    }
    if !matched_name {
        return Some("name_not_permitted");
    }
    let win = [
        req.perceived_at <= now,
        (reg.fields.starts_at()..reg.fields.ends_at()).contains(&req.perceived_at),
        perm.query_reason(purpose, req.perceived_at) == PolicyReason::MatchesNamePolicy,
    ];
    (!win.into_iter().all(|b| b)).then_some("outside_operating_window")
}

pub fn verify_mission_permission(
    reader: &mut (impl M1PolicyReader + MissionStore),
    req: &TerrainDnsRequest,
) -> Res<Result<(OperationId, EventId, i64), &'static str>> {
    req.validate()?;
    let name = req.subject_name.clone();
    let purpose = match req.asset_ref {
        Some(asset_ref) => NamePurpose::Contact { name, asset_ref },
        None => NamePurpose::DiscoveryDisclosure { name },
    };
    let query = M1PolicyQuery {
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        expected_mission_revision: u64::try_from(req.expected_mission_revision)
            .map_err(|_| Fail::Input("invalid_bounds"))?,
        goal_ref: req.goal_ref,
        exercise_mode: req.exercise_mode,
        purpose: purpose.clone(),
    };
    let res = check_name_policy(reader, &query)?;
    if let Err(code) = map_policy_reason(res.reason) {
        return Ok(Err(code));
    }
    let now = res.checked_at.ok_or(Fail::Store("contract_decode"))?;
    let prov = res
        .snapshot
        .as_ref()
        .ok_or(Fail::Store("contract_decode"))?;
    let reg = reader
        .outbox_event(
            req.engagement_id,
            req.campaign_id,
            prov.registration_operation_id,
        )?
        .ok_or(Fail::Store("contract_decode"))?;
    if reg.event_id != prov.registration_event_id {
        return Err(Fail::Store("contract_decode"));
    }
    let matched = res.reason == PolicyReason::MatchesNamePolicy;
    if let Some(code) = check_m1_bindings(&reg, &purpose, req, now, matched) {
        return Ok(Err(code));
    }
    Ok(Ok((reg.operation_id, reg.event_id, now)))
}

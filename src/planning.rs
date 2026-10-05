//! Mission-owned planning assessment contract; never grants current permission.

use crate::mission::{
    AssetRef, CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, OperationId,
};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[path = "planning_decision.rs"]
mod planning_decision;
#[path = "planning_withdrawal.rs"]
mod planning_withdrawal;
pub use planning_decision::PlanningDecision;
pub use planning_withdrawal::WithdrawalBasis;

/// Source-compatible name for decisions recorded before positive admission.
pub type NonpositiveDecision = PlanningDecision;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningRequest {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub purpose_ref: GoalRef,
    pub asset_ref: AssetRef,
    pub expected_mission_revision: u64,
    pub current_authority_confirmed: bool,
}

impl PlanningRequest {
    pub fn validate(&self) -> Res<()> {
        let references = [
            self.engagement_id.0,
            self.campaign_id.0,
            self.purpose_ref.0,
            self.asset_ref.0,
        ];
        if references.iter().any(uuid::Uuid::is_nil) {
            return Err(Fail::Input("nil_reference"));
        }
        if self.expected_mission_revision == 0 || self.expected_mission_revision > i64::MAX as u64 {
            return Err(Fail::Input("invalid_revision"));
        }
        Ok(())
    }
}

/// Bounded snapshot of the Mission's current scope, copied into a version-2
/// basis at assessment time. Absent on version-1 records (serde default keeps
/// the old wire shape); never inferred or broadened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissionScope {
    pub goal_ref: GoalRef,
    pub included_assets: Vec<AssetRef>,
    pub excluded_assets: Vec<AssetRef>,
}

impl MissionScope {
    fn validate(&self) -> Res<()> {
        if self.goal_ref.0.is_nil()
            || self.included_assets.is_empty()
            || self.included_assets.len() > 64
            || self.excluded_assets.len() > 64
            || self
                .included_assets
                .iter()
                .chain(&self.excluded_assets)
                .any(|a| a.0.is_nil())
        {
            return Err(Fail::Store("unsupported_basis"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissionBasis {
    pub registration_operation_id: OperationId,
    pub revision: u64,
    pub exercise_mode: ExerciseMode,
    pub starts_at: i64,
    pub ends_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MissionScope>,
}

impl MissionBasis {
    pub fn validate(&self) -> Res<()> {
        if self.registration_operation_id.0.is_nil()
            || self.revision != 1
            || self.starts_at >= self.ends_at
        {
            return Err(Fail::Store("unsupported_basis"));
        }
        if let Some(scope) = &self.scope {
            scope.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentTimeBasis {
    ProducerTransactionStart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningAssessed {
    pub event_id: EventId,
    pub operation_id: OperationId,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub producer: String,
    pub kind: String,
    pub version: u32,
    pub affected_entity: OperationId,
    pub owner_revision: u64,
    pub causation_id: OperationId,
    pub correlation_id: OperationId,
    pub request: PlanningRequest,
    pub basis: Option<MissionBasis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withdrawal: Option<WithdrawalBasis>,
    pub decision: NonpositiveDecision,
    pub evaluated_at: i64,
    pub occurred_at: i64,
    pub recorded_at: i64,
    pub time_basis: AssessmentTimeBasis,
}

impl PlanningAssessed {
    pub fn new(
        request: PlanningRequest,
        basis: Option<MissionBasis>,
        operation_id: OperationId,
        event_id: EventId,
        timestamp: i64,
    ) -> Res<Self> {
        let decision = NonpositiveDecision::for_request(2, &request, basis.as_ref(), timestamp)?;
        let event = Self {
            event_id,
            operation_id,
            engagement_id: request.engagement_id,
            campaign_id: request.campaign_id,
            producer: "mission".into(),
            kind: "planning_assessed".into(),
            version: 2,
            affected_entity: operation_id,
            owner_revision: 1,
            causation_id: operation_id,
            correlation_id: operation_id,
            request,
            basis,
            withdrawal: None,
            decision,
            evaluated_at: timestamp,
            occurred_at: timestamp,
            recorded_at: timestamp,
            time_basis: AssessmentTimeBasis::ProducerTransactionStart,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn new_current(
        request: PlanningRequest,
        basis: Option<MissionBasis>,
        operation_id: OperationId,
        event_id: EventId,
        timestamp: i64,
    ) -> Res<Self> {
        let mut event = Self::new(request, basis, operation_id, event_id, timestamp)?;
        if event.decision == PlanningDecision::UnresolvedEvaluationIncomplete {
            event.version = 4;
            event.decision = PlanningDecision::Eligible;
            event.validate()?;
        }
        Ok(event)
    }

    fn validate_scope(&self) -> Res<()> {
        if self.event_id.0.is_nil()
            || self.operation_id.0.is_nil()
            || self.engagement_id != self.request.engagement_id
            || self.campaign_id != self.request.campaign_id
        {
            return Err(Fail::Unresolved("scope_violation"));
        }
        Ok(())
    }

    fn validate_causation(&self) -> Res<()> {
        if self.affected_entity != self.operation_id
            || self.causation_id != self.operation_id
            || self.correlation_id != self.operation_id
        {
            return Err(Fail::Unresolved("scope_violation"));
        }
        Ok(())
    }

    fn validate_contract(&self) -> Res<()> {
        if self.contract_header_invalid() {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        match self.version {
            3 => self.validate_withdrawal_source()?,
            _ => self.validate_legacy_contract()?,
        }
        Ok(())
    }

    fn contract_header_invalid(&self) -> bool {
        self.producer != "mission"
            || self.kind != "planning_assessed"
            || !matches!(self.version, 1..=4)
    }

    fn validate_legacy_contract(&self) -> Res<()> {
        if self.owner_revision != 1 || self.withdrawal.is_some() {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        self.validate_basis_contract()
    }

    /// A present basis carries a scope snapshot exactly on version 2 or 4;
    /// v1 records keep the old wire shape and v2 never downscopes.
    fn validate_basis_contract(&self) -> Res<()> {
        if self.version == 4 && self.basis.is_none() {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        if let Some(basis) = &self.basis {
            basis.validate()?;
            let coherent = matches!(
                (self.version, basis.scope.is_some()),
                (1, false) | (2 | 4, true)
            );
            if !coherent {
                return Err(Fail::Unresolved("unsupported_contract"));
            }
        }
        Ok(())
    }

    fn validate_decision(&self) -> Res<()> {
        if self.decision != self.expected_decision()? || self.timestamps_incoherent() {
            return Err(Fail::Unresolved("invalid_assessment"));
        }
        Ok(())
    }

    fn expected_decision(&self) -> Res<PlanningDecision> {
        if self.version == 3 {
            return Ok(PlanningDecision::RefusedAuthorityWithdrawn);
        }
        let version = if self.version == 4 { 2 } else { self.version };
        let decision = PlanningDecision::for_request(
            version,
            &self.request,
            self.basis.as_ref(),
            self.evaluated_at,
        )?;
        if self.version == 4 {
            if decision != PlanningDecision::UnresolvedEvaluationIncomplete {
                return Err(Fail::Unresolved("invalid_assessment"));
            }
            return Ok(PlanningDecision::Eligible);
        }
        Ok(decision)
    }

    fn timestamps_incoherent(&self) -> bool {
        self.evaluated_at < 0
            || self.occurred_at != self.evaluated_at
            || self.recorded_at != self.evaluated_at
    }

    pub fn validate(&self) -> Res<()> {
        self.request.validate()?;
        self.validate_scope()?;
        self.validate_causation()?;
        self.validate_contract()?;
        self.validate_decision()
    }

    pub fn recorded_eligible(&self) -> bool {
        self.version == 4 && self.decision == PlanningDecision::Eligible && self.validate().is_ok()
    }

    pub fn assessment_labels(&self) -> (&'static str, &'static str) {
        self.decision.labels(self.version)
    }

    pub fn corresponds_to(&self, request: &PlanningRequest, operation_id: OperationId) -> Res<()> {
        self.validate()?;
        if &self.request != request || self.operation_id != operation_id {
            return Err(Fail::Conflict("integrity_conflict"));
        }
        Ok(())
    }
}

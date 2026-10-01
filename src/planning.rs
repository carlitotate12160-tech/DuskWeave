//! Mission-owned nonpositive assessment contract; never grants permission.

use crate::mission::{
    AssetRef, CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, OperationId,
};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissionBasis {
    pub registration_operation_id: OperationId,
    pub revision: u64,
    pub exercise_mode: ExerciseMode,
    pub starts_at: i64,
    pub ends_at: i64,
}

impl MissionBasis {
    pub fn validate(&self) -> Res<()> {
        if self.registration_operation_id.0.is_nil()
            || self.revision != 1
            || self.starts_at >= self.ends_at
        {
            return Err(Fail::Store("unsupported_basis"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonpositiveDecision {
    UnresolvedMissionBasis,
    RefusedRevisionMismatch,
    UnresolvedAuthorityUnconfirmed,
    UnresolvedEvaluationIncomplete,
}

impl NonpositiveDecision {
    pub fn for_request(request: &PlanningRequest, basis: Option<&MissionBasis>) -> Self {
        match basis {
            None => Self::UnresolvedMissionBasis,
            Some(basis) if request.expected_mission_revision != basis.revision => {
                Self::RefusedRevisionMismatch
            }
            Some(_) if !request.current_authority_confirmed => Self::UnresolvedAuthorityUnconfirmed,
            Some(_) => Self::UnresolvedEvaluationIncomplete,
        }
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
        let decision = NonpositiveDecision::for_request(&request, basis.as_ref());
        let event = Self {
            event_id,
            operation_id,
            engagement_id: request.engagement_id,
            campaign_id: request.campaign_id,
            producer: "mission".into(),
            kind: "planning_assessed".into(),
            version: 1,
            affected_entity: operation_id,
            owner_revision: 1,
            causation_id: operation_id,
            correlation_id: operation_id,
            request,
            basis,
            decision,
            evaluated_at: timestamp,
            occurred_at: timestamp,
            recorded_at: timestamp,
            time_basis: AssessmentTimeBasis::ProducerTransactionStart,
        };
        event.validate()?;
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
        if self.producer != "mission"
            || self.kind != "planning_assessed"
            || self.version != 1
            || self.owner_revision != 1
        {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        if let Some(basis) = &self.basis {
            basis.validate()?;
        }
        Ok(())
    }

    fn validate_decision(&self) -> Res<()> {
        if self.decision != NonpositiveDecision::for_request(&self.request, self.basis.as_ref())
            || self.evaluated_at < 0
            || self.occurred_at != self.evaluated_at
            || self.recorded_at != self.evaluated_at
        {
            return Err(Fail::Unresolved("invalid_assessment"));
        }
        Ok(())
    }

    pub fn validate(&self) -> Res<()> {
        self.request.validate()?;
        self.validate_scope()?;
        self.validate_causation()?;
        self.validate_contract()?;
        self.validate_decision()
    }

    pub fn corresponds_to(&self, request: &PlanningRequest, operation_id: OperationId) -> Res<()> {
        self.validate()?;
        if &self.request != request || self.operation_id != operation_id {
            return Err(Fail::Conflict("integrity_conflict"));
        }
        Ok(())
    }
}

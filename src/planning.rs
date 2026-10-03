//! Mission-owned nonpositive assessment contract; never grants permission.

use crate::mission::{
    AssetRef, CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, OperationId,
};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[path = "planning_withdrawal.rs"]
mod planning_withdrawal;
pub use planning_withdrawal::WithdrawalBasis;

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
pub enum NonpositiveDecision {
    UnresolvedMissionBasis,
    RefusedRevisionMismatch,
    UnresolvedAuthorityUnconfirmed,
    UnresolvedEvaluationIncomplete,
    RefusedPurposeMismatch,
    RefusedAssetExcluded,
    RefusedAssetUnknown,
    RefusedNotYetValid,
    RefusedExpired,
    RefusedAuthorityWithdrawn,
}

impl NonpositiveDecision {
    /// Expected decision for a given contract version. Version 1 retains its
    /// original semantics (no scope/window evaluation); version 2 orders the
    /// scoped guards. Every outcome is nonpositive; nothing here grants
    /// eligibility or permission.
    pub fn for_request(
        version: u32,
        request: &PlanningRequest,
        basis: Option<&MissionBasis>,
        evaluated_at: i64,
    ) -> Res<Self> {
        match version {
            1 => Ok(Self::v1(request, basis)),
            2 => Ok(Self::v2(request, basis, evaluated_at)),
            _ => Err(Fail::Unresolved("unsupported_contract")),
        }
    }

    fn v1(request: &PlanningRequest, basis: Option<&MissionBasis>) -> Self {
        match basis {
            None => Self::UnresolvedMissionBasis,
            Some(basis) if request.expected_mission_revision != basis.revision => {
                Self::RefusedRevisionMismatch
            }
            Some(_) if !request.current_authority_confirmed => Self::UnresolvedAuthorityUnconfirmed,
            Some(_) => Self::UnresolvedEvaluationIncomplete,
        }
    }

    fn v2(request: &PlanningRequest, basis: Option<&MissionBasis>, evaluated_at: i64) -> Self {
        let Some(basis) = basis else {
            return Self::UnresolvedMissionBasis;
        };
        if request.expected_mission_revision != basis.revision {
            return Self::RefusedRevisionMismatch;
        }
        if !request.current_authority_confirmed {
            return Self::UnresolvedAuthorityUnconfirmed;
        }
        let Some(scope) = &basis.scope else {
            // Unreachable for a validated v2 contract; fail nonpositive.
            return Self::UnresolvedMissionBasis;
        };
        Self::scope_window(request, scope, basis, evaluated_at)
    }

    fn scope_window(
        request: &PlanningRequest,
        scope: &MissionScope,
        basis: &MissionBasis,
        evaluated_at: i64,
    ) -> Self {
        if request.purpose_ref != scope.goal_ref {
            return Self::RefusedPurposeMismatch;
        }
        // Exclusion wins even when an asset is also included.
        if scope.excluded_assets.contains(&request.asset_ref) {
            return Self::RefusedAssetExcluded;
        }
        if !scope.included_assets.contains(&request.asset_ref) {
            return Self::RefusedAssetUnknown;
        }
        if evaluated_at < basis.starts_at {
            return Self::RefusedNotYetValid;
        }
        if evaluated_at >= basis.ends_at {
            return Self::RefusedExpired;
        }
        Self::UnresolvedEvaluationIncomplete
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
            1 | 2 => self.validate_legacy_contract()?,
            _ => self.validate_withdrawal_source()?,
        }
        Ok(())
    }

    fn contract_header_invalid(&self) -> bool {
        self.producer != "mission"
            || self.kind != "planning_assessed"
            || !matches!(self.version, 1..=3)
    }

    fn validate_legacy_contract(&self) -> Res<()> {
        if self.owner_revision != 1 || self.withdrawal.is_some() {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        if let Some(basis) = &self.basis {
            basis.validate()?;
            // A present basis carries a scope snapshot exactly on version 2;
            // v1 records keep the old wire shape and v2 never downscopes.
            let coherent = matches!(
                (self.version, basis.scope.is_some()),
                (1, false) | (2, true)
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

    fn expected_decision(&self) -> Res<NonpositiveDecision> {
        match self.version {
            3 => Ok(NonpositiveDecision::RefusedAuthorityWithdrawn),
            _ => NonpositiveDecision::for_request(
                self.version,
                &self.request,
                self.basis.as_ref(),
                self.evaluated_at,
            ),
        }
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

    /// Historical (scope, window) result labels describing the original
    /// assessment time. Version 1 never evaluated scope or window; for v2 the
    /// labels follow which guards the stored decision reached.
    pub fn assessment_labels(&self) -> (&'static str, &'static str) {
        use NonpositiveDecision as D;
        if self.version != 2 {
            return ("not_evaluated", "not_evaluated");
        }
        match self.decision {
            D::RefusedPurposeMismatch => ("purpose_mismatch", "not_evaluated"),
            D::RefusedAssetExcluded => ("excluded", "not_evaluated"),
            D::RefusedAssetUnknown => ("unknown", "not_evaluated"),
            D::RefusedNotYetValid => ("matched", "not_yet_valid"),
            D::RefusedExpired => ("matched", "expired"),
            D::UnresolvedEvaluationIncomplete => ("matched", "within_window"),
            _ => ("not_evaluated", "not_evaluated"),
        }
    }

    pub fn corresponds_to(&self, request: &PlanningRequest, operation_id: OperationId) -> Res<()> {
        self.validate()?;
        if &self.request != request || self.operation_id != operation_id {
            return Err(Fail::Conflict("integrity_conflict"));
        }
        Ok(())
    }
}

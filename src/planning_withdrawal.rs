//! Version-3 recorded-withdrawal refusal: the domain type, its narrow
//! constructor and source validation. A v3 `PlanningAssessed` is the durable
//! refusal of a fresh assessment once a committed, scoped mission-authority
//! withdrawal dominates it; it never grants eligibility or permission and
//! cannot represent any other decision.

use crate::mission::{EventId, OperationId};
use crate::planning::{
    AssessmentTimeBasis, MissionBasis, NonpositiveDecision, PlanningAssessed, PlanningRequest,
};
use crate::withdrawal::MissionAuthorityWithdrawn;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

/// Strict typed reference to the recorded withdrawal that grounded a v3
/// refusal. Both identities are required and never inferred from a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalBasis {
    pub operation_id: OperationId,
    pub event_id: EventId,
}

impl WithdrawalBasis {
    fn validate(&self) -> Res<()> {
        if self.operation_id.0.is_nil() || self.event_id.0.is_nil() {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        Ok(())
    }
}

impl PlanningAssessed {
    /// Version-3 constructor: a fresh assessment recorded as a durable refusal
    /// because a committed, scoped mission-authority withdrawal dominates it.
    /// The validated withdrawal contract must bind to this request and
    /// registration basis; the marker's engagement/campaign and registration
    /// operation are pinned here, never inferred from request contents.
    pub fn new_withdrawn(
        request: PlanningRequest,
        basis: MissionBasis,
        withdrawal: MissionAuthorityWithdrawn,
        operation_id: OperationId,
        event_id: EventId,
        timestamp: i64,
    ) -> Res<Self> {
        if withdrawal.request.engagement_id != request.engagement_id
            || withdrawal.request.campaign_id != request.campaign_id
            || withdrawal.registration_operation_id != basis.registration_operation_id
        {
            return Err(Fail::Unresolved("scope_violation"));
        }
        let event = Self {
            event_id,
            operation_id,
            engagement_id: request.engagement_id,
            campaign_id: request.campaign_id,
            producer: "mission".into(),
            kind: "planning_assessed".into(),
            version: 3,
            affected_entity: operation_id,
            owner_revision: 2,
            causation_id: operation_id,
            correlation_id: operation_id,
            request,
            basis: Some(basis),
            withdrawal: Some(WithdrawalBasis {
                operation_id: withdrawal.operation_id,
                event_id: withdrawal.event_id,
            }),
            decision: NonpositiveDecision::RefusedAuthorityWithdrawn,
            evaluated_at: timestamp,
            occurred_at: timestamp,
            recorded_at: timestamp,
            time_basis: AssessmentTimeBasis::ProducerTransactionStart,
        };
        event.validate()?;
        Ok(event)
    }

    /// Version-3 source validation: owner_revision 2, the sole withdrawn
    /// decision, a present non-nil withdrawal reference and a valid scoped
    /// registration basis. Called from the common envelope validation.
    pub(super) fn validate_withdrawal_source(&self) -> Res<()> {
        if self.owner_revision != 2
            || self.version != 3
            || self.decision != NonpositiveDecision::RefusedAuthorityWithdrawn
        {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        self.validate_withdrawal_basis()?;
        self.validate_scoped_basis()
    }

    fn validate_withdrawal_basis(&self) -> Res<()> {
        self.withdrawal
            .as_ref()
            .ok_or(Fail::Unresolved("unsupported_contract"))?
            .validate()
    }

    fn validate_scoped_basis(&self) -> Res<()> {
        let basis = self
            .basis
            .as_ref()
            .ok_or(Fail::Unresolved("unsupported_contract"))?;
        basis.validate()?;
        if basis.scope.is_none() {
            return Err(Fail::Unresolved("unsupported_contract"));
        }
        Ok(())
    }
}

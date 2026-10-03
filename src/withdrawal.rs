//! Mission-owned restrictive intent and immutable withdrawal history contracts.
use crate::mission::{CampaignId, EngagementId, EventId, OperationId, OperatorRef};
use crate::registration::OperationAllocator;
use crate::trajectory::Delivered;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

pub const OBLIGATION: &str = "trajectory.withdrawal_history.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalReason {
    OperatorRequested,
    AuthorizationEnded,
    ScopeConcern,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalRequest {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: u64,
    pub reason: WithdrawalReason,
}

impl WithdrawalRequest {
    pub fn validate(&self) -> Res<()> {
        if [
            self.engagement_id.0,
            self.campaign_id.0,
            self.operator_ref.0,
        ]
        .iter()
        .any(uuid::Uuid::is_nil)
        {
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
pub struct MissionAuthorityWithdrawn {
    pub event_id: EventId,
    pub operation_id: OperationId,
    pub request: WithdrawalRequest,
    pub registration_operation_id: OperationId,
    pub producer: String,
    pub affected_entity: CampaignId,
    pub owner_revision: u64,
    pub kind: String,
    pub version: u32,
    pub causation_id: OperationId,
    pub correlation_id: OperationId,
    pub occurred_at: i64,
    pub recorded_at: i64,
}

impl MissionAuthorityWithdrawn {
    pub fn new(
        request: WithdrawalRequest,
        operation: OperationId,
        registration: OperationId,
        event_id: EventId,
        timestamp: i64,
    ) -> Res<Self> {
        let event = Self {
            event_id,
            operation_id: operation,
            registration_operation_id: registration,
            affected_entity: request.campaign_id,
            request,
            producer: "mission".into(),
            owner_revision: 2,
            kind: "mission_authority_withdrawn".into(),
            version: 1,
            causation_id: operation,
            correlation_id: operation,
            occurred_at: timestamp,
            recorded_at: timestamp,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn validate(&self) -> Res<()> {
        self.request.validate()?;
        if [
            self.event_id.0,
            self.operation_id.0,
            self.registration_operation_id.0,
        ]
        .iter()
        .any(uuid::Uuid::is_nil)
        {
            return Err(Fail::Input("nil_identity"));
        }
        if (
            self.producer.as_str(),
            self.kind.as_str(),
            self.version,
            self.owner_revision,
            self.request.expected_mission_revision,
        ) != ("mission", "mission_authority_withdrawn", 1, 2, 1)
        {
            return Err(Fail::State("unsupported_contract"));
        }
        if !self.bound_consistently() {
            return Err(Fail::State("invalid_header"));
        }
        Ok(())
    }

    /// The event binds exactly to its scope, own operation and one timestamp.
    fn bound_consistently(&self) -> bool {
        self.affected_entity == self.request.campaign_id
            && self.causation_id == self.operation_id
            && self.correlation_id == self.operation_id
            && self.occurred_at == self.recorded_at
    }
}

pub trait WithdrawalStore {
    fn withdraw(
        &mut self,
        request: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
        allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<MissionAuthorityWithdrawn>>;
}

pub trait WithdrawalHistoryPort {
    fn publish(&mut self, event: &MissionAuthorityWithdrawn) -> Res<Delivered>;
    fn inspect(&mut self, event: &MissionAuthorityWithdrawn) -> Res<Delivered>;
}

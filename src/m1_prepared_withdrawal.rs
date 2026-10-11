//! Prepared-only Broker coordination; Mission still owns withdrawal and publication.
use crate::m1_session::{SessionRequest, original_source};
use crate::mission::{CampaignId, EngagementId, MissionRegistered, OperationId};
use crate::registration::MissionStore;
use crate::withdrawal::{MissionAuthorityWithdrawn, WithdrawalRequest};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedWithdrawalRequest {
    pub withdrawal: WithdrawalRequest,
    pub session_operation_id: OperationId,
    pub expected_session_generation: i64,
}

impl PreparedWithdrawalRequest {
    pub fn validate(&self, op: OperationId) -> Res<()> {
        self.withdrawal.validate()?;
        if self.withdrawal.expected_mission_revision != 1
            || self.expected_session_generation <= 0
            || self.session_operation_id.0.is_nil()
            || op.0.is_nil()
            || op == self.session_operation_id
        {
            return Err(Fail::Input("invalid_prepared_withdrawal"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedWithdrawalRecord {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub session_operation_id: OperationId,
    pub generation: i64,
    pub writer_oid: u32,
    pub event: MissionAuthorityWithdrawn,
}

impl PreparedWithdrawalRecord {
    fn checked(self, r: &PreparedWithdrawalRequest, op: OperationId) -> Res<Self> {
        self.event
            .validate()
            .map_err(|_| Fail::Unresolved("prepared_withdrawal_decode"))?;
        let identity = (
            self.engagement_id,
            self.campaign_id,
            self.operation_id,
            self.session_operation_id,
            self.generation,
        );
        if identity
            != (
                r.withdrawal.engagement_id,
                r.withdrawal.campaign_id,
                op,
                r.session_operation_id,
                r.expected_session_generation,
            )
            || self.writer_oid == 0
            || self.event.operation_id != op
            || self.event.request != r.withdrawal
        {
            return Err(Fail::Unresolved("prepared_withdrawal_decode"));
        }
        Ok(self)
    }
}

pub trait PreparedWithdrawalStore {
    fn execute(
        &mut self,
        request: &PreparedWithdrawalRequest,
        operation: OperationId,
        recover: bool,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<PreparedWithdrawalRecord>>;
}

/// The original source is obtained through Mission's validated public port.
/// Recovery obtains no new source, allocates nothing and changes no authority.
pub fn withdraw_prepared(
    store: &mut impl PreparedWithdrawalStore,
    reader: Option<&mut impl MissionStore>,
    request: &PreparedWithdrawalRequest,
    operation: OperationId,
    recover: bool,
) -> Res<Option<PreparedWithdrawalRecord>> {
    request.validate(operation)?;
    let source = if recover {
        None
    } else {
        let r = SessionRequest {
            engagement_id: request.withdrawal.engagement_id,
            campaign_id: request.withdrawal.campaign_id,
            operator_ref: request.withdrawal.operator_ref,
            expected_mission_revision: 1,
        };
        Some(original_source(
            reader.ok_or(Fail::Config("session_reader_required"))?,
            &r,
        )?)
    };
    store
        .execute(request, operation, recover, source.as_ref())?
        .map(|record| record.checked(request, operation))
        .transpose()
}

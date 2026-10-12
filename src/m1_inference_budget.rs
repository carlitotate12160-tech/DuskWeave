//! Broker-owned pre-send capacity reservation. No request bytes or transport grant.
use crate::mission::{
    CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, MissionRegistered, OperationId,
    OperatorRef,
};
use crate::registration::MissionStore;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservationRequest {
    pub version: String,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: i64,
    pub permission_id: Uuid,
    pub goal_ref: GoalRef,
    pub exercise_mode: ExerciseMode,
}

impl ReservationRequest {
    pub fn validate(&self, op: OperationId) -> Res<()> {
        if self.version != "m1_inference_reservation_request_v1"
            || self.expected_mission_revision != 1
            || [
                self.engagement_id.0,
                self.campaign_id.0,
                self.operator_ref.0,
                self.permission_id,
                self.goal_ref.0,
                op.0,
            ]
            .contains(&Uuid::nil())
        {
            return Err(Fail::Input("invalid_inference_reservation"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum ReservationAction {
    Reserve,
    Recover,
    Cancel,
}
impl ReservationAction {
    pub fn name(self) -> &'static str {
        match self {
            Self::Reserve => "reserve",
            Self::Recover => "recover",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservedCapacity {
    pub calls: i64,
    pub request_bytes: i64,
    pub response_bytes: i64,
    pub output_token_capacity: i64,
    pub wall_time_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservationRecord {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub generation: i64,
    pub record_revision: i64,
    pub writer_oid: String,
    pub request: ReservationRequest,
    pub registration_operation_id: OperationId,
    pub registration_event_id: EventId,
    pub generation_call_id: Uuid,
    pub repair_call_id: Uuid,
    pub capacity: ReservedCapacity,
    pub kind: String,
    pub recorded_at: String,
}

impl ReservationRecord {
    fn check(&self, request: &ReservationRequest, op: OperationId) -> Res<()> {
        if self.request != *request
            || self.operation_id != op
            || (self.engagement_id, self.campaign_id)
                != (request.engagement_id, request.campaign_id)
            || self.generation <= 0
            || !self.writer_oid.parse::<u32>().is_ok_and(|n| n > 0)
            || !matches!(
                (self.kind.as_str(), self.record_revision),
                ("reserved_not_sent", 1) | ("cancelled_never_sent", 2)
            )
        {
            return Err(Fail::Unresolved("inference_record_decode"));
        }
        self.check_capacity()
    }

    fn check_capacity(&self) -> Res<()> {
        let c = &self.capacity;
        if (c.calls, c.output_token_capacity) != (2, 8192)
            || [
                (c.request_bytes, 32768),
                (c.response_bytes, 131072),
                (c.wall_time_ms, 20000),
            ]
            .iter()
            .any(|(n, cap)| *n <= 0 || n > cap)
            || [
                self.registration_operation_id.0,
                self.registration_event_id.0,
                self.generation_call_id,
                self.repair_call_id,
            ]
            .contains(&Uuid::nil())
            || self.generation_call_id == self.repair_call_id
        {
            return Err(Fail::Unresolved("inference_record_decode"));
        }
        Ok(())
    }
}

pub trait InferenceReservationStore {
    fn execute(
        &mut self,
        action: ReservationAction,
        request: &ReservationRequest,
        op: OperationId,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<ReservationRecord>>;
}

/// Reads the public immutable Mission publication only for reserve. SQL rechecks
/// current authority after ordering; recovery/cancel never require renewed consent.
pub fn reservation(
    store: &mut impl InferenceReservationStore,
    reader: Option<&mut impl MissionStore>,
    action: ReservationAction,
    request: &ReservationRequest,
    op: OperationId,
) -> Res<Option<ReservationRecord>> {
    request.validate(op)?;
    let source = matches!(action, ReservationAction::Reserve)
        .then(|| {
            publication(
                reader.ok_or(Fail::State("inference_authority_unavailable"))?,
                request,
            )
        })
        .transpose()?;
    let record = store.execute(action, request, op, source.as_ref())?;
    if record.is_none() && action.name() != "recover" {
        return Err(Fail::Unresolved("inference_record_decode"));
    }
    record.map(|r| r.check(request, op).map(|()| r)).transpose()
}

fn publication(reader: &mut impl MissionStore, q: &ReservationRequest) -> Res<MissionRegistered> {
    let view = reader
        .mission_view(q.engagement_id, q.campaign_id)?
        .ok_or(Fail::State("mission_missing"))?;
    let event = reader
        .outbox_event(q.engagement_id, q.campaign_id, view.operation_id)?
        .ok_or(Fail::Store("contract_decode"))?;
    if event.fields.m1_inference_permission().is_none() {
        return Err(Fail::State("inference_consent_absent"));
    }
    Ok(event)
}

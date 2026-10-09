//! Execution-owned prepared/no-effects session fence: typed requests,
//! lifecycle records and the narrow fence port. Composition reads the
//! current public Mission source through the MissionStore port and hands a
//! fully validated claim to the guarded SQL adapter. A prepared session
//! grants no current permission, dispatch, acquisition or host-control
//! qualification, and historical records never become a reusable permit.

use crate::mission::{
    CampaignId, EngagementId, EventId, MissionRegistered, OperationId, OperatorRef,
};
use crate::registration::{MissionStore, MissionView};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

/// Fixed lifecycle vocabulary of the execution session ledger.
pub const KINDS: &[&str] = &["prepared_no_effects", "released_no_effects"];

/// Fixed refusal vocabulary of the guarded SQL functions; any other server
/// text decodes to the conservative unrecognized reason.
pub const REFUSALS: &[&str] = &[
    "already_claimed",
    "writer_mismatch",
    "operator_mismatch",
    "source_conflict",
    "window_closed",
    "permission_absent",
    "mission_missing",
    "withdrawn",
    "stale_revision",
    "operation_reuse",
    "not_owner",
    "no_prepared_claim",
    "not_authorized",
];

pub fn decode_refusal(raw: &str) -> &'static str {
    REFUSALS
        .iter()
        .find(|reason| **reason == raw)
        .copied()
        .unwrap_or("unrecognized_refusal")
}

/// One prepare/recover/release request. The expected Mission revision is
/// exactly 1: this slice fences only the original registered generation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: u64,
}

impl SessionRequest {
    pub fn validate(&self) -> Res<()> {
        if self.engagement_id.0.is_nil()
            || self.campaign_id.0.is_nil()
            || self.operator_ref.0.is_nil()
        {
            return Err(Fail::Input("nil_reference"));
        }
        if self.expected_mission_revision != 1 {
            return Err(Fail::Input("invalid_revision"));
        }
        Ok(())
    }
}

/// One safe durable session-history record. Everything here is a scoped
/// identifier, counter or category; no policy payload, endpoint, credential
/// or proof content is retained.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct SessionRecord {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: u64,
    pub registration_operation_id: OperationId,
    pub registration_event_id: EventId,
    pub generation: u64,
    pub writer_oid: u32,
    pub kind: String,
    pub recorded_at: i64,
}

fn has_nil_identity(record: &SessionRecord) -> bool {
    record.engagement_id.0.is_nil()
        || record.campaign_id.0.is_nil()
        || record.operation_id.0.is_nil()
        || record.operator_ref.0.is_nil()
        || record.registration_operation_id.0.is_nil()
        || record.registration_event_id.0.is_nil()
}

impl SessionRecord {
    /// Decode and validate one fenced history record: fixed lifecycle
    /// vocabulary, revision 1, positive generation and writer, non-nil
    /// identities. Invalid records never decode into a grant.
    pub fn decode(value: serde_json::Value) -> Res<Self> {
        let record: Self =
            serde_json::from_value(value).map_err(|_| Fail::Store("contract_decode"))?;
        if !KINDS.contains(&record.kind.as_str())
            || record.expected_mission_revision != 1
            || record.generation == 0
            || record.writer_oid == 0
            || has_nil_identity(&record)
        {
            return Err(Fail::Store("contract_decode"));
        }
        Ok(record)
    }

    /// The record must name exactly the requested scope, operation, operator
    /// and revision and, when claimed, the validated registration identity.
    pub fn binds(
        &self,
        request: &SessionRequest,
        operation: OperationId,
        registration: Option<OperationId>,
    ) -> bool {
        self.engagement_id == request.engagement_id
            && self.campaign_id == request.campaign_id
            && self.operation_id == operation
            && self.operator_ref == request.operator_ref
            && self.expected_mission_revision == request.expected_mission_revision
            && registration.is_none_or(|r| r == self.registration_operation_id)
    }
}

/// Categorical fence outcome. `Unknown` marks a post-mutation decode or
/// binding ambiguity: the server may already have committed, so recovery
/// must confirm the durable state before any explicit retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceOutcome {
    Durable(SessionRecord),
    Missing,
    Refused(&'static str),
    Unknown,
}

/// The validated claim handed to the guarded SQL prepare function: exact
/// scope, session operation, bound operator, expected revision, the original
/// registration operation and the exact validated contract JSON.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionClaim {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: u64,
    pub registration_operation_id: OperationId,
    pub contract: serde_json::Value,
}

/// Narrow execution session-fence port. Implementations are guarded SQL
/// adapters only; no caller mutates fence state directly.
pub trait SessionFence {
    fn prepare(&mut self, claim: &SessionClaim) -> Res<FenceOutcome>;
    fn release(&mut self, request: &SessionRequest, operation: OperationId) -> Res<FenceOutcome>;
    fn recover(&mut self, request: &SessionRequest, operation: OperationId) -> Res<FenceOutcome>;
}

fn unix_now() -> Res<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|_| Fail::State("clock_unavailable"))
}

fn ensure_operation(operation: OperationId) -> Res<()> {
    if operation.0.is_nil() {
        Err(Fail::Input("nil_identity"))
    } else {
        Ok(())
    }
}

/// One public-Mission-source binding: the requesting operator must own the
/// current Mission, an M1 permission attachment must exist and both
/// half-open Mission and permission windows must be current at `now`.
fn check_source(
    event: &MissionRegistered,
    request: &SessionRequest,
    mission: &MissionView,
    now: i64,
) -> Option<&'static str> {
    if event.fields.operator_ref != request.operator_ref {
        return Some("operator_mismatch");
    }
    let Some(permission) = event.fields.m1_permission() else {
        return Some("permission_absent");
    };
    let summary = permission.summary(&event.fields);
    let in_mission = mission.starts_at <= now && now < mission.ends_at;
    let in_permission = summary.starts_at <= now && now < summary.ends_at;
    if in_mission && in_permission {
        None
    } else {
        Some("window_closed")
    }
}

fn bound(
    record: SessionRecord,
    request: &SessionRequest,
    operation: OperationId,
    registration: Option<OperationId>,
) -> FenceOutcome {
    if record.binds(request, operation, registration) {
        FenceOutcome::Durable(record)
    } else {
        FenceOutcome::Unknown
    }
}

/// Claim the prepared/no-effects session fence through the current public
/// Mission source. The SQL function re-verifies every premise under the
/// scope fence before a fresh claim; the composition read alone is never
/// current authority.
pub fn prepare(
    store: &mut impl MissionStore,
    fence: &mut impl SessionFence,
    request: &SessionRequest,
    operation: OperationId,
) -> Res<FenceOutcome> {
    request.validate()?;
    ensure_operation(operation)?;
    let now = unix_now()?;
    let mission = store
        .mission_view(request.engagement_id, request.campaign_id)?
        .ok_or(Fail::State("mission_missing"))?;
    let event = store
        .outbox_event(
            request.engagement_id,
            request.campaign_id,
            mission.operation_id,
        )?
        .ok_or(Fail::State("mission_missing"))?;
    if let Some(reason) = check_source(&event, request, &mission, now) {
        return Ok(FenceOutcome::Refused(reason));
    }
    let claim = SessionClaim {
        engagement_id: request.engagement_id,
        campaign_id: request.campaign_id,
        operation_id: operation,
        operator_ref: request.operator_ref,
        expected_mission_revision: request.expected_mission_revision,
        registration_operation_id: event.operation_id,
        contract: serde_json::to_value(&event).map_err(|_| Fail::Store("encode"))?,
    };
    match fence.prepare(&claim)? {
        FenceOutcome::Durable(record) => Ok(bound(
            record,
            request,
            operation,
            Some(claim.registration_operation_id),
        )),
        outcome => Ok(outcome),
    }
}

/// Release the caller's own exact current prepared generation. Only the
/// original op/operator/revision/login releases; an already released
/// operation returns its original released record and clears nothing.
pub fn release(
    fence: &mut impl SessionFence,
    request: &SessionRequest,
    operation: OperationId,
) -> Res<FenceOutcome> {
    request.validate()?;
    ensure_operation(operation)?;
    match fence.release(request, operation)? {
        FenceOutcome::Durable(record) => Ok(bound(record, request, operation, None)),
        outcome => Ok(outcome),
    }
}

/// Read-only recovery by original scope/op. It never claims, transfers,
/// increments or dispatches; a non-binding record is a decode failure, and
/// transport/commit ambiguity stays with the caller's recovery duty.
pub fn recover(
    fence: &mut impl SessionFence,
    request: &SessionRequest,
    operation: OperationId,
) -> Res<FenceOutcome> {
    request.validate()?;
    ensure_operation(operation)?;
    match fence.recover(request, operation)? {
        FenceOutcome::Durable(record) if !record.binds(request, operation, None) => {
            Err(Fail::Store("contract_decode"))
        }
        outcome => Ok(outcome),
    }
}

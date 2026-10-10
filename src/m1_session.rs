//! Broker-owned prepared/no-effects session contract. A durable database
//! fence is neither a host START grant nor qualified current effect
//! admission: this layer only validates the request, binds the original
//! Mission source through the public port and verifies the returned record
//! against the same identity.
use crate::mission::{
    CampaignId, EngagementId, EventId, MissionRegistered, OperationId, OperatorRef,
};
use crate::registration::MissionStore;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: i64,
}

impl SessionRequest {
    pub fn validate(&self, op: OperationId) -> Res<()> {
        if [
            self.engagement_id.0,
            self.campaign_id.0,
            self.operator_ref.0,
            op.0,
        ]
        .contains(&uuid::Uuid::nil())
            || self.expected_mission_revision != 1
        {
            return Err(Fail::Input("invalid_session_request"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum SessionAction {
    Prepare,
    Recover,
    Release,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRecord {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: i64,
    pub registration_operation_id: OperationId,
    pub registration_event_id: EventId,
    pub generation: i64,
    /// `to_jsonb` renders `oid` as text; accept either JSON shape.
    #[serde(deserialize_with = "oid_json")]
    pub writer_oid: u32,
    pub kind: String,
    pub recorded_at: String,
}

fn oid_json<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Num(u32),
        Text(String),
    }
    match Repr::deserialize(d)? {
        Repr::Num(v) => Ok(v),
        Repr::Text(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

pub trait SessionStore {
    fn execute(
        &mut self,
        action: SessionAction,
        request: &SessionRequest,
        op: OperationId,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<SessionRecord>>;
}

impl SessionRecord {
    /// Bind a returned record to the exact request/operation identity and the
    /// fixed lifecycle vocabulary; anything else fails closed as unknown —
    /// a decode failure after mutating SQL can already be committed.
    fn checked(self, r: &SessionRequest, op: OperationId) -> Res<Self> {
        let expected = SessionRequest {
            engagement_id: self.engagement_id,
            campaign_id: self.campaign_id,
            operator_ref: self.operator_ref,
            expected_mission_revision: self.expected_mission_revision,
        };
        let scoped = (expected, self.operation_id) == (r.clone(), op);
        if !scoped
            || self.generation <= 0
            || self.writer_oid == 0
            || self.registration_operation_id.0.is_nil()
            || self.registration_event_id.0.is_nil()
            || !matches!(
                self.kind.as_str(),
                "prepared_no_effects" | "released_no_effects"
            )
        {
            return Err(Fail::Unresolved("session_contract_decode"));
        }
        Ok(self)
    }
}

/// The durable source must be the Mission owner's validated original
/// registration bound to this scope, operation and operator with an M1
/// permission attachment; a corrupt or mismatched durable record refuses
/// without a false claim.
fn check_source(
    event: MissionRegistered,
    r: &SessionRequest,
    original_op: OperationId,
) -> Res<MissionRegistered> {
    crate::trajectory::check_event(&event).map_err(|_| Fail::Store("contract_decode"))?;
    if (event.engagement_id, event.campaign_id, event.operation_id)
        != (r.engagement_id, r.campaign_id, original_op)
    {
        return Err(Fail::Store("contract_decode"));
    }
    if event.fields.operator_ref != r.operator_ref || event.fields.m1_permission().is_none() {
        return Err(Fail::State("session_prepare_refused"));
    }
    Ok(event)
}

pub(crate) fn original_source(
    reader: &mut impl MissionStore,
    r: &SessionRequest,
) -> Res<MissionRegistered> {
    let view = reader
        .mission_view(r.engagement_id, r.campaign_id)?
        .ok_or(Fail::State("session_mission_missing"))?;
    let event = reader
        .outbox_event(r.engagement_id, r.campaign_id, view.operation_id)?
        .ok_or(Fail::Store("contract_decode"))?;
    check_source(event, r, view.operation_id)
}

pub fn session(
    store: &mut impl SessionStore,
    reader: Option<&mut impl MissionStore>,
    action: SessionAction,
    request: &SessionRequest,
    op: OperationId,
) -> Res<Option<SessionRecord>> {
    request.validate(op)?;
    let source = match action {
        SessionAction::Prepare => Some(original_source(
            reader.ok_or(Fail::Config("session_reader_required"))?,
            request,
        )?),
        _ => None,
    };
    let record = store.execute(action, request, op, source.as_ref())?;
    record.map(|record| record.checked(request, op)).transpose()
}

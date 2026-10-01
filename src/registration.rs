//! Application layer: narrow ports and the M0A use cases.
//! Composition may hold these ports; it never holds both mutable aggregates.

use crate::mission::*;
use crate::trajectory::{Delivered, HistoryStatus};
use crate::{Fail, Res};

pub trait OperationAllocator {
    fn allocate(&mut self) -> Res<uuid::Uuid>;
}

pub trait MissionStore {
    fn commit_registration(
        &mut self,
        mission: &Mission,
        event: &MissionRegistered,
    ) -> Res<CommitEffect>;
    fn mission_view(&mut self, e: EngagementId, c: CampaignId) -> Res<Option<MissionView>>;
    fn outbox_event(
        &mut self,
        e: EngagementId,
        c: CampaignId,
        op: OperationId,
    ) -> Res<Option<MissionRegistered>>;
}

pub trait TrajectoryPort {
    fn deliver(&mut self, event: &MissionRegistered) -> Res<Delivered>;
    fn status(&mut self, e: EngagementId, c: CampaignId, ev: EventId) -> Res<HistoryStatus>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitEffect {
    Fresh,
    Existing(Box<MissionRegistered>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissionView {
    pub operation_id: OperationId,
    pub revision: u64,
    pub exercise_mode: ExerciseMode,
    pub starts_at: i64,
    pub ends_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Receipt {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub event_id: EventId,
    pub history: HistoryStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectView {
    pub mission: MissionView,
    pub event_id: Option<EventId>,
    pub history: HistoryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileOutcome {
    Committed,
    NotCommitted,
    Conflicted,
}

fn unix_now() -> Res<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|_| Fail::State("clock_unavailable"))
}

pub fn prepare_operation(a: &mut impl OperationAllocator) -> Res<OperationId> {
    Ok(OperationId(a.allocate()?))
}

pub fn register(
    alloc: &mut impl OperationAllocator,
    store: &mut impl MissionStore,
    traj: &mut impl TrajectoryPort,
    operation_id: OperationId,
    input: &RegistrationInput,
) -> Res<Receipt> {
    let event_id = EventId(alloc.allocate()?);
    let (mission, event) = Mission::register(input, operation_id, event_id, unix_now()?)?;
    let committed = match store.commit_registration(&mission, &event)? {
        CommitEffect::Fresh => event,
        CommitEffect::Existing(stored) => *stored,
    };
    let history = match traj.deliver(&committed) {
        Ok(Delivered::Completed | Delivered::Duplicate) => HistoryStatus::Completed,
        Ok(Delivered::Anomaly) => HistoryStatus::Anomaly,
        Ok(Delivered::Unresolved(_)) | Err(_) => HistoryStatus::Pending,
    };
    Ok(Receipt {
        engagement_id: input.engagement_id,
        campaign_id: input.campaign_id,
        operation_id,
        event_id: committed.event_id,
        history,
    })
}

pub fn inspect(
    store: &mut impl MissionStore,
    traj: &mut impl TrajectoryPort,
    e: EngagementId,
    c: CampaignId,
) -> Res<Option<InspectView>> {
    let Some(mission) = store.mission_view(e, c)? else {
        return Ok(None);
    };
    let (event_id, history) = match store.outbox_event(e, c, mission.operation_id)? {
        Some(ev) => (Some(ev.event_id), traj.status(e, c, ev.event_id)?),
        None => (None, HistoryStatus::Pending),
    };
    Ok(Some(InspectView {
        mission,
        event_id,
        history,
    }))
}

pub fn reconcile(
    store: &mut impl MissionStore,
    traj: &mut impl TrajectoryPort,
    e: EngagementId,
    c: CampaignId,
    op: OperationId,
) -> Res<ReconcileOutcome> {
    let Some(ev) = store.outbox_event(e, c, op)? else {
        return Ok(ReconcileOutcome::NotCommitted);
    };
    match traj.status(e, c, ev.event_id)? {
        HistoryStatus::Completed => Ok(ReconcileOutcome::Committed),
        HistoryStatus::Anomaly => Ok(ReconcileOutcome::Conflicted),
        HistoryStatus::Pending => match traj.deliver(&ev)? {
            Delivered::Completed | Delivered::Duplicate => Ok(ReconcileOutcome::Committed),
            Delivered::Anomaly => Ok(ReconcileOutcome::Conflicted),
            Delivered::Unresolved(cat) => Err(Fail::Unresolved(cat)),
        },
    }
}

//! Bounded publication/recovery of an already durable Mission decision.

use crate::mission::OperationId;
use crate::planning::{PlanningAssessed, PlanningRequest};
use crate::planning_assessment::{self, PlanningStore};
use crate::registration::OperationAllocator;
use crate::trajectory::Delivered;
use crate::{Fail, Res};

pub trait PlanningHistoryPort {
    fn publish(&mut self, event: &PlanningAssessed) -> Res<Delivered>;
    fn inspect(&mut self, event: &PlanningAssessed) -> Res<Delivered>;
}

struct NoAllocation;

impl OperationAllocator for NoAllocation {
    fn allocate(&mut self) -> Res<uuid::Uuid> {
        Err(Fail::State("recovery_allocation_forbidden"))
    }
}

pub fn read_decision(
    store: &mut impl PlanningStore,
    request: &PlanningRequest,
    operation: OperationId,
) -> Res<Option<PlanningAssessed>> {
    planning_assessment::assess(&mut NoAllocation, store, request, operation, true)
}

#[derive(Debug, PartialEq, Eq)]
pub struct HistoryView {
    pub state: &'static str,
    pub reason: &'static str,
    pub complete: bool,
}

pub fn history_result(outcome: Res<Delivered>) -> HistoryView {
    let (state, reason) = match outcome {
        Ok(Delivered::Completed | Delivered::Duplicate) => ("completed", "recorded"),
        Ok(Delivered::Anomaly) => ("anomaly", "conflicting_identity"),
        Ok(Delivered::Unresolved(reason)) => ("pending", reason),
        Err(Fail::Store("commit_unknown")) => ("unknown", "consumer_commit_unknown"),
        Err(Fail::Store("serialization_retry")) => ("unknown", "serialization_retry"),
        Err(_) => ("unknown", "history_unavailable"),
    };
    HistoryView {
        state,
        reason,
        complete: state == "completed",
    }
}

pub fn history_view(
    consumer: &mut impl PlanningHistoryPort,
    event: &PlanningAssessed,
    recover: bool,
) -> HistoryView {
    let outcome = event.validate().and_then(|()| {
        if recover {
            consumer.inspect(event)
        } else {
            consumer.publish(event)
        }
    });
    history_result(outcome)
}

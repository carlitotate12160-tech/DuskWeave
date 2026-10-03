//! Bounded producer application; delivery is a later, separately owned slice.

use crate::mission::OperationId;
use crate::planning::{PlanningAssessed, PlanningRequest};
use crate::registration::OperationAllocator;
use crate::{Fail, Res};

pub trait PlanningStore {
    fn assess(
        &mut self,
        request: &PlanningRequest,
        operation_id: OperationId,
        recover: bool,
        allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<PlanningAssessed>>;

    /// Conservative owner query: does this store prove a recorded
    /// mission-authority withdrawal for the request scope? The default is
    /// Ok(false), so an unimplemented or unreadable store can never bypass
    /// the fresh-confirmation exchange. A true result authorizes refusal only.
    fn authority_withdrawn(&mut self, _request: &PlanningRequest) -> Res<bool> {
        Ok(false)
    }
}

pub fn assess(
    allocator: &mut dyn OperationAllocator,
    store: &mut impl PlanningStore,
    request: &PlanningRequest,
    operation_id: OperationId,
    recover: bool,
) -> Res<Option<PlanningAssessed>> {
    request.validate()?;
    if operation_id.0.is_nil() {
        return Err(Fail::Input("nil_identity"));
    }
    let event = store.assess(request, operation_id, recover, allocator)?;
    if let Some(event) = &event {
        event.corresponds_to(request, operation_id)?;
    }
    Ok(event)
}

//! Fresh planning-assessment phases of the Mission producer's SERIALIZABLE
//! transaction. The parent owns request validation, recovery and the shared
//! row readers; the same mutable transaction flows through every phase and
//! only `assess_new` commits.
use super::{prior_assessment, store_err};
use crate::mission::{EventId, OperationId};
use crate::planning::{PlanningAssessed, PlanningRequest};
use crate::registration::OperationAllocator;
use crate::{Fail, Res};
use postgres::IsolationLevel;

pub(super) fn assess_new(
    client: &mut postgres::Client,
    request: &PlanningRequest,
    operation_id: OperationId,
    allocator: &mut dyn OperationAllocator,
) -> Res<Option<PlanningAssessed>> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::Serializable)
        .start()
        .map_err(|error| store_err(&error))?;
    if let Some(event) = prior_assessment(&mut tx, request, operation_id)? {
        return Ok(Some(event));
    }
    let event = persist_fresh_assessment(&mut tx, request, operation_id, allocator)?;
    tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
    Ok(Some(event))
}

fn persist_fresh_assessment(
    tx: &mut postgres::Transaction,
    request: &PlanningRequest,
    operation_id: OperationId,
    allocator: &mut dyn OperationAllocator,
) -> Res<PlanningAssessed> {
    if tx.query_opt(
        "SELECT 1 FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 \
         UNION ALL SELECT 1 FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 LIMIT 1",
        &[&request.engagement_id.0, &request.campaign_id.0, &operation_id.0],
    ).map_err(|error| store_err(&error))?.is_some() {
        return Err(Fail::Conflict("integrity_conflict"));
    }
    let event = create_assessment(tx, request, operation_id, allocator)?;
    let contract = serde_json::to_value(&event).map_err(|_| Fail::Store("encode"))?;
    tx.execute(
        "INSERT INTO mission.planning_assessments \
         (engagement_id,campaign_id,operation_id,event_id,contract,publication_obligation) \
         VALUES ($1,$2,$3,$4,$5,'trajectory.planning_history.v1')",
        &[
            &request.engagement_id.0,
            &request.campaign_id.0,
            &operation_id.0,
            &event.event_id.0,
            &contract,
        ],
    )
    .map_err(|error| store_err(&error))?;
    Ok(event)
}

fn create_assessment(
    tx: &mut postgres::Transaction,
    request: &PlanningRequest,
    operation_id: OperationId,
    allocator: &mut dyn OperationAllocator,
) -> Res<PlanningAssessed> {
    let basis = super::postgres_mission_basis::current_basis(tx, request)?;
    let timestamp: i64 = tx
        .query_one(
            "SELECT floor(extract(epoch FROM transaction_timestamp()))::bigint",
            &[],
        )
        .map_err(|error| store_err(&error))?
        .get(0);
    let event_id = EventId(allocator.allocate()?);
    PlanningAssessed::new(request.clone(), basis, operation_id, event_id, timestamp)
}

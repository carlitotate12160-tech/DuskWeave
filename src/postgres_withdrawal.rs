//! Mission-local acceptance and the scoped current-marker reader over the
//! immutable withdrawal contract. Both producer reads share one strict codec:
//! the decoded payload is bound to the explicitly projected owner catalog, so
//! corruption is a bounded decode failure and never absence or a conflict.

use super::{PgMissionStore, store_err};
use crate::mission::{EventId, OperationId};
use crate::planning::PlanningRequest;
use crate::registration::OperationAllocator;
use crate::withdrawal::{MissionAuthorityWithdrawn, WithdrawalRequest, WithdrawalStore};
use crate::{Fail, Res};
use postgres::{GenericClient, IsolationLevel, Transaction};

/// Fixed header, obligation and identity projection shared by the duplicate/
/// recovery read and the scoped current-marker read.
const MARKER_PROJECTION: &str = "SELECT contract, owner_revision=2 AND producer='mission' AND version=1      AND kind='mission_authority_withdrawn' AND publication_obligation='trajectory.withdrawal_history.v1',      jsonb_build_object('event_id',event_id,'registration_operation_id',registration_operation_id,      'operation_id',operation_id,      'engagement_id',engagement_id,'campaign_id',campaign_id)      FROM mission.withdrawals";

/// Decode one validated withdrawal contract from a projected marker row and
/// bind it to the explicitly projected owner catalog.
fn decode_marker(row: &postgres::Row) -> Res<MissionAuthorityWithdrawn> {
    let event: MissionAuthorityWithdrawn =
        serde_json::from_value(row.get(0)).map_err(|_| Fail::Store("contract_decode"))?;
    event
        .validate()
        .map_err(|_| Fail::Store("contract_decode"))?;
    // Bind the decoded contract to the explicitly projected owner catalog.
    let catalog = serde_json::json!({
        "event_id": event.event_id, "registration_operation_id": event.registration_operation_id,
        "operation_id": event.operation_id,
        "engagement_id": event.request.engagement_id, "campaign_id": event.request.campaign_id,
    });
    if !row.get::<_, bool>(1) || row.get::<_, serde_json::Value>(2) != catalog {
        return Err(Fail::Store("contract_decode"));
    }
    Ok(event)
}

fn original(
    client: &mut impl GenericClient,
    request: &WithdrawalRequest,
    operation: OperationId,
) -> Res<Option<MissionAuthorityWithdrawn>> {
    let row = client
        .query_opt(
            &format!(
                "{MARKER_PROJECTION} WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3"
            ),
            &[
                &request.engagement_id.0,
                &request.campaign_id.0,
                &operation.0,
            ],
        )
        .map_err(|e| store_err(&e))?;
    row.map(|row| {
        let event = decode_marker(&row)?;
        if event.request != *request {
            return Err(Fail::Conflict("integrity_conflict"));
        }
        Ok(event)
    })
    .transpose()
}

/// Scoped current-marker read for the planning producer: the one validated
/// withdrawal for this engagement/campaign, or None. A different withdrawal
/// operation is never conflated with this scope; malformed rows fail safely,
/// never becoming absence.
pub(super) fn current_marker(
    client: &mut impl GenericClient,
    request: &PlanningRequest,
) -> Res<Option<MissionAuthorityWithdrawn>> {
    let row = client
        .query_opt(
            &format!("{MARKER_PROJECTION} WHERE engagement_id=$1 AND campaign_id=$2"),
            &[&request.engagement_id.0, &request.campaign_id.0],
        )
        .map_err(|e| store_err(&e))?;
    row.map(|row| decode_marker(&row)).transpose()
}

/// One scoped eligibility read: collisions dominate missing/stale registration.
/// The transaction timestamp is fixed; allocation still follows the SSI read.
fn fresh_event(
    tx: &mut Transaction,
    request: &WithdrawalRequest,
    operation: OperationId,
    allocator: &mut dyn OperationAllocator,
) -> Res<MissionAuthorityWithdrawn> {
    let registration = tx.query_one(
        "SELECT m.operation_id,m.revision,floor(extract(epoch FROM transaction_timestamp()))::bigint,          EXISTS(SELECT 1 FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3          UNION ALL SELECT 1 FROM mission.planning_assessments WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3          UNION ALL SELECT 1 FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2)          FROM (SELECT 1) anchor LEFT JOIN mission.missions m ON m.engagement_id=$1 AND m.campaign_id=$2",
        &[&request.engagement_id.0, &request.campaign_id.0, &operation.0],
    ).map_err(|e| store_err(&e))?;
    if registration.get::<_, bool>(3) {
        return Err(Fail::Conflict("integrity_conflict"));
    }
    let registered_operation = registration
        .get::<_, Option<uuid::Uuid>>(0)
        .ok_or(Fail::State("mission_missing"))?;
    if registration.get::<_, Option<i64>>(1) != Some(1) || request.expected_mission_revision != 1 {
        return Err(Fail::State("stale_revision"));
    }
    // SSI dependency pairs with the assessment's scoped withdrawal-absence read.
    tx.query_one("SELECT COUNT(*) FROM mission.planning_assessments WHERE engagement_id=$1 AND campaign_id=$2",
        &[&request.engagement_id.0, &request.campaign_id.0]).map_err(|e| store_err(&e))?;
    allocator.allocate().and_then(|event_id| {
        MissionAuthorityWithdrawn::new(
            request.clone(),
            operation,
            OperationId(registered_operation),
            EventId(event_id),
            registration.get(2),
        )
    })
}

impl WithdrawalStore for PgMissionStore {
    fn withdraw(
        &mut self,
        request: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
        allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        request.validate()?;
        if operation.0.is_nil() {
            return Err(Fail::Input("nil_identity"));
        }
        if recover {
            return original(&mut self.client, request, operation);
        }
        self.client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))
            .and_then(|mut tx| {
                // A committed duplicate leaves without allocation or a new commit.
                if let Some(event) = original(&mut tx, request, operation)? {
                    return Ok(Some(event));
                }
                let event = fresh_event(&mut tx, request, operation, allocator)?;
                let contract = serde_json::to_value(&event).map_err(|_| Fail::Store("encode"))?;
                tx.execute(
                    "INSERT INTO mission.withdrawals (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract)                      VALUES ($1,$2,$3,$4,$5,$6)",
                    &[&request.engagement_id.0, &request.campaign_id.0, &operation.0,
                      &event.event_id.0, &event.registration_operation_id.0, &contract],
                ).map_err(|e| store_err(&e))?;
                tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
                Ok(Some(event))
            })
    }
}

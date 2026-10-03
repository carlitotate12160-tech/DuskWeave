//! Mission-local acceptance: marker, immutable event and obligation commit together.
use super::{PgMissionStore, store_err};
use crate::mission::{EventId, OperationId};
use crate::registration::OperationAllocator;
use crate::withdrawal::{
    MissionAuthorityWithdrawn, OBLIGATION, WithdrawalRequest, WithdrawalStore,
};
use crate::{Fail, Res};
use postgres::{GenericClient, IsolationLevel};

fn original(
    client: &mut impl GenericClient,
    request: &WithdrawalRequest,
    operation: OperationId,
) -> Res<Option<MissionAuthorityWithdrawn>> {
    let row = client.query_opt(
        "SELECT contract,event_id,publication_obligation,registration_operation_id,owner_revision,producer,kind,version \
         FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
        &[&request.engagement_id.0, &request.campaign_id.0, &operation.0],
    ).map_err(|e| store_err(&e))?;
    row.map(|row| {
        let event: MissionAuthorityWithdrawn =
            serde_json::from_value(row.get(0)).map_err(|_| Fail::Store("contract_decode"))?;
        event
            .validate()
            .map_err(|_| Fail::Store("contract_decode"))?;
        let catalog = (
            row.get::<_, uuid::Uuid>(1),
            row.get::<_, &str>(2),
            row.get::<_, uuid::Uuid>(3),
            row.get::<_, i64>(4),
            row.get::<_, &str>(5),
            row.get::<_, &str>(6),
            row.get::<_, i32>(7),
            operation,
            request.engagement_id,
            request.campaign_id,
        );
        if catalog
            != (
                event.event_id.0,
                OBLIGATION,
                event.registration_operation_id.0,
                2,
                "mission",
                "mission_authority_withdrawn",
                1,
                event.operation_id,
                event.request.engagement_id,
                event.request.campaign_id,
            )
        {
            return Err(Fail::Store("contract_decode"));
        }
        if event.request != *request {
            return Err(Fail::Conflict("integrity_conflict"));
        }
        Ok(event)
    })
    .transpose()
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
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
        if let Some(event) = original(&mut tx, request, operation)? {
            return Ok(Some(event));
        }
        if tx.query_opt(
            "SELECT 1 FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 \
             UNION ALL SELECT 1 FROM mission.planning_assessments WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 \
             UNION ALL SELECT 1 FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2 LIMIT 1",
            &[&request.engagement_id.0, &request.campaign_id.0, &operation.0],
        ).map_err(|e| store_err(&e))?.is_some() {
            return Err(Fail::Conflict("integrity_conflict"));
        }
        let registration = tx.query_opt(
            "SELECT operation_id,revision FROM mission.missions WHERE engagement_id=$1 AND campaign_id=$2",
            &[&request.engagement_id.0, &request.campaign_id.0],
        ).map_err(|e| store_err(&e))?.ok_or(Fail::State("mission_missing"))?;
        if registration.get::<_, i64>(1) != 1 || request.expected_mission_revision != 1 {
            return Err(Fail::State("stale_revision"));
        }
        // SSI dependency pairs with the assessment's scoped withdrawal-absence read.
        tx.query_one("SELECT COUNT(*) FROM mission.planning_assessments WHERE engagement_id=$1 AND campaign_id=$2",
            &[&request.engagement_id.0, &request.campaign_id.0]).map_err(|e| store_err(&e))?;
        let timestamp: i64 = tx
            .query_one(
                "SELECT floor(extract(epoch FROM transaction_timestamp()))::bigint",
                &[],
            )
            .map_err(|e| store_err(&e))?
            .get(0);
        let event = MissionAuthorityWithdrawn::new(
            request.clone(),
            operation,
            OperationId(registration.get(0)),
            EventId(allocator.allocate()?),
            timestamp,
        )?;
        let contract = serde_json::to_value(&event).map_err(|_| Fail::Store("encode"))?;
        tx.execute(
            "INSERT INTO mission.withdrawals (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract) \
             VALUES ($1,$2,$3,$4,$5,$6)",
            &[&request.engagement_id.0, &request.campaign_id.0, &operation.0,
                &event.event_id.0, &event.registration_operation_id.0, &contract],
        ).map_err(|e| store_err(&e))?;
        tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
        Ok(Some(event))
    }
}

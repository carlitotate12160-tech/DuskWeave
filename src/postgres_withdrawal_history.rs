//! Trajectory-owned withdrawal codec and predecessor boundary over the fixed journal.
use super::{PgTrajectory, planning};
use crate::mission::MissionRegistered;
use crate::postgres_trajectory_history::store_err;
use crate::postgres_trajectory_journal::{self as journal, JournalRecord, JournalTable};
use crate::trajectory::{Delivered, check_event};
use crate::withdrawal::{MissionAuthorityWithdrawn, OBLIGATION, WithdrawalHistoryPort};
use crate::{Fail, Res};
use postgres::{GenericClient, IsolationLevel, Transaction};
use serde_json::Value;

fn record(event: &MissionAuthorityWithdrawn) -> Res<JournalRecord> {
    Ok(JournalRecord {
        engagement_id: event.request.engagement_id,
        campaign_id: event.request.campaign_id,
        event_id: event.event_id,
        operation_id: event.operation_id,
        producer: event.producer.clone(),
        kind: event.kind.clone(),
        version: event.version,
        obligation: OBLIGATION.into(),
        contract: serde_json::to_value(event).map_err(|_| Fail::Store("encode"))?,
    })
}

fn decode(raw: Value) -> Res<JournalRecord> {
    let event: MissionAuthorityWithdrawn =
        serde_json::from_value(raw).map_err(|_| Fail::Store("contract_decode"))?;
    event
        .validate()
        .map_err(|_| Fail::Store("contract_decode"))?;
    record(&event)
}

fn predecessor(
    tx: &mut Transaction,
    event: &MissionAuthorityWithdrawn,
) -> Res<Option<&'static str>> {
    let rows = tx.query(
        "SELECT status,producer,obligation,event_id,operation_id,contract,completed_at IS NOT NULL \
         FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2 AND \
         (operation_id=$3 OR event_id IN (SELECT event_id FROM trajectory.registration_history \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3)) LIMIT 2",
        &[&event.request.engagement_id.0, &event.request.campaign_id.0, &event.registration_operation_id.0],
    ).map_err(|e| store_err(&e))?;
    if rows.is_empty() {
        return Ok(Some("missing_predecessor"));
    }
    if rows.len() != 1 {
        return Ok(Some("unsupported_predecessor"));
    }
    let row = &rows[0];
    if (
        row.get::<_, &str>(0),
        row.get::<_, &str>(1),
        row.get::<_, &str>(2),
        row.get::<_, bool>(6),
    ) != (
        "accepted",
        "mission",
        "trajectory.registration_history.v1",
        true,
    ) {
        return Ok(Some("unsupported_predecessor"));
    }
    let registered: MissionRegistered =
        serde_json::from_value(row.get(5)).map_err(|_| Fail::Store("contract_decode"))?;
    let identity = (
        registered.engagement_id,
        registered.campaign_id,
        registered.operation_id.0,
        registered.event_id.0,
    );
    let required = (
        event.request.engagement_id,
        event.request.campaign_id,
        event.registration_operation_id.0,
        row.get::<_, uuid::Uuid>(3),
    );
    if (
        check_event(&registered),
        identity,
        registered.operation_id.0,
    ) != (Ok(()), required, row.get::<_, uuid::Uuid>(4))
        || [registered.event_id.0, registered.operation_id.0]
            .iter()
            .any(uuid::Uuid::is_nil)
    {
        return Ok(Some("unsupported_predecessor"));
    }
    Ok(None)
}

fn existing(
    client: &mut impl GenericClient,
    event: &MissionAuthorityWithdrawn,
) -> Res<Option<Delivered>> {
    journal::existing(client, JournalTable::Withdrawal, &record(event)?, decode)
}

impl WithdrawalHistoryPort for PgTrajectory {
    fn publish(&mut self, event: &MissionAuthorityWithdrawn) -> Res<Delivered> {
        event.validate()?;
        planning::qualify(&mut self.client, true)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
        let record = record(event)?;
        let outcome = match existing(&mut tx, event)? {
            Some(Delivered::Anomaly) => {
                journal::append_anomaly(&mut tx, JournalTable::Withdrawal, &record)?;
                Delivered::Anomaly
            }
            Some(_) => Delivered::Duplicate,
            None => match predecessor(&mut tx, event)? {
                Some(reason) => Delivered::Unresolved(reason),
                None => {
                    journal::append_accepted(&mut tx, JournalTable::Withdrawal, &record)?;
                    Delivered::Completed
                }
            },
        };
        tx.commit().map_err(|e| planning::commit_error(&e))?;
        Ok(outcome)
    }

    fn inspect(&mut self, event: &MissionAuthorityWithdrawn) -> Res<Delivered> {
        event.validate()?;
        planning::qualify(&mut self.client, false)?;
        Ok(existing(&mut self.client, event)?.unwrap_or(Delivered::Unresolved("not_recorded")))
    }
}

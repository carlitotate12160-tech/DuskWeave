//! CampaignTrajectory owner's PostgreSQL adapter. Queries only
//! trajectory.* tables; history insertion and completion commit atomically.

use crate::mission::{CampaignId, EngagementId, EventId, MissionRegistered};
use crate::registration::TrajectoryPort;
use crate::trajectory::{Delivered, HistoryStatus, check_event};
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel, Transaction};

fn store_err(e: &postgres::Error) -> Fail {
    use postgres::error::SqlState;
    match e.code() {
        Some(&SqlState::T_R_SERIALIZATION_FAILURE) | Some(&SqlState::T_R_DEADLOCK_DETECTED) => {
            Fail::Store("serialization_retry")
        }
        Some(&SqlState::UNIQUE_VIOLATION) => Fail::Conflict("duplicate_identity"),
        _ => Fail::Store("storage_error"),
    }
}

fn has_anomaly(tx: &mut Transaction, ev: &MissionRegistered) -> Res<bool> {
    Ok(tx
        .query_opt(
            "SELECT 1 FROM trajectory.registration_history \
             WHERE engagement_id = $1 AND campaign_id = $2 AND event_id = $3 \
             AND status = 'anomaly'",
            &[&ev.engagement_id.0, &ev.campaign_id.0, &ev.event_id.0],
        )
        .map_err(|e| store_err(&e))?
        .is_some())
}

fn accepted_match(
    tx: &mut Transaction,
    ev: &MissionRegistered,
    contract: &serde_json::Value,
) -> Res<Option<bool>> {
    Ok(tx
        .query_opt(
            "SELECT contract = $4::jsonb FROM trajectory.registration_history \
             WHERE engagement_id = $1 AND campaign_id = $2 AND event_id = $3 \
             AND status = 'accepted'",
            &[
                &ev.engagement_id.0,
                &ev.campaign_id.0,
                &ev.event_id.0,
                &contract,
            ],
        )
        .map_err(|e| store_err(&e))?
        .map(|r| r.get(0)))
}

fn insert_anomaly(tx: &mut Transaction, ev: &MissionRegistered) -> Res<()> {
    tx.execute(
        "INSERT INTO trajectory.registration_history \
         (engagement_id, campaign_id, producer, operation_id, event_id, \
          status, anomaly_category) \
         VALUES ($1,$2,$3,$4,$5,'anomaly','conflicting_identity')",
        &[
            &ev.engagement_id.0,
            &ev.campaign_id.0,
            &ev.producer,
            &ev.operation_id.0,
            &ev.event_id.0,
        ],
    )
    .map_err(|e| store_err(&e))?;
    Ok(())
}

fn insert_accepted(
    tx: &mut Transaction,
    ev: &MissionRegistered,
    contract: &serde_json::Value,
) -> Res<()> {
    tx.execute(
        "INSERT INTO trajectory.registration_history \
         (engagement_id, campaign_id, producer, operation_id, event_id, \
          status, contract, completed_at) \
         VALUES ($1,$2,$3,$4,$5,'accepted',$6,transaction_timestamp())",
        &[
            &ev.engagement_id.0,
            &ev.campaign_id.0,
            &ev.producer,
            &ev.operation_id.0,
            &ev.event_id.0,
            &contract,
        ],
    )
    .map_err(|e| store_err(&e))?;
    Ok(())
}

fn deliver_tx(
    tx: &mut Transaction,
    ev: &MissionRegistered,
    contract: &serde_json::Value,
) -> Res<Delivered> {
    if has_anomaly(tx, ev)? {
        return Ok(Delivered::Anomaly);
    }
    match accepted_match(tx, ev, contract)? {
        Some(true) => Ok(Delivered::Duplicate),
        Some(false) => {
            insert_anomaly(tx, ev)?;
            Ok(Delivered::Anomaly)
        }
        None => {
            insert_accepted(tx, ev, contract)?;
            Ok(Delivered::Completed)
        }
    }
}

pub struct PgTrajectory {
    client: Client,
}

impl PgTrajectory {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl TrajectoryPort for PgTrajectory {
    fn deliver(&mut self, ev: &MissionRegistered) -> Res<Delivered> {
        if let Err(cat) = check_event(ev) {
            return Ok(Delivered::Unresolved(cat));
        }
        let contract = serde_json::to_value(ev).map_err(|_| Fail::Store("encode"))?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
        let outcome = deliver_tx(&mut tx, ev, &contract)?;
        tx.commit().map_err(|e| store_err(&e))?;
        Ok(outcome)
    }

    fn status(&mut self, e: EngagementId, c: CampaignId, ev: EventId) -> Res<HistoryStatus> {
        let rows = self
            .client
            .query(
                "SELECT status FROM trajectory.registration_history \
                 WHERE engagement_id = $1 AND campaign_id = $2 AND event_id = $3",
                &[&e.0, &c.0, &ev.0],
            )
            .map_err(|e| store_err(&e))?;
        let mut completed = false;
        for r in &rows {
            match r.get::<_, &str>(0) {
                "anomaly" => return Ok(HistoryStatus::Anomaly),
                _ => completed = true,
            }
        }
        Ok(if completed {
            HistoryStatus::Completed
        } else {
            HistoryStatus::Pending
        })
    }
}

use crate::trajectory::{Delivered, HistoryStatus};
use crate::{Fail, Res};
use postgres::{Client, Transaction};
use serde_json::Value;
use uuid::Uuid;

pub(super) struct RegistrationRecord<'a> {
    pub(super) engagement_id: Uuid,
    pub(super) campaign_id: Uuid,
    pub(super) producer: &'a str,
    pub(super) operation_id: Uuid,
    pub(super) event_id: Uuid,
    pub(super) obligation: &'a str,
    pub(super) contract: &'a Value,
}

pub(super) fn store_err(error: &postgres::Error) -> Fail {
    use postgres::error::SqlState;
    match error.code() {
        Some(&SqlState::T_R_SERIALIZATION_FAILURE) | Some(&SqlState::T_R_DEADLOCK_DETECTED) => {
            Fail::Store("serialization_retry")
        }
        Some(&SqlState::UNIQUE_VIOLATION) => Fail::Conflict("duplicate_identity"),
        _ => Fail::Store("storage_error"),
    }
}

fn has_anomaly(tx: &mut Transaction, record: &RegistrationRecord<'_>) -> Res<bool> {
    Ok(tx
        .query_opt(
            "SELECT 1 FROM trajectory.registration_history \
             WHERE engagement_id = $1 AND campaign_id = $2 AND event_id = $3 \
             AND status = 'anomaly'",
            &[&record.engagement_id, &record.campaign_id, &record.event_id],
        )
        .map_err(|error| store_err(&error))?
        .is_some())
}

fn accepted_match(tx: &mut Transaction, record: &RegistrationRecord<'_>) -> Res<Option<bool>> {
    Ok(tx
        .query_opt(
            "SELECT contract = $4::jsonb AND obligation = $5 \
             FROM trajectory.registration_history \
             WHERE engagement_id = $1 AND campaign_id = $2 AND event_id = $3 \
             AND status = 'accepted'",
            &[
                &record.engagement_id,
                &record.campaign_id,
                &record.event_id,
                record.contract,
                &record.obligation,
            ],
        )
        .map_err(|error| store_err(&error))?
        .map(|row| row.get(0)))
}

fn insert_anomaly(tx: &mut Transaction, record: &RegistrationRecord<'_>) -> Res<()> {
    tx.execute(
        "INSERT INTO trajectory.registration_history \
         (engagement_id, campaign_id, producer, operation_id, event_id, \
          obligation, status, anomaly_category) \
         VALUES ($1,$2,$3,$4,$5,$6,'anomaly','conflicting_identity')",
        &[
            &record.engagement_id,
            &record.campaign_id,
            &record.producer,
            &record.operation_id,
            &record.event_id,
            &record.obligation,
        ],
    )
    .map_err(|error| store_err(&error))?;
    Ok(())
}

fn insert_accepted(tx: &mut Transaction, record: &RegistrationRecord<'_>) -> Res<()> {
    tx.execute(
        "INSERT INTO trajectory.registration_history \
         (engagement_id, campaign_id, producer, operation_id, event_id, \
          obligation, status, contract, completed_at) \
         VALUES ($1,$2,$3,$4,$5,$6,'accepted',$7,transaction_timestamp())",
        &[
            &record.engagement_id,
            &record.campaign_id,
            &record.producer,
            &record.operation_id,
            &record.event_id,
            &record.obligation,
            record.contract,
        ],
    )
    .map_err(|error| store_err(&error))?;
    Ok(())
}

/// Appends to a scope with no anomaly marker: an equal accepted row dedups,
/// a conflicting accepted row records an append-only anomaly, and an absent
/// row accepts with its completion timestamp.
fn append_unmarked(tx: &mut Transaction, record: &RegistrationRecord<'_>) -> Res<Delivered> {
    match accepted_match(tx, record)? {
        Some(true) => Ok(Delivered::Duplicate),
        Some(false) => {
            insert_anomaly(tx, record)?;
            Ok(Delivered::Anomaly)
        }
        None => {
            insert_accepted(tx, record)?;
            Ok(Delivered::Completed)
        }
    }
}

pub(super) fn append(tx: &mut Transaction, record: &RegistrationRecord<'_>) -> Res<Delivered> {
    if has_anomaly(tx, record)? {
        return Ok(Delivered::Anomaly);
    }
    append_unmarked(tx, record)
}

pub(super) fn status(
    client: &mut Client,
    engagement_id: Uuid,
    campaign_id: Uuid,
    event_id: Uuid,
) -> Res<HistoryStatus> {
    let rows = client
        .query(
            "SELECT status FROM trajectory.registration_history \
             WHERE engagement_id = $1 AND campaign_id = $2 AND event_id = $3",
            &[&engagement_id, &campaign_id, &event_id],
        )
        .map_err(|error| store_err(&error))?;
    let mut completed = false;
    for row in &rows {
        match row.get::<_, &str>(0) {
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

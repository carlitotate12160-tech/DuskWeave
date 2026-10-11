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
             WHERE engagement_id = $1 AND campaign_id = $2 \
             AND (event_id = $3 OR ($4::jsonb->>'version') = '3') \
             AND status = 'anomaly' LIMIT 1",
            &[
                &record.engagement_id,
                &record.campaign_id,
                &record.event_id,
                record.contract,
            ],
        )
        .map_err(|error| store_err(&error))?
        .is_some())
}

fn accepted_match(tx: &mut Transaction, record: &RegistrationRecord<'_>) -> Res<Option<bool>> {
    Ok(tx
        .query_opt(
            "SELECT event_id = $3 AND contract = $4::jsonb AND obligation = $5 \
             AND (($4->>'version') <> '3' OR (operation_id=$6 AND producer=$7 \
                  AND completed_at IS NOT NULL)) \
             FROM trajectory.registration_history \
             WHERE engagement_id = $1 AND campaign_id = $2 \
             AND (event_id = $3 OR ($4->>'version') = '3') \
             AND status = 'accepted'",
            &[
                &record.engagement_id,
                &record.campaign_id,
                &record.event_id,
                record.contract,
                &record.obligation,
                &record.operation_id,
                &record.producer,
            ],
        )
        .map_err(|error| store_err(&error))?
        .map(|row| row.get::<_, Option<bool>>(0) == Some(true)))
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

/// A v3 completed receipt binds both the safe contract and its history catalog.
/// Anomaly status still dominates; a corrupt accepted row fails closed.
pub(super) fn bound_status(
    client: &mut Client,
    event: &crate::mission::MissionRegistered,
) -> Res<HistoryStatus> {
    let contract = serde_json::to_value(event).map_err(|_| Fail::Store("encode"))?;
    let rows = client
        .query(
            "SELECT status, event_id=$3 AND contract=$4::jsonb AND operation_id=$5 AND producer='mission'
         AND obligation='trajectory.registration_history.v1' AND completed_at IS NOT NULL
         FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2",
            &[
                &event.engagement_id.0,
                &event.campaign_id.0,
                &event.event_id.0,
                &contract,
                &event.operation_id.0,
            ],
        )
        .map_err(|e| store_err(&e))?;
    if rows.iter().any(|r| r.get::<_, &str>(0) == "anomaly") {
        return Ok(HistoryStatus::Anomaly);
    }
    if rows.iter().any(|row| {
        row.get::<_, &str>(0) != "accepted" || row.get::<_, Option<bool>>(1) != Some(true)
    }) {
        return Err(Fail::Store("contract_decode"));
    }
    Ok(if rows.is_empty() {
        HistoryStatus::Pending
    } else {
        HistoryStatus::Completed
    })
}

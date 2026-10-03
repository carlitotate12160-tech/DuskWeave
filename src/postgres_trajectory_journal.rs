//! Trajectory-owned scoped journal: identity, deduplication and conflict
//! persistence for accepted history rows. Mission authority, contract
//! interpretation, predecessor validation and transaction ownership stay with
//! their existing components; the caller supplies its own client/transaction.

use crate::mission::{CampaignId, EngagementId, EventId, OperationId};
use crate::postgres_trajectory_history::store_err;
use crate::trajectory::Delivered;
use crate::{Fail, Res};
use postgres::{GenericClient, Row, Transaction};
use serde_json::Value;
use uuid::Uuid;

/// Closed set of Trajectory-owned journal tables. The SQL name is a
/// compile-time mapping: no caller-provided table or schema reaches SQL, and
/// every identity/content value remains a bound parameter.
#[derive(Clone, Copy)]
pub(super) enum JournalTable {
    Planning,
    Withdrawal,
}

impl JournalTable {
    fn name(self) -> &'static str {
        match self {
            Self::Planning => "trajectory.planning_history",
            Self::Withdrawal => "trajectory.withdrawal_history",
        }
    }
}

/// Owned persistence record for one scoped journal row: typed identities,
/// validated header, fixed obligation and canonical contract produced by the
/// owning adapter's codec. This is a persistence record, not a domain event;
/// it carries no raw input or current-state handle.
pub(super) struct JournalRecord {
    pub(super) engagement_id: EngagementId,
    pub(super) campaign_id: CampaignId,
    pub(super) event_id: EventId,
    pub(super) operation_id: OperationId,
    pub(super) producer: String,
    pub(super) kind: String,
    pub(super) version: u32,
    pub(super) obligation: String,
    pub(super) contract: Value,
}

/// Decodes one accepted row's contract through the owning codec and verifies
/// that the validated record's provenance matches its scoped catalog exactly
/// before comparing canonical contract content.
fn matches(
    row: &Row,
    incoming: &JournalRecord,
    decode: fn(Value) -> Res<JournalRecord>,
) -> Res<bool> {
    let raw: Value = row
        .try_get("contract")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let stored = decode(raw)?;
    let event: Uuid = row
        .try_get("event_id")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let operation: Uuid = row
        .try_get("operation_id")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let producer: String = row
        .try_get("producer")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let kind: String = row
        .try_get("kind")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let version: i32 = row
        .try_get("version")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let obligation: String = row
        .try_get("obligation")
        .map_err(|_| Fail::Store("contract_decode"))?;
    // Catalog columns must carry the validated record's provenance exactly.
    let catalog = (
        stored.engagement_id,
        stored.campaign_id,
        stored.event_id.0,
        stored.operation_id.0,
        stored.producer.as_str(),
        stored.kind.as_str(),
        i64::from(stored.version),
        stored.obligation.as_str(),
    );
    if catalog
        != (
            incoming.engagement_id,
            incoming.campaign_id,
            event,
            operation,
            producer.as_str(),
            kind.as_str(),
            i64::from(version),
            obligation.as_str(),
        )
    {
        return Err(Fail::Store("contract_decode"));
    }
    Ok(stored.contract == incoming.contract)
}

/// Scoped identity lookup over both accepted axes plus anomaly markers: an
/// anomaly dominates, an equal accepted row completes, and an unequal
/// accepted row under a reused identity is an integrity conflict.
pub(super) fn existing(
    client: &mut impl GenericClient,
    table: JournalTable,
    incoming: &JournalRecord,
    decode: fn(Value) -> Res<JournalRecord>,
) -> Res<Option<Delivered>> {
    let rows = client
        .query(
            &format!(
                "SELECT status, event_id, operation_id, producer, kind, version, obligation, contract \
                 FROM {} WHERE engagement_id=$1 AND campaign_id=$2 \
                 AND (event_id=$3 OR operation_id=$4)",
                table.name()
            ),
            &[
                &incoming.engagement_id.0,
                &incoming.campaign_id.0,
                &incoming.event_id.0,
                &incoming.operation_id.0,
            ],
        )
        .map_err(|e| store_err(&e))?;
    let mut result = None;
    for row in rows {
        let status: &str = row
            .try_get("status")
            .map_err(|_| Fail::Store("contract_decode"))?;
        match status {
            "anomaly" => return Ok(Some(Delivered::Anomaly)),
            "accepted" if matches(&row, incoming, decode)? => result = Some(Delivered::Completed),
            "accepted" => return Ok(Some(Delivered::Anomaly)),
            _ => return Err(Fail::Store("contract_decode")),
        }
    }
    Ok(result)
}

/// Exact conflicting-pair marker. Only anomaly markers use ON CONFLICT DO
/// NOTHING; accepted content is never idempotent-merged, and conflicting
/// payloads are never stored.
pub(super) fn append_anomaly(
    tx: &mut Transaction,
    table: JournalTable,
    record: &JournalRecord,
) -> Res<()> {
    tx.execute(
        &format!(
            "INSERT INTO {} \
             (engagement_id,campaign_id,operation_id,event_id,status,anomaly_category) \
             VALUES ($1,$2,$3,$4,'anomaly','conflicting_identity') ON CONFLICT DO NOTHING",
            table.name()
        ),
        &[
            &record.engagement_id.0,
            &record.campaign_id.0,
            &record.operation_id.0,
            &record.event_id.0,
        ],
    )
    .map_err(|e| store_err(&e))?;
    Ok(())
}

/// Accepted insertion records the validated header, obligation, canonical
/// contract and completion together, preserving the record's actual version.
pub(super) fn append_accepted(
    tx: &mut Transaction,
    table: JournalTable,
    record: &JournalRecord,
) -> Res<()> {
    tx.execute(
        &format!(
            "INSERT INTO {} \
             (engagement_id,campaign_id,operation_id,event_id,producer,kind,version,\
             status,contract,completed_at,obligation) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,'accepted',$8,transaction_timestamp(),$9)",
            table.name()
        ),
        &[
            &record.engagement_id.0,
            &record.campaign_id.0,
            &record.operation_id.0,
            &record.event_id.0,
            &record.producer,
            &record.kind,
            &(record.version as i32),
            &record.contract,
            &record.obligation,
        ],
    )
    .map_err(|e| store_err(&e))?;
    Ok(())
}

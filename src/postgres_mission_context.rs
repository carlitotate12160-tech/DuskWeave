//! Mission-owned publication qualification, composed with Broker's public read port.
use super::{REGISTRATION_READ, decode_registration, store_err};
use crate::m1_context::{ContextBasis, ContextReader};
use crate::m1_context_input::ContextRequest;
use crate::mission::OperationId;
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel};
use serde_json::Value;
use uuid::Uuid;

pub struct PgContextReader {
    client: Client,
}
impl PgContextReader {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl ContextReader for PgContextReader {
    fn read(&mut self, q: &ContextRequest, op: OperationId) -> Res<ContextBasis> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .map_err(|e| store_err(&e))?;
        let row = tx
            .query_one(
                "SELECT mission.current_inference_publication($1,$2,$3), \
            execution.inference_capacity_snapshot($1,$2), \
            floor(extract(epoch FROM transaction_timestamp()))::bigint",
                &[&q.mission.engagement_id.0, &q.mission.campaign_id.0, &op.0],
            )
            .map_err(|e| store_err(&e))?;
        let mission = qualify_publication(&mut tx, &row, q)?;
        let (capacity, evaluated_at) = decode_snapshot(&row)?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("context_snapshot_unknown"))?;
        Ok(ContextBasis {
            mission,
            capacity,
            evaluated_at,
        })
    }
}

// Strict Mission codec must agree with its committed publication in this snapshot.
fn qualify_publication(
    tx: &mut postgres::Transaction<'_>,
    row: &postgres::Row,
    q: &ContextRequest,
) -> Res<crate::mission::MissionRegistered> {
    let raw: Option<Value> = row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?;
    let raw = raw.ok_or(Fail::State("context_mission_unavailable"))?;
    let registration = publication_operation(&raw)?;
    let owner_row = tx
        .query_one(
            REGISTRATION_READ,
            &[
                &q.mission.engagement_id.0,
                &q.mission.campaign_id.0,
                &registration,
            ],
        )
        .map_err(|e| store_err(&e))?;
    let mission = decode_registration(&owner_row)?;
    if !serde_json::to_value(&mission).is_ok_and(|value| value == raw) {
        return Err(Fail::Store("contract_decode"));
    }
    Ok(mission)
}

fn decode_snapshot(row: &postgres::Row) -> Res<(crate::m1_context_capacity::ContextCapacity, i64)> {
    let capacity: Option<Value> = row
        .try_get(1)
        .map_err(|_| Fail::Store("context_capacity_unavailable"))?;
    let capacity =
        serde_json::from_value(capacity.ok_or(Fail::Store("context_capacity_unavailable"))?)
            .map_err(|_| Fail::Store("context_capacity_unavailable"))?;
    let evaluated_at = row
        .try_get(2)
        .map_err(|_| Fail::Store("context_clock_unavailable"))?;
    Ok((capacity, evaluated_at))
}

fn publication_operation(raw: &Value) -> Res<Uuid> {
    raw.get("operation_id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok())
        .filter(|id| !id.is_nil())
        .ok_or(Fail::Store("contract_decode"))
}

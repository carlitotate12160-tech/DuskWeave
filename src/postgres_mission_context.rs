//! Mission-owned publication qualification, composed with Broker's public read port.
use super::{REGISTRATION_READ, decode_registration, store_err};
use crate::m1_context::{ContextBasis, ContextReader, TerrainContextState};
use crate::m1_context_input::ContextRequest;
use crate::mission::{AssetRef, MissionRegistered, OperationId};
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

    pub(crate) fn read_current_publication(
        tx: &mut postgres::Transaction<'_>,
        engagement: Uuid,
        campaign: Uuid,
        decision: Uuid,
    ) -> Res<(MissionRegistered, i64)> {
        let row = tx
            .query_one(
                "SELECT mission.current_inference_publication($1,$2,$3), \
                floor(extract(epoch FROM transaction_timestamp()))::bigint",
                &[&engagement, &campaign, &decision],
            )
            .map_err(|e| store_err(&e))?;
        let mission = qualify_publication_for(tx, &row, engagement, campaign)?;
        let evaluated_at = row
            .try_get(1)
            .map_err(|_| Fail::Store("context_clock_unavailable"))?;
        Ok((mission, evaluated_at))
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
        let terrain = read_terrain(&mut tx, &mission, q, evaluated_at)?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("context_snapshot_unknown"))?;
        Ok(ContextBasis {
            mission,
            capacity,
            evaluated_at,
            terrain,
        })
    }
}

fn read_terrain(
    tx: &mut postgres::Transaction<'_>,
    mission: &MissionRegistered,
    q: &ContextRequest,
    evaluated_at: i64,
) -> Res<TerrainContextState> {
    let assets = match terrain_assets(mission, q) {
        Ok(assets) => assets,
        Err(state) => return Ok(*state),
    };
    let mut sp = tx
        .savepoint("terrain_context_view")
        .map_err(|e| store_err(&e))?;
    match crate::postgres_terrain_view::read_in_tx(&mut sp, mission, &assets, None) {
        Ok(view) => {
            sp.commit().map_err(|e| store_err(&e))?;
            Ok(TerrainContextState::Available(
                view.evaluate_at(evaluated_at, true),
            ))
        }
        Err(err) => {
            let _ = sp.rollback();
            degrade_terrain_error(err)
        }
    }
}

fn terrain_assets(
    mission: &MissionRegistered,
    q: &ContextRequest,
) -> Result<Vec<AssetRef>, Box<TerrainContextState>> {
    let Some(perm) = mission.fields.m1_inference_permission() else {
        return Err(Box::new(TerrainContextState::DisclosureNotPermitted));
    };
    let d = perm.context_disclosure();
    if !d.owner_qualified_summaries {
        return Err(Box::new(TerrainContextState::DisclosureNotPermitted));
    }
    if q.candidates.is_empty() || !d.operator_premises {
        return Err(Box::new(TerrainContextState::NotRequested));
    }
    collect_selected_assets(mission, q, d.candidates)
}

fn collect_selected_assets(
    mission: &MissionRegistered,
    q: &ContextRequest,
    limit: usize,
) -> Result<Vec<AssetRef>, Box<TerrainContextState>> {
    if q.candidates.iter().any(|c| {
        !mission.fields.included_assets.contains(&c.asset_ref)
            || mission.fields.excluded_assets.contains(&c.asset_ref)
    }) {
        return Err(Box::new(TerrainContextState::Unavailable(
            "owner_view_unavailable",
        )));
    }
    let count = q.candidates.len().min(limit);
    let mut assets = Vec::with_capacity(count);
    for c in &q.candidates[..count] {
        if !assets.contains(&c.asset_ref) {
            assets.push(c.asset_ref);
        }
    }
    Ok(assets)
}

fn degrade_terrain_error(err: Fail) -> Res<TerrainContextState> {
    match err {
        Fail::Input(code) => Err(Fail::Input(code)),
        Fail::State("incompatible_lineage") => {
            Ok(TerrainContextState::Unavailable("incompatible_lineage"))
        }
        _ => Ok(TerrainContextState::Unavailable("owner_view_unavailable")),
    }
}

// Strict Mission codec must agree with its committed publication in this snapshot.
fn qualify_publication(
    tx: &mut postgres::Transaction<'_>,
    row: &postgres::Row,
    q: &ContextRequest,
) -> Res<crate::mission::MissionRegistered> {
    qualify_publication_for(tx, row, q.mission.engagement_id.0, q.mission.campaign_id.0)
}

fn qualify_publication_for(
    tx: &mut postgres::Transaction<'_>,
    row: &postgres::Row,
    engagement: Uuid,
    campaign: Uuid,
) -> Res<crate::mission::MissionRegistered> {
    let raw: Option<Value> = row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?;
    let raw = raw.ok_or(Fail::State("context_mission_unavailable"))?;
    let registration = publication_operation(&raw)?;
    let owner_row = tx
        .query_one(REGISTRATION_READ, &[&engagement, &campaign, &registration])
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

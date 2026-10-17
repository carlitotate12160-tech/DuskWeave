//! PostgreSQL adapter for the bounded Terrain public DNS context view.
use crate::m1_terrain_view::{RawTerrainView, TerrainPublicView, TerrainViewQuery};
use crate::mission::{AssetRef, MissionRegistered};
use crate::{Fail, Res};
use postgres::{Client, GenericClient, IsolationLevel};
use serde_json::Value;
use uuid::Uuid;

pub fn read(
    client: &mut Client,
    mission: &MissionRegistered,
    selected_assets: &[AssetRef],
    claim_ids: Option<Vec<Uuid>>,
) -> Res<TerrainPublicView> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .map_err(|e| map_view_err(&e))?;
    let view = read_in_tx(&mut tx, mission, selected_assets, claim_ids)?;
    tx.commit()
        .map_err(|_| Fail::Store("terrain_view_unavailable"))?;
    Ok(view)
}

pub fn read_in_tx(
    client: &mut impl GenericClient,
    mission: &MissionRegistered,
    selected_assets: &[AssetRef],
    claim_ids: Option<Vec<Uuid>>,
) -> Res<TerrainPublicView> {
    let query = TerrainViewQuery::from_mission(mission, selected_assets.to_vec(), claim_ids)?;
    let payload = serde_json::to_value(&query).map_err(|_| Fail::Store("terrain_view_encode"))?;
    let row = client
        .query_one("SELECT terrain.current_dns_context_view($1)", &[&payload])
        .map_err(|e| map_view_err(&e))?;
    decode_row(&row, &query, mission, selected_assets)
}

fn decode_row(
    row: &postgres::Row,
    query: &TerrainViewQuery,
    mission: &MissionRegistered,
    selected_assets: &[AssetRef],
) -> Res<TerrainPublicView> {
    let raw_val: Value = row
        .try_get(0)
        .map_err(|_| Fail::Store("terrain_view_decode"))?;
    let raw: RawTerrainView =
        serde_json::from_value(raw_val).map_err(|_| Fail::Store("terrain_view_decode"))?;
    let view = TerrainPublicView::from_raw(raw, mission, selected_assets)?;
    if let Some(ids) = &query.claim_ids
        && view.claims.iter().any(|c| !ids.contains(&c.claim_id))
    {
        return Err(Fail::Input("forged_terrain_summary"));
    }
    Ok(view)
}

fn map_view_err(err: &postgres::Error) -> Fail {
    match err.as_db_error().map(|d| d.message()) {
        Some("unqualified_terrain_reader") => Fail::Config("inference_database_unqualified"),
        Some("terrain_view_query_refused") => Fail::Input("terrain_view_query_refused"),
        Some("terrain_authority_unavailable") => Fail::State("terrain_context_scope_unavailable"),
        Some("terrain_lineage_unavailable") => Fail::State("incompatible_lineage"),
        _ => Fail::Store("terrain_view_unavailable"),
    }
}

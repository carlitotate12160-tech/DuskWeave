//! Restricted HTTPS issued-result intake for Terrain (`dw_m1_terrain`).
use super::{PgTerrainStore, map_pg_err};
use crate::m1_https::{HttpsHeadResult, sha256_hex};
use crate::m1_https_terrain::{
    TerrainHttpsChangedEvent, TerrainHttpsObservation, TerrainIds, TrustTier, evaluate_observation,
};
use crate::mission::{CampaignId, EngagementId, OperationId};
use crate::registration::MissionStore;
use crate::{Fail, Res};
use postgres::IsolationLevel;
use serde_json::Value;

pub struct PgHttpsTerrainStore<'a> {
    inner: &'a mut PgTerrainStore,
}

impl<'a> PgHttpsTerrainStore<'a> {
    pub fn new(inner: &'a mut PgTerrainStore) -> Self {
        Self { inner }
    }

    pub fn record_https(
        &mut self,
        mission_store: &mut impl MissionStore,
        result: &HttpsHeadResult,
        op: OperationId,
    ) -> Res<TerrainHttpsChangedEvent> {
        validate_record_inputs(result, op)?;
        let mission = load_mission(mission_store, result)?;
        let req_val = serde_json::to_value(result).map_err(|_| Fail::Store("contract_encode"))?;
        let req_hash = sha256_hex(req_val.to_string().as_bytes());
        self.record_https_tx(&mission, result, op, &req_val, &req_hash)
    }

    fn record_https_tx(
        &mut self,
        mission: &crate::mission::MissionRegistered,
        result: &HttpsHeadResult,
        op: OperationId,
        req_val: &Value,
        req_hash: &str,
    ) -> Res<TerrainHttpsChangedEvent> {
        let mut tx = self
            .inner
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| map_pg_err(&e))?;
        if let Some(existing) = load_existing(
            &mut tx,
            result.engagement_id,
            result.campaign_id,
            op,
            req_hash,
        )? {
            tx.commit().map_err(|e| map_pg_err(&e))?;
            return Ok(existing);
        }
        let event = insert_https_observation(&mut tx, mission, result, op, req_val, req_hash)?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("terrain_commit_unknown"))?;
        Ok(event)
    }
}

fn db_now_seconds(tx: &mut postgres::Transaction<'_>) -> Res<i64> {
    let row = tx
        .query_one(
            "SELECT floor(extract(epoch FROM statement_timestamp()))::bigint",
            &[],
        )
        .map_err(|e| map_pg_err(&e))?;
    row.try_get(0).map_err(|_| Fail::Store("contract_decode"))
}

fn validate_record_inputs(result: &HttpsHeadResult, op: OperationId) -> Res<()> {
    result.validate()?;
    (!op.0.is_nil())
        .then_some(())
        .ok_or(Fail::Input("operation_id"))
}

fn load_mission(
    mission_store: &mut impl MissionStore,
    result: &HttpsHeadResult,
) -> Res<crate::mission::MissionRegistered> {
    let view = mission_store
        .mission_view(result.engagement_id, result.campaign_id)?
        .ok_or(Fail::State("terrain_mission_absent"))?;
    mission_store
        .outbox_event(result.engagement_id, result.campaign_id, view.operation_id)?
        .ok_or(Fail::State("terrain_mission_absent"))
}

fn insert_https_observation(
    tx: &mut postgres::Transaction<'_>,
    mission: &crate::mission::MissionRegistered,
    result: &HttpsHeadResult,
    op: OperationId,
    req_val: &Value,
    req_hash: &str,
) -> Res<TerrainHttpsChangedEvent> {
    let claim_rev = next_claim_revision(tx, result)?;
    let obs = TerrainHttpsObservation {
        ids: TerrainIds::derive(result.engagement_id, result.campaign_id, op)?,
        result: result.clone(),
        trusted_tier: TrustTier::Tier1,
    };
    let now_s = db_now_seconds(tx)?;
    let (_, _, event) =
        evaluate_observation(mission, &obs, claim_rev, now_s.max(result.perceived_at))?;
    let payload = serde_json::to_value(&event).map_err(|_| Fail::Store("contract_encode"))?;
    tx.query_one(
        "SELECT terrain.accept_https_observation($1,$2,$3,$4)",
        &[req_val, &req_hash, &payload, &(mission.operation_id.0)],
    )
    .map_err(|e| map_pg_err(&e))?;
    Ok(event)
}

fn load_existing(
    tx: &mut postgres::Transaction<'_>,
    e: EngagementId,
    c: CampaignId,
    op: OperationId,
    expected_hash: &str,
) -> Res<Option<TerrainHttpsChangedEvent>> {
    let row = tx
        .query_opt(
            "SELECT request_hash, contract FROM terrain.https_outbox \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[&e.0, &c.0, &op.0],
        )
        .map_err(|e| map_pg_err(&e))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let hash: String = row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?;
    if hash != expected_hash {
        return Err(Fail::Conflict("input_mismatch"));
    }
    let contract: Value = row.try_get(1).map_err(|_| Fail::Store("contract_decode"))?;
    serde_json::from_value(contract)
        .map(Some)
        .map_err(|_| Fail::Store("contract_decode"))
}

fn next_claim_revision(tx: &mut postgres::Transaction<'_>, result: &HttpsHeadResult) -> Res<u32> {
    let prev: i32 = tx
        .query_one(
            "SELECT coalesce(max(claim_revision), 0) FROM terrain.https_observations \
             WHERE engagement_id=$1 AND campaign_id=$2 AND asset_ref=$3 \
             AND hostname=$4 AND ipv4=$5 AND approved_path=$6",
            &[
                &result.engagement_id.0,
                &result.campaign_id.0,
                &result.asset_ref.0,
                &result.hostname,
                &result.ipv4,
                &result.approved_path,
            ],
        )
        .map_err(|e| map_pg_err(&e))?
        .try_get(0)
        .map_err(|_| Fail::Store("contract_decode"))?;
    u32::try_from(prev + 1).map_err(|_| Fail::Store("contract_decode"))
}

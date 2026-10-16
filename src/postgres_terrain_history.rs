//! Trajectory verification, idempotent acceptance, anomaly dominance, and receipts for M1 Terrain.
use crate::m1_terrain::{
    TerrainAcceptedEvent, TerrainClaimRevision, TerrainClaimView, TerrainDnsRequest,
    TerrainEvaluation, TerrainSourceKind,
};
use crate::mission::{CampaignId, EngagementId, EventId, OperationId};
use crate::trajectory::HistoryStatus;
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainIds {
    pub operation_id: OperationId,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub evaluation_id: Uuid,
    pub claim_id: Uuid,
    pub event_id: EventId,
}

fn scoped_uuid(e: EngagementId, c: CampaignId, op: OperationId, tag: &str) -> Uuid {
    let mut h = Sha256::new();
    for part in [
        b"dw-terrain-id-v1\0".as_slice(),
        e.0.as_bytes(),
        c.0.as_bytes(),
        op.0.as_bytes(),
        tag.as_bytes(),
    ] {
        h.update(part);
    }
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    Uuid::from_bytes(b)
}

impl TerrainIds {
    pub fn derive(e: EngagementId, c: CampaignId, operation_id: OperationId) -> Res<Self> {
        let ids = Self {
            operation_id,
            observation_id: scoped_uuid(e, c, operation_id, "observation"),
            material_id: scoped_uuid(e, c, operation_id, "material"),
            evaluation_id: scoped_uuid(e, c, operation_id, "evaluation"),
            claim_id: scoped_uuid(e, c, operation_id, "claim"),
            event_id: EventId(scoped_uuid(e, c, operation_id, "event")),
        };
        ids.validate()?;
        Ok(ids)
    }

    pub fn validate(&self) -> Res<()> {
        let all = [
            self.operation_id.0,
            self.observation_id,
            self.material_id,
            self.evaluation_id,
            self.claim_id,
            self.event_id.0,
        ];
        for (i, id) in all.iter().enumerate() {
            let bad = [id.is_nil(), all[..i].contains(id)];
            if bad.into_iter().any(|b| b) {
                return Err(Fail::State("terrain_identity_invalid"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainReceipt {
    pub kind: String,
    pub result: String,
    pub reason: String,
    pub publication: String,
    pub history: String,
    pub dispatch_granted: bool,
    pub acquisition_qualified: bool,
    pub corroborated: bool,
    pub engagement_id: Option<EngagementId>,
    pub campaign_id: Option<CampaignId>,
    pub operation_id: Option<OperationId>,
    pub event_id: Option<EventId>,
    pub observation_id: Option<Uuid>,
    pub material_id: Option<Uuid>,
    pub evaluation_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub source_kind: Option<TerrainSourceKind>,
    pub evaluation: Option<TerrainEvaluation>,
    pub claim: Option<TerrainClaimView>,
    pub current_claim: Option<TerrainClaimView>,
}

impl TerrainReceipt {
    pub fn failure(
        result: &str,
        reason: &str,
        req: Option<&TerrainDnsRequest>,
        op: Option<OperationId>,
    ) -> Self {
        Self {
            kind: "m1_terrain_receipt_v1".to_string(),
            result: result.to_string(),
            reason: reason.to_string(),
            publication: "not_committed".to_string(),
            history: "not_recorded".to_string(),
            dispatch_granted: false,
            acquisition_qualified: false,
            corroborated: false,
            engagement_id: req.map(|r| r.engagement_id),
            campaign_id: req.map(|r| r.campaign_id),
            operation_id: op,
            event_id: None,
            observation_id: None,
            material_id: None,
            evaluation_id: None,
            claim_id: None,
            source_kind: req.map(|r| r.source_kind),
            evaluation: None,
            claim: None,
            current_claim: None,
        }
    }

    pub fn from_event(
        ev: &TerrainAcceptedEvent,
        current: &TerrainClaimRevision,
        history: &str,
        now: i64,
        authority_active: bool,
    ) -> Self {
        let (result, reason) = match (history, ev.claim.conflicted) {
            ("anomaly", _) => ("conflict", "history_anomaly"),
            ("pending", _) => ("pending", "history_pending"),
            (_, true) => ("conflict", ev.evaluation.outcome.as_str()),
            _ => ("accepted", ev.evaluation.outcome.as_str()),
        };
        let mut r = Self::failure(result, reason, None, Some(ev.operation_id));
        r.publication = "committed".to_string();
        r.history = history.to_string();
        r.engagement_id = Some(ev.engagement_id);
        r.campaign_id = Some(ev.campaign_id);
        r.event_id = Some(ev.event_id);
        r.observation_id = Some(ev.observation.observation_id);
        r.material_id = Some(ev.material.material_id);
        r.evaluation_id = Some(ev.evaluation.evaluation_id);
        r.claim_id = Some(ev.claim.claim_id);
        r.source_kind = Some(ev.observation.source_kind);
        r.evaluation = Some(ev.evaluation.clone());
        r.claim = Some(ev.claim.view_with_authority(now, authority_active));
        r.current_claim = Some(current.view_with_authority(now, authority_active));
        r
    }
}

const QUALIFY_TRAJECTORY_SQL: &str = "SELECT pg_has_role(session_user,'dw_m1_trajectory','MEMBER') \
AND NOT EXISTS(SELECT 1 FROM pg_roles r WHERE pg_has_role(session_user,r.oid,'SET') AND (r.rolsuper OR r.rolbypassrls OR r.rolcreatedb OR r.rolcreaterole)) \
AND NOT EXISTS(SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_terrain']) g WHERE pg_has_role(session_user,g,'MEMBER')) \
AND NOT EXISTS(SELECT 1 FROM pg_roles a WHERE pg_has_role(session_user,a.oid,'SET') AND EXISTS(SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname='terrain' AND t.relkind='r' AND (pg_has_role(a.oid,t.relowner,'MEMBER') OR has_table_privilege(a.oid,t.oid,'INSERT,UPDATE,DELETE,TRUNCATE,TRIGGER') OR has_any_column_privilege(a.oid,t.oid,'INSERT,UPDATE')))) \
AND current_setting('fsync')='on' AND current_setting('full_page_writes')='on' AND current_setting('synchronous_commit')='on'";

const INSPECT_HISTORY_SQL: &str = "SELECT \
EXISTS(SELECT 1 FROM trajectory.terrain_history WHERE ((engagement_id=$1 AND campaign_id=$2 AND operation_id=$3) OR event_id=$4) AND status='anomaly'), \
(SELECT contract FROM trajectory.terrain_history WHERE ((engagement_id=$1 AND campaign_id=$2 AND operation_id=$3) OR event_id=$4) AND status='accepted' LIMIT 1)";

fn map_pg_err(e: &postgres::Error) -> Fail {
    match e.code().map(postgres::error::SqlState::code) {
        Some("40001" | "40P01") => Fail::Store("serialization_retry"),
        Some("42501") => Fail::Config("unqualified_trajectory_role"),
        _ => Fail::Store("trajectory_history_error"),
    }
}

pub fn qualify_terrain_trajectory(client: &mut Client) -> Res<()> {
    let ok: bool = client
        .query_one(QUALIFY_TRAJECTORY_SQL, &[])
        .map_err(|_| Fail::Config("unqualified_trajectory_role"))?
        .try_get(0)
        .map_err(|_| Fail::Config("unqualified_trajectory_role"))?;
    ok.then_some(())
        .ok_or(Fail::Config("unqualified_trajectory_role"))
}

pub fn record_terrain_history(
    client: &mut Client,
    event: &TerrainAcceptedEvent,
) -> Res<HistoryStatus> {
    event.validate()?;
    let payload = serde_json::to_value(event).map_err(|_| Fail::Store("contract_encode"))?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::Serializable)
        .start()
        .map_err(|e| map_pg_err(&e))?;
    let row = tx
        .query_one("SELECT trajectory.record_terrain_history($1)", &[&payload])
        .map_err(|e| map_pg_err(&e))?;
    let status: String = row
        .try_get(0)
        .map_err(|_| Fail::Store("trajectory_history_error"))?;
    tx.commit()
        .map_err(|_| Fail::Unresolved("trajectory_commit_unknown"))?;
    match status.as_str() {
        "completed" => Ok(HistoryStatus::Completed),
        "pending" => Ok(HistoryStatus::Pending),
        "anomaly" => Ok(HistoryStatus::Anomaly),
        "uncommitted" => Err(Fail::State("uncommitted_terrain_event")),
        _ => Err(Fail::Store("trajectory_history_error")),
    }
}

pub fn inspect_terrain_history(
    client: &mut Client,
    event: &TerrainAcceptedEvent,
) -> Res<&'static str> {
    event.validate()?;
    let payload = serde_json::to_value(event).map_err(|_| Fail::Store("contract_encode"))?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .map_err(|e| map_pg_err(&e))?;
    let row = tx
        .query_one(
            INSPECT_HISTORY_SQL,
            &[
                &event.engagement_id.0,
                &event.campaign_id.0,
                &event.operation_id.0,
                &event.event_id.0,
            ],
        )
        .map_err(|e| map_pg_err(&e))?;
    tx.commit().map_err(|e| map_pg_err(&e))?;
    let has_anomaly: bool = row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?;
    let accepted: Option<Value> = row.try_get(1).map_err(|_| Fail::Store("contract_decode"))?;
    if has_anomaly {
        return Ok("anomaly");
    }
    match accepted {
        Some(stored) if stored == payload => Ok("completed"),
        Some(_) => Ok("anomaly"),
        None => Ok("pending"),
    }
}

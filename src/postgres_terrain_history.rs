//! Trajectory verification, idempotent acceptance, anomaly dominance, and receipts for M1 Terrain.
use crate::m1_terrain::TerrainAcceptedEvent;
use crate::trajectory::HistoryStatus;
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel};
use serde_json::Value;

const QUALIFY_TRAJECTORY_SQL: &str = "SELECT pg_has_role(session_user,'dw_m1_trajectory','MEMBER') \
AND NOT EXISTS(SELECT 1 FROM pg_roles r WHERE pg_has_role(session_user,r.oid,'SET') AND (r.rolsuper OR r.rolbypassrls OR r.rolcreatedb OR r.rolcreaterole)) \
AND NOT EXISTS(SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_terrain']) g WHERE pg_has_role(session_user,g,'MEMBER')) \
AND NOT EXISTS(SELECT 1 FROM pg_roles a WHERE pg_has_role(session_user,a.oid,'SET') AND (EXISTS(SELECT 1 FROM pg_namespace n WHERE n.nspname IN ('mission','execution','cognition','trajectory','terrain') AND (has_schema_privilege(a.oid,n.oid,'CREATE') OR pg_has_role(a.oid,n.nspowner,'MEMBER'))) OR EXISTS(SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE t.relkind='r' AND ((n.nspname IN ('terrain','mission','execution','cognition') AND (pg_has_role(a.oid,t.relowner,'MEMBER') OR has_table_privilege(a.oid,t.oid,'INSERT,UPDATE,DELETE,TRUNCATE,TRIGGER') OR has_any_column_privilege(a.oid,t.oid,'INSERT,UPDATE'))) OR (n.nspname='trajectory' AND (pg_has_role(a.oid,t.relowner,'MEMBER') OR has_table_privilege(a.oid,t.oid,'UPDATE,DELETE,TRUNCATE,TRIGGER') OR has_any_column_privilege(a.oid,t.oid,'UPDATE') OR (has_any_column_privilege(a.oid,t.oid,'INSERT') AND t.relname<>'reasoning_history'))))))) \
AND current_setting('fsync')='on' AND current_setting('full_page_writes')='on' AND current_setting('synchronous_commit')='on'";

const INSPECT_HISTORY_SQL: &str = "SELECT \
(EXISTS(SELECT 1 FROM trajectory.terrain_history WHERE ((engagement_id=$1 AND campaign_id=$2 AND operation_id=$3) OR event_id=$4) AND status='anomaly') \
 OR EXISTS(SELECT 1 FROM terrain.integrity_conflicts WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 AND blocks_use)), \
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

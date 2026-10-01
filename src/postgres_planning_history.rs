//! Trajectory-owned planning journal; never queries Mission storage.

use crate::mission::MissionRegistered;
use crate::planning::PlanningAssessed;
use crate::postgres_trajectory_history::store_err;
use crate::trajectory::{Delivered, check_planning_predecessor};
use crate::{Fail, Res};
use postgres::{Client, GenericClient, Row, Transaction};
use serde_json::Value;
use uuid::Uuid;

const OBLIGATION: &str = "trajectory.planning_history.v1";

pub(super) fn qualify(client: &mut Client, publish: bool) -> Res<()> {
    let row = client
        .query_one(
            "SELECT current_setting('fsync')='on' AND current_setting('full_page_writes')='on' \
         AND current_setting('synchronous_commit')='on' AND NOT rolsuper AND NOT rolbypassrls \
         AND has_table_privilege(current_user,'trajectory.planning_history','SELECT') \
         AND has_table_privilege(current_user,'trajectory.registration_history','SELECT') \
         AND (NOT $1 OR has_table_privilege(current_user,'trajectory.planning_history','INSERT')) \
         AND NOT has_schema_privilege(current_user,'trajectory','CREATE') \
         AND NOT EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace \
         WHERE n.nspname='trajectory' AND c.relkind='r' AND \
         (pg_has_role(current_user,c.relowner,'USAGE') OR \
         has_table_privilege(current_user,c.oid,'UPDATE,DELETE,TRUNCATE'))) \
         FROM pg_roles WHERE rolname=current_user",
            &[&publish],
        )
        .map_err(|e| store_err(&e))?;
    if row
        .try_get::<_, bool>(0)
        .map_err(|_| Fail::Store("contract_decode"))?
    {
        Ok(())
    } else {
        Err(Fail::Config("unsafe_history_role"))
    }
}

fn accepted(row: &Row, incoming: &PlanningAssessed) -> Res<bool> {
    let raw: Value = row
        .try_get("contract")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let stored: PlanningAssessed =
        serde_json::from_value(raw).map_err(|_| Fail::Store("contract_decode"))?;
    stored.validate()?;
    let event: Uuid = row
        .try_get("event_id")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let operation: Uuid = row
        .try_get("operation_id")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let catalog = (
        stored.engagement_id,
        stored.campaign_id,
        stored.event_id.0,
        stored.operation_id.0,
    );
    if catalog
        != (
            incoming.engagement_id,
            incoming.campaign_id,
            event,
            operation,
        )
    {
        return Err(Fail::Store("contract_decode"));
    }
    Ok(stored == *incoming)
}

pub(super) fn existing(
    client: &mut impl GenericClient,
    ev: &PlanningAssessed,
) -> Res<Option<Delivered>> {
    let rows = client
        .query(
            "SELECT status, event_id, operation_id, contract FROM trajectory.planning_history \
         WHERE engagement_id=$1 AND campaign_id=$2 AND (event_id=$3 OR operation_id=$4)",
            &[
                &ev.engagement_id.0,
                &ev.campaign_id.0,
                &ev.event_id.0,
                &ev.operation_id.0,
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
            "accepted" if accepted(&row, ev)? => result = Some(Delivered::Completed),
            "accepted" => return Ok(Some(Delivered::Anomaly)),
            _ => return Err(Fail::Store("contract_decode")),
        }
    }
    Ok(result)
}

fn predecessor(tx: &mut Transaction, ev: &PlanningAssessed) -> Res<Option<&'static str>> {
    let Some(basis) = &ev.basis else {
        return Ok(None);
    };
    let rows = tx.query(
        "SELECT status, producer, obligation, event_id, contract FROM trajectory.registration_history \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
        &[&ev.engagement_id.0, &ev.campaign_id.0, &basis.registration_operation_id.0],
    ).map_err(|e| store_err(&e))?;
    if rows.is_empty() {
        return Ok(Some("missing_predecessor"));
    }
    if rows.len() != 1 {
        return Ok(Some("unsupported_predecessor"));
    }
    let row = &rows[0];
    let status: &str = row
        .try_get("status")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let producer: &str = row
        .try_get("producer")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let obligation: &str = row
        .try_get("obligation")
        .map_err(|_| Fail::Store("contract_decode"))?;
    if (status, producer, obligation)
        != ("accepted", "mission", "trajectory.registration_history.v1")
    {
        return Ok(Some("unsupported_predecessor"));
    }
    let raw: Value = row
        .try_get("contract")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let registered: MissionRegistered =
        serde_json::from_value(raw).map_err(|_| Fail::Store("contract_decode"))?;
    let event: Uuid = row
        .try_get("event_id")
        .map_err(|_| Fail::Store("contract_decode"))?;
    if registered.event_id.0 != event {
        return Ok(Some("unsupported_predecessor"));
    }
    Ok(check_planning_predecessor(&registered, ev).err())
}

fn anomaly(tx: &mut Transaction, ev: &PlanningAssessed) -> Res<()> {
    // Only idempotent safe conflict markers use DO NOTHING; accepted content never does.
    tx.execute(
        "INSERT INTO trajectory.planning_history \
         (engagement_id,campaign_id,operation_id,event_id,status,anomaly_category) \
         VALUES ($1,$2,$3,$4,'anomaly','conflicting_identity') ON CONFLICT DO NOTHING",
        &[
            &ev.engagement_id.0,
            &ev.campaign_id.0,
            &ev.operation_id.0,
            &ev.event_id.0,
        ],
    )
    .map_err(|e| store_err(&e))?;
    Ok(())
}

pub(super) fn append(tx: &mut Transaction, ev: &PlanningAssessed) -> Res<Delivered> {
    match existing(tx, ev)? {
        Some(Delivered::Anomaly) => {
            anomaly(tx, ev)?;
            return Ok(Delivered::Anomaly);
        }
        Some(_) => return Ok(Delivered::Duplicate),
        None => (),
    }
    if let Some(reason) = predecessor(tx, ev)? {
        return Ok(Delivered::Unresolved(reason));
    }
    let contract = serde_json::to_value(ev).map_err(|_| Fail::Store("encode"))?;
    tx.execute(
        "INSERT INTO trajectory.planning_history \
         (engagement_id,campaign_id,operation_id,event_id,status,contract,completed_at,obligation) \
         VALUES ($1,$2,$3,$4,'accepted',$5,transaction_timestamp(),$6)",
        &[
            &ev.engagement_id.0,
            &ev.campaign_id.0,
            &ev.operation_id.0,
            &ev.event_id.0,
            &contract,
            &OBLIGATION,
        ],
    )
    .map_err(|e| store_err(&e))?;
    Ok(Delivered::Completed)
}

pub(super) fn commit_error(e: &postgres::Error) -> Fail {
    match store_err(e) {
        Fail::Store("serialization_retry") => Fail::Store("serialization_retry"),
        _ => Fail::Store("commit_unknown"),
    }
}

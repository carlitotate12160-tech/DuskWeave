//! Planning-assessed history adapter over the Trajectory journal: role and
//! durability qualification, the strict typed contract codec, predecessor
//! interpretation and append orchestration. Never queries Mission storage.

use crate::mission::MissionRegistered;
use crate::planning::PlanningAssessed;
use crate::postgres_trajectory_history::store_err;
use crate::postgres_trajectory_journal::{self, JournalRecord, JournalTable};
use crate::trajectory::{Delivered, check_planning_predecessor};
use crate::{Fail, Res};
use postgres::{Client, GenericClient, Transaction};
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

/// Canonical journal record of an already validated planning contract.
fn record(ev: &PlanningAssessed) -> Res<JournalRecord> {
    let contract = serde_json::to_value(ev).map_err(|_| Fail::Store("encode"))?;
    Ok(JournalRecord {
        engagement_id: ev.engagement_id,
        campaign_id: ev.campaign_id,
        event_id: ev.event_id,
        operation_id: ev.operation_id,
        producer: ev.producer.clone(),
        kind: ev.kind.clone(),
        version: ev.version,
        obligation: OBLIGATION.into(),
        contract,
    })
}

/// Strict typed decode of a stored contract into its validated canonical
/// record. Bounded categories are preserved and corruption is never
/// reinterpreted as absence or conflict: an invalid basis remains
/// unsupported_basis and every other decode/validation defect is
/// contract_decode.
fn decode(raw: Value) -> Res<JournalRecord> {
    let stored: PlanningAssessed =
        serde_json::from_value(raw).map_err(|_| Fail::Store("contract_decode"))?;
    stored.validate().map_err(|error| match error {
        Fail::Store("unsupported_basis") => error,
        _ => Fail::Store("contract_decode"),
    })?;
    record(&stored)
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
    interpret_registration_predecessor(&rows[0], ev)
}

/// A single registration row is a valid predecessor only when its catalog
/// header is the accepted mission record, its payload decodes as the typed
/// MissionRegistered contract, and the catalog event identity equals the
/// payload's. The header check precedes payload decoding.
fn interpret_registration_predecessor(
    row: &postgres::Row,
    ev: &PlanningAssessed,
) -> Res<Option<&'static str>> {
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
    let (registered, event) = decode_registration_predecessor(row)?;
    if registered.event_id.0 != event {
        return Ok(Some("unsupported_predecessor"));
    }
    Ok(check_planning_predecessor(&registered, ev).err())
}

/// Strict decode order: raw contract value, typed MissionRegistered
/// payload, then the catalog event identity column.
fn decode_registration_predecessor(row: &postgres::Row) -> Res<(MissionRegistered, Uuid)> {
    let raw: Value = row
        .try_get("contract")
        .map_err(|_| Fail::Store("contract_decode"))?;
    let registered: MissionRegistered =
        serde_json::from_value(raw).map_err(|_| Fail::Store("contract_decode"))?;
    let event: Uuid = row
        .try_get("event_id")
        .map_err(|_| Fail::Store("contract_decode"))?;
    Ok((registered, event))
}

pub(super) fn existing(
    client: &mut impl GenericClient,
    ev: &PlanningAssessed,
) -> Res<Option<Delivered>> {
    postgres_trajectory_journal::existing(client, JournalTable::Planning, &record(ev)?, decode)
}

/// An existing anomaly dominates and records its anomaly marker; any other
/// existing outcome is a duplicate; absence returns None.
fn existing_result(tx: &mut Transaction, record: &JournalRecord) -> Res<Option<Delivered>> {
    match postgres_trajectory_journal::existing(tx, JournalTable::Planning, record, decode)? {
        Some(Delivered::Anomaly) => {
            postgres_trajectory_journal::append_anomaly(tx, JournalTable::Planning, record)?;
            Ok(Some(Delivered::Anomaly))
        }
        Some(_) => Ok(Some(Delivered::Duplicate)),
        None => Ok(None),
    }
}

pub(super) fn append(tx: &mut Transaction, ev: &PlanningAssessed) -> Res<Delivered> {
    let record = record(ev)?;
    if let Some(result) = existing_result(tx, &record)? {
        return Ok(result);
    }
    if let Some(reason) = predecessor(tx, ev)? {
        return Ok(Delivered::Unresolved(reason));
    }
    postgres_trajectory_journal::append_accepted(tx, JournalTable::Planning, &record)?;
    Ok(Delivered::Completed)
}

pub(super) fn commit_error(e: &postgres::Error) -> Fail {
    match store_err(e) {
        Fail::Store("serialization_retry") => Fail::Store("serialization_retry"),
        _ => Fail::Store("commit_unknown"),
    }
}

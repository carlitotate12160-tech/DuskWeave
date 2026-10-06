use super::postgres_withdrawal::current_marker_for_scope;
use super::{PgMissionStore, REGISTRATION_READ, bind_registration, decode_registration, store_err};
use crate::m1_policy::{M1PolicyReader, M1PolicySnapshot};
use crate::mission::{CampaignId, EngagementId, MissionRegistered};
use crate::{Fail, Res};
use postgres::{IsolationLevel, Transaction};

fn registration(
    tx: &mut Transaction,
    e: EngagementId,
    c: CampaignId,
    operation: Option<uuid::Uuid>,
) -> Res<Option<MissionRegistered>> {
    let Some(operation) = operation else {
        return Ok(None);
    };
    let row = tx
        .query_opt(REGISTRATION_READ, &[&e.0, &c.0, &operation])
        .map_err(|error| store_err(&error))?
        .ok_or(Fail::Store("contract_decode"))?;
    let event = decode_registration(&row)?;
    if event.version == crate::mission::CONTRACT_VERSION {
        bind_registration(&row, &event)?;
    }
    Ok(Some(event))
}

impl M1PolicyReader for PgMissionStore {
    fn read_policy_snapshot(&mut self, e: EngagementId, c: CampaignId) -> Res<M1PolicySnapshot> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .map_err(|error| store_err(&error))?;
        let row = tx.query_one(
            "SELECT m.operation_id, floor(extract(epoch FROM statement_timestamp()))::bigint \
             FROM (SELECT 1) anchor LEFT JOIN mission.missions m ON m.engagement_id=$1 AND m.campaign_id=$2",
            &[&e.0, &c.0]).map_err(|error| store_err(&error))?;
        let event = registration(&mut tx, e, c, row.get(0))?;
        let marker = current_marker_for_scope(&mut tx, e, c)?;
        let snapshot = M1PolicySnapshot::new(e, c, event, marker, row.get(1))?;
        tx.commit().map_err(|error| store_err(&error))?;
        Ok(snapshot)
    }
}

//! Narrow withdrawal-predecessor qualification for version-3 planning
//! history. Reads only Trajectory-owned history; never queries Mission
//! storage. A v3 refusal completes only after its referenced withdrawal is
//! accepted and completed under the fixed withdrawal contract.

use crate::planning::{PlanningAssessed, WithdrawalBasis};
use crate::postgres_trajectory_history::store_err;
use crate::withdrawal::MissionAuthorityWithdrawn;
use crate::{Fail, Res};
use postgres::Transaction;
use serde_json::Value;

/// Qualify the referenced withdrawal for a v3 event. Non-v3 events carry no
/// withdrawal basis and pass through with no additional requirement.
pub(super) fn withdrawal_predecessor(
    tx: &mut Transaction,
    ev: &PlanningAssessed,
) -> Res<Option<&'static str>> {
    let Some(withdrawal) = &ev.withdrawal else {
        return Ok(None);
    };
    let rows = tx
        .query(
            "SELECT status='accepted' AND producer='mission' AND version=1 \
             AND kind='mission_authority_withdrawn' AND completed_at IS NOT NULL \
             AND obligation='trajectory.withdrawal_history.v1', contract, \
             jsonb_build_object('engagement_id',engagement_id,'campaign_id',campaign_id, \
             'event_id',event_id,'operation_id',operation_id) \
             FROM trajectory.withdrawal_history \
             WHERE engagement_id=$1 AND campaign_id=$2 \
             AND (event_id=$3 OR operation_id=$4) LIMIT 2",
            &[
                &ev.engagement_id.0,
                &ev.campaign_id.0,
                &withdrawal.event_id.0,
                &withdrawal.operation_id.0,
            ],
        )
        .map_err(|e| store_err(&e))?;
    if rows.is_empty() {
        return Ok(Some("missing_predecessor"));
    }
    if rows.len() != 1 {
        return Ok(Some("unsupported_predecessor"));
    }
    check_withdrawal_predecessor(&rows[0], ev, withdrawal)
}

fn check_withdrawal_predecessor(
    row: &postgres::Row,
    ev: &PlanningAssessed,
    withdrawal: &WithdrawalBasis,
) -> Res<Option<&'static str>> {
    if !row.get::<_, bool>(0) {
        return Ok(Some("unsupported_predecessor"));
    }
    let stored = decode_withdrawal(row)?;
    if withdrawal_binds(&stored, ev, withdrawal, row) {
        Ok(None)
    } else {
        Ok(Some("unsupported_predecessor"))
    }
}

fn decode_withdrawal(row: &postgres::Row) -> Res<MissionAuthorityWithdrawn> {
    let stored: MissionAuthorityWithdrawn =
        serde_json::from_value(row.get(1)).map_err(|_| Fail::Store("contract_decode"))?;
    stored
        .validate()
        .map_err(|_| Fail::Store("contract_decode"))?;
    Ok(stored)
}

fn withdrawal_binds(
    stored: &MissionAuthorityWithdrawn,
    ev: &PlanningAssessed,
    withdrawal: &WithdrawalBasis,
    row: &postgres::Row,
) -> bool {
    let catalog = serde_json::json!({
        "engagement_id": stored.request.engagement_id, "campaign_id": stored.request.campaign_id,
        "event_id": stored.event_id, "operation_id": stored.operation_id,
    });
    let Some(basis) = &ev.basis else {
        return false;
    };
    let ids_match =
        (stored.operation_id, stored.event_id) == (withdrawal.operation_id, withdrawal.event_id);
    let scope_matches = (
        stored.request.engagement_id,
        stored.request.campaign_id,
        stored.registration_operation_id,
    ) == (
        ev.engagement_id,
        ev.campaign_id,
        basis.registration_operation_id,
    );
    ids_match
        && scope_matches
        && row.get::<_, Value>(2) == catalog
        && !stored.event_id.0.is_nil()
        && !stored.operation_id.0.is_nil()
}

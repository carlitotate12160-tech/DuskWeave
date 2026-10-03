//! Immediately consumed current-Mission row reader. A private member of the
//! Mission adapter family, called inside the producer's existing SERIALIZABLE
//! assessment transaction after the duplicate/recovery lookup. Malformed
//! owner data fails safely; a missing row stays an absent basis.

use super::store_err;
use crate::mission::{AssetRef, ExerciseMode, GoalRef, OperationId};
use crate::planning::{MissionBasis, MissionScope, PlanningRequest};
use crate::{Fail, Res};

pub(super) fn current_basis(
    tx: &mut postgres::Transaction,
    request: &PlanningRequest,
) -> Res<Option<MissionBasis>> {
    if tx
        .query_opt(
            "SELECT 1 FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",
            &[&request.engagement_id.0, &request.campaign_id.0],
        )
        .map_err(|error| store_err(&error))?
        .is_some()
    {
        return Err(Fail::State("authority_withdrawn"));
    }
    let row = tx
        .query_opt(
            "SELECT operation_id, revision, exercise_mode, starts_at, ends_at, \
             goal_ref, included_assets, excluded_assets FROM mission.missions \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&request.engagement_id.0, &request.campaign_id.0],
        )
        .map_err(|error| store_err(&error))?;
    row.map(decode_basis).transpose()
}

fn decode_basis(row: postgres::Row) -> Res<MissionBasis> {
    let revision: i64 = row.try_get(1).map_err(|_| Fail::Store("contract_decode"))?;
    if revision <= 0 {
        return Err(Fail::Store("unsupported_basis"));
    }
    let mode: String = row.try_get(2).map_err(|_| Fail::Store("contract_decode"))?;
    let exercise_mode = match mode.as_str() {
        "blind" => ExerciseMode::Blind,
        "defender_informed" => ExerciseMode::DefenderInformed,
        _ => return Err(Fail::Store("unsupported_basis")),
    };
    let scope = MissionScope {
        goal_ref: GoalRef(row.try_get(5).map_err(|_| Fail::Store("contract_decode"))?),
        included_assets: decode_assets(&row, 6)?,
        excluded_assets: decode_assets(&row, 7)?,
    };
    let basis = MissionBasis {
        registration_operation_id: OperationId(
            row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?,
        ),
        revision: revision as u64,
        exercise_mode,
        starts_at: row.try_get(3).map_err(|_| Fail::Store("contract_decode"))?,
        ends_at: row.try_get(4).map_err(|_| Fail::Store("contract_decode"))?,
        scope: Some(scope),
    };
    basis.validate()?;
    Ok(basis)
}

fn decode_assets(row: &postgres::Row, column: usize) -> Res<Vec<AssetRef>> {
    let raw: serde_json::Value = row
        .try_get(column)
        .map_err(|_| Fail::Store("contract_decode"))?;
    serde_json::from_value(raw).map_err(|_| Fail::Store("contract_decode"))
}

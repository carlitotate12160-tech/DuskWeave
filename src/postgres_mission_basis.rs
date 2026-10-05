//! Immediately consumed current-Mission row reader plus the scoped withdrawal
//! marker, returned as one adapter-local pair inside the producer's existing
//! SERIALIZABLE assessment transaction after the duplicate/recovery lookup.
//! Malformed owner data fails safely; a missing row stays an absent basis.
//! A marker with a missing/corrupt registration is the caller's unsupported
//! state, never a fabricated durable refusal here.

use super::store_err;
use crate::mission::{AssetRef, ExerciseMode, GoalRef, OperationId};
use crate::planning::{MissionBasis, MissionScope, PlanningRequest};
use crate::withdrawal::MissionAuthorityWithdrawn;
use crate::{Fail, Res};

/// Narrow adapter-local pair of the current registration basis and the
/// optional validated withdrawal marker, both read inside the caller's
/// transaction. Only the main adapter decides which event version follows.
pub(super) struct CurrentAuthority {
    pub(super) basis: Option<MissionBasis>,
    pub(super) withdrawal: Option<MissionAuthorityWithdrawn>,
}

pub(super) fn current_authority(
    tx: &mut postgres::Transaction,
    request: &PlanningRequest,
) -> Res<CurrentAuthority> {
    // The withdrawal-absence SSI read stays in place even when the marker is
    // absent, preserving the producer's race behavior against fresh withdrawal.
    let withdrawal = super::postgres_withdrawal::current_marker(tx, request)?;
    let basis = read_basis(tx, request)?;
    Ok(CurrentAuthority { basis, withdrawal })
}

fn read_basis(
    tx: &mut postgres::Transaction,
    request: &PlanningRequest,
) -> Res<Option<MissionBasis>> {
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
    let (revision, exercise_mode) = decode_basis_header(&row)?;
    let scope = decode_scope(&row)?;
    let basis = MissionBasis {
        registration_operation_id: OperationId(
            row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?,
        ),
        revision,
        exercise_mode,
        starts_at: row.try_get(3).map_err(|_| Fail::Store("contract_decode"))?,
        ends_at: row.try_get(4).map_err(|_| Fail::Store("contract_decode"))?,
        scope: Some(scope),
    };
    basis.validate()?;
    Ok(basis)
}

fn decode_basis_header(row: &postgres::Row) -> Res<(u64, ExerciseMode)> {
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
    Ok((revision as u64, exercise_mode))
}

fn decode_scope(row: &postgres::Row) -> Res<MissionScope> {
    Ok(MissionScope {
        goal_ref: GoalRef(row.try_get(5).map_err(|_| Fail::Store("contract_decode"))?),
        included_assets: decode_assets(row, 6)?,
        excluded_assets: decode_assets(row, 7)?,
    })
}

fn decode_assets(row: &postgres::Row, column: usize) -> Res<Vec<AssetRef>> {
    let raw: serde_json::Value = row
        .try_get(column)
        .map_err(|_| Fail::Store("contract_decode"))?;
    serde_json::from_value(raw).map_err(|_| Fail::Store("contract_decode"))
}

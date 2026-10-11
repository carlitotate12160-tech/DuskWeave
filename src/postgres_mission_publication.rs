//! Read-only Mission publication provenance through the owner's adapter.
//! The caller owns its transaction; no mutable Mission aggregate is imported.
use super::{REGISTRATION_READ, decode_registration, store_err};
use crate::mission::MissionRegistered;
use crate::{Fail, Res};
use postgres::GenericClient;
use serde_json::Value;

pub(crate) fn source_matches(
    client: &mut impl GenericClient,
    event: &MissionRegistered,
) -> Res<bool> {
    let row = client
        .query_opt(
            REGISTRATION_READ,
            &[
                &event.engagement_id.0,
                &event.campaign_id.0,
                &event.operation_id.0,
            ],
        )
        .map_err(|e| store_err(&e))?;
    row.map(|row| decode_registration(&row).map(|source| source == *event))
        .transpose()
        .map(|value| value == Some(true))
}

/// Reject extra or normalized-away v3 content at the owner publication boundary.
pub(super) fn check_contract(raw: &Value, event: &MissionRegistered) -> Res<()> {
    if event.version == 3
        && raw != &serde_json::to_value(event).map_err(|_| Fail::Store("encode"))?
    {
        return Err(Fail::Store("contract_decode"));
    }
    Ok(())
}

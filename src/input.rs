//! Input boundary: strict bounded register parsing. Rejects malformed,
//! unknown or sensitive input before domain code/SQL; no rejected echo.

use crate::mission::*;
use crate::{Fail, Res};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

pub const MAX_INPUT_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterDto {
    engagement_id: Uuid,
    campaign_id: Uuid,
    operator_ref: Uuid,
    authority_ref: Uuid,
    authority_revision: i64,
    goal_ref: Uuid,
    included_assets: Vec<Uuid>,
    excluded_assets: Vec<Uuid>,
    exercise_mode: ExerciseMode,
    starts_at: i64,
    ends_at: i64,
}

pub fn read_register_file(path: &Path) -> Res<RegistrationInput> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_register(&raw)
}

pub fn parse_register(raw: &[u8]) -> Res<RegistrationInput> {
    if raw.is_empty() || raw.len() > MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    let dto: RegisterDto = serde_json::from_slice(raw).map_err(|e| {
        if e.is_syntax() {
            Fail::Input("malformed_json")
        } else {
            Fail::Input("schema_violation")
        }
    })?;
    build(dto)
}

fn build(dto: RegisterDto) -> Res<RegistrationInput> {
    for u in [
        dto.engagement_id,
        dto.campaign_id,
        dto.operator_ref,
        dto.authority_ref,
        dto.goal_ref,
    ] {
        if u.is_nil() {
            return Err(Fail::Input("nil_reference"));
        }
    }
    if dto.authority_revision <= 0 {
        return Err(Fail::Input("invalid_revision"));
    }
    let fields = RegistrationFields::new(
        OperatorRef(dto.operator_ref),
        AuthorityRef(dto.authority_ref),
        dto.authority_revision as u64,
        GoalRef(dto.goal_ref),
        dto.included_assets.iter().map(|u| AssetRef(*u)).collect(),
        dto.excluded_assets.iter().map(|u| AssetRef(*u)).collect(),
        dto.exercise_mode,
        dto.starts_at,
        dto.ends_at,
    )?;
    Ok(RegistrationInput {
        engagement_id: EngagementId(dto.engagement_id),
        campaign_id: CampaignId(dto.campaign_id),
        fields,
    })
}

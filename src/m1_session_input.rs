//! Strict bounded session ingress: exactly the four fixed fields, 16 KiB
//! limit, expected revision 1, no rejected echo of input content.

use crate::m1_session::SessionRequest;
use crate::{Fail, Res};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;

pub const MAX_INPUT_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDto {
    engagement_id: crate::mission::EngagementId,
    campaign_id: crate::mission::CampaignId,
    operator_ref: crate::mission::OperatorRef,
    expected_mission_revision: u64,
}

pub fn parse_session_request(raw: &[u8]) -> Res<SessionRequest> {
    if raw.is_empty() || raw.len() > MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    let dto: SessionDto = serde_json::from_slice(raw).map_err(|error| {
        if error.is_syntax() {
            Fail::Input("malformed_json")
        } else {
            Fail::Input("schema_violation")
        }
    })?;
    let request = SessionRequest {
        engagement_id: dto.engagement_id,
        campaign_id: dto.campaign_id,
        operator_ref: dto.operator_ref,
        expected_mission_revision: dto.expected_mission_revision,
    };
    request.validate()?;
    Ok(request)
}

pub fn read_session_file(path: &Path) -> Res<SessionRequest> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_session_request(&raw)
}

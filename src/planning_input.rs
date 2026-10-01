//! Strict bounded input for the local assessment command.

use crate::input::MAX_INPUT_BYTES;
use crate::planning::PlanningRequest;
use crate::{Fail, Res};
use std::io::Read;
use std::path::Path;

pub fn read_planning_file(path: &Path) -> Res<PlanningRequest> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_planning(&raw)
}

pub fn parse_planning(raw: &[u8]) -> Res<PlanningRequest> {
    if raw.is_empty() || raw.len() > MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    let request: PlanningRequest = serde_json::from_slice(raw).map_err(|error| {
        if error.is_syntax() || error.is_eof() {
            Fail::Input("malformed_json")
        } else {
            Fail::Input("schema_violation")
        }
    })?;
    request.validate()?;
    Ok(request)
}

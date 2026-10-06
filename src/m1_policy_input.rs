use crate::m1_policy::M1PolicyQuery;
use crate::{Fail, Res};
use std::io::Read;
use std::path::Path;

pub const MAX_INPUT_BYTES: usize = 16_384;

pub fn parse_policy_query(raw: &[u8]) -> Res<M1PolicyQuery> {
    if raw.is_empty() || raw.len() > MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    let query: M1PolicyQuery = serde_json::from_slice(raw).map_err(|error| {
        if error.is_syntax() {
            Fail::Input("malformed_json")
        } else {
            Fail::Input("schema_violation")
        }
    })?;
    query.validate()?;
    Ok(query)
}

pub fn read_policy_file(path: &Path) -> Res<M1PolicyQuery> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_policy_query(&raw)
}

//! Strict bounded withdrawal ingress; rejected bytes never enter diagnostics.
use crate::input::MAX_INPUT_BYTES;
use crate::withdrawal::WithdrawalRequest;
use crate::{Fail, Res};
use std::io::Read;
use std::path::Path;

pub fn read_withdrawal_file(path: &Path) -> Res<WithdrawalRequest> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_withdrawal(&raw)
}

pub fn parse_withdrawal(raw: &[u8]) -> Res<WithdrawalRequest> {
    if raw.is_empty() || raw.len() > MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    let request: WithdrawalRequest = serde_json::from_slice(raw).map_err(|error| {
        if error.is_syntax() || error.is_eof() {
            Fail::Input("malformed_json")
        } else {
            Fail::Input("schema_violation")
        }
    })?;
    request.validate()?;
    Ok(request)
}

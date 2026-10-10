//! Bounded, strict session input before any database access; rejected bytes
//! and paths are never echoed.
use crate::m1_session::SessionRequest;
use crate::{Fail, Res};
use std::io::Read;
use std::path::Path;

pub fn parse_session(raw: &[u8]) -> Res<SessionRequest> {
    if raw.is_empty() || raw.len() > crate::input::MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    serde_json::from_slice(raw).map_err(|_| Fail::Input("invalid_session_request"))
}

pub fn read_session_file(path: &Path) -> Res<SessionRequest> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((crate::input::MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_session(&raw)
}

pub fn parse_prepared_withdrawal(
    raw: &[u8],
) -> Res<crate::m1_prepared_withdrawal::PreparedWithdrawalRequest> {
    if raw.is_empty() || raw.len() > crate::input::MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    serde_json::from_slice(raw).map_err(|_| Fail::Input("invalid_prepared_withdrawal"))
}

pub fn read_prepared_withdrawal_file(
    path: &Path,
) -> Res<crate::m1_prepared_withdrawal::PreparedWithdrawalRequest> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((crate::input::MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_prepared_withdrawal(&raw)
}

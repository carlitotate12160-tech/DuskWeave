//! Binary-private CLI ingress: fresh local-authority confirmation for a new
//! assessment. A saved `current_authority_confirmed` boolean is an assertion
//! inside the stored request, never proof of current authority; a brand-new
//! assessment requires one bounded live exchange on stderr/stdin bound to a
//! transient challenge issued by this invocation. Nothing here is persisted,
//! logged, or reusable across invocations; the unchanged Mission assessment
//! runs only after the exchange succeeds.

use duskweave::mission::OperationId;
use duskweave::planning::{PlanningAssessed, PlanningRequest};
use duskweave::planning_assessment::{self, PlanningStore};
use duskweave::planning_history;
use duskweave::registration::OperationAllocator;
use duskweave::{Fail, Res};
use serde::Deserialize;
use std::io::{BufRead, Read, Write};

const STATEMENT: &str = "current_authority_within_original_bounds_and_no_unreconciled_withdrawal";
const MAX_RESPONSE_BYTES: usize = 1024;

fn denied() -> Fail {
    Fail::State("authority_confirmation_failed")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    challenge_id: uuid::Uuid,
    statement: String,
}

fn emit_challenge(
    out: &mut impl Write,
    request: &PlanningRequest,
    operation: OperationId,
    challenge_id: uuid::Uuid,
) -> Res<()> {
    let line = serde_json::json!({
        "action": "confirm_current_authority",
        "engagement_id": request.engagement_id,
        "campaign_id": request.campaign_id,
        "operation_id": operation,
        "expected_mission_revision": request.expected_mission_revision,
        "challenge_id": challenge_id,
        "statement": STATEMENT,
    });
    writeln!(out, "{line}")
        .and_then(|()| out.flush())
        .map_err(|_| denied())
}

/// Read one newline-delimited response, bounded to MAX_RESPONSE_BYTES
/// including the line ending (LF or CRLF). Never waits for EOF after a
/// complete line; EOF before one completes denies the exchange.
fn read_response(input: &mut impl BufRead) -> Res<Vec<u8>> {
    let mut line = Vec::new();
    input
        .by_ref()
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_until(b'\n', &mut line)
        .map_err(|_| denied())?;
    if line.len() > MAX_RESPONSE_BYTES || line.last() != Some(&b'\n') {
        return Err(denied());
    }
    line.pop();
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Ok(line)
}

fn verify_response(input: &mut impl BufRead, challenge_id: uuid::Uuid) -> Res<()> {
    let response: Response =
        serde_json::from_slice(&read_response(input)?).map_err(|_| denied())?;
    if response.challenge_id.is_nil()
        || response.challenge_id != challenge_id
        || response.statement != STATEMENT
    {
        return Err(denied());
    }
    Ok(())
}

/// One bounded exchange: a transient non-nil challenge on stderr, then a
/// single strict response. Any failure denies the new assessment attempt
/// without effects; nothing is stored or reused.
fn fresh_confirmation(
    allocator: &mut dyn OperationAllocator,
    request: &PlanningRequest,
    operation: OperationId,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Res<()> {
    let challenge_id = allocator.allocate()?;
    if challenge_id.is_nil() {
        return Err(denied());
    }
    emit_challenge(out, request, operation, challenge_id)?;
    verify_response(input, challenge_id)
}

/// `assess` ingress guard. `recover=true` and already-durable decisions read
/// through the unchanged recovery path with no interaction, preserving the
/// historic producer receipt. A new `current_authority_confirmed=false`
/// request follows the existing unconfirmed assessment path with no prompt.
/// Only a brand-new `true` request pays the fresh live confirmation before
/// the ordinary Mission assessment; confirmation overrides no guard.
pub(super) fn confirm_if_new(
    allocator: &mut dyn OperationAllocator,
    store: &mut impl PlanningStore,
    request: &PlanningRequest,
    operation: OperationId,
    recover: bool,
    input: &mut impl BufRead,
    challenge_out: &mut impl Write,
) -> Res<Option<PlanningAssessed>> {
    if recover {
        return planning_history::read_decision(store, request, operation);
    }
    if let Some(existing) = planning_history::read_decision(store, request, operation)? {
        return Ok(Some(existing));
    }
    if request.current_authority_confirmed {
        fresh_confirmation(allocator, request, operation, input, challenge_out)?;
    }
    planning_assessment::assess(allocator, store, request, operation, false)
}

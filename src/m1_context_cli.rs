//! Read-only preparation ingress and exact immutable pack bytes in a safe receipt.
use duskweave::m1_context;
use duskweave::m1_context_input::ContextRequest;
use duskweave::mission::OperationId;
use duskweave::postgres_mission::{PgAllocator, PgContextReader};
use duskweave::registration::OperationAllocator;
use duskweave::{Fail, Res};
use std::io::Read;

pub(super) fn command(args: &[String]) -> Res<()> {
    let flags = super::flags(args, &["input"])?;
    let q = read_request(super::flag(&flags, "input")?)?;
    let op = OperationId(PgAllocator::new(super::connect()?).allocate()?);
    let outcome = m1_context::prepare(&mut PgContextReader::new(super::connect()?), &q, op);
    emit_receipt(op, &outcome);
    outcome.map(|_| ())
}

// Input admission is complete before allocating identities or opening database ports.
fn read_request(path: &str) -> Res<ContextRequest> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((duskweave::input::MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    if raw.is_empty() || raw.len() > duskweave::input::MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    let q: ContextRequest =
        serde_json::from_slice(&raw).map_err(|_| Fail::Input("invalid_context_input"))?;
    q.validate_ingress()?;
    Ok(q)
}

fn emit_receipt(op: OperationId, outcome: &Res<m1_context::PreparedContext>) {
    let status = match outcome {
        Ok(_) => "prepared",
        Err(Fail::State(_)) => "deferred",
        Err(Fail::Input(_)) => "refused",
        _ => "unavailable",
    };
    // codeql[rust/cleartext-logging] only admitted opaque metadata and compiled question, never raw ingress
    println!(
        "{}",
        serde_json::json!({"kind":"m1_context_preparation_receipt_v1","decision_id":op,
        "outcome":status,"pack_utf8":outcome.as_ref().ok().map(|p|p.utf8()),
        "current_permission":false,"dispatch_granted":false,"provider_qualified":false,
        "acquisition_qualified":false,"send_qualified":false,"reservation_granted":false,
        "decision_published":false,"history_completed":false,"request_binding_qualified":false})
    );
}

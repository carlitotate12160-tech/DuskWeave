//! Pre-send reservation ingress and safe receipt, never cognitive or dispatch success.
use duskweave::m1_inference_budget::{self, ReservationAction, ReservationRequest};
use duskweave::mission::OperationId;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::{Fail, Res};
use std::io::Read;
use std::path::Path;

fn action(raw: &str) -> Res<ReservationAction> {
    match raw {
        "reserve" => Ok(ReservationAction::Reserve),
        "recover" => Ok(ReservationAction::Recover),
        "cancel" => Ok(ReservationAction::Cancel),
        _ => Err(Fail::Input("invalid_reservation_action")),
    }
}

fn read(path: &Path) -> Res<ReservationRequest> {
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((duskweave::input::MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    if raw.is_empty() || raw.len() > duskweave::input::MAX_INPUT_BYTES {
        return Err(Fail::Input("size_limit"));
    }
    serde_json::from_slice(&raw).map_err(|_| Fail::Input("invalid_inference_reservation"))
}

fn identity(f: &[(&str, &str)]) -> Res<(ReservationAction, OperationId)> {
    let action = action(super::flag(f, "action")?)?;
    let op =
        OperationId::parse(super::flag(f, "operation")?).ok_or(Fail::Input("invalid_operation"))?;
    Ok((action, op))
}

fn execute(
    action: ReservationAction,
    op: OperationId,
    q: &ReservationRequest,
) -> Res<Option<m1_inference_budget::ReservationRecord>> {
    let mut store = PgInferenceReservationStore::new(super::connect()?);
    let mut reader = matches!(action, ReservationAction::Reserve)
        .then(|| super::connect().map(PgMissionStore::new))
        .transpose()?;
    m1_inference_budget::reservation(&mut store, reader.as_mut(), action, q, op)
}

pub(super) fn command(args: &[String]) -> Res<()> {
    let f = super::flags(args, &["action", "operation", "input"])?;
    let (action, op) = identity(&f)?;
    let request = read(Path::new(super::flag(&f, "input")?))?;
    request.validate(op)?;
    let outcome = execute(action, op, &request);
    receipt(&request, op, &outcome);
    outcome.map(|_| ())
}

fn receipt(
    request: &ReservationRequest,
    op: OperationId,
    outcome: &Res<Option<m1_inference_budget::ReservationRecord>>,
) {
    let disposition = match outcome {
        Ok(Some(_)) => "durable",
        Ok(None) => "missing",
        Err(Fail::Unresolved(_)) => "unknown",
        _ => "refused",
    };
    // codeql[rust/cleartext-logging] validated non-sensitive IDs/capacity/status only
    println!(
        "{}",
        serde_json::json!({"kind":"m1_inference_reservation_receipt_v1",
        "request":request,"operation_id":op,"outcome":disposition,
        "record":outcome.as_ref().ok().and_then(|r|r.as_ref()),
        "current_permission":false,"dispatch_granted":false,"provider_qualified":false,
        "acquisition_qualified":false,"send_qualified":false})
    );
}

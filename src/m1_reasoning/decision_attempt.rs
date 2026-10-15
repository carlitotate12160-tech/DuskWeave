//! Generation/one-repair execution, with durable release and usage classification.
use super::inference::{Broker, Provider, Received, ReleaseFailure, Reservation, Scope};
use super::proposal::{Admission, ProposalBasis};
use super::request::{self, BoundCall};
use uuid::Uuid;
type Res<T> = Result<T, &'static str>;

fn call(
    broker: &mut impl Broker,
    provider: &mut impl Provider,
    basis: &mut impl ProposalBasis,
    scope: &Scope,
    reservation: &Reservation,
    call_id: Uuid,
    bound: &BoundCall,
) -> Result<Received, (bool, &'static str)> {
    provider
        .prepare(bound, reservation.shared_deadline)
        .map_err(|code| (false, code))?;
    if let Err(code) = broker.may_send(scope, call_id, bound) {
        let recorded = matches!(
            code,
            "inference_marker_commit_unknown" | "marker_commit_unknown"
        );
        return Err((recorded, code));
    }
    let result = broker.release_into(scope, reservation, call_id, bound, provider, basis);
    account(broker, scope, call_id, result)
}

fn account(
    broker: &mut impl Broker,
    scope: &Scope,
    call_id: Uuid,
    result: Result<Received, ReleaseFailure>,
) -> Result<Received, (bool, &'static str)> {
    match result {
        Ok(mut received) => {
            received.accounting_confirmed =
                broker.completed(scope, call_id, &received.usage).is_ok();
            if !received.accounting_confirmed {
                let _accounting_disposition =
                    broker.uncertain(scope, call_id, "usage_commit_unknown");
            }
            Ok(received)
        }
        Err(ReleaseFailure::NeverReleased(code)) => {
            broker
                .never_released(scope, call_id, code)
                .map_err(|e| (true, e))?;
            Err((true, code))
        }
        Err(ReleaseFailure::MayHaveSent(code)) => {
            broker
                .uncertain(scope, call_id, code)
                .map_err(|e| (true, e))?;
            Err((true, code))
        }
    }
}
// Preserve byte-binding membership identically for generation and repair.
fn record(
    result: Result<Received, (bool, &'static str)>,
    digest: String,
    bindings: &mut Vec<String>,
) -> Res<Received> {
    match result {
        Ok(received) => {
            bindings.push(digest);
            Ok(received)
        }
        Err((recorded, code)) => {
            if recorded {
                bindings.push(digest);
            }
            Err(code)
        }
    }
}
pub(super) fn decide(
    broker: &mut impl Broker,
    provider: &mut impl Provider,
    basis: &mut impl ProposalBasis,
    scope: &Scope,
    pack: &[u8],
) -> Res<(Admission, Vec<Uuid>, Vec<String>)> {
    let reservation = broker.reserve(scope, pack)?;
    let first = request::bind(pack, None)?;
    let mut bindings = Vec::new();
    let admission = generate(
        broker,
        provider,
        basis,
        scope,
        first,
        &reservation,
        &mut bindings,
    )
    .unwrap_or_else(Admission::Rejected);
    Ok((
        admission,
        vec![reservation.generation_call, reservation.repair_call],
        bindings,
    ))
}
fn generate(
    broker: &mut impl Broker,
    provider: &mut impl Provider,
    basis: &mut impl ProposalBasis,
    scope: &Scope,
    first: BoundCall,
    reservation: &Reservation,
    bindings: &mut Vec<String>,
) -> Res<Admission> {
    let result = call(
        broker,
        provider,
        basis,
        scope,
        reservation,
        reservation.generation_call,
        &first,
    );
    let received = record(result, first.digest, bindings)?;
    match generation_admission(received) {
        Admission::Repairable(codes) => {
            let repair = request::bind(&first.pack, Some(&codes))?;
            let result = call(
                broker,
                provider,
                basis,
                scope,
                reservation,
                reservation.repair_call,
                &repair,
            );
            Ok(record(result, repair.digest, bindings)?.admission)
        }
        admission => {
            // The unused slot remains conservatively charged.
            let _unused_slot_disposition = broker.close_unused_repair(scope, reservation);
            Ok(admission)
        }
    }
}

fn generation_admission(received: Received) -> Admission {
    // An admitted proposal survives uncertain accounting. Another send does not.
    if received.accounting_confirmed {
        return received.admission;
    }
    match received.admission {
        Admission::Accepted(p) => Admission::Accepted(p),
        _ => Admission::Rejected("accounting_unknown"),
    }
}

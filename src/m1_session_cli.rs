//! Prepared-session ingress for `duskweave m1-session`: prepare, recover and
//! release only. No endpoint, host command or effect grant exists here, and
//! every receipt keeps all four qualification flags false.
use duskweave::m1_session::{self, SessionAction, SessionRecord, SessionRequest};
use duskweave::m1_session_input::read_session_file;
use duskweave::mission::OperationId;
use duskweave::postgres_m1_session::PgSessionStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::{Fail, Res};
use std::path::Path;

fn action(raw: &str) -> Res<SessionAction> {
    match raw {
        "prepare" => Ok(SessionAction::Prepare),
        "recover" => Ok(SessionAction::Recover),
        "release" => Ok(SessionAction::Release),
        _ => Err(Fail::Input("invalid_session_action")),
    }
}

fn outcome_name(outcome: &Res<Option<SessionRecord>>) -> &'static str {
    match outcome {
        Ok(Some(_)) => "durable",
        Ok(None) => "missing",
        Err(Fail::Unresolved(_)) => "unknown",
        Err(_) => "refused",
    }
}

fn print_receipt(r: &SessionRequest, op: OperationId, outcome: &Res<Option<SessionRecord>>) {
    // codeql[rust/cleartext-logging] bounded receipt: safe IDs/status only
    let receipt = serde_json::json!({
        "session_kind": "m1_prepared_no_effects_v1",
        "engagement_id": r.engagement_id,
        "campaign_id": r.campaign_id,
        "operation_id": op,
        "record": outcome.as_ref().ok().and_then(|v| v.as_ref()),
        "outcome": outcome_name(outcome),
        "current_permission": false,
        "dispatch_granted": false,
        "acquisition_qualified": false,
        "host_control_qualified": false,
    });
    println!("{receipt}");
}

fn identity(flags: &[(&str, &str)]) -> Res<(SessionAction, OperationId)> {
    let action = action(super::flag(flags, "action")?)?;
    let op = OperationId::parse(super::flag(flags, "operation")?)
        .ok_or(Fail::Input("invalid_operation"))?;
    Ok((action, op))
}

fn parse(args: &[String]) -> Res<(SessionAction, OperationId, SessionRequest)> {
    let flags = super::flags(args, &["action", "operation", "input"])?;
    let (action, op) = identity(&flags)?;
    let request = read_session_file(Path::new(super::flag(&flags, "input")?))?;
    request.validate(op)?;
    Ok((action, op, request))
}

/// Main's default dispatch delegates only this exact additional command.
pub(super) fn command(command: &str, args: &[String]) -> Res<()> {
    if command != "m1-session" {
        return Err(Fail::Input("unknown_command"));
    }
    let (action, op, request) = parse(args)?;
    let outcome = (|| {
        let mut store = PgSessionStore::new(super::connect()?);
        let mut reader = matches!(action, SessionAction::Prepare)
            .then(|| super::connect().map(PgMissionStore::new))
            .transpose()?;
        m1_session::session(&mut store, reader.as_mut(), action, &request, op)
    })();
    print_receipt(&request, op, &outcome);
    outcome.map(|_| ())
}

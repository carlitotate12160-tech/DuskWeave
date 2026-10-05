//! Planning CLI ingress and bounded receipts; domain decisions remain in their owners.
use super::{authority_confirmation, connect, flag, flags};
use duskweave::mission::OperationId;
use duskweave::planning::{PlanningAssessed, PlanningRequest};
use duskweave::planning_history::{self, HistoryView};
use duskweave::planning_input::read_planning_file;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::{Fail, Res};
use std::path::Path;

fn parse_command(args: &[String]) -> Res<(OperationId, bool, PlanningRequest)> {
    let f = flags(args, &["operation", "input", "recover"])?;
    let operation =
        OperationId::parse(flag(&f, "operation")?).ok_or(Fail::Input("invalid_args"))?;
    let recover = flag(&f, "recover")?
        .parse::<bool>()
        .map_err(|_| Fail::Input("invalid_args"))?;
    let request = flag(&f, "input").and_then(|path| read_planning_file(Path::new(path)))?;
    Ok((operation, recover, request))
}

fn assess_attempt(
    request: &PlanningRequest,
    operation: OperationId,
    recover: bool,
) -> Res<Option<PlanningAssessed>> {
    let mut allocator = PgAllocator::new(connect()?);
    let mut store = PgMissionStore::new(connect()?);
    let mut response_in = std::io::stdin().lock();
    let mut challenge_out = std::io::stderr().lock();
    authority_confirmation::confirm_if_new(
        &mut allocator,
        &mut store,
        request,
        operation,
        recover,
        &mut response_in,
        &mut challenge_out,
    )
}

fn assessment_receipt(event: PlanningAssessed) -> Res<serde_json::Value> {
    let (scope, window) = event.assessment_labels();
    let contract = serde_json::to_value(&event).map_err(|_| Fail::Store("encode"))?;
    Ok(serde_json::json!({
        "result": "durable", "contract": contract,
        "decision_origin": "durable_record",
        "publication_obligation": "trajectory.planning_history.v1",
        "history": "pending", "history_reason": "not_published_at_decision",
        "history_view": "producer_receipt_as_of_decision",
        "basis_status": if event.basis.is_some() { "available" } else { "unavailable" },
        "scope": scope, "window": window,
        "complete_assessment": false, "current_permission": false,
    }))
}

fn emit_assessment(outcome: Res<Option<PlanningAssessed>>, operation: OperationId) -> Res<()> {
    match outcome {
        Ok(Some(event)) => {
            let receipt = assessment_receipt(event)?;
            println!("{receipt}");
            Ok(())
        }
        Ok(None) => {
            println!("{{\"result\":\"not_committed\",\"operation\":\"{operation}\"}}");
            Ok(())
        }
        Err(Fail::Store("commit_unknown")) => {
            println!(
                "{{\"result\":\"unknown\",\"operation\":\"{operation}\",\"action\":\"recover_before_retry\"}}"
            );
            Err(Fail::Store("commit_unknown"))
        }
        Err(error) => Err(error),
    }
}

pub(super) fn cmd_assess(args: &[String]) -> Res<()> {
    let (operation, recover, request) = parse_command(args)?;
    emit_assessment(assess_attempt(&request, operation, recover), operation)
}

fn history_view(event: &PlanningAssessed, recover: bool) -> HistoryView {
    match connect() {
        Ok(client) => {
            planning_history::history_view(&mut PgTrajectory::new(client), event, recover)
        }
        Err(error) => planning_history::history_result(Err(error)),
    }
}

fn history_receipt(event: PlanningAssessed, view: HistoryView) -> serde_json::Value {
    let (scope, window) = event.assessment_labels();
    serde_json::json!({
        "result": "durable", "contract": event,
        "decision_origin": "durable_record",
        "basis_status": if event.basis.is_some() { "available" } else { "unavailable" },
        "publication_obligation": "trajectory.planning_history.v1",
        "history_source": "trajectory", "history": view.state, "history_reason": view.reason,
        "complete_history": view.complete,
        "action": if view.state == "unknown" { "recover_history_before_retry" } else { "none" },
        "scope": scope, "window": window,
        "complete_assessment": event.recorded_eligible() && view.complete,
        "current_permission": false,
    })
}

pub(super) fn cmd_planning_history(args: &[String]) -> Res<()> {
    let (operation, recover, request) = parse_command(args)?;
    let mut store = PgMissionStore::new(connect()?);
    let Some(event) = planning_history::read_decision(&mut store, &request, operation)? else {
        println!("{{\"result\":\"not_committed\",\"operation\":\"{operation}\"}}");
        return Ok(());
    };
    let view = history_view(&event, recover);
    let receipt = history_receipt(event, view);
    println!("{receipt}");
    Ok(())
}

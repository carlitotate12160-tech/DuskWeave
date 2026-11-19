//! Serial indexed continuation and DB-only recovery for `m1_https_loop_v1`.
use super::{drive_phase, run_deadline};
use duskweave::Res;
use duskweave::m1_run::RunRecord;
use duskweave::m1_run::https_loop::{
    MAX_DECISIONS, acquisition_phase_for_step, decision_phase_for_step,
};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_run::https_loop::StepAction;
use std::time::Instant;

pub(super) fn supervise(store: &mut PgRunStore, run: &RunRecord) -> Res<()> {
    let deadline = run_deadline(run)?;
    run_steps(store, run, deadline)
}

fn run_steps(store: &mut PgRunStore, run: &RunRecord, deadline: Instant) -> Res<()> {
    for step_index in 1..=MAX_DECISIONS {
        if run_one_step(store, run, step_index, deadline)? {
            return Ok(());
        }
    }
    Ok(())
}

fn run_one_step(
    store: &mut PgRunStore,
    run: &RunRecord,
    step_index: usize,
    deadline: Instant,
) -> Res<bool> {
    drive_phase(store, run, decision_phase_for_step(step_index)?, deadline)?;
    let action = store.resolve_https_step_action(run, step_index)?;
    handle_step_action(store, run, step_index, deadline, action)
}

fn handle_step_action(
    store: &mut PgRunStore,
    run: &RunRecord,
    step_index: usize,
    deadline: Instant,
    action: StepAction,
) -> Res<bool> {
    match action {
        StepAction::CompletedAtDecisionFour => Ok(true),
        StepAction::Terminal { reason, .. } => {
            store.terminate_https_loop(run.request.run_id, step_index, reason)?;
            Ok(true)
        }
        StepAction::AcquireDns { .. } | StepAction::AcquireHead { .. } => {
            acquire_step(store, run, step_index, deadline)
        }
    }
}

fn acquire_step(
    store: &mut PgRunStore,
    run: &RunRecord,
    step_index: usize,
    deadline: Instant,
) -> Res<bool> {
    let r = run.request.run_id;
    if store.protect_next_decision(r, step_index).is_err() {
        store.terminate_https_loop(r, step_index, "blocked_budget_or_time")?;
        return Ok(true);
    }
    let acq_phase = acquisition_phase_for_step(step_index)?;
    if let Err(err) = drive_phase(store, run, acq_phase, deadline) {
        let _ = store.release_protected_capacity(r, step_index, "unknown");
        return Err(err);
    }
    let step = store.https_step(r, step_index)?;
    if step.status == "unknown" {
        let _ = store.release_protected_capacity(r, step_index, "unknown");
        return Ok(true);
    }
    Ok(false)
}

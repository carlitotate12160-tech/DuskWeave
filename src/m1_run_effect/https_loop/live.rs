//! Closed HTTPS LAB live effect acquisition through Broker controllers and launcher boundaries.
use super::delivery::{deliver_dns, deliver_https_step};
use crate::m1_reasoning_cli;
use duskweave::m1_effect_control::{LiveBoundary, LiveHttpsBoundary};
use duskweave::m1_inference_profile::HTTPS_LAB_RUN_PROFILE;
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::postgres_m1_effect::https_loop::{PgAttemptDnsStore, PgHttpsAttemptStore};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_run::https_loop::RefreshDecision;
use duskweave::{Fail, Res};
use uuid::Uuid;

const PRE_EFFECT_REFUSALS: &[&str] = &[
    "live_policy_denied",
    "live_budget_exhausted",
    "live_time_insufficient",
    "live_premise_ineligible",
    "live_refresh_exhausted",
    "live_head_exhausted",
    "live_dns_exhausted",
    "live_effect_expired",
    "live_qualification_unavailable",
    "live_sender_not_closed",
    "live_effect_scope_refused",
    "live_effect_session_busy",
    "live_effect_refused",
];

fn step_intent(
    broker: &mut PgRunStore,
    run_id: Uuid,
    phase: RunPhase,
) -> Res<(usize, Option<String>)> {
    let step = broker.https_step_for_phase(run_id, phase)?;
    let idx = usize::try_from(step.step_index).map_err(|_| Fail::State("run_phase_refused"))?;
    Ok((idx, step.next_intent_kind))
}

pub(super) struct LiveOps<'a> {
    pub run_dns: &'a mut dyn FnMut(Uuid, u32, &'static str) -> Res<()>,
    pub run_head: &'a mut dyn FnMut(Uuid, u32) -> Res<()>,
    pub deliver_dns: &'a mut dyn FnMut(&RunRecord, Uuid) -> Res<()>,
    pub deliver_head: &'a mut dyn FnMut(&mut PgRunStore, &RunRecord, usize) -> Res<()>,
}

pub(super) fn acquire(run: &RunRecord, phase: RunPhase) -> Res<()> {
    let valid_mode = [
        run.request.https_loop(),
        run.request.execution_mode == "live",
        run.request.run_profile == HTTPS_LAB_RUN_PROFILE,
    ]
    .into_iter()
    .all(std::convert::identity);
    if !valid_mode {
        return Err(Fail::State("live_effect_refused"));
    }
    let mut broker = PgRunStore::new(m1_reasoning_cli::broker_connection()?);
    let (mut dns, mut head, mut d_dns, mut d_head) = (
        run_dns_controller,
        run_head_controller,
        deliver_dns,
        deliver_https_step,
    );
    let mut ops = LiveOps {
        run_dns: &mut dns,
        run_head: &mut head,
        deliver_dns: &mut d_dns,
        deliver_head: &mut d_head,
    };
    acquire_with(&mut broker, run, phase, &mut ops)
}

pub(super) fn acquire_with(
    broker: &mut PgRunStore,
    run: &RunRecord,
    phase: RunPhase,
    ops: &mut LiveOps<'_>,
) -> Res<()> {
    let (idx, intent) = step_intent(broker, run.request.run_id, phase)?;
    match intent.as_deref() {
        Some("dns_udp") => acquire_initial_dns(broker, run, idx, ops),
        Some("https-head-v1") => acquire_head_chain(broker, run, idx, ops),
        _ => Err(Fail::State("live_effect_refused")),
    }
}

fn acquire_initial_dns(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
    ops: &mut LiveOps<'_>,
) -> Res<()> {
    let id = run.request.run_id;
    let step_u32 = u32::try_from(step_idx).map_err(|_| Fail::State("run_phase_refused"))?;
    if let Err(err) = (ops.run_dns)(id, step_u32, "dns") {
        let _ = broker.mark_https_effect_unknown(id, step_idx, "dns", failure_tag(&err));
        return Err(err);
    }
    let step = broker.https_step(id, step_idx)?;
    let res_id = step
        .dns_result_id
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))?;
    (ops.deliver_dns)(run, res_id)
}

fn acquire_head_chain(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
    ops: &mut LiveOps<'_>,
) -> Res<()> {
    if !prepare_head_refresh(broker, run, step_idx, ops)? {
        return Ok(());
    }
    acquire_head_only(broker, run, step_idx, ops)
}

fn prepare_head_refresh(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
    ops: &mut LiveOps<'_>,
) -> Res<bool> {
    let id = run.request.run_id;
    match broker.https_refresh_decision(id, step_idx) {
        Ok(RefreshDecision::NotNeeded) => Ok(true),
        Ok(RefreshDecision::Blocked(reason)) => broker
            .block_https_head(id, step_idx, &reason)
            .map(|()| false),
        Ok(RefreshDecision::NeedRefresh { .. }) => run_refresh(broker, run, step_idx, ops),
        Err(Fail::State(reason)) => broker
            .block_https_head(id, step_idx, reason)
            .map(|()| false),
        Err(other) => {
            let _ = broker.mark_https_effect_unknown(id, step_idx, "refresh", failure_tag(&other));
            Err(other)
        }
    }
}

fn handle_refresh_error(
    broker: &mut PgRunStore,
    id: Uuid,
    step_idx: usize,
    err: Fail,
) -> Res<bool> {
    let step = broker.https_step(id, step_idx)?;
    if step.refresh_attempt_id.is_none()
        && let Fail::State(reason) = err
        && is_pre_effect_refusal(reason)
    {
        return broker
            .block_https_head(id, step_idx, reason)
            .map(|()| false);
    }
    let _ = broker.mark_https_effect_unknown(id, step_idx, "refresh", failure_tag(&err));
    Err(err)
}

fn run_refresh(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
    ops: &mut LiveOps<'_>,
) -> Res<bool> {
    let id = run.request.run_id;
    let step_u32 = u32::try_from(step_idx).map_err(|_| Fail::State("run_phase_refused"))?;
    if let Err(err) = (ops.run_dns)(id, step_u32, "dns_refresh") {
        return handle_refresh_error(broker, id, step_idx, err);
    }
    let step = broker.https_step(id, step_idx)?;
    let res_id = step
        .refresh_result_id
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))?;
    (ops.deliver_dns)(run, res_id)?;
    Ok(true)
}

fn acquire_head_only(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
    ops: &mut LiveOps<'_>,
) -> Res<()> {
    let id = run.request.run_id;
    let step_u32 = u32::try_from(step_idx).map_err(|_| Fail::State("run_phase_refused"))?;
    if let Err(err) = (ops.run_head)(id, step_u32) {
        let step = broker.https_step(id, step_idx)?;
        if step.head_attempt_id.is_none()
            && let Fail::State(reason) = err
            && is_pre_effect_refusal(reason)
        {
            return broker.block_https_head(id, step_idx, reason);
        }
        let _ = broker.mark_https_effect_unknown(id, step_idx, "head", failure_tag(&err));
        return Err(err);
    }
    (ops.deliver_head)(broker, run, step_idx)
}

fn run_dns_controller(run_id: Uuid, step: u32, kind: &'static str) -> Res<()> {
    let mut store = PgAttemptDnsStore::new(m1_reasoning_cli::broker_connection()?, step, kind);
    let mut boundary = LiveBoundary::new(run_id);
    duskweave::m1_effect_control::acquire(&mut store, &mut boundary, run_id)
}

fn run_head_controller(run_id: Uuid, step: u32) -> Res<()> {
    let mut store = PgHttpsAttemptStore::new(m1_reasoning_cli::broker_connection()?);
    let mut boundary = LiveHttpsBoundary::new(run_id);
    duskweave::m1_effect_control::https::acquire(&mut store, &mut boundary, run_id, step)
}

fn is_pre_effect_refusal(code: &str) -> bool {
    PRE_EFFECT_REFUSALS.contains(&code)
}

fn failure_tag(err: &Fail) -> &'static str {
    match err {
        Fail::State(code)
        | Fail::Unresolved(code)
        | Fail::Input(code)
        | Fail::Store(code)
        | Fail::Config(code)
        | Fail::Conflict(code) => code,
    }
}

#[cfg(test)]
#[path = "../../../tests/support/m1_https_loop_live_effects.rs"]
mod tests;

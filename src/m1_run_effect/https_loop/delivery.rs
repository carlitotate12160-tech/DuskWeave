//! Shared DB-only idempotent Terrain and Trajectory delivery for `m1_https_loop_v1`.
use crate::{connect, m1_reasoning_cli, m1_terrain_cli};
use duskweave::m1_https::HttpsHeadResult;
use duskweave::m1_run::RunRecord;
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::OperationId;
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_run::https_loop::HttpsLoopStep;
use duskweave::postgres_m1_terrain::{PgTerrainStore, https::PgHttpsTerrainStore};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::{record_https_terrain_history, record_terrain_history};
use duskweave::trajectory::HistoryStatus;
use duskweave::{Fail, Res};
use postgres::Client;
use uuid::Uuid;

/// Deliver all committed DNS and HEAD step results in the run to Terrain and Trajectory.
pub(in super::super) fn deliver(run: &RunRecord) -> Res<()> {
    let mut broker = PgRunStore::new(m1_reasoning_cli::broker_connection()?);
    for step in broker.https_steps(run.request.run_id)? {
        deliver_step_artifacts(&mut broker, run, &step)?;
    }
    Ok(())
}

fn deliver_step_artifacts(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step: &HttpsLoopStep,
) -> Res<()> {
    for res_id in step.dns_result_id.into_iter().chain(step.refresh_result_id) {
        deliver_dns(run, res_id)?;
    }
    if step.head_result_id.is_some() {
        let idx = usize::try_from(step.step_index).map_err(|_| Fail::Store("run_decode"))?;
        deliver_https_step(broker, run, idx)?;
    }
    Ok(())
}

fn delivery_stores() -> Res<(PgTerrainStore, PgMissionStore, Client)> {
    let terrain = PgTerrainStore::new(m1_terrain_cli::terrain_connection()?);
    let mission = PgMissionStore::new(connect()?);
    Ok((terrain, mission, m1_terrain_cli::trajectory_connection()?))
}

fn require_completed_history(status: HistoryStatus) -> Res<()> {
    (status == HistoryStatus::Completed)
        .then_some(())
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))
}

fn issued_dns_ref(run: &RunRecord, result_id: Uuid) -> Res<IssuedDnsReference> {
    let q = &run.request.context.mission;
    if run.request.live() {
        IssuedDnsReference::live(
            q.engagement_id,
            q.campaign_id,
            result_id,
            &run.request.run_profile,
        )
    } else {
        IssuedDnsReference::controlled_fixture(
            q.engagement_id,
            q.campaign_id,
            result_id,
            &run.request.run_profile,
        )
    }
}

/// Admit one committed DNS result into Terrain and Trajectory using the mode's reference contract.
pub(super) fn deliver_dns(run: &RunRecord, result_id: Uuid) -> Res<()> {
    let reference = issued_dns_ref(run, result_id)?;
    let (mut terrain, mut mission, mut traj) = delivery_stores()?;
    let ev = terrain
        .record_issued(&mut mission, &reference)?
        .map_err(Fail::State)?
        .0;
    require_completed_history(record_terrain_history(&mut traj, &ev)?)
}

fn head_result_and_op(
    broker: &mut PgRunStore,
    id: Uuid,
    step_idx: usize,
) -> Res<(HttpsHeadResult, OperationId)> {
    let step = broker.https_step(id, step_idx)?;
    let result = broker
        .read_committed_https_result(id, step_idx)?
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))?;
    let op = step
        .head_terrain_operation_id
        .ok_or(Fail::Store("run_decode"))?;
    Ok((result, OperationId(op)))
}

/// Admit one committed HTTPS HEAD result into Terrain and Trajectory.
pub(super) fn deliver_https_step(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
) -> Res<()> {
    let (result, op) = head_result_and_op(broker, run.request.run_id, step_idx)?;
    let (mut terrain, mut mission, mut traj) = delivery_stores()?;
    let event = PgHttpsTerrainStore::new(&mut terrain).record_https(&mut mission, &result, op)?;
    require_completed_history(record_https_terrain_history(&mut traj, &event)?)
}

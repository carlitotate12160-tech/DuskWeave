//! Broker fixture effect, independent Terrain acceptance, and required Trajectory delivery.
use duskweave::m1_run::{NextStep, RunRecord};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::record_terrain_history;
use duskweave::trajectory::HistoryStatus;
use duskweave::{Fail, Res};

pub(super) fn acquire(run: &RunRecord) -> Res<()> {
    let mut broker = PgRunStore::new(super::m1_reasoning_cli::broker_connection()?);
    let proposal = broker.first_proposal(run)?;
    let NextStep::Dns { seed_index } =
        duskweave::m1_run::next_step(&proposal, run.request.seed_names.len())?
    else {
        return Err(Fail::State("fixture_effect_not_selected"));
    };
    let issued = broker.reserve_effect(run.request.run_id, seed_index)?;
    let result = duskweave::m1_fixture_dns::acquire(issued)?;
    broker.commit_result(run.request.run_id, &result)?;
    deliver(run)
}

fn accept_issued(run: &RunRecord) -> Res<duskweave::m1_terrain::TerrainAcceptedEvent> {
    let q = &run.request.context.mission;
    let mut terrain = PgTerrainStore::new(super::m1_terrain_cli::terrain_connection()?);
    let mut mission = PgMissionStore::new(super::connect()?);
    let reference = duskweave::m1_terrain_issued::IssuedDnsReference::fixture(
        q.engagement_id,
        q.campaign_id,
        run.result,
    )?;
    let (event, _, _, _) = terrain
        .record_issued(&mut mission, &reference)?
        .map_err(Fail::State)?;
    Ok(event)
}
pub(super) fn deliver(run: &RunRecord) -> Res<()> {
    let event = accept_issued(run)?;
    let history =
        record_terrain_history(&mut super::m1_terrain_cli::trajectory_connection()?, &event)?;
    if history != HistoryStatus::Completed {
        return Err(Fail::Unresolved("run_terrain_history_pending"));
    }
    Ok(())
}

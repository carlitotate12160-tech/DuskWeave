//! Effect-boundary router and shared Terrain delivery for `m1_https_loop_v1`.
mod delivery;
mod fixture;
mod live;
use duskweave::m1_inference_profile::HTTPS_LAB_RUN_PROFILE;
use duskweave::m1_run::{FIXTURE_MODE, RunPhase, RunRecord};
use duskweave::{Fail, Res};

pub(super) use delivery::deliver;

/// Dispatch acquisition for one `m1_https_loop_v1` phase to the fixture or closed LAB handler.
pub(crate) fn acquire(run: &RunRecord, phase: RunPhase) -> Res<()> {
    if !run.request.https_loop() {
        return Err(Fail::State("fixture_source_refused"));
    }
    match (
        run.request.execution_mode.as_str(),
        run.request.run_profile.as_str(),
    ) {
        (FIXTURE_MODE, "m1_https_loop_v1") => fixture::acquire(run, phase),
        ("live", HTTPS_LAB_RUN_PROFILE) => live::acquire(run, phase),
        _ => Err(Fail::State("fixture_source_refused")),
    }
}

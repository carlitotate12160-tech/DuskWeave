//! Original phase/run bounds projected onto the local monotonic clock, and
//! the absolute state-aware barrier the real-CLI temporal witnesses wait on.
#![allow(dead_code)]

use duskweave::m1_run::RunPhase;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Conservative earliest competing stops, projected from the real owner record.
/// Selection of the parent's phase reaper occurs after phase-start commit.
pub struct PhaseBounds {
    /// Phase-start record projected onto the local clock: the lower bracket of
    /// local phase-deadline selection and parent-owned child launch.
    pub started: Instant,
    /// Same-run, same-phase claim record projected onto the local clock: the
    /// upper bracket of deadline selection and child launch, once observed.
    pub claimed: Option<Instant>,
    /// Conservative lower bound of the phase reaper: the selected deadline is
    /// never earlier than phase start plus the phase ceiling, and the admitted
    /// run deadline bounds it independently.
    pub earliest_stop: Instant,
    /// Admitted run deadline projected onto the local clock: the outer bound
    /// of every observation inside the run.
    pub run_stop: Instant,
    /// Round trip of the server-time sample used by the projections.
    pub uncertainty: Duration,
}
impl PhaseBounds {
    /// Upper bracket of the phase expiry: deadline selection precedes launch
    /// and the same-phase claim follows both, so expiry is at most claim plus
    /// the ceiling. A missing claim leaves only the phase-start bracket, and
    /// the admitted run deadline bounds either case independently.
    pub fn expiry_upper(&self, ceiling_ms: u64) -> Instant {
        (self.claimed.unwrap_or(self.started)
            + Duration::from_millis(ceiling_ms)
            + self.uncertainty)
            .min(self.run_stop)
    }
}

fn project_run_stop(sampled: Instant, deadline_ms: i64, server_remaining_ms: i64) -> Instant {
    let remaining = u64::try_from(server_remaining_ms).unwrap_or(0);
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let local = u64::try_from(deadline_ms - wall).unwrap_or(0);
    (sampled + Duration::from_millis(remaining)).min(Instant::now() + Duration::from_millis(local))
}

/// Admitted run deadline projected onto the local clock, or None while the
/// admitted run record is not yet observable.
pub fn run_stop(client: &mut postgres::Client, run: Uuid) -> Option<Instant> {
    let sampled = Instant::now();
    let row = client
        .query_opt(
            "SELECT x.deadline_at_ms, \
             x.deadline_at_ms-floor(extract(epoch FROM clock_timestamp())*1000)::bigint \
             FROM execution.autonomous_runs x WHERE x.run_id=$1",
            &[&run],
        )
        .unwrap_or_else(|_| panic!("restricted run observation unavailable"))?;
    Some(project_run_stop(sampled, row.get(0), row.get(1)))
}

/// Original bounds of one requested phase. The SQL phase-start precedes the
/// parent's local deadline selection and the same-run, same-phase claim
/// follows selection and launch, so the two validated records bracket the
/// local origin with the measured sampling uncertainty; neither record is the
/// exact local origin itself.
pub fn phase_bounds_for(
    client: &mut postgres::Client,
    run: Uuid,
    phase: RunPhase,
    ceiling_ms: u64,
) -> Option<PhaseBounds> {
    let sampled = Instant::now();
    let row=client.query_opt("SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint-p.recorded_at_ms, x.deadline_at_ms-floor(extract(epoch FROM clock_timestamp())*1000)::bigint, x.deadline_at_ms, (SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint-y.recorded_at_ms FROM execution.autonomous_phases y WHERE y.run_id=x.run_id AND y.phase=p.phase AND y.status='claimed') FROM execution.autonomous_runs x JOIN execution.autonomous_phases p USING(run_id) WHERE x.run_id=$1 AND p.phase=$2 AND p.status='started'",&[&run,&phase.name()]).unwrap_or_else(|_|panic!("restricted phase observation unavailable"))?;
    let uncertainty = sampled.elapsed();
    let started = sampled - Duration::from_millis(u64::try_from(row.get::<_, i64>(0)).unwrap());
    let run_stop = project_run_stop(sampled, row.get(2), row.get(1));
    let claimed = row
        .get::<_, Option<i64>>(3)
        .and_then(|n| u64::try_from(n).ok())
        .map(|n| sampled - Duration::from_millis(n));
    Some(PhaseBounds {
        started,
        claimed,
        uncertainty,
        run_stop,
        earliest_stop: (started + Duration::from_millis(ceiling_ms)).min(run_stop),
    })
}

/// Unchanged decision-one view for existing callers; other phases must ask
/// for themselves explicitly through `phase_bounds_for`.
pub fn phase_bounds(
    client: &mut postgres::Client,
    run: Uuid,
    ceiling_ms: u64,
) -> Option<PhaseBounds> {
    phase_bounds_for(client, run, RunPhase::DecisionOne, ceiling_ms)
}

/// Outcome of one absolute, state-aware barrier wait.
pub enum Barrier {
    /// The withheld needle was forwarded inside the original bounds.
    Reached,
    /// The owned parent terminated before the needle arrived.
    ParentExited,
    /// The applicable original bound passed without the needle; the returned
    /// instant is the exceeded bound, never a restarted clock.
    BoundPassed(Instant),
}

/// Poll a barrier against an absolute original bound while the owned parent
/// still runs. `observed` receives only the fixed poll slice; `bound` returns
/// the current applicable absolute instant, or None while no original limit
/// is observable yet — even then the wait cannot outlive the parent. No poll
/// restarts a clock and the wait never continues past an earlier refusal.
pub fn original_barrier(
    mut observed: impl FnMut(Duration) -> bool,
    mut parent_alive: impl FnMut() -> bool,
    mut bound: impl FnMut() -> Option<Instant>,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration),
) -> Barrier {
    const POLL: Duration = Duration::from_millis(50);
    loop {
        if observed(POLL) {
            return Barrier::Reached;
        }
        if !parent_alive() {
            return Barrier::ParentExited;
        }
        if let Some(end) = bound()
            && now() >= end
        {
            return Barrier::BoundPassed(end);
        }
        sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// Deterministic shared clock: each poll slice advances the scaled tick
    /// without real sleeping, keeping every bound absolute while the control
    /// completes in microseconds.
    struct Rig {
        base: Instant,
        ticks: Rc<Cell<u64>>,
    }
    impl Rig {
        fn new() -> Self {
            Self {
                base: Instant::now(),
                ticks: Rc::new(Cell::new(0)),
            }
        }
        fn now(&self) -> impl FnMut() -> Instant {
            let (base, ticks) = (self.base, self.ticks.clone());
            move || base + Duration::from_millis(ticks.get() * 50)
        }
        fn sleep(&self) -> impl FnMut(Duration) {
            let ticks = self.ticks.clone();
            move |_| ticks.set(ticks.get() + 1)
        }
    }

    #[test]
    fn delayed_phase_entry_keeps_the_original_window_eligible() {
        let rig = Rig::new();
        // The needle fires only 250 scaled ticks after launch (~12.5s of
        // scaled time): a fixed launch-relative eight-second assumption would
        // already fail while the original phase/run envelope still admits it.
        let fired = rig.ticks.clone();
        let mut observed = move |_: Duration| fired.get() >= 250;
        rig.ticks.set(160);
        assert!(
            !observed(Duration::from_secs(8)),
            "the launch-relative assumption must miss this delayed entry"
        );
        let base = rig.base;
        let outcome = original_barrier(
            &mut observed,
            || true,
            move || Some(base + Duration::from_secs(45)),
            rig.now(),
            rig.sleep(),
        );
        assert!(matches!(outcome, Barrier::Reached));
    }

    #[test]
    fn relay_not_reached_fails_at_the_original_bound() {
        let rig = Rig::new();
        let bound = rig.base + Duration::from_millis(1_000);
        let outcome = original_barrier(
            |_| false,
            || true,
            move || Some(bound),
            rig.now(),
            rig.sleep(),
        );
        match outcome {
            Barrier::BoundPassed(end) => assert_eq!(end, bound),
            _ => panic!("a missing barrier must fail at the original bound"),
        }
    }

    #[test]
    fn early_parent_exit_fails_the_witness() {
        let rig = Rig::new();
        let alive = rig.ticks.clone();
        let outcome = original_barrier(
            |_| false,
            move || alive.get() < 3,
            || None,
            rig.now(),
            rig.sleep(),
        );
        assert!(matches!(outcome, Barrier::ParentExited));
    }

    #[test]
    fn bound_appearing_already_exhausted_fails_without_a_fresh_clock() {
        let rig = Rig::new();
        let bound = rig.base + Duration::from_millis(500);
        // The first phase record becomes observable only after the bound has
        // already passed; the wait reports that bound, never a restarted one.
        let calls = Cell::new(0_u32);
        let outcome = original_barrier(
            |_| false,
            || true,
            move || {
                calls.set(calls.get() + 1);
                (calls.get() > 2).then_some(bound)
            },
            rig.now(),
            rig.sleep(),
        );
        match outcome {
            Barrier::BoundPassed(end) => assert_eq!(end, bound),
            _ => panic!("a late bound must fail at its own instant"),
        }
    }
}

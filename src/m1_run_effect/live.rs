//! Live acquisition delegates admission/STOP ordering to the Broker controller.
use duskweave::Res;
use duskweave::m1_effect_control::LiveBoundary;
use duskweave::m1_run::RunRecord;
use duskweave::postgres_m1_effect::PgEffectStore;

pub(super) fn acquire(run: &RunRecord) -> Res<()> {
    let mut store = PgEffectStore::new(crate::m1_reasoning_cli::broker_connection()?);
    let mut boundary = LiveBoundary::new(run.request.run_id);
    duskweave::m1_effect_control::acquire(&mut store, &mut boundary, run.request.run_id)?;
    super::deliver(run)
}

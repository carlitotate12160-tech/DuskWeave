//! Live acquisition delegates admission/STOP ordering to the Broker controller.
use duskweave::Res;
use duskweave::m1_effect_control::LiveBoundary;
use duskweave::m1_run::RunRecord;
use duskweave::postgres_m1_effect::PgEffectStore;

pub(super) fn acquire(run: &RunRecord) -> Res<()> {
    let mut store = PgEffectStore::new(crate::m1_reasoning_cli::broker_connection()?);
    let mut boundary = LiveBoundary::new(run.request.run_id);
    acquire_with(&mut store, &mut boundary, run)
}

fn acquire_with(
    store: &mut dyn duskweave::m1_effect_session::EffectStore,
    boundary: &mut dyn duskweave::m1_dns::DnsBoundary,
    run: &RunRecord,
) -> Res<()> {
    duskweave::m1_effect_control::acquire(store, boundary, run.request.run_id)?;
    super::deliver(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_acquire_fails_closed() {
        let id = uuid::Uuid::from_u128(0x77);
        let rec: RunRecord = serde_json::from_value(serde_json::json!({
            "request": {
                "version": "m1_autonomous_run_request_v2",
                "run_id": id,
                "run_profile": "autonomous-dns-fixture-v2",
                "execution_mode": "controlled_fixture",
                "deadline_at_ms": 1700004000000i64,
                "context": {
                    "version": "m1_context_prepare_request_v1",
                    "mission": {
                        "version": "m1_inference_reservation_request_v1",
                        "engagement_id": id,
                        "campaign_id": id,
                        "operator_ref": id,
                        "expected_mission_revision": 1,
                        "permission_id": id,
                        "goal_ref": id,
                        "exercise_mode": "blind"
                    },
                    "candidates": [{"asset_ref": id, "priority": "high"}]
                },
                "seed_names": [{"asset_ref": id, "name": "seed.example.invalid"}]
            },
            "decision_one": id,
            "decision_two": id,
            "attempt": id,
            "result": id,
            "terrain_operation": id,
            "release_sha256": "ab",
            "qualification_ref": id,
            "status": "running",
            "deadline_at_ms": 1700004000000i64
        }))
        .unwrap();
        assert!(acquire(&rec).is_err());
    }
}

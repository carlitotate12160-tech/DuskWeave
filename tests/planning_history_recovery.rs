use duskweave::planning::PlanningAssessed;
use duskweave::planning_history::{PlanningHistoryPort, history_view, read_decision};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::trajectory::Delivered;
use duskweave::{Fail, Res};
use serde_json::Value;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use db_support::*;
use history_support::*;

struct LostAcknowledgment(PgTrajectory);

impl PlanningHistoryPort for LostAcknowledgment {
    fn publish(&mut self, event: &PlanningAssessed) -> Res<Delivered> {
        assert_eq!(self.0.publish(event)?, Delivered::Completed);
        // Independent observer proves durable consumer acceptance AND completion
        // before the caller sees failure. This is not a pre-commit fault.
        let mut observer = runtime_client();
        let row = observer.query_one(
            "SELECT contract, completed_at IS NOT NULL, obligation FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND event_id=$3 AND status='accepted'",
            &[&event.engagement_id.0, &event.campaign_id.0, &event.event_id.0],
        ).unwrap();
        assert_eq!(row.get::<_, Value>(0), serde_json::to_value(event).unwrap());
        assert!(row.get::<_, bool>(1));
        assert_eq!(row.get::<_, String>(2), "trajectory.planning_history.v1");
        let obligation: String = observer
            .query_one(
                "SELECT publication_obligation FROM mission.planning_assessments \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
                &[
                    &event.engagement_id.0,
                    &event.campaign_id.0,
                    &event.operation_id.0,
                ],
            )
            .unwrap()
            .get(0);
        assert_eq!(obligation, "trajectory.planning_history.v1");
        effects(event, 1, 0);
        Err(Fail::Store("commit_unknown"))
    }

    fn inspect(&mut self, _: &PlanningAssessed) -> Res<Delivered> {
        panic!("publication must not silently retry or inspect after ack loss")
    }
}

#[test]
fn durable_before_lost_ack_then_fresh_ports_recover_without_effects() {
    let _guard = db();
    let original = decision(0xb1b9, true);
    let mut interrupted = LostAcknowledgment(PgTrajectory::new(runtime_client()));
    let failed = history_view(&mut interrupted, &original, false);
    assert_eq!(
        (failed.state, failed.reason, failed.complete),
        ("unknown", "consumer_commit_unknown", false)
    );
    drop(interrupted);
    // Remove current authority via fixture admin: recovery must still read the
    // original immutable decision, rather than assess the now-absent Mission.
    admin()
        .execute(
            "DELETE FROM mission.missions WHERE engagement_id=$1 AND campaign_id=$2",
            &[&original.engagement_id.0, &original.campaign_id.0],
        )
        .unwrap();
    let recovered = read_decision(
        &mut PgMissionStore::new(runtime_client()),
        &original.request,
        original.operation_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered, original);
    let view = history_view(&mut PgTrajectory::new(runtime_client()), &recovered, true);
    assert_eq!((view.state, view.complete), ("completed", true));
    effects(&original, 1, 0);
}

#[test]
fn precommit_fault_preserves_producer_and_read_only_recovery_stays_pending() {
    let _guard = db();
    let original = decision(0xb1ba, false);
    let function = format!("b1b_fault_{}", original.engagement_id.0.simple());
    let mut administrator = admin();
    administrator
        .batch_execute(&format!(
            "CREATE FUNCTION trajectory.{function}() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'precommit test fault'; END $$; \
         CREATE TRIGGER {function} BEFORE INSERT ON trajectory.planning_history \
         FOR EACH ROW WHEN (NEW.engagement_id='{engagement}'::uuid) \
         EXECUTE FUNCTION trajectory.{function}();",
            engagement = original.engagement_id,
        ))
        .unwrap();
    let mut consumer = PgTrajectory::new(runtime_client());
    assert_eq!(
        consumer.publish(&original),
        Err(Fail::Store("storage_error"))
    );
    effects(&original, 0, 0);
    let recovered = read_decision(
        &mut PgMissionStore::new(runtime_client()),
        &original.request,
        original.operation_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered, original);
    let view = history_view(&mut PgTrajectory::new(runtime_client()), &recovered, true);
    assert_eq!(
        (view.state, view.reason, view.complete),
        ("pending", "not_recorded", false)
    );
    effects(&original, 0, 0);
    administrator.batch_execute(&format!(
        "DROP TRIGGER {function} ON trajectory.planning_history; DROP FUNCTION trajectory.{function}();",
    )).unwrap();
    assert_eq!(
        retry(|| consumer.publish(&original)).unwrap(),
        Delivered::Completed
    );
    effects(&original, 1, 0);
}

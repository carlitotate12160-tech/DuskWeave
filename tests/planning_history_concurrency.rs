use duskweave::planning::PlanningAssessed;
use duskweave::planning_history::PlanningHistoryPort;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::trajectory::Delivered;
use duskweave::{Fail, Res};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use db_support::*;
use history_support::*;

struct Coordination {
    admin: postgres::Client,
    function: String,
    key: i32,
}

impl Coordination {
    fn install(ev: &PlanningAssessed, key: i32) -> Self {
        let mut admin = admin();
        let function = format!("b1b_wait_{}", ev.engagement_id.0.simple());
        admin
            .batch_execute(&format!(
                "CREATE FUNCTION trajectory.{function}() RETURNS trigger LANGUAGE plpgsql AS \
             $$ BEGIN PERFORM pg_advisory_xact_lock(202,{key}); RETURN NEW; END $$; \
             CREATE TRIGGER {function} BEFORE INSERT ON trajectory.planning_history \
             FOR EACH ROW WHEN (NEW.engagement_id='{engagement}'::uuid) \
             EXECUTE FUNCTION trajectory.{function}();",
                engagement = ev.engagement_id,
            ))
            .unwrap();
        Self {
            admin,
            function,
            key,
        }
    }

    fn wait_for_two(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let waiting: i64 = self
                .admin
                .query_one(
                    "SELECT count(*) FROM pg_locks WHERE locktype='advisory' \
                 AND classid=202 AND objid=$1::oid AND NOT granted",
                    &[&(self.key as u32)],
                )
                .unwrap()
                .get(0);
            if waiting == 2 {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "both consumers must reach BEFORE INSERT barrier"
            );
            std::thread::yield_now();
        }
    }
}

impl Drop for Coordination {
    fn drop(&mut self) {
        self.admin
            .batch_execute(&format!(
                "DROP TRIGGER {function} ON trajectory.planning_history; \
             DROP FUNCTION trajectory.{function}();",
                function = self.function,
            ))
            .unwrap();
    }
}

fn publish_thread(
    barrier: Arc<Barrier>,
    event: PlanningAssessed,
) -> std::thread::JoinHandle<Res<Delivered>> {
    std::thread::spawn(move || {
        let mut trajectory = PgTrajectory::new(runtime_client());
        barrier.wait();
        trajectory.publish(&event)
    })
}

fn race(slot: u128, variant: u8) {
    let original = decision(slot, false);
    let mut other = original.clone();
    match variant {
        1 => other.event_id.0 = Uuid::from_u128(slot),
        2 => other.request.purpose_ref.0 = Uuid::from_u128(slot),
        _ => (),
    }
    let mut coordination = Coordination::install(&original, slot as i32);
    let mut lock_client = admin();
    let mut lock = lock_client.transaction().unwrap();
    lock.query_one(
        "SELECT pg_advisory_xact_lock(202,$1::int)",
        &[&coordination.key],
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let a = publish_thread(barrier.clone(), original.clone());
    let b = publish_thread(barrier.clone(), other.clone());
    barrier.wait();
    coordination.wait_for_two();
    lock.commit().unwrap();
    let outcomes = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == Ok(Delivered::Completed))
            .count(),
        1
    );
    assert!(outcomes.iter().all(|result| matches!(
        result,
        Ok(Delivered::Completed)
            | Err(Fail::Store("serialization_retry"))
            | Err(Fail::Conflict("duplicate_identity"))
    )));
    effects(&original, 1, 0);
    let winner: PlanningAssessed = serde_json::from_value(runtime_client().query_one(
        "SELECT contract FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
        &[&original.engagement_id.0, &original.campaign_id.0],
    ).unwrap().get(0)).unwrap();
    let mut fresh = PgTrajectory::new(runtime_client());
    assert_eq!(fresh.inspect(&winner).unwrap(), Delivered::Completed);
    if variant != 0 {
        let loser = if winner == original {
            &other
        } else {
            &original
        };
        assert_eq!(fresh.publish(loser).unwrap(), Delivered::Anomaly);
        assert_eq!(fresh.inspect(&winner).unwrap(), Delivered::Anomaly);
        effects(&original, 1, 1);
    } else {
        assert_eq!(fresh.publish(&original).unwrap(), Delivered::Duplicate);
        effects(&original, 1, 0);
    }
}

#[test]
fn concurrent_identical_identity_has_one_effect_then_fresh_recovery() {
    let _guard = db();
    race(0xb1bc, 0);
}

#[test]
fn concurrent_changed_event_same_operation_preserves_winner_and_records_conflict() {
    let _guard = db();
    race(0xb1bd, 1);
}

#[test]
fn concurrent_changed_intent_same_event_is_an_integrity_conflict() {
    let _guard = db();
    race(0xb1be, 2);
}

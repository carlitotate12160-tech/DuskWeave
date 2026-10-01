use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef, OperationId};
use duskweave::planning::{PlanningAssessed, PlanningRequest};
use duskweave::planning_assessment;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::registration::OperationAllocator;
use duskweave::{Fail, Res};
use postgres::NoTls;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

fn request(engagement: EngagementId, campaign: CampaignId, confirmed: bool) -> PlanningRequest {
    PlanningRequest {
        engagement_id: engagement,
        campaign_id: campaign,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: confirmed,
    }
}

struct NeverAllocate;

impl OperationAllocator for NeverAllocate {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("recovery must not allocate")
    }
}

fn admin_in_test_database() -> postgres::Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    config.connect(NoTls).unwrap()
}

struct Coordination {
    admin: postgres::Client,
    function: String,
    trigger: String,
    key: i32,
}

impl Coordination {
    fn install(slot: u128) -> Self {
        let mut admin = admin_in_test_database();
        let function = format!("b1a_hold_{slot:x}");
        let trigger = format!("b1a_wait_{slot:x}");
        let key = (slot & 0x7fff_ffff) as i32;
        admin
            .batch_execute(&format!(
                "CREATE FUNCTION mission.{function}() RETURNS trigger LANGUAGE plpgsql AS \
             $$ BEGIN PERFORM pg_advisory_xact_lock(101,{key}); RETURN NEW; END $$; \
             CREATE TRIGGER {trigger} BEFORE INSERT ON mission.planning_assessments \
             FOR EACH ROW EXECUTE FUNCTION mission.{function}(); \
             CREATE TRIGGER {trigger} BEFORE INSERT ON mission.registration_outbox \
             FOR EACH ROW EXECUTE FUNCTION mission.{function}();"
            ))
            .unwrap();
        Self {
            admin,
            function,
            trigger,
            key,
        }
    }

    fn wait_for_two(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let waiting: i64 = self
                .admin
                .query_one(
                    "SELECT count(*) FROM pg_locks WHERE locktype='advisory' \
                 AND classid=101 AND objid=$1::oid AND NOT granted",
                    &[&(self.key as u32)],
                )
                .unwrap()
                .get(0);
            if waiting >= 2 {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "both inserts did not reach the trigger"
            );
            std::thread::yield_now();
        }
    }

    fn remove(&mut self) {
        self.admin
            .batch_execute(&format!(
                "DROP TRIGGER {0} ON mission.planning_assessments; \
             DROP TRIGGER {0} ON mission.registration_outbox; \
             DROP FUNCTION mission.{1}();",
                self.trigger, self.function
            ))
            .unwrap();
    }
}

fn assessment_thread(
    barrier: Arc<Barrier>,
    input: PlanningRequest,
    operation: OperationId,
) -> std::thread::JoinHandle<Res<Option<PlanningAssessed>>> {
    std::thread::spawn(move || {
        let mut allocator = PgAllocator::new(runtime_client());
        let mut store = PgMissionStore::new(runtime_client());
        barrier.wait();
        planning_assessment::assess(&mut allocator, &mut store, &input, operation, false)
    })
}

fn run_assessment_race(changed: bool, slot: u128) {
    let (engagement, campaign) = scope(slot);
    let operation = OperationId(Uuid::from_u128(slot + 0x1000));
    let first = request(engagement, campaign, true);
    let second = request(engagement, campaign, !changed);
    let mut coordination = Coordination::install(slot);
    let mut lock_client = admin_in_test_database();
    let mut lock = lock_client.build_transaction().start().unwrap();
    lock.query_one(
        "SELECT pg_advisory_xact_lock(101,$1::int)",
        &[&coordination.key],
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let a = assessment_thread(barrier.clone(), first.clone(), operation);
    let b = assessment_thread(barrier.clone(), second.clone(), operation);
    barrier.wait();
    coordination.wait_for_two();
    lock.commit().unwrap();
    let outcomes = [a.join().unwrap(), b.join().unwrap()];
    assert!(
        outcomes.iter().any(Result::is_ok),
        "one producer must commit"
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
    let mut fresh = PgMissionStore::new(runtime_client());
    let winner = outcomes
        .iter()
        .find_map(|outcome| outcome.as_ref().ok())
        .and_then(Option::as_ref)
        .unwrap();
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut fresh,
            &winner.request,
            operation,
            true
        )
        .unwrap(),
        Some(winner.clone())
    );
    if changed {
        let loser = if winner.request == first {
            &second
        } else {
            &first
        };
        assert_eq!(
            planning_assessment::assess(&mut NeverAllocate, &mut fresh, loser, operation, true),
            Err(Fail::Conflict("integrity_conflict"))
        );
    } else {
        assert_eq!(
            planning_assessment::assess(&mut NeverAllocate, &mut fresh, &second, operation, true)
                .unwrap(),
            Some(winner.clone())
        );
    }
    coordination.remove();
}

#[test]
fn concurrent_equal_and_changed_intent_keep_one_durable_identity() {
    let _guard = db();
    run_assessment_race(false, 0xb1ab);
    run_assessment_race(true, 0xb1ac);
}

#[test]
fn concurrent_registration_and_assessment_cannot_share_scoped_operation() {
    let _guard = db();
    let slot = 0xb1ad;
    let (engagement, campaign) = scope(slot);
    let operation = OperationId(Uuid::from_u128(slot + 0x1000));
    let input = request(engagement, campaign, true);
    let mut coordination = Coordination::install(slot);
    let mut lock_client = admin_in_test_database();
    let mut lock = lock_client.build_transaction().start().unwrap();
    lock.query_one(
        "SELECT pg_advisory_xact_lock(101,$1::int)",
        &[&coordination.key],
    )
    .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let assessment = assessment_thread(barrier.clone(), input.clone(), operation);
    let registration_barrier = barrier.clone();
    let registration = std::thread::spawn(move || {
        let (mut allocator, mut store, mut trajectory) = ports();
        registration_barrier.wait();
        reg(
            &mut allocator,
            &mut store,
            &mut trajectory,
            operation,
            engagement,
            campaign,
        )
    });
    barrier.wait();
    coordination.wait_for_two();
    lock.commit().unwrap();
    let assessment_result = assessment.join().unwrap();
    let registration_result = registration.join().unwrap();
    assert!(assessment_result.is_ok() || registration_result.is_ok());
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign)
            + count("mission.registration_outbox", engagement, campaign),
        1
    );
    if let Ok(Some(event)) = assessment_result {
        assert_eq!(event.request, input);
        let (mut allocator, mut store, mut trajectory) = ports();
        assert_eq!(
            reg(
                &mut allocator,
                &mut store,
                &mut trajectory,
                operation,
                engagement,
                campaign
            ),
            Err(Fail::Conflict("integrity_conflict"))
        );
    } else {
        let mut allocator = PgAllocator::new(runtime_client());
        let mut store = PgMissionStore::new(runtime_client());
        assert_eq!(
            planning_assessment::assess(&mut allocator, &mut store, &input, operation, false),
            Err(Fail::Conflict("integrity_conflict"))
        );
    }
    coordination.remove();
}

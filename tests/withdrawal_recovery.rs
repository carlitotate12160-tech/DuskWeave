use duskweave::mission::*;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::{self, OperationAllocator};
use duskweave::trajectory::Delivered;
use duskweave::withdrawal::*;
use duskweave::{Fail, Res};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

struct Never;
impl OperationAllocator for Never {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("recovery/duplicate must not allocate")
    }
}
struct ProducerLostAck(PgMissionStore);
impl WithdrawalStore for ProducerLostAck {
    fn withdraw(
        &mut self,
        request: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
        allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        let committed = self
            .0
            .withdraw(request, operation, recover, allocator)?
            .unwrap();
        let row = runtime_client()
            .query_one(
                "SELECT event_id,contract,publication_obligation FROM mission.withdrawals \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
                &[
                    &request.engagement_id.0,
                    &request.campaign_id.0,
                    &operation.0,
                ],
            )
            .unwrap();
        assert_eq!(row.get::<_, Uuid>(0), committed.event_id.0);
        assert_eq!(row.get::<_, Value>(1), json!(committed));
        assert_eq!(row.get::<_, &str>(2), OBLIGATION);
        Err(Fail::Store("commit_unknown"))
    }
}
struct ConsumerLostAck(PgTrajectory);
impl WithdrawalHistoryPort for ConsumerLostAck {
    fn publish(&mut self, event: &MissionAuthorityWithdrawn) -> Res<Delivered> {
        assert_eq!(self.0.publish(event)?, Delivered::Completed);
        let row = runtime_client().query_one(
            "SELECT event_id,contract,completed_at IS NOT NULL FROM trajectory.withdrawal_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&event.request.engagement_id.0, &event.request.campaign_id.0]).unwrap();
        assert_eq!(row.get::<_, Uuid>(0), event.event_id.0);
        assert_eq!(row.get::<_, Value>(1), json!(event));
        assert!(row.get::<_, bool>(2));
        Err(Fail::Store("commit_unknown"))
    }
    fn inspect(&mut self, _: &MissionAuthorityWithdrawn) -> Res<Delivered> {
        panic!("fresh recovery must use a new consumer")
    }
}

#[test]
fn real_postcommit_producer_and_consumer_ack_loss_recover_identity_and_one_effect() {
    let _guard = db();
    let (e, c) = scope(1);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let request = WithdrawalRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(99)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::ScopeConcern,
    };
    let operation = registration::prepare_operation(&mut alloc).unwrap();
    let mut lost = ProducerLostAck(PgMissionStore::new(runtime_client()));
    assert_eq!(
        lost.withdraw(&request, operation, false, &mut alloc),
        Err(Fail::Store("commit_unknown"))
    );
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    let original: Value = runtime_client()
        .query_one(
            "SELECT contract FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    drop(lost);
    let mut fresh = PgMissionStore::new(runtime_client());
    let recovered = fresh
        .withdraw(&request, operation, true, &mut Never)
        .unwrap()
        .unwrap();
    assert_eq!(json!(recovered), original);
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    // A new actual process recovers read-only, with no consumer publication.
    let path = std::env::temp_dir().join(format!("dw-c1a-recover-{operation}.json"));
    std::fs::write(&path, json!(request).to_string()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        )
        .args(["withdraw", "--operation", &operation.to_string(), "--input"])
        .arg(&path)
        .args(["--recover", "true"])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success());
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["contract"], original);
    assert_eq!(receipt["history"], "pending");
    assert_eq!(receipt["continuation_blocked"], true);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    let durable = fresh
        .withdraw(&request, operation, false, &mut Never)
        .unwrap()
        .unwrap();
    let mut consumer_lost = ConsumerLostAck(PgTrajectory::new(runtime_client()));
    assert_eq!(
        consumer_lost.publish(&durable),
        Err(Fail::Store("commit_unknown"))
    );
    assert_eq!(
        count_where(
            "trajectory.withdrawal_history",
            "AND status='accepted' AND completed_at IS NOT NULL",
            e,
            c
        ),
        1
    );
    drop(consumer_lost);
    let mut fresh_consumer = PgTrajectory::new(runtime_client());
    assert_eq!(
        fresh_consumer.inspect(&recovered).unwrap(),
        Delivered::Completed
    );
    assert_eq!(
        fresh_consumer.publish(&recovered).unwrap(),
        Delivered::Duplicate
    );
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 1);
}

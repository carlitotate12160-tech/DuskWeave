use duskweave::mission::*;
use duskweave::planning::PlanningRequest;
use duskweave::planning_assessment::PlanningStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::registration::{self, OperationAllocator};
use duskweave::withdrawal::*;
use duskweave::{Fail, Res};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Duration;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

fn request(e: EngagementId, c: CampaignId) -> WithdrawalRequest {
    WithdrawalRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(99)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    }
}
fn planning(e: EngagementId, c: CampaignId) -> PlanningRequest {
    PlanningRequest {
        engagement_id: e,
        campaign_id: c,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}
struct PausedAllocator {
    read: SyncSender<()>,
    release: Receiver<()>,
    identity: Uuid,
}
impl OperationAllocator for PausedAllocator {
    fn allocate(&mut self) -> Res<Uuid> {
        self.read.send(()).unwrap();
        self.release
            .recv_timeout(Duration::from_secs(30))
            .expect("bounded allocator release");
        Ok(self.identity)
    }
}
struct Never;
impl OperationAllocator for Never {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("denial/recovery must not allocate")
    }
}

#[test]
fn withdrawal_acknowledged_while_assessment_paused_prevents_late_assessment_commit() {
    let _guard = db();
    let (e, c) = scope(1);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let assessment_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawal_op = registration::prepare_operation(&mut alloc).unwrap();
    let (read_tx, read_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let assessment = std::thread::spawn(move || {
        PgMissionStore::new(runtime_client()).assess(
            &planning(e, c),
            assessment_op,
            false,
            &mut PausedAllocator {
                read: read_tx,
                release: release_rx,
                identity: Uuid::from_u128(101),
            },
        )
    });
    read_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("basis read occurred");
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    let withdrawn = store
        .withdraw(&request(e, c), withdrawal_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    release_tx.send(()).unwrap();
    assert!(matches!(
        assessment.join().unwrap(),
        Err(Fail::Store("serialization_retry" | "commit_unknown"))
    ));
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    let mut fresh = PgMissionStore::new(runtime_client());
    assert_eq!(
        fresh
            .assess(&planning(e, c), assessment_op, true, &mut Never)
            .unwrap(),
        None
    );
    assert_eq!(
        fresh.assess(&planning(e, c), assessment_op, false, &mut Never),
        Err(Fail::State("authority_withdrawn"))
    );
    assert_eq!(
        fresh
            .withdraw(&request(e, c), withdrawal_op, true, &mut Never)
            .unwrap(),
        Some(withdrawn)
    );
    assert_eq!(count("mission.planning_assessments", e, c), 0);
}

#[test]
fn inverse_legal_order_keeps_committed_prewithdrawal_result_historical() {
    let _guard = db();
    let (e, c) = scope(2);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let assessment_op = registration::prepare_operation(&mut alloc).unwrap();
    let original = store
        .assess(&planning(e, c), assessment_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    let withdrawal_op = registration::prepare_operation(&mut alloc).unwrap();
    store
        .withdraw(&request(e, c), withdrawal_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .assess(&planning(e, c), assessment_op, true, &mut Never)
            .unwrap(),
        Some(original.clone())
    );
    assert_eq!(original.basis.unwrap().revision, 1);
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    assert_eq!(count("mission.withdrawals", e, c), 1);
}

#[test]
fn competing_withdrawals_accept_one_immutable_owner_transition() {
    let _guard = db();
    let (e, c) = scope(3);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let first_op = registration::prepare_operation(&mut alloc).unwrap();
    let second_op = registration::prepare_operation(&mut alloc).unwrap();
    let spawn = |op, identity| {
        let (read_tx, read_rx) = sync_channel(1);
        let (release_tx, release_rx) = sync_channel(1);
        let thread = std::thread::spawn(move || {
            PgMissionStore::new(runtime_client()).withdraw(
                &request(e, c),
                op,
                false,
                &mut PausedAllocator {
                    read: read_tx,
                    release: release_rx,
                    identity,
                },
            )
        });
        (thread, read_rx, release_tx)
    };
    let (first, first_read, first_release) = spawn(first_op, Uuid::from_u128(102));
    first_read.recv_timeout(Duration::from_secs(30)).unwrap();
    let (second, second_read, second_release) = spawn(second_op, Uuid::from_u128(103));
    second_read.recv_timeout(Duration::from_secs(30)).unwrap();
    first_release.send(()).unwrap();
    let accepted = first.join().unwrap().unwrap().unwrap();
    second_release.send(()).unwrap();
    assert!(matches!(
        second.join().unwrap(),
        Err(Fail::Store("serialization_retry" | "commit_unknown"))
            | Err(Fail::Conflict("duplicate_identity"))
    ));
    assert_eq!(count("mission.withdrawals", e, c), 1);
    let mut fresh = PgMissionStore::new(runtime_client());
    assert_eq!(
        fresh
            .withdraw(&request(e, c), first_op, true, &mut Never)
            .unwrap(),
        Some(accepted)
    );
    assert_eq!(
        fresh
            .withdraw(&request(e, c), second_op, true, &mut Never)
            .unwrap(),
        None
    );
    assert_eq!(count("mission.withdrawals", e, c), 1);
}

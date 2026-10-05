use duskweave::input::parse_register;
use duskweave::mission::*;
use duskweave::planning::{PlanningAssessed, PlanningDecision, PlanningRequest};
use duskweave::planning_assessment::{self, assess};
use duskweave::planning_history::{PlanningHistoryPort, history_view, read_decision};
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::{self, MissionStore, OperationAllocator, TrajectoryPort};
use duskweave::trajectory::{Delivered, HistoryStatus};
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest, WithdrawalStore};
use duskweave::{Fail, Res};
use serde_json::{Value, json};
use std::sync::{Arc, Barrier};
use uuid::Uuid;

#[path = "support/upgrade_db.rs"]
mod upgrade_support;
use upgrade_support::{OwnedUpgradeDatabase, admin_in, runtime_in};

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

#[test]
fn matching_current_mission_records_eligible_v4() {
    let _guard = db();
    let (engagement, campaign) = scope(0xd40);
    let (mut allocator, mut store, mut trajectory) = ports();
    let registration = registration::prepare_operation(&mut allocator).unwrap();
    registration::register(
        &mut allocator,
        &mut store,
        &mut trajectory,
        registration,
        &registered_input(engagement, campaign),
    )
    .unwrap();
    let wire = serde_json::to_value(assess_now(
        &mut allocator,
        &mut store,
        &request(engagement, campaign),
    ))
    .unwrap();
    assert_eq!(wire["decision"], "eligible");
    assert_eq!(wire["version"], 4);
}

fn request(engagement: EngagementId, campaign: CampaignId) -> PlanningRequest {
    PlanningRequest {
        engagement_id: engagement,
        campaign_id: campaign,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}

fn assess_now(
    allocator: &mut PgAllocator,
    store: &mut PgMissionStore,
    request: &PlanningRequest,
) -> PlanningAssessed {
    let operation = registration::prepare_operation(allocator).unwrap();
    assess(allocator, store, request, operation, false)
        .unwrap()
        .unwrap()
}

fn registered_input(engagement: EngagementId, campaign: CampaignId) -> RegistrationInput {
    let now: i64 = runtime_client()
        .query_one("SELECT floor(extract(epoch FROM now()))::bigint", &[])
        .unwrap()
        .get(0);
    let mut body = reg_json(engagement, campaign);
    body["starts_at"] = json!(now - 7_200);
    body["ends_at"] = json!(now + 7_200);
    parse_register(&serde_json::to_vec(&body).unwrap()).unwrap()
}

struct PendingRegistration;
impl TrajectoryPort for PendingRegistration {
    fn deliver(&mut self, _: &MissionRegistered) -> Res<Delivered> {
        Ok(Delivered::Unresolved("history_unavailable"))
    }
    fn status(&mut self, _: EngagementId, _: CampaignId, _: EventId) -> Res<HistoryStatus> {
        Ok(HistoryStatus::Pending)
    }
}

fn positive(slot: u128, pending_registration: bool) -> (MissionRegistered, PlanningAssessed) {
    let (engagement, campaign) = scope(slot);
    let (mut allocator, mut store, mut trajectory) = ports();
    let reg_op = registration::prepare_operation(&mut allocator).unwrap();
    let input = registered_input(engagement, campaign);
    if pending_registration {
        registration::register(
            &mut allocator,
            &mut store,
            &mut PendingRegistration,
            reg_op,
            &input,
        )
        .unwrap();
    } else {
        registration::register(&mut allocator, &mut store, &mut trajectory, reg_op, &input)
            .unwrap();
    }
    let predecessor = store
        .outbox_event(engagement, campaign, reg_op)
        .unwrap()
        .unwrap();
    let event = assess_now(&mut allocator, &mut store, &request(engagement, campaign));
    assert!(event.recorded_eligible());
    (predecessor, event)
}

struct NeverAllocate;
impl OperationAllocator for NeverAllocate {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("recovery/duplicate must not allocate")
    }
}

#[test]
fn eligibility_requires_predecessor_and_history_then_preserves_identity_on_recovery() {
    let _guard = db();
    let (registered, original) = positive(0xd41, true);
    let (engagement, campaign) = (original.engagement_id, original.campaign_id);
    let mut trajectory = PgTrajectory::new(runtime_client());
    let pending = history_view(&mut trajectory, &original, false);
    assert_eq!(
        (pending.state, pending.reason, pending.complete),
        ("pending", "missing_predecessor", false)
    );
    assert_eq!(
        count("trajectory.planning_history", engagement, campaign),
        0
    );
    assert_eq!(
        trajectory.deliver(&registered).unwrap(),
        Delivered::Completed
    );
    assert_eq!(
        history_view(&mut trajectory, &original, false).state,
        "completed"
    );
    assert_eq!(trajectory.publish(&original).unwrap(), Delivered::Duplicate);
    let mut store = PgMissionStore::new(runtime_client());
    let recovered = read_decision(&mut store, &original.request, original.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(recovered, original);
    assert!(history_view(&mut PgTrajectory::new(runtime_client()), &recovered, true).complete);
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
    assert_eq!(
        count_where(
            "trajectory.planning_history",
            "AND status='accepted' AND version=4",
            engagement,
            campaign
        ),
        1
    );
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut store,
            &original.request,
            original.operation_id,
            false
        )
        .unwrap(),
        Some(original.clone())
    );
    let mut conflict = original.request.clone();
    conflict.purpose_ref = GoalRef(Uuid::from_u128(99));
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut store,
            &conflict,
            original.operation_id,
            false
        ),
        Err(Fail::Conflict("integrity_conflict"))
    );
}

struct LostAcknowledgment(PgTrajectory);
impl PlanningHistoryPort for LostAcknowledgment {
    fn publish(&mut self, event: &PlanningAssessed) -> Res<Delivered> {
        assert_eq!(self.0.publish(event)?, Delivered::Completed);
        let row = runtime_client().query_one(
            "SELECT contract,completed_at IS NOT NULL FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND event_id=$3 AND status='accepted'",
            &[&event.engagement_id.0, &event.campaign_id.0, &event.event_id.0],
        ).unwrap();
        assert_eq!(row.get::<_, Value>(0), serde_json::to_value(event).unwrap());
        assert!(row.get::<_, bool>(1));
        Err(Fail::Store("commit_unknown"))
    }
    fn inspect(&mut self, _: &PlanningAssessed) -> Res<Delivered> {
        panic!("no implicit retry after lost acknowledgment")
    }
}

#[test]
fn eligible_consumer_commit_then_ack_loss_is_unknown_before_fresh_recovery() {
    let _guard = db();
    let (_, event) = positive(0xd42, false);
    let (engagement, campaign) = (event.engagement_id, event.campaign_id);
    let unknown = history_view(
        &mut LostAcknowledgment(PgTrajectory::new(runtime_client())),
        &event,
        false,
    );
    assert_eq!(
        (unknown.state, unknown.reason, unknown.complete),
        ("unknown", "consumer_commit_unknown", false)
    );
    assert_eq!(
        count_where(
            "trajectory.planning_history",
            "AND status='accepted' AND version=4",
            engagement,
            campaign
        ),
        1
    );
    let row = runtime_client().query_one(
        "SELECT event_id,contract,completed_at FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
        &[&engagement.0, &campaign.0],
    ).unwrap();
    let (id, contract, completed): (Uuid, Value, std::time::SystemTime) =
        (row.get(0), row.get(1), row.get(2));
    let recovered = read_decision(
        &mut PgMissionStore::new(runtime_client()),
        &event.request,
        event.operation_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered, event);
    let mut fresh = PgTrajectory::new(runtime_client());
    let view = history_view(&mut fresh, &recovered, true);
    assert_eq!((view.state, view.complete), ("completed", true));
    assert_eq!(fresh.publish(&recovered).unwrap(), Delivered::Duplicate);
    let after = runtime_client().query_one(
        "SELECT event_id,contract,completed_at FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
        &[&engagement.0, &campaign.0],
    ).unwrap();
    assert_eq!(
        (
            after.get::<_, Uuid>(0),
            after.get::<_, Value>(1),
            after.get::<_, std::time::SystemTime>(2)
        ),
        (id, contract, completed)
    );
    assert_eq!(
        count("trajectory.planning_history", engagement, campaign),
        1
    );
}

fn withdrawal(engagement: EngagementId, campaign: CampaignId) -> WithdrawalRequest {
    WithdrawalRequest {
        engagement_id: engagement,
        campaign_id: campaign,
        operator_ref: OperatorRef(Uuid::from_u128(99)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    }
}

#[test]
fn concurrent_positive_assessment_and_withdrawal_have_serial_owner_results() {
    let _guard = db();
    let (engagement, campaign) = scope(0xd44);
    let (mut allocator, mut store, mut trajectory) = ports();
    let reg_op = registration::prepare_operation(&mut allocator).unwrap();
    registration::register(
        &mut allocator,
        &mut store,
        &mut trajectory,
        reg_op,
        &registered_input(engagement, campaign),
    )
    .unwrap();
    let op = registration::prepare_operation(&mut allocator).unwrap();
    let wop = registration::prepare_operation(&mut allocator).unwrap();
    let gate = Arc::new(Barrier::new(3));
    let a_gate = gate.clone();
    let assessment = std::thread::spawn(move || {
        let (mut allocator, mut store, _) = ports();
        a_gate.wait();
        retry(|| {
            assess(
                &mut allocator,
                &mut store,
                &request(engagement, campaign),
                op,
                false,
            )
        })
    });
    let w_gate = gate.clone();
    let withdrawal = std::thread::spawn(move || {
        let (mut allocator, mut store, _) = ports();
        w_gate.wait();
        retry(|| {
            store.withdraw(
                &withdrawal(engagement, campaign),
                wop,
                false,
                &mut allocator,
            )
        })
    });
    gate.wait();
    let historical = assessment.join().unwrap().unwrap().unwrap();
    let withdrawn = withdrawal.join().unwrap().unwrap().unwrap();
    assert_eq!(withdrawn.owner_revision, 2);
    assert!(matches!(historical.version, 3 | 4));
    assert_eq!(historical.recorded_eligible(), historical.version == 4);
    let (mut allocator, mut store, _) = ports();
    assert_eq!(
        assess_now(&mut allocator, &mut store, &request(engagement, campaign)).version,
        3
    );
    assert_eq!(count("mission.withdrawals", engagement, campaign), 1);
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        2
    );
}

#[test]
fn owned_upgrade_preserves_old_rows_indexes_and_runtime_permissions_then_accepts_v4() {
    let _guard = db();
    let owned = OwnedUpgradeDatabase::create().unwrap();
    {
        let mut admin = admin_in(&owned.name);
        for migration in [
            MIGRATION,
            PLANNING_MIGRATION,
            HISTORY_MIGRATION,
            V2_MIGRATION,
            WITHDRAWAL_MIGRATION,
            WITHDRAWAL_REFUSAL_MIGRATION,
        ] {
            admin.batch_execute(migration).unwrap();
        }
        let (mut alloc, mut store, mut traj) = (
            PgAllocator::new(runtime_in(&owned.name)),
            PgMissionStore::new(runtime_in(&owned.name)),
            PgTrajectory::new(runtime_in(&owned.name)),
        );
        let (engagement, campaign) = scope(0xd46);
        let reg_op = registration::prepare_operation(&mut alloc).unwrap();
        registration::register(
            &mut alloc,
            &mut store,
            &mut traj,
            reg_op,
            &reg_input(engagement, campaign),
        )
        .unwrap();
        let v2 = assess_now(&mut alloc, &mut store, &request(engagement, campaign));
        assert_eq!(
            (v2.version, v2.decision),
            (2, PlanningDecision::RefusedExpired)
        );
        assert_eq!(traj.publish(&v2).unwrap(), Delivered::Completed);
        let mut v1 = v2.clone();
        v1.version = 1;
        v1.operation_id = OperationId(Uuid::from_u128(0xd470));
        v1.event_id = EventId(Uuid::from_u128(0xd471));
        v1.affected_entity = v1.operation_id;
        v1.causation_id = v1.operation_id;
        v1.correlation_id = v1.operation_id;
        v1.basis.as_mut().unwrap().scope = None;
        v1.decision = PlanningDecision::UnresolvedEvaluationIncomplete;
        v1.validate().unwrap();
        let wire = serde_json::to_value(&v1).unwrap();
        admin.execute("INSERT INTO mission.planning_assessments (engagement_id,campaign_id,operation_id,event_id,contract,publication_obligation) VALUES ($1,$2,$3,$4,$5,'trajectory.planning_history.v1')", &[&engagement.0,&campaign.0,&v1.operation_id.0,&v1.event_id.0,&wire]).unwrap();
        assert_eq!(traj.publish(&v1).unwrap(), Delivered::Completed);
        let wop = registration::prepare_operation(&mut alloc).unwrap();
        let withdrawn = store
            .withdraw(&withdrawal(engagement, campaign), wop, false, &mut alloc)
            .unwrap()
            .unwrap();
        assert_eq!(
            duskweave::withdrawal::WithdrawalHistoryPort::publish(&mut traj, &withdrawn).unwrap(),
            Delivered::Completed
        );
        let v3 = assess_now(&mut alloc, &mut store, &request(engagement, campaign));
        assert_eq!(v3.version, 3);
        assert_eq!(traj.publish(&v3).unwrap(), Delivered::Completed);
        let snapshot = |a: &mut postgres::Client| -> Value {
            a.query_one("SELECT jsonb_agg(to_jsonb(t) ORDER BY version) FROM (SELECT version,contract,recorded_at,completed_at FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted') t", &[&engagement.0,&campaign.0]).unwrap().get(0)
        };
        let indexes = |a: &mut postgres::Client| -> Vec<String> {
            a.query("SELECT indexdef FROM pg_indexes WHERE schemaname='trajectory' AND tablename='planning_history' ORDER BY indexname", &[]).unwrap().iter().map(|r|r.get(0)).collect()
        };
        let old = snapshot(&mut admin);
        assert_eq!(old.as_array().unwrap().len(), 3);
        let old_indexes = indexes(&mut admin);
        let role = dsn("DW_TEST_DATABASE_URL").get_user().unwrap().to_owned();
        let insert_granted = |client: &mut postgres::Client| -> bool {
            client
                .query_one(
                    "SELECT has_table_privilege($1,'trajectory.planning_history','INSERT')",
                    &[&role],
                )
                .unwrap()
                .get(0)
        };
        let privilege = insert_granted(&mut admin);
        for _ in 0..2 {
            admin.batch_execute(ELIGIBILITY_MIGRATION).unwrap();
            assert_eq!(snapshot(&mut admin), old);
            assert_eq!(indexes(&mut admin), old_indexes);
            assert_eq!(insert_granted(&mut admin), privilege);
        }
        let (fresh_e, fresh_c) = scope(0xd47);
        let fresh_reg = registration::prepare_operation(&mut alloc).unwrap();
        registration::register(
            &mut alloc,
            &mut store,
            &mut traj,
            fresh_reg,
            &registered_input(fresh_e, fresh_c),
        )
        .unwrap();
        let eligible = assess_now(&mut alloc, &mut store, &request(fresh_e, fresh_c));
        assert!(eligible.recorded_eligible());
        assert_eq!(traj.publish(&eligible).unwrap(), Delivered::Completed);
        assert_eq!(traj.inspect(&eligible).unwrap(), Delivered::Completed);
        assert_eq!(snapshot(&mut admin), old);
        assert_eq!(indexes(&mut admin), old_indexes);
    }
    owned.finish().unwrap();
}

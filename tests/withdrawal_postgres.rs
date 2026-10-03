use duskweave::mission::*;
use duskweave::planning::PlanningRequest;
use duskweave::planning_assessment::PlanningStore;
use duskweave::registration::{self, MissionStore, OperationAllocator};
use duskweave::withdrawal::*;
use duskweave::{Fail, Res};
use postgres::NoTls;
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

fn planning(e: EngagementId, c: CampaignId, confirmed: bool) -> PlanningRequest {
    PlanningRequest {
        engagement_id: e,
        campaign_id: c,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: confirmed,
    }
}

struct NoAllocation;
impl OperationAllocator for NoAllocation {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("must deny/recover before allocation")
    }
}

#[test]
fn transition_is_immutable_scoped_and_denies_before_allocation_without_history() {
    let _guard = db();
    let (e, c) = scope(1);
    let (other_e, other_c) = scope(2);
    let (mut alloc, mut store, mut trajectory) = ports();
    let registered = accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    accepted(&mut alloc, &mut store, &mut trajectory, other_e, other_c);
    let old_view = store.mission_view(e, c).unwrap().unwrap();
    let old_event = store
        .outbox_event(e, c, registered.operation_id)
        .unwrap()
        .unwrap();
    let historical_op = registration::prepare_operation(&mut alloc).unwrap();
    let historical_request = planning(e, c, false);
    let historical = store
        .assess(&historical_request, historical_op, false, &mut alloc)
        .unwrap();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let input = request(e, c);
    for collision in [registered.operation_id, historical_op] {
        assert_eq!(
            store.withdraw(&input, collision, false, &mut NoAllocation),
            Err(Fail::Conflict("integrity_conflict"))
        );
    }
    let withdrawn = store
        .withdraw(&input, op, false, &mut alloc)
        .unwrap()
        .unwrap();
    assert_eq!(withdrawn.registration_operation_id, registered.operation_id);
    assert_eq!(withdrawn.owner_revision, 2);
    assert_eq!(withdrawn.occurred_at, withdrawn.recorded_at);
    assert_eq!(count("mission.withdrawals", e, c), 1);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    let row = runtime_client().query_one(
        "SELECT contract, publication_obligation, owner_revision FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0]).unwrap();
    assert_eq!(
        row.get::<_, serde_json::Value>(0),
        serde_json::to_value(&withdrawn).unwrap()
    );
    assert_eq!(row.get::<_, &str>(1), OBLIGATION);
    assert_eq!(row.get::<_, i64>(2), 2);
    assert_eq!(
        store.outbox_event(e, c, registered.operation_id).unwrap(),
        Some(old_event)
    );
    let mut new_view = store.mission_view(e, c).unwrap().unwrap();
    assert_eq!(new_view.revision, 2);
    new_view.revision = 1;
    assert_eq!(new_view, old_view);
    for recover in [true, false] {
        assert_eq!(
            store
                .withdraw(&input, op, recover, &mut NoAllocation)
                .unwrap(),
            Some(withdrawn.clone())
        );
        assert_eq!(
            store
                .assess(
                    &historical_request,
                    historical_op,
                    recover,
                    &mut NoAllocation
                )
                .unwrap(),
            historical
        );
        let mut changed = input.clone();
        changed.reason = WithdrawalReason::ScopeConcern;
        assert_eq!(
            store.withdraw(&changed, op, recover, &mut NoAllocation),
            Err(Fail::Conflict("integrity_conflict"))
        );
    }
    for confirmed in [true, false] {
        let fresh_op = registration::prepare_operation(&mut alloc).unwrap();
        assert_eq!(
            store.assess(
                &planning(e, c, confirmed),
                fresh_op,
                false,
                &mut NoAllocation
            ),
            Err(Fail::State("authority_withdrawn"))
        );
        assert_eq!(
            store.assess(&planning(e, c, confirmed), op, false, &mut NoAllocation),
            Err(Fail::Conflict("integrity_conflict"))
        );
    }
    let distinct_op = registration::prepare_operation(&mut alloc).unwrap();
    assert_eq!(
        store.withdraw(&input, distinct_op, false, &mut NoAllocation),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(
        store
            .withdraw(&input, distinct_op, true, &mut NoAllocation)
            .unwrap(),
        None
    );
    let (m, ev) =
        Mission::register(&reg_input(e, c), op, EventId(Uuid::from_u128(77)), 10).unwrap();
    assert_eq!(
        store.commit_registration(&m, &ev),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(
        store
            .assess(&historical_request, historical_op, true, &mut NoAllocation)
            .unwrap(),
        historical
    );
    assert!(
        store
            .assess(&planning(other_e, other_c, true), op, false, &mut alloc)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        store
            .mission_view(other_e, other_c)
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    assert_eq!(count("mission.planning_assessments", e, c), 1);
}

#[test]
fn missing_stale_invalid_allocator_and_precommit_failure_cannot_accept() {
    let _guard = db();
    let (e, c) = scope(3);
    let (mut alloc, mut store, mut trajectory) = ports();
    let input = request(e, c);
    let op = registration::prepare_operation(&mut alloc).unwrap();
    assert_eq!(
        store.withdraw(&input, op, false, &mut NoAllocation),
        Err(Fail::State("mission_missing"))
    );
    assert_eq!(
        store.withdraw(&input, OperationId(Uuid::nil()), false, &mut NoAllocation),
        Err(Fail::Input("nil_identity"))
    );
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let mut stale = input.clone();
    stale.expected_mission_revision = 2;
    assert_eq!(
        store.withdraw(&stale, op, false, &mut NoAllocation),
        Err(Fail::State("stale_revision"))
    );
    struct Fault(Res<Uuid>);
    impl OperationAllocator for Fault {
        fn allocate(&mut self) -> Res<Uuid> {
            self.0
        }
    }
    for failure in [Err(Fail::Store("allocator_failed")), Ok(Uuid::nil())] {
        assert!(
            store
                .withdraw(&input, op, false, &mut Fault(failure))
                .is_err()
        );
        assert_eq!(count("mission.withdrawals", e, c), 0);
    }
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = config.connect(NoTls).unwrap();
    admin
        .batch_execute(&format!(
            "CREATE FUNCTION mission.c1a_insert_fault() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'SYNTHETIC_SECRET_SENTINEL'; END $$; \
         CREATE TRIGGER c1a_insert_fault BEFORE INSERT ON mission.withdrawals FOR EACH ROW \
         WHEN (NEW.engagement_id='{e}'::uuid) EXECUTE FUNCTION mission.c1a_insert_fault();"
        ))
        .unwrap();
    let result = store.withdraw(&input, op, false, &mut alloc);
    admin.batch_execute("DROP TRIGGER c1a_insert_fault ON mission.withdrawals; DROP FUNCTION mission.c1a_insert_fault();").unwrap();
    assert_eq!(result, Err(Fail::Store("storage_error")));
    assert_eq!(count("mission.withdrawals", e, c), 0);
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    assert_eq!(
        store.withdraw(&input, op, true, &mut NoAllocation).unwrap(),
        None
    );
}

#[test]
fn producer_catalog_corruption_is_not_recovered_as_success() {
    let _guard = db();
    let (e, c) = scope(4);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let input = request(e, c);
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let event = store
        .withdraw(&input, op, false, &mut alloc)
        .unwrap()
        .unwrap();
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = config.connect(NoTls).unwrap();
    for changed in [serde_json::json!({}), {
        let mut value = serde_json::to_value(&event).unwrap();
        value["event_id"] = serde_json::json!(Uuid::from_u128(33));
        value
    }] {
        admin.execute("UPDATE mission.withdrawals SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &changed]).unwrap();
        assert_eq!(
            store.withdraw(&input, op, true, &mut NoAllocation),
            Err(Fail::Store("contract_decode"))
        );
    }
    admin
        .execute(
            "UPDATE mission.withdrawals SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &serde_json::to_value(event).unwrap()],
        )
        .unwrap();
}

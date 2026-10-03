//! Version-3 recorded-withdrawal refusal: producer, history and fault
//! assertions over the real PostgreSQL ports. A fresh assessment after a
//! committed withdrawal records one durable v3 refusal; duplicates, conflicts,
//! collisions, faults and history completion are exercised here.

use duskweave::mission::*;
use duskweave::planning::{
    NonpositiveDecision, PlanningAssessed, PlanningRequest, WithdrawalBasis,
};
use duskweave::planning_assessment::PlanningStore;
use duskweave::planning_history::PlanningHistoryPort;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::registration::{self, OperationAllocator};
use duskweave::trajectory::Delivered;
use duskweave::withdrawal::*;
use duskweave::{Fail, Res};
use postgres::NoTls;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

fn withdrawal(e: EngagementId, c: CampaignId) -> WithdrawalRequest {
    WithdrawalRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(99)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    }
}

fn planning(e: EngagementId, c: CampaignId, revision: u64, confirmed: bool) -> PlanningRequest {
    PlanningRequest {
        engagement_id: e,
        campaign_id: c,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: revision,
        current_authority_confirmed: confirmed,
    }
}

fn assert_v3(
    event: &PlanningAssessed,
    w_op: OperationId,
    w_event: EventId,
    e: EngagementId,
    c: CampaignId,
) {
    assert_eq!(event.version, 3);
    assert_eq!(event.owner_revision, 2);
    assert_eq!(
        event.decision,
        NonpositiveDecision::RefusedAuthorityWithdrawn
    );
    assert_eq!(event.engagement_id, e);
    assert_eq!(event.campaign_id, c);
    assert_eq!(
        event.withdrawal,
        Some(WithdrawalBasis {
            operation_id: w_op,
            event_id: w_event,
        })
    );
    event.validate().unwrap();
}

#[test]
fn fresh_assessment_after_withdrawal_records_v3_across_all_guards() {
    let _guard = db();
    let (e, c) = scope(0xc10);
    let (other_e, other_c) = scope(0xc11);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    accepted(&mut alloc, &mut store, &mut traj, other_e, other_c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawn = store
        .withdraw(&withdrawal(e, c), w_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    // true/false, stale revision and mismatched scope all record v3; the
    // recorded fixture window is already expired, so out-of-window is covered.
    for confirmed in [true, false] {
        for (revision, purpose) in [
            (1, Uuid::from_u128(0x13)),
            (99, Uuid::from_u128(0x13)),
            (1, Uuid::from_u128(0x9999)),
        ] {
            let op = registration::prepare_operation(&mut alloc).unwrap();
            let mut request = planning(e, c, revision, confirmed);
            request.purpose_ref = GoalRef(purpose);
            let refused = store
                .assess(&request, op, false, &mut alloc)
                .unwrap()
                .unwrap();
            assert_v3(&refused, w_op, withdrawn.event_id, e, c);
        }
    }
    // Other campaign is unaffected by this campaign's withdrawal.
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let other = store
        .assess(&planning(other_e, other_c, 1, false), op, false, &mut alloc)
        .unwrap()
        .unwrap();
    assert_eq!(other.version, 2);
    assert_eq!(other.withdrawal, None);
    // Nil/malformed requests still reject before effects.
    let nil_op = OperationId(Uuid::nil());
    assert_eq!(
        store.assess(&planning(e, c, 1, false), nil_op, false, &mut alloc),
        Err(Fail::Input("nil_identity"))
    );
    let mut nil_req = planning(e, c, 1, false);
    nil_req.asset_ref = AssetRef(Uuid::nil());
    let bad_op = registration::prepare_operation(&mut alloc).unwrap();
    assert_eq!(
        store.assess(&nil_req, bad_op, false, &mut alloc),
        Err(Fail::Input("nil_reference"))
    );
}

#[test]
fn v3_duplicate_changed_payload_and_collision_deny() {
    let _guard = db();
    let (e, c) = scope(0xc12);
    let (mut alloc, mut store, mut traj) = ports();
    let registered = accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawn = store
        .withdraw(&withdrawal(e, c), w_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let request = planning(e, c, 1, true);
    let refused = store
        .assess(&request, op, false, &mut alloc)
        .unwrap()
        .unwrap();
    assert_v3(&refused, w_op, withdrawn.event_id, e, c);
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    // Duplicate/recovery yields the same single effect without allocation.
    struct Never;
    impl OperationAllocator for Never {
        fn allocate(&mut self) -> Res<Uuid> {
            panic!("duplicate must not allocate")
        }
    }
    assert_eq!(
        store.assess(&request, op, true, &mut Never).unwrap(),
        Some(refused.clone())
    );
    assert_eq!(
        store.assess(&request, op, false, &mut Never).unwrap(),
        Some(refused.clone())
    );
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    // Changed payload under the same operation identity conflicts.
    let mut changed = planning(e, c, 1, true);
    changed.purpose_ref = GoalRef(Uuid::from_u128(0x7777));
    assert_eq!(
        store.assess(&changed, op, false, &mut Never),
        Err(Fail::Conflict("integrity_conflict"))
    );
    // Registration and withdrawal operation identities collide and deny.
    for collision in [registered.operation_id, w_op] {
        assert_eq!(
            store.assess(&planning(e, c, 1, false), collision, false, &mut Never),
            Err(Fail::Conflict("integrity_conflict"))
        );
    }
}

#[test]
fn v3_fault_and_lost_ack_preserve_durable_decision() {
    let _guard = db();
    let (e, c) = scope(0xc14);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawn = store
        .withdraw(&withdrawal(e, c), w_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    // Allocator failure leaves no durable claim.
    struct FaultAllocator(Res<Uuid>);
    impl OperationAllocator for FaultAllocator {
        fn allocate(&mut self) -> Res<Uuid> {
            self.0
        }
    }
    assert!(
        store
            .assess(
                &planning(e, c, 1, false),
                op,
                false,
                &mut FaultAllocator(Err(Fail::Store("allocator_failed")))
            )
            .is_err()
    );
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    // Pre-insert fault leaves no false durable claim.
    let mut admin = dsn("DW_TEST_ADMIN_DATABASE_URL");
    admin.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = admin.connect(NoTls).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fn_name = format!("c1b_preinsert_{:x}", nonce & 0xffff_ffff);
    admin
        .batch_execute(&format!(
            "CREATE FUNCTION mission.{fn_name}() RETURNS trigger LANGUAGE plpgsql AS          $$ BEGIN RAISE EXCEPTION 'SYNTHETIC_SECRET_SENTINEL'; END $$;          CREATE TRIGGER {fn_name} BEFORE INSERT ON mission.planning_assessments          FOR EACH ROW WHEN (NEW.engagement_id='{e}'::uuid) EXECUTE FUNCTION mission.{fn_name}();"
        ))
        .unwrap();
    let result = store.assess(&planning(e, c, 1, false), op, false, &mut alloc);
    admin
        .batch_execute(&format!(
            "DROP TRIGGER {fn_name} ON mission.planning_assessments; DROP FUNCTION mission.{fn_name}();"
        ))
        .unwrap();
    assert_eq!(result, Err(Fail::Store("storage_error")));
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    // Real commit then injected ACK loss: the stored decision is recovered.
    struct LostAck {
        real: PgMissionStore,
    }
    impl PlanningStore for LostAck {
        fn assess(
            &mut self,
            request: &PlanningRequest,
            operation: OperationId,
            recover: bool,
            allocator: &mut dyn OperationAllocator,
        ) -> Res<Option<PlanningAssessed>> {
            let committed = self
                .real
                .assess(request, operation, recover, allocator)?
                .unwrap();
            let mut observer = runtime_client();
            let row = observer
                .query_one(
                    "SELECT event_id, contract, publication_obligation FROM mission.planning_assessments                  WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
                    &[&request.engagement_id.0, &request.campaign_id.0, &operation.0],
                )
                .unwrap();
            assert_eq!(row.get::<_, Uuid>(0), committed.event_id.0);
            assert_eq!(
                row.get::<_, serde_json::Value>(1),
                serde_json::to_value(&committed).unwrap()
            );
            assert_eq!(row.get::<_, String>(2), "trajectory.planning_history.v1");
            Err(Fail::Store("simulated_ack_loss"))
        }
    }
    let mut decorator = LostAck {
        real: PgMissionStore::new(runtime_client()),
    };
    assert_eq!(
        decorator.assess(&planning(e, c, 1, false), op, false, &mut alloc),
        Err(Fail::Store("simulated_ack_loss"))
    );
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    let mut fresh = PgMissionStore::new(runtime_client());
    let recovered = fresh
        .assess(
            &planning(e, c, 1, false),
            op,
            true,
            &mut FaultAllocator(Ok(Uuid::nil())),
        )
        .unwrap()
        .unwrap();
    assert_eq!(recovered.version, 3);
    assert_v3(&recovered, w_op, withdrawn.event_id, e, c);
}

#[test]
fn v3_history_pending_until_withdrawal_history_then_corruption_never_completes() {
    let _guard = db();
    let (e, c) = scope(0xc16);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawn = store
        .withdraw(&withdrawal(e, c), w_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let refused = store
        .assess(&planning(e, c, 1, false), op, false, &mut alloc)
        .unwrap()
        .unwrap();
    // Withdrawal history not yet published: the v3 history is pending.
    assert_eq!(
        PlanningHistoryPort::publish(&mut traj, &refused).unwrap(),
        Delivered::Unresolved("missing_predecessor")
    );
    // Publishing the withdrawal history lets the v3 history complete.
    assert_eq!(
        WithdrawalHistoryPort::publish(&mut traj, &withdrawn).unwrap(),
        Delivered::Completed
    );
    assert_eq!(
        PlanningHistoryPort::publish(&mut traj, &refused).unwrap(),
        Delivered::Completed
    );
    assert_eq!(
        count_where("trajectory.planning_history", "AND status='accepted'", e, c),
        1
    );
    assert_eq!(
        count_where(
            "trajectory.withdrawal_history",
            "AND status='accepted'",
            e,
            c
        ),
        1
    );
    // Corrupting the referenced withdrawal contract is a bounded decode failure.
    let mut admin = dsn("DW_TEST_ADMIN_DATABASE_URL");
    admin.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = admin.connect(NoTls).unwrap();
    admin
        .execute(
            "UPDATE trajectory.withdrawal_history SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0, &serde_json::json!({})],
        )
        .unwrap();
    // New scope so the prior accepted planning row does not short-circuit as duplicate.
    let (e2, c2) = scope(0xc17);
    let (mut alloc2, mut store2, mut traj2) = ports();
    accepted(&mut alloc2, &mut store2, &mut traj2, e2, c2);
    let w2_op = registration::prepare_operation(&mut alloc2).unwrap();
    let withdrawn2 = store2
        .withdraw(&withdrawal(e2, c2), w2_op, false, &mut alloc2)
        .unwrap()
        .unwrap();
    let op2 = registration::prepare_operation(&mut alloc2).unwrap();
    let refused2 = store2
        .assess(&planning(e2, c2, 1, false), op2, false, &mut alloc2)
        .unwrap()
        .unwrap();
    assert_eq!(
        WithdrawalHistoryPort::publish(&mut traj2, &withdrawn2).unwrap(),
        Delivered::Completed
    );
    // Corrupt the withdrawal history contract bound to this refusal.
    admin
        .execute(
            "UPDATE trajectory.withdrawal_history SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e2.0, &c2.0, &serde_json::json!({})],
        )
        .unwrap();
    assert_eq!(
        PlanningHistoryPort::publish(&mut traj2, &refused2),
        Err(Fail::Store("contract_decode"))
    );
}

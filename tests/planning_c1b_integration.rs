//! C1b focused integration test for durability and corruption
use duskweave::mission::*;
use duskweave::planning::{NonpositiveDecision, PlanningRequest};
use duskweave::planning_assessment::PlanningStore;
use duskweave::registration::{self, OperationAllocator};
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

struct NeverAllocate;
impl OperationAllocator for NeverAllocate {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("allocator should not be called")
    }
}

#[test]
fn focused_c1b_corruption_and_collision_deny() {
    let _guard = db();
    let (e, c) = scope(0xc1b);
    let (mut alloc, mut store, mut traj) = ports();

    // 1. Register and withdraw
    let registered = accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawn = store
        .withdraw(&withdrawal(e, c), w_op, false, &mut alloc)
        .unwrap()
        .unwrap();

    // 2. Corrupt the isolated admin fixture's stored withdrawal payload in mission.mission_withdrawals
    let mut admin = dsn("DW_TEST_ADMIN_DATABASE_URL");
    admin.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = admin.connect(NoTls).unwrap();

    // Save exact marker
    let row = admin
        .query_one(
            "SELECT contract FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    let exact_marker: serde_json::Value = row.get(0);

    // Corrupt payload
    admin
        .execute(
            "UPDATE mission.withdrawals SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &serde_json::json!({})],
        )
        .unwrap();

    // 3. Reused registration/withdrawal operation identity returns integrity_conflict before corrupted marker
    assert_eq!(
        store.assess(
            &planning(e, c),
            registered.operation_id,
            false,
            &mut NeverAllocate
        ),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(
        store.assess(&planning(e, c), w_op, false, &mut NeverAllocate),
        Err(Fail::Conflict("integrity_conflict"))
    );

    // 4. Fresh preallocated operation returns contract_decode with NeverAllocate and unchanged rows
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let pre_count = count("mission.planning_assessments", e, c);

    assert_eq!(
        store.assess(&planning(e, c), op, false, &mut NeverAllocate),
        Err(Fail::Store("contract_decode"))
    );
    assert_eq!(count("mission.planning_assessments", e, c), pre_count);

    // 5. Restore exact marker and prove fresh valid control records v3 with original withdrawal reference
    admin
        .execute(
            "UPDATE mission.withdrawals SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &exact_marker],
        )
        .unwrap();

    let op2 = registration::prepare_operation(&mut alloc).unwrap();
    let refused = store
        .assess(&planning(e, c), op2, false, &mut alloc)
        .unwrap()
        .unwrap();

    assert_eq!(refused.version, 3);
    assert_eq!(refused.owner_revision, 2);
    assert_eq!(
        refused.decision,
        NonpositiveDecision::RefusedAuthorityWithdrawn
    );
    assert_eq!(refused.withdrawal.unwrap().operation_id, w_op);
    assert_eq!(refused.withdrawal.unwrap().event_id, withdrawn.event_id);
}

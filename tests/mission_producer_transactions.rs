//! Combined-condition characterization of the Mission producer's two
//! transaction phases. Real-port scenarios pin the observable precedence
//! when a rejection category is dominated by a stronger one: cross-family
//! identity reuse beats an occupied registration scope, and the registration
//! outbox identity conflict beats an unsupported corrupted owner basis.
//! Both scenarios pass unchanged before and after the phase extraction.

use duskweave::Fail;
use duskweave::Res;
use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef};
use duskweave::planning::{NonpositiveDecision, PlanningRequest};
use duskweave::planning_assessment;
use duskweave::registration::{self, OperationAllocator};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

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

struct NeverAllocate;

impl OperationAllocator for NeverAllocate {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("conflict rejection must not allocate an event")
    }
}

/// Complete scoped rows of every producer/history table, so a rejected
/// combined-condition attempt proves no observable row change at all.
fn scoped_rows(table: &str, e: EngagementId, c: CampaignId) -> Vec<String> {
    let mut client = runtime_client();
    let rows = client
        .query(
            &format!(
                "SELECT row_to_json(t)::text FROM {table} t \
                 WHERE engagement_id=$1 AND campaign_id=$2 ORDER BY 1"
            ),
            &[&e.0, &c.0],
        )
        .unwrap();
    rows.iter().map(|r| r.get::<_, String>(0)).collect()
}

fn scoped_state(e: EngagementId, c: CampaignId) -> Vec<Vec<String>> {
    [
        "mission.missions",
        "mission.registration_outbox",
        "mission.planning_assessments",
        "mission.withdrawals",
        "trajectory.registration_history",
        "trajectory.planning_history",
        "trajectory.withdrawal_history",
    ]
    .iter()
    .map(|table| scoped_rows(table, e, c))
    .collect()
}

fn admin_in_test_database() -> postgres::Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    config.connect(postgres::NoTls).unwrap()
}

#[test]
fn planning_operation_registration_collision_dominates_occupied_scope() {
    let _guard = db();
    let (engagement, campaign) = scope(0x9f2a1);
    let (mut allocator, mut store, mut trajectory) = ports();
    accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    let planning_operation = registration::prepare_operation(&mut allocator).unwrap();
    let assessed = planning_assessment::assess(
        &mut allocator,
        &mut store,
        &request(engagement, campaign),
        planning_operation,
        false,
    )
    .unwrap()
    .expect("durable planning decision in the occupied scope");
    assert_eq!(assessed.operation_id, planning_operation);
    let before = scoped_state(engagement, campaign);
    // Both rejections apply: the scope is already registered and the
    // operation identity belongs to the planning family. Identity wins.
    assert_eq!(
        reg(
            &mut allocator,
            &mut store,
            &mut trajectory,
            planning_operation,
            engagement,
            campaign
        ),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(scoped_state(engagement, campaign), before);
    assert_eq!(count("mission.missions", engagement, campaign), 1);
    assert_eq!(
        count("mission.registration_outbox", engagement, campaign),
        1
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
    assert_eq!(
        count("trajectory.registration_history", engagement, campaign),
        1
    );
    assert_eq!(
        count("trajectory.planning_history", engagement, campaign),
        0
    );
    assert_eq!(count("mission.withdrawals", engagement, campaign), 0);
}

#[test]
fn corrupted_owner_registration_operation_dominates_unsupported_basis() {
    let _guard = db();
    let (engagement, campaign) = scope(0x9f2a2);
    let (mut allocator, mut store, mut trajectory) = ports();
    let registration = accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    let intact = scoped_state(engagement, campaign);
    let mut admin = admin_in_test_database();
    // Corrupt the stored registration operation without relaxing any
    // constraint: nil stays a valid uuid, but a decoded owner basis built
    // from this row would be unsupported.
    admin
        .execute(
            "UPDATE mission.missions SET operation_id=$3 \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&engagement.0, &campaign.0, &Uuid::nil()],
        )
        .unwrap();
    let corrupted = scoped_state(engagement, campaign);
    assert_ne!(corrupted, intact);
    // Assessing with the original registration operation: the registration
    // outbox identity conflict is returned before the corrupted basis is
    // read, and no event identity is allocated on the rejected path.
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut store,
            &request(engagement, campaign),
            registration.operation_id,
            false
        ),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(scoped_state(engagement, campaign), corrupted);
    // Restore the original row exactly; the restored scope must remain
    // usable for a fresh valid assessment control.
    admin
        .execute(
            "UPDATE mission.missions SET operation_id=$3 \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&engagement.0, &campaign.0, &registration.operation_id.0],
        )
        .unwrap();
    assert_eq!(scoped_state(engagement, campaign), intact);
    let control_operation = registration::prepare_operation(&mut allocator).unwrap();
    let control = planning_assessment::assess(
        &mut allocator,
        &mut store,
        &request(engagement, campaign),
        control_operation,
        false,
    )
    .unwrap()
    .expect("fresh control assessment after restoration");
    assert_eq!(control.decision, NonpositiveDecision::RefusedExpired);
    assert_eq!(
        control.basis.as_ref().unwrap().registration_operation_id,
        registration.operation_id
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
}

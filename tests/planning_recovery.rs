use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef, OperationId};
use duskweave::planning::{PlanningAssessed, PlanningRequest};
use duskweave::planning_assessment::{self, PlanningStore};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::registration::OperationAllocator;
use duskweave::{Fail, Res};
use postgres::NoTls;
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
        panic!("recovery must not allocate")
    }
}

struct LostAcknowledgment {
    real: PgMissionStore,
}

impl PlanningStore for LostAcknowledgment {
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
        let row = observer.query_one(
            "SELECT event_id, contract, publication_obligation FROM mission.planning_assessments \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[&request.engagement_id.0, &request.campaign_id.0, &operation.0],
        ).unwrap();
        assert_eq!(row.get::<_, Uuid>(0), committed.event_id.0);
        assert_eq!(
            row.get::<_, serde_json::Value>(1),
            serde_json::to_value(&committed).unwrap()
        );
        assert_eq!(row.get::<_, String>(2), "trajectory.planning_history.v1");
        Err(Fail::Store("simulated_ack_loss"))
    }
}

#[test]
fn precommit_failure_has_no_durable_decision_and_recovery_does_not_allocate() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1a8);
    let (mut allocator, mut store, _trajectory) = ports();
    let operation = OperationId(allocator.allocate().unwrap());
    let input = request(engagement, campaign);
    let mut admin_config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    admin_config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = admin_config.connect(NoTls).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let function = format!("b1a_precommit_{:x}", nonce & 0xffff_ffff);
    admin
        .batch_execute(&format!(
            "CREATE FUNCTION mission.{function}() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'test precommit fault'; END $$; \
         CREATE TRIGGER {function} BEFORE INSERT ON mission.planning_assessments \
         FOR EACH ROW WHEN (NEW.engagement_id = '{engagement}'::uuid) \
         EXECUTE FUNCTION mission.{function}();"
        ))
        .unwrap();
    assert_eq!(
        planning_assessment::assess(&mut allocator, &mut store, &input, operation, false),
        Err(Fail::Store("storage_error"))
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        0
    );
    let mut fresh = PgMissionStore::new(runtime_client());
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut fresh, &input, operation, true)
            .unwrap(),
        None
    );
    admin
        .batch_execute(&format!(
            "DROP TRIGGER {function} ON mission.planning_assessments; \
         DROP FUNCTION mission.{function}();"
        ))
        .unwrap();
}

#[test]
fn committed_before_lost_ack_recovers_original_from_new_ports() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1a9);
    let (mut allocator, _store, _trajectory) = ports();
    let operation = OperationId(allocator.allocate().unwrap());
    let input = request(engagement, campaign);
    let mut decorator = LostAcknowledgment {
        real: PgMissionStore::new(runtime_client()),
    };
    assert_eq!(
        planning_assessment::assess(&mut allocator, &mut decorator, &input, operation, false),
        Err(Fail::Store("simulated_ack_loss"))
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
    let mut fresh = PgMissionStore::new(runtime_client());
    let recovered =
        planning_assessment::assess(&mut NeverAllocate, &mut fresh, &input, operation, true)
            .unwrap()
            .unwrap();
    let mut observer = runtime_client();
    let row = observer.query_one(
        "SELECT contract FROM mission.planning_assessments WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
        &[&engagement.0, &campaign.0, &operation.0],
    ).unwrap();
    assert_eq!(
        serde_json::to_value(recovered).unwrap(),
        row.get::<_, serde_json::Value>(0)
    );
    assert_eq!(
        count("trajectory.registration_history", engagement, campaign),
        0
    );
}

#[test]
fn recovery_uses_durable_record_without_current_owner_privilege() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1aa);
    let (mut allocator, mut store, mut trajectory) = ports();
    accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    let operation = OperationId(allocator.allocate().unwrap());
    let input = request(engagement, campaign);
    let original =
        planning_assessment::assess(&mut allocator, &mut store, &input, operation, false)
            .unwrap()
            .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let role = format!("dw_b1a_recovery_{:x}", nonce & 0xffff_ffff);
    let password = format!("fixture_{nonce:x}");
    let mut admin_config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    admin_config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = admin_config.connect(NoTls).unwrap();
    admin
        .batch_execute(&format!(
            "CREATE ROLE {role} NOINHERIT LOGIN PASSWORD '{password}'; \
         GRANT USAGE ON SCHEMA mission, trajectory TO {role}; \
         GRANT SELECT, INSERT ON mission.planning_assessments TO {role}; \
         GRANT SELECT ON mission.registration_outbox TO {role}; \
         GRANT SELECT ON trajectory.registration_history TO {role};"
        ))
        .unwrap();
    let mut config = dsn("DW_TEST_DATABASE_URL");
    config.user(&role).password(&password);
    let mut client = config.connect(NoTls).unwrap();
    let privileges = client
        .query_one(
            "SELECT pg_has_role(current_user, 'dw_runtime', 'member'), \
         has_table_privilege(current_user, 'mission.missions', 'SELECT')",
            &[],
        )
        .unwrap();
    assert!(!privileges.get::<_, bool>(0));
    assert!(!privileges.get::<_, bool>(1));
    let mut restricted = PgMissionStore::new(client);
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut restricted, &input, operation, true)
            .unwrap(),
        Some(original)
    );
    let new_operation = OperationId(allocator.allocate().unwrap());
    assert_eq!(
        planning_assessment::assess(
            &mut allocator,
            &mut restricted,
            &input,
            new_operation,
            false
        ),
        Err(Fail::Store("storage_error"))
    );
    drop(restricted);
    admin
        .batch_execute(&format!(
            "REVOKE ALL ON mission.planning_assessments, mission.registration_outbox FROM {role}; \
         REVOKE ALL ON trajectory.registration_history FROM {role}; \
         REVOKE ALL ON SCHEMA mission, trajectory FROM {role}; DROP ROLE {role};"
        ))
        .unwrap();
}

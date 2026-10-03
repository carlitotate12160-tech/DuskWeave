//! Version-1 contract compatibility and the real 0001..0003 -> 0004 -> 0005 upgrade.
//! v1 events are explicit legacy JSON; production has no v1 constructor.

use duskweave::Fail;
use duskweave::input::parse_register;
use duskweave::mission::{EngagementId, EventId, Mission, OperationId};
use duskweave::planning::{NonpositiveDecision, PlanningAssessed};
use duskweave::planning_history::{PlanningHistoryPort, read_decision};
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::trajectory::Delivered;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

#[path = "support/upgrade_db.rs"]
mod upgrade_support;
use upgrade_support::*;

fn v1_event_json(
    e: EngagementId,
    c: duskweave::mission::CampaignId,
    identities: (Uuid, Uuid, Uuid),
    window: (i64, i64),
    at: i64,
) -> Value {
    let (op, event, reg_op) = identities;
    let (starts_at, ends_at) = window;
    json!({
        "event_id": event, "operation_id": op,
        "engagement_id": e.0, "campaign_id": c.0,
        "producer": "mission", "kind": "planning_assessed", "version": 1,
        "affected_entity": op, "owner_revision": 1,
        "causation_id": op, "correlation_id": op,
        "request": {
            "engagement_id": e.0, "campaign_id": c.0,
            "purpose_ref": Uuid::from_u128(0x13),
            "asset_ref": Uuid::from_u128(0x21),
            "expected_mission_revision": 1,
            "current_authority_confirmed": true
        },
        "basis": {
            "registration_operation_id": reg_op, "revision": 1,
            "exercise_mode": "blind", "starts_at": starts_at, "ends_at": ends_at
        },
        "decision": "unresolved_evaluation_incomplete",
        "evaluated_at": at, "occurred_at": at, "recorded_at": at,
        "time_basis": "producer_transaction_start"
    })
}

#[test]
fn legacy_v1_json_decodes_and_validates_under_v1_semantics() {
    let (e, c) = scope(0xb2e0);
    // evaluated_at outside its recorded window is still a valid v1 record:
    // v1 never evaluated scope or window.
    let event: PlanningAssessed = serde_json::from_value(v1_event_json(
        e,
        c,
        (
            Uuid::from_u128(0xb2e1),
            Uuid::from_u128(0xb2e2),
            Uuid::from_u128(0xb2e3),
        ),
        (1_700_000_000, 1_700_086_400),
        2_000_000_000,
    ))
    .unwrap();
    event.validate().unwrap();
    assert_eq!(
        event.decision,
        NonpositiveDecision::UnresolvedEvaluationIncomplete
    );
    assert_eq!(event.version, 1);
    assert_eq!(
        event.assessment_labels(),
        ("not_evaluated", "not_evaluated")
    );
    // Round trip keeps the old wire shape: no scope key materializes.
    let rewritten = serde_json::to_value(&event).unwrap();
    assert!(rewritten["basis"].get("scope").is_none());
}

#[test]
fn v1_rejects_scope_snapshot_v2_decisions_and_other_versions() {
    let (e, c) = scope(0xb2e4);
    let ids = (
        Uuid::from_u128(0xb2e5),
        Uuid::from_u128(0xb2e6),
        Uuid::from_u128(0xb2e7),
    );
    let mut scoped = v1_event_json(e, c, ids, (100, 200), 150);
    scoped["basis"]["scope"] = json!({
        "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21)],
        "excluded_assets": []
    });
    let event: PlanningAssessed = serde_json::from_value(scoped).unwrap();
    assert_eq!(
        event.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    let mut v2_only = v1_event_json(e, c, ids, (100, 200), 150);
    v2_only["decision"] = json!("refused_expired");
    let event: PlanningAssessed = serde_json::from_value(v2_only).unwrap();
    assert_eq!(
        event.validate(),
        Err(Fail::Unresolved("invalid_assessment"))
    );
    let mut versioned = v1_event_json(e, c, ids, (100, 200), 150);
    versioned["version"] = json!(99);
    let event: PlanningAssessed = serde_json::from_value(versioned).unwrap();
    assert_eq!(
        event.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    let mut v2_unscoped = v1_event_json(e, c, ids, (100, 200), 150);
    v2_unscoped["version"] = json!(2);
    let event: PlanningAssessed = serde_json::from_value(v2_unscoped).unwrap();
    assert_eq!(
        event.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
}

#[test]
fn upgrade_identity_is_bounded_and_cannot_name_a_template_or_sql() {
    for name in [
        "",
        "postgres",
        "POSTGRES",
        "template0",
        "template1",
        "bad-name",
        "é",
        "x\";DROP",
    ] {
        assert_eq!(upgrade_identifier(name), Err("invalid_upgrade_identifier"));
    }
    assert!(upgrade_identifier(&"a".repeat(63)).is_ok());
    assert!(upgrade_identifier(&"a".repeat(64)).is_err());
    assert!(upgrade_identifier("dw_test_v1_upgrade_123").is_ok());
}

fn register_contract(
    e: EngagementId,
    c: duskweave::mission::CampaignId,
    op: OperationId,
) -> (duskweave::mission::MissionRegistered, Value) {
    let input = parse_register(&serde_json::to_vec(&reg_json(e, c)).unwrap()).unwrap();
    let (_mission, registered) =
        Mission::register(&input, op, EventId(Uuid::from_u128(0xbeef)), 1_700_000_000).unwrap();
    let contract = serde_json::to_value(&registered).unwrap();
    (registered, contract)
}

#[test]
fn upgrade_from_0001_0003_schema_preserves_v1_rows_and_admits_v2() {
    let _guard = db();
    let owned = OwnedUpgradeDatabase::create().unwrap();
    assert_eq!(
        OwnedUpgradeDatabase::create().err(),
        Some("upgrade_identity_collision")
    );
    let mut admin = admin_in(&owned.name);
    assert!(
        admin
            .query_one("SELECT to_regclass('mission.withdrawals') IS NULL", &[])
            .unwrap_or_else(|_| panic!("upgrade initial schema check failed"))
            .get::<_, bool>(0)
    );
    for migration in [MIGRATION, PLANNING_MIGRATION, HISTORY_MIGRATION] {
        admin
            .batch_execute(migration)
            .unwrap_or_else(|_| panic!("upgrade legacy migration failed"));
    }

    // Scope A: completed v1 producer + consumer rows on the pre-upgrade schema.
    let (e_a, c_a) = scope(0xb2f0);
    let reg_op_a = Uuid::from_u128(0xb2f1);
    let op_a = Uuid::from_u128(0xb2f2);
    let (registered_a, reg_contract_a) = register_contract(e_a, c_a, OperationId(reg_op_a));
    let assessed_a = v1_event_json(
        e_a,
        c_a,
        (op_a, Uuid::from_u128(0xb2f3), reg_op_a),
        (100, 200),
        150,
    );
    admin
        .execute(
            "INSERT INTO trajectory.registration_history \
             (engagement_id,campaign_id,producer,operation_id,event_id,status,contract,completed_at) \
             VALUES ($1,$2,'mission',$3,$4,'accepted',$5,transaction_timestamp())",
            &[&e_a.0, &c_a.0, &reg_op_a, &registered_a.event_id.0, &reg_contract_a],
        )
        .unwrap_or_else(|_| panic!("upgrade legacy insert failed"));
    admin
        .execute(
            "INSERT INTO mission.planning_assessments \
             (engagement_id,campaign_id,operation_id,event_id,contract,publication_obligation) \
             VALUES ($1,$2,$3,$4,$5,'trajectory.planning_history.v1')",
            &[&e_a.0, &c_a.0, &op_a, &Uuid::from_u128(0xb2f3), &assessed_a],
        )
        .unwrap_or_else(|_| panic!("upgrade legacy insert failed"));
    admin
        .execute(
            "INSERT INTO trajectory.planning_history \
             (engagement_id,campaign_id,operation_id,event_id,status,contract,completed_at) \
             VALUES ($1,$2,$3,$4,'accepted',$5,transaction_timestamp())",
            &[&e_a.0, &c_a.0, &op_a, &Uuid::from_u128(0xb2f3), &assessed_a],
        )
        .unwrap_or_else(|_| panic!("upgrade legacy insert failed"));

    // Scope B: durable v1 producer decision whose publication is still pending.
    let (e_b, c_b) = scope(0xb2f8);
    let reg_op_b = Uuid::from_u128(0xb2f9);
    let op_b = Uuid::from_u128(0xb2fa);
    let (registered_b, reg_contract_b) = register_contract(e_b, c_b, OperationId(reg_op_b));
    // Its recorded window predates its evaluated_at: v1 never checked bounds.
    let assessed_b = v1_event_json(
        e_b,
        c_b,
        (op_b, Uuid::from_u128(0xb2fb), reg_op_b),
        (1_700_000_000, 1_700_086_400),
        2_000_000_000,
    );
    admin
        .execute(
            "INSERT INTO trajectory.registration_history \
             (engagement_id,campaign_id,producer,operation_id,event_id,status,contract,completed_at) \
             VALUES ($1,$2,'mission',$3,$4,'accepted',$5,transaction_timestamp())",
            &[&e_b.0, &c_b.0, &reg_op_b, &registered_b.event_id.0, &reg_contract_b],
        )
        .unwrap_or_else(|_| panic!("upgrade legacy insert failed"));
    admin
        .execute(
            "INSERT INTO mission.planning_assessments \
             (engagement_id,campaign_id,operation_id,event_id,contract,publication_obligation) \
             VALUES ($1,$2,$3,$4,$5,'trajectory.planning_history.v1')",
            &[&e_b.0, &c_b.0, &op_b, &Uuid::from_u128(0xb2fb), &assessed_b],
        )
        .unwrap_or_else(|_| panic!("upgrade legacy insert failed"));

    // The upgrade is transactional and safe to reapply.
    admin
        .batch_execute(V2_MIGRATION)
        .unwrap_or_else(|_| panic!("upgrade 0004 failed"));
    admin
        .batch_execute(V2_MIGRATION)
        .unwrap_or_else(|_| panic!("upgrade 0004 failed"));
    let preserved: i64 = admin
        .query_one(
            "SELECT count(*) FROM mission.planning_assessments pa \
             WHERE (pa.engagement_id,pa.campaign_id) IN (($1,$2),($3,$4)) \
             AND pa.contract = ANY(ARRAY[$5::jsonb,$6::jsonb])",
            &[&e_a.0, &c_a.0, &e_b.0, &c_b.0, &assessed_a, &assessed_b],
        )
        .unwrap_or_else(|_| panic!("upgrade SQL read failed"))
        .get(0);
    assert_eq!(preserved, 2);
    let history_versions: Vec<i32> = admin
        .query(
            "SELECT version FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e_a.0, &c_a.0],
        )
        .unwrap_or_else(|_| panic!("upgrade SQL read failed"))
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(history_versions, vec![1]);

    for _ in 0..2 {
        admin
            .batch_execute(WITHDRAWAL_MIGRATION)
            .unwrap_or_else(|_| panic!("upgrade 0005 failed"));
    }
    let row = admin.query_one(
        "SELECT (SELECT count(*) FROM mission.withdrawals), \
         (SELECT count(*) FROM trajectory.withdrawal_history), \
         (SELECT count(*) FROM mission.planning_assessments WHERE contract=ANY(ARRAY[$1::jsonb,$2::jsonb])), \
         (SELECT count(*) FROM trajectory.planning_history WHERE version=1 AND contract=$1)",
        &[&assessed_a, &assessed_b])
        .unwrap_or_else(|_| panic!("upgrade 0005 preservation check failed"));
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3)
        ),
        (0, 0, 2, 1)
    );
    eprintln!(
        "upgrade_database={} stage=0005_preserved marker=0 history=0 legacy_producer=2 legacy_history=1",
        owned.name
    );

    // Post-upgrade recovery returns the original v1 record unchanged through
    // fresh ports, and a pending v1 publication completes under version 1.
    let mut store = PgMissionStore::new(runtime_in(&owned.name));
    let request_a: duskweave::planning::PlanningRequest =
        serde_json::from_value(assessed_a["request"].clone()).unwrap();
    let recovered = read_decision(&mut store, &request_a, OperationId(op_a))
        .unwrap()
        .unwrap();
    assert_eq!(recovered.version, 1);
    assert_eq!(
        serde_json::to_value(&recovered).unwrap(),
        assessed_a,
        "v1 contract, timestamps and identities are not rewritten"
    );
    let mut trajectory = PgTrajectory::new(runtime_in(&owned.name));
    assert_eq!(
        trajectory.inspect(&recovered).unwrap(),
        Delivered::Completed
    );
    let event_b: PlanningAssessed = serde_json::from_value(assessed_b.clone()).unwrap();
    assert_eq!(
        trajectory.inspect(&event_b).unwrap(),
        Delivered::Unresolved("not_recorded")
    );
    assert_eq!(trajectory.publish(&event_b).unwrap(), Delivered::Completed);
    let v: i32 = admin
        .query_one(
            "SELECT version FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e_b.0, &c_b.0],
        )
        .unwrap_or_else(|_| panic!("upgrade SQL read failed"))
        .get(0);
    assert_eq!(v, 1);

    // A new v2 assessment publishes on the upgraded schema as version 2.
    let (e_c, c_c) = scope(0xb2fc);
    let mut allocator = PgAllocator::new(runtime_in(&owned.name));
    let mut store_c = PgMissionStore::new(runtime_in(&owned.name));
    let mut traj_c = PgTrajectory::new(runtime_in(&owned.name));
    let op_c = registration::prepare_operation(&mut allocator).unwrap();
    registration::register(
        &mut allocator,
        &mut store_c,
        &mut traj_c,
        op_c,
        &reg_input(e_c, c_c),
    )
    .unwrap();
    let op_d = registration::prepare_operation(&mut allocator).unwrap();
    let request_c = {
        let mut r = serde_json::to_value(&assessed_b["request"]).unwrap();
        r["engagement_id"] = json!(e_c.0);
        r["campaign_id"] = json!(c_c.0);
        serde_json::from_value::<duskweave::planning::PlanningRequest>(r).unwrap()
    };
    let event_c = duskweave::planning_assessment::assess(
        &mut allocator,
        &mut store_c,
        &request_c,
        op_d,
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(event_c.version, 2);
    assert_eq!(traj_c.publish(&event_c).unwrap(), Delivered::Completed);
    let v2: i32 = admin
        .query_one(
            "SELECT version FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e_c.0, &c_c.0],
        )
        .unwrap_or_else(|_| panic!("upgrade SQL read failed"))
        .get(0);
    assert_eq!(v2, 2);
    drop((store, trajectory, allocator, store_c, traj_c, admin));
    owned.finish().unwrap();
}

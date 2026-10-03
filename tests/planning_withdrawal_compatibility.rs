//! Migration 0005 -> 0006 compatibility: exact preservation of seeded v1/v2
//! producer/history rows across a repeated 0006 application, then record,
//! publish and recover a v3 refusal through the restricted runtime login.

use duskweave::input::parse_register;
use duskweave::mission::*;
use duskweave::planning::PlanningRequest;
use duskweave::planning_assessment;
use duskweave::planning_history::PlanningHistoryPort;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::trajectory::Delivered;
use duskweave::withdrawal::*;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

#[path = "support/upgrade_db.rs"]
mod upgrade_support;
use upgrade_support::*;

const REFUSAL_MIGRATION: &str = include_str!("../migrations/0006_planning_withdrawal_refusal.sql");

fn upgrade_ports(owned: &OwnedUpgradeDatabase) -> (PgAllocator, PgMissionStore, PgTrajectory) {
    (
        PgAllocator::new(runtime_in(&owned.name)),
        PgMissionStore::new(runtime_in(&owned.name)),
        PgTrajectory::new(runtime_in(&owned.name)),
    )
}

fn v1_event_json(
    e: EngagementId,
    c: CampaignId,
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

fn register_contract(
    e: EngagementId,
    c: CampaignId,
    op: OperationId,
) -> (duskweave::mission::MissionRegistered, Value) {
    let input = parse_register(&serde_json::to_vec(&reg_json(e, c)).unwrap()).unwrap();
    let (_mission, registered) =
        Mission::register(&input, op, EventId(Uuid::from_u128(0xbeef)), 1_700_000_000).unwrap();
    let contract = serde_json::to_value(&registered).unwrap();
    (registered, contract)
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

#[test]
fn upgrade_0006_preserves_v1_v2_and_admits_v3_refusal() {
    let _guard = db();
    let owned = OwnedUpgradeDatabase::create().unwrap();
    let mut admin = admin_in(&owned.name);
    for migration in [
        MIGRATION,
        PLANNING_MIGRATION,
        HISTORY_MIGRATION,
        V2_MIGRATION,
        WITHDRAWAL_MIGRATION,
    ] {
        admin
            .batch_execute(migration)
            .unwrap_or_else(|_| panic!("upgrade 0001-0005 failed"));
    }

    // Seed a legacy v1 producer + history row.
    let (e_a, c_a) = scope(0xc30);
    let reg_op_a = Uuid::from_u128(0xc31);
    let op_a = Uuid::from_u128(0xc32);
    let (registered_a, reg_contract_a) = register_contract(e_a, c_a, OperationId(reg_op_a));
    let assessed_a = v1_event_json(
        e_a,
        c_a,
        (op_a, Uuid::from_u128(0xc33), reg_op_a),
        (100, 200),
        150,
    );
    admin
        .execute(
            "INSERT INTO trajectory.registration_history              (engagement_id,campaign_id,producer,operation_id,event_id,status,contract,completed_at)              VALUES ($1,$2,'mission',$3,$4,'accepted',$5,transaction_timestamp())",
            &[&e_a.0, &c_a.0, &reg_op_a, &registered_a.event_id.0, &reg_contract_a],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO mission.planning_assessments              (engagement_id,campaign_id,operation_id,event_id,contract,publication_obligation)              VALUES ($1,$2,$3,$4,$5,'trajectory.planning_history.v1')",
            &[&e_a.0, &c_a.0, &op_a, &Uuid::from_u128(0xc33), &assessed_a],
        )
        .unwrap();
    admin
        .execute(
            "INSERT INTO trajectory.planning_history              (engagement_id,campaign_id,operation_id,event_id,status,contract,completed_at)              VALUES ($1,$2,$3,$4,'accepted',$5,transaction_timestamp())",
            &[&e_a.0, &c_a.0, &op_a, &Uuid::from_u128(0xc33), &assessed_a],
        )
        .unwrap();

    // Seed a v2 producer + history row through the production ports.
    let (e_b, c_b) = scope(0xc40);
    let (mut alloc, mut store, mut traj) = upgrade_ports(&owned);
    let reg_op_b = registration::prepare_operation(&mut alloc).unwrap();
    registration::register(
        &mut alloc,
        &mut store,
        &mut traj,
        reg_op_b,
        &reg_input(e_b, c_b),
    )
    .unwrap();
    let op_b = registration::prepare_operation(&mut alloc).unwrap();
    let assessed_b = planning_assessment::assess(
        &mut alloc,
        &mut store,
        &planning(e_b, c_b, false),
        op_b,
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        PlanningHistoryPort::publish(&mut traj, &assessed_b).unwrap(),
        Delivered::Completed
    );
    drop((alloc, store, traj));

    // Snapshot identity/content before 0006.
    let before_v1: Value = admin
        .query_one(
            "SELECT contract FROM mission.planning_assessments WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e_a.0, &c_a.0],
        )
        .unwrap()
        .get(0);
    let before_hist_v1: Value = admin
        .query_one(
            "SELECT contract FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND version=1",
            &[&e_a.0, &c_a.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(before_v1, assessed_a);
    assert_eq!(before_hist_v1, assessed_a);

    // Apply 0006 twice; reapplying is safe and preserves all rows.
    admin.batch_execute(REFUSAL_MIGRATION).unwrap();
    admin.batch_execute(REFUSAL_MIGRATION).unwrap();
    let after_v1: Value = admin
        .query_one(
            "SELECT contract FROM mission.planning_assessments WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e_a.0, &c_a.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(after_v1, before_v1);
    let versions: Vec<i32> = admin
        .query(
            "SELECT version FROM trajectory.planning_history              WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e_a.0, &c_a.0],
        )
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(versions, vec![1]);
    let v2_version: i32 = admin
        .query_one(
            "SELECT version FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e_b.0, &c_b.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(v2_version, 2);

    // Record, publish and recover a v3 refusal using the restricted login.
    let (mut alloc, mut store, mut traj) = upgrade_ports(&owned);
    let (e_c, c_c) = scope(0xc50);
    let reg_op_c = registration::prepare_operation(&mut alloc).unwrap();
    registration::register(
        &mut alloc,
        &mut store,
        &mut traj,
        reg_op_c,
        &reg_input(e_c, c_c),
    )
    .unwrap();
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let withdrawn = store
        .withdraw(
            &WithdrawalRequest {
                engagement_id: e_c,
                campaign_id: c_c,
                operator_ref: OperatorRef(Uuid::from_u128(99)),
                expected_mission_revision: 1,
                reason: WithdrawalReason::OperatorRequested,
            },
            w_op,
            false,
            &mut alloc,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        WithdrawalHistoryPort::publish(&mut traj, &withdrawn).unwrap(),
        Delivered::Completed
    );
    let op_c = registration::prepare_operation(&mut alloc).unwrap();
    let refused = planning_assessment::assess(
        &mut alloc,
        &mut store,
        &planning(e_c, c_c, false),
        op_c,
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(refused.version, 3);
    assert_eq!(
        PlanningHistoryPort::publish(&mut traj, &refused).unwrap(),
        Delivered::Completed
    );
    let v3_version: i32 = admin
        .query_one(
            "SELECT version FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e_c.0, &c_c.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(v3_version, 3);
    let recovered = planning_assessment::assess(
        &mut alloc,
        &mut store,
        &planning(e_c, c_c, false),
        op_c,
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered, refused);
    drop((alloc, store, traj, admin));
    owned.finish().unwrap();
}

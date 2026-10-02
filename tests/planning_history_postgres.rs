use duskweave::Fail;
use duskweave::mission::{EventId, OperationId};
use duskweave::planning_history::{PlanningHistoryPort, read_decision};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::TrajectoryPort;
use duskweave::trajectory::Delivered;
use serde_json::Value;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use db_support::*;
use history_support::*;

#[test]
fn durable_publication_and_fresh_read_keep_original_identity_and_timestamps() {
    let _guard = db();
    for registered in [false, true] {
        let ev = decision(0xb1b1 + u128::from(registered), registered);
        let mut trajectory = PgTrajectory::new(runtime_client());
        assert_eq!(
            trajectory.inspect(&ev).unwrap(),
            Delivered::Unresolved("not_recorded")
        );
        effects(&ev, 0, 0);
        assert_eq!(
            retry(|| trajectory.publish(&ev)).unwrap(),
            Delivered::Completed
        );
        assert_eq!(trajectory.publish(&ev).unwrap(), Delivered::Duplicate);
        let mut fresh = PgTrajectory::new(runtime_client());
        assert_eq!(fresh.inspect(&ev).unwrap(), Delivered::Completed);
        assert_eq!(
            read_decision(
                &mut PgMissionStore::new(runtime_client()),
                &ev.request,
                ev.operation_id
            )
            .unwrap(),
            Some(ev.clone())
        );
        effects(&ev, 1, 0);
        let row = runtime_client().query_one(
            "SELECT contract, obligation, producer, kind, version, recorded_at=completed_at \
             FROM trajectory.planning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&ev.engagement_id.0, &ev.campaign_id.0],
        ).unwrap();
        assert_eq!(row.get::<_, Value>(0), serde_json::to_value(&ev).unwrap());
        assert_eq!(row.get::<_, String>(1), "trajectory.planning_history.v1");
        assert_eq!(row.get::<_, String>(2), "mission");
        assert_eq!(row.get::<_, String>(3), "planning_assessed");
        assert_eq!(row.get::<_, i32>(4), 1);
        assert!(row.get::<_, bool>(5));
    }
}

#[test]
fn both_identity_collisions_are_monotonic_safe_anomalies() {
    let _guard = db();
    for slot in 0xb1b3..0xb1b6 {
        let ev = decision(slot, false);
        let mut trajectory = PgTrajectory::new(runtime_client());
        trajectory.publish(&ev).unwrap();
        let mut changed = ev.clone();
        match slot {
            0xb1b3 => changed.request.purpose_ref.0 = Uuid::from_u128(123),
            0xb1b4 => changed.event_id = EventId(Uuid::from_u128(124)),
            _ => changed.operation_id = OperationId(Uuid::from_u128(125)),
        }
        changed.affected_entity = changed.operation_id;
        changed.causation_id = changed.operation_id;
        changed.correlation_id = changed.operation_id;
        changed.validate().unwrap();
        // Read-only inspection detects the conflict without creating a marker.
        assert_eq!(trajectory.inspect(&changed).unwrap(), Delivered::Anomaly);
        effects(&ev, 1, 0);
        for _ in 0..2 {
            assert_eq!(trajectory.publish(&changed).unwrap(), Delivered::Anomaly);
        }
        assert_eq!(trajectory.inspect(&ev).unwrap(), Delivered::Anomaly);
        assert_eq!(trajectory.publish(&ev).unwrap(), Delivered::Anomaly);
        // Markers record the exact conflicting pair: the changed variant's
        // pair plus the original pair once republication is attempted under
        // anomaly dominance. Pairs that coincide collapse to one row.
        let mut expected = vec![
            (changed.event_id.0, changed.operation_id.0),
            (ev.event_id.0, ev.operation_id.0),
        ];
        expected.sort();
        expected.dedup();
        effects(&ev, 1, expected.len() as i64);
        let mut pairs: Vec<(Uuid, Uuid)> = runtime_client()
            .query(
                "SELECT event_id, operation_id FROM trajectory.planning_history \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='anomaly' \
                 AND contract IS NULL AND completed_at IS NULL \
                 AND anomaly_category='conflicting_identity'",
                &[&ev.engagement_id.0, &ev.campaign_id.0],
            )
            .unwrap()
            .iter()
            .map(|r| (r.get(0), r.get(1)))
            .collect();
        pairs.sort();
        pairs.dedup();
        assert_eq!(pairs, expected);
    }
}

#[test]
fn predecessor_is_scoped_and_delivery_can_resume_without_reassessment() {
    let _guard = db();
    let ev = decision(0xb1b6, true);
    let mut administrator = admin();
    let row = administrator
        .query_one(
            "DELETE FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2 \
         RETURNING contract",
            &[&ev.engagement_id.0, &ev.campaign_id.0],
        )
        .unwrap();
    let registered: duskweave::mission::MissionRegistered =
        serde_json::from_value(row.get(0)).unwrap();
    let mut other = registered.clone();
    other.campaign_id.0 = Uuid::from_u128(987);
    other.affected_entity = other.campaign_id;
    let mut trajectory = PgTrajectory::new(runtime_client());
    trajectory.deliver(&other).unwrap();
    assert_eq!(
        trajectory.publish(&ev).unwrap(),
        Delivered::Unresolved("missing_predecessor")
    );
    effects(&ev, 0, 0);
    trajectory.deliver(&registered).unwrap();
    assert_eq!(
        retry(|| trajectory.publish(&ev)).unwrap(),
        Delivered::Completed
    );
    // A historical recovery must not query current owner state or recheck predecessors.
    administrator
        .execute(
            "DELETE FROM mission.missions WHERE engagement_id=$1 AND campaign_id=$2",
            &[&ev.engagement_id.0, &ev.campaign_id.0],
        )
        .unwrap();
    administrator
        .execute(
            "DELETE FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2",
            &[&ev.engagement_id.0, &ev.campaign_id.0],
        )
        .unwrap();
    assert_eq!(
        read_decision(
            &mut PgMissionStore::new(runtime_client()),
            &ev.request,
            ev.operation_id
        )
        .unwrap(),
        Some(ev.clone())
    );
    assert_eq!(trajectory.inspect(&ev).unwrap(), Delivered::Completed);
    assert_eq!(trajectory.publish(&ev).unwrap(), Delivered::Duplicate);
    effects(&ev, 1, 0);
}

#[test]
fn invalid_predecessor_and_unsupported_planning_contract_never_complete() {
    let _guard = db();
    let ev = decision(0xb1b7, true);
    let mut trajectory = PgTrajectory::new(runtime_client());
    let mut unsupported = ev.clone();
    unsupported.version = 2;
    assert_eq!(
        trajectory.publish(&unsupported),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    unsupported = ev.clone();
    unsupported.producer = "other".into();
    assert_eq!(
        trajectory.inspect(&unsupported),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    unsupported = ev.clone();
    unsupported.causation_id.0 = Uuid::from_u128(126);
    assert_eq!(
        trajectory.publish(&unsupported),
        Err(Fail::Unresolved("scope_violation"))
    );
    admin().execute(
        "UPDATE trajectory.registration_history SET contract=jsonb_set(contract,'{fields,starts_at}','42') \
         WHERE engagement_id=$1 AND campaign_id=$2", &[&ev.engagement_id.0, &ev.campaign_id.0],
    ).unwrap();
    assert_eq!(
        trajectory.publish(&ev).unwrap(),
        Delivered::Unresolved("unsupported_predecessor")
    );
    effects(&ev, 0, 0);
}

#[test]
fn cross_campaign_event_and_operation_reuse_is_isolated_and_decode_is_not_absence() {
    let _guard = db();
    let original = decision(0xb1b8, false);
    let mut trajectory = PgTrajectory::new(runtime_client());
    trajectory.publish(&original).unwrap();
    let mut other = original.clone();
    other.campaign_id.0 = Uuid::from_u128(777);
    other.request.campaign_id = other.campaign_id;
    assert_eq!(
        trajectory.inspect(&other).unwrap(),
        Delivered::Unresolved("not_recorded")
    );
    assert_eq!(trajectory.publish(&other).unwrap(), Delivered::Completed);
    assert_eq!(trajectory.inspect(&original).unwrap(), Delivered::Completed);
    admin().execute(
        "UPDATE trajectory.planning_history SET contract='{}' WHERE engagement_id=$1 AND campaign_id=$2",
        &[&original.engagement_id.0, &original.campaign_id.0],
    ).unwrap();
    assert_eq!(
        trajectory.inspect(&original),
        Err(Fail::Store("contract_decode"))
    );
    effects(&original, 1, 0);
}

#[test]
fn consumer_roles_do_not_need_mission_permissions_and_read_only_recovery_can_inspect() {
    let _guard = db();
    let ev = decision(0xb1bb, true);
    let role = format!("dw_ph_{}", ev.engagement_id.0.simple());
    let mut config = dsn("DW_TEST_DATABASE_URL");
    let password = std::str::from_utf8(config.get_password().unwrap())
        .unwrap()
        .replace('\'', "''");
    let mut administrator = admin();
    administrator
        .batch_execute(&format!(
            "CREATE ROLE {role} LOGIN PASSWORD '{password}'; \
         GRANT USAGE ON SCHEMA trajectory TO {role}; \
         GRANT SELECT ON trajectory.registration_history,trajectory.planning_history TO {role};"
        ))
        .unwrap();
    config.user(&role);
    let mut restricted = config.connect(postgres::NoTls).unwrap();
    assert!(
        restricted
            .query("SELECT * FROM mission.missions", &[])
            .is_err()
    );
    let mut consumer = PgTrajectory::new(restricted);
    assert_eq!(
        consumer.inspect(&ev).unwrap(),
        Delivered::Unresolved("not_recorded")
    );
    assert_eq!(
        consumer.publish(&ev),
        Err(Fail::Config("unsafe_history_role"))
    );
    effects(&ev, 0, 0);
    administrator
        .batch_execute(&format!(
            "GRANT INSERT ON trajectory.planning_history TO {role}"
        ))
        .unwrap();
    assert_eq!(consumer.publish(&ev).unwrap(), Delivered::Completed);
    administrator
        .batch_execute(&format!(
            "REVOKE INSERT ON trajectory.planning_history FROM {role}"
        ))
        .unwrap();
    let mut read_only = PgTrajectory::new(config.connect(postgres::NoTls).unwrap());
    assert_eq!(read_only.inspect(&ev).unwrap(), Delivered::Completed);
    for privilege in ["UPDATE", "DELETE", "TRUNCATE"] {
        administrator
            .batch_execute(&format!(
                "GRANT {privilege} ON trajectory.planning_history TO {role}"
            ))
            .unwrap();
        assert_eq!(
            read_only.inspect(&ev),
            Err(Fail::Config("unsafe_history_role"))
        );
        administrator
            .batch_execute(&format!(
                "REVOKE {privilege} ON trajectory.planning_history FROM {role}"
            ))
            .unwrap();
    }
    administrator
        .batch_execute(&format!("GRANT CREATE ON SCHEMA trajectory TO {role}"))
        .unwrap();
    assert_eq!(
        read_only.inspect(&ev),
        Err(Fail::Config("unsafe_history_role"))
    );
    administrator
        .batch_execute(&format!("REVOKE CREATE ON SCHEMA trajectory FROM {role}"))
        .unwrap();
    let mut privileged = PgTrajectory::new(admin());
    assert_eq!(
        privileged.inspect(&ev),
        Err(Fail::Config("unsafe_history_role"))
    );
    effects(&ev, 1, 0);
    drop(consumer);
    drop(read_only);
    administrator
        .batch_execute(&format!("DROP OWNED BY {role}; DROP ROLE {role}"))
        .unwrap();
}

use duskweave::mission::*;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::{self, OperationAllocator};
use duskweave::trajectory::Delivered;
use duskweave::withdrawal::*;
use duskweave::{Fail, Res};
use postgres::{Client, NoTls};
use serde_json::{Value, json};
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
        reason: WithdrawalReason::AuthorizationEnded,
    }
}

fn admin() -> Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    config.connect(NoTls).unwrap()
}

fn event(n: u128) -> MissionAuthorityWithdrawn {
    let (e, c) = scope(n);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let op = registration::prepare_operation(&mut alloc).unwrap();
    store
        .withdraw(&request(e, c), op, false, &mut alloc)
        .unwrap()
        .unwrap()
}

#[test]
fn accepted_effect_completion_dedup_and_conflicting_identity_are_visible() {
    let _guard = db();
    let event = event(1);
    let (e, c) = (event.request.engagement_id, event.request.campaign_id);
    let mut consumer = PgTrajectory::new(runtime_client());
    assert_eq!(
        consumer.inspect(&event).unwrap(),
        Delivered::Unresolved("not_recorded")
    );
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    assert_eq!(consumer.publish(&event).unwrap(), Delivered::Completed);
    // Full accepted-row snapshot across duplicate publication and later
    // anomaly/recovery: each read is a fresh connection, so post-anomaly
    // equality also proves what fresh recovery sees is byte-identical.
    let accepted_row = || {
        runtime_client()
            .query_one(
                "SELECT to_jsonb(h) FROM trajectory.withdrawal_history h \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
                &[&e.0, &c.0],
            )
            .unwrap()
            .get::<_, Value>(0)
    };
    let before = accepted_row();
    assert_eq!(consumer.publish(&event).unwrap(), Delivered::Duplicate);
    assert_eq!(accepted_row(), before);
    let row = runtime_client().query_one(
        "SELECT contract,completed_at IS NOT NULL,obligation FROM trajectory.withdrawal_history \
         WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'", &[&e.0, &c.0]).unwrap();
    assert_eq!(row.get::<_, Value>(0), json!(event));
    assert!(row.get::<_, bool>(1));
    assert_eq!(row.get::<_, &str>(2), OBLIGATION);
    for changed in [
        {
            let mut v = event.clone();
            v.request.reason = WithdrawalReason::ScopeConcern;
            v
        },
        {
            let mut v = event.clone();
            v.event_id = EventId(Uuid::from_u128(50));
            v
        },
        {
            let mut v = event.clone();
            v.operation_id = OperationId(Uuid::from_u128(51));
            v.causation_id = v.operation_id;
            v.correlation_id = v.operation_id;
            v
        },
    ] {
        let before = count("trajectory.withdrawal_history", e, c);
        assert_eq!(consumer.inspect(&changed).unwrap(), Delivered::Anomaly);
        assert_eq!(count("trajectory.withdrawal_history", e, c), before);
        assert_eq!(consumer.publish(&changed).unwrap(), Delivered::Anomaly);
    }
    assert_eq!(consumer.inspect(&event).unwrap(), Delivered::Anomaly);
    assert_eq!(
        count_where(
            "trajectory.withdrawal_history",
            "AND status='accepted'",
            e,
            c
        ),
        1
    );
    assert_eq!(
        count_where(
            "trajectory.withdrawal_history",
            "AND status='anomaly'",
            e,
            c
        ),
        3
    );
    assert_eq!(accepted_row(), before);
}

#[test]
fn missing_corrupt_unsupported_and_anomalous_predecessors_never_complete() {
    let _guard = db();
    let mut admin = admin();
    let mut consumer = PgTrajectory::new(runtime_client());
    let (e, c) = scope(2);
    let missing = MissionAuthorityWithdrawn::new(
        request(e, c),
        OperationId(Uuid::from_u128(52)),
        OperationId(Uuid::from_u128(53)),
        EventId(Uuid::from_u128(54)),
        1,
    )
    .unwrap();
    assert_eq!(
        consumer.publish(&missing).unwrap(),
        Delivered::Unresolved("missing_predecessor")
    );
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    let event = event(3);
    let (e, c) = (event.request.engagement_id, event.request.campaign_id);
    let original: Value = runtime_client().query_one(
        "SELECT contract FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0]).unwrap().get(0);
    for (key, value) in [
        ("version", json!(99)),
        ("event_id", json!(Uuid::nil())),
        ("operation_id", json!(Uuid::nil())),
        ("kind", json!("wrong")),
        ("owner_revision", json!(2)),
        ("campaign_id", json!(Uuid::from_u128(55))),
        ("causation_id", json!(Uuid::nil())),
        ("producer", json!("other")),
    ] {
        let mut changed = original.clone();
        changed[key] = value;
        admin.execute("UPDATE trajectory.registration_history SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &changed]).unwrap();
        assert_eq!(
            consumer.publish(&event).unwrap(),
            Delivered::Unresolved("unsupported_predecessor"),
            "accepted {key}"
        );
        assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
    }
    admin.execute("UPDATE trajectory.registration_history SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0, &json!({})]).unwrap();
    assert_eq!(
        consumer.publish(&event),
        Err(Fail::Store("contract_decode"))
    );
    admin.execute("UPDATE trajectory.registration_history SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0, &original]).unwrap();
    for assignment in [
        "producer='other'",
        "obligation='wrong'",
        "completed_at=NULL",
    ] {
        admin.execute(&format!("UPDATE trajectory.registration_history SET {assignment} WHERE engagement_id=$1 AND campaign_id=$2"), &[&e.0, &c.0]).unwrap();
        assert_eq!(
            consumer.publish(&event).unwrap(),
            Delivered::Unresolved("unsupported_predecessor")
        );
        admin.execute("UPDATE trajectory.registration_history SET producer='mission',obligation='trajectory.registration_history.v1',completed_at=transaction_timestamp() WHERE engagement_id=$1 AND campaign_id=$2", &[&e.0, &c.0]).unwrap();
    }
    admin.execute("INSERT INTO trajectory.registration_history (engagement_id,campaign_id,event_id,operation_id,producer,status,anomaly_category) \
        VALUES ($1,$2,$3,$4,'mission','anomaly','conflicting_identity')",
        &[&e.0, &c.0, &Uuid::parse_str(original["event_id"].as_str().unwrap()).unwrap(), &Uuid::from_u128(90)]).unwrap();
    assert_eq!(
        consumer.publish(&event).unwrap(),
        Delivered::Unresolved("unsupported_predecessor")
    );
    assert_eq!(count("trajectory.withdrawal_history", e, c), 0);
}

#[test]
fn invalid_incoming_and_corrupt_journal_are_not_success_or_absence() {
    let _guard = db();
    let event = event(4);
    let mut consumer = PgTrajectory::new(runtime_client());
    let mut unsupported = event.clone();
    unsupported.version = 99;
    assert_eq!(
        consumer.publish(&unsupported),
        Err(Fail::State("unsupported_contract"))
    );
    assert_eq!(
        consumer.inspect(&unsupported),
        Err(Fail::State("unsupported_contract"))
    );
    consumer.publish(&event).unwrap();
    let mut admin = admin();
    let (e, c) = (event.request.engagement_id, event.request.campaign_id);
    for raw in [json!({}), {
        let mut v = json!(event);
        v["version"] = json!(99);
        v
    }] {
        admin.execute("UPDATE trajectory.withdrawal_history SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'", &[&e.0, &c.0, &raw]).unwrap();
        assert_eq!(
            consumer.inspect(&event),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(
            consumer.publish(&event),
            Err(Fail::Store("contract_decode"))
        );
    }
    admin.execute("UPDATE trajectory.withdrawal_history SET contract=$3,operation_id=$4 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
        &[&e.0, &c.0, &json!(event), &Uuid::from_u128(56)]).unwrap();
    assert_eq!(
        consumer.inspect(&event),
        Err(Fail::Store("contract_decode"))
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
}

#[test]
fn actual_new_table_queries_require_append_only_runtime_permissions() {
    let _guard = db();
    let event = event(5);
    let mut admin = admin();
    let mut runtime = runtime_client();
    let safe: bool = runtime.query_one(
        "SELECT NOT rolsuper AND NOT rolbypassrls AND NOT rolcreaterole AND NOT rolcreatedb FROM pg_roles WHERE rolname=current_user", &[]).unwrap().get(0);
    assert!(safe);
    for table in ["mission.withdrawals", "trajectory.withdrawal_history"] {
        let privileges: bool = runtime.query_one(
            "SELECT NOT has_table_privilege(current_user,$1,'UPDATE,DELETE,TRUNCATE') \
             AND NOT pg_has_role(current_user,(SELECT relowner FROM pg_class WHERE oid=$1::regclass),'USAGE')",
            &[&table]).unwrap().get(0);
        assert!(privileges);
        for sql in [
            format!("UPDATE {table} SET event_id=event_id"),
            format!("DELETE FROM {table}"),
            format!("TRUNCATE {table}"),
            format!("ALTER TABLE {table} OWNER TO CURRENT_USER"),
        ] {
            assert!(runtime.batch_execute(&sql).is_err());
        }
    }
    assert!(
        runtime
            .batch_execute("CREATE TABLE mission.c1a_unauthorized(i integer)")
            .is_err()
    );
    assert!(
        runtime
            .batch_execute("CREATE TABLE trajectory.c1a_unauthorized(i integer)")
            .is_err()
    );
    let mut consumer = PgTrajectory::new(runtime_client());
    for privilege in ["SELECT", "INSERT"] {
        admin
            .batch_execute(&format!(
                "REVOKE {privilege} ON trajectory.withdrawal_history FROM dw_runtime"
            ))
            .unwrap();
        let publication = consumer.publish(&event);
        let inspection = consumer.inspect(&event);
        admin
            .batch_execute(&format!(
                "GRANT {privilege} ON trajectory.withdrawal_history TO dw_runtime"
            ))
            .unwrap();
        assert!(publication.is_err());
        if privilege == "SELECT" {
            assert!(inspection.is_err());
        } else {
            assert_eq!(inspection.unwrap(), Delivered::Unresolved("not_recorded"));
        }
    }
    let (e, c) = scope(6);
    let (mut alloc, mut store, mut trajectory) = ports();
    accepted(&mut alloc, &mut store, &mut trajectory, e, c);
    let operation = registration::prepare_operation(&mut alloc).unwrap();
    for privilege in ["SELECT", "INSERT"] {
        admin
            .batch_execute(&format!(
                "REVOKE {privilege} ON mission.withdrawals FROM dw_runtime"
            ))
            .unwrap();
        let result = store.withdraw(&request(e, c), operation, false, &mut alloc);
        admin
            .batch_execute(&format!(
                "GRANT {privilege} ON mission.withdrawals TO dw_runtime"
            ))
            .unwrap();
        assert_eq!(result, Err(Fail::Store("storage_error")));
        assert_eq!(count("mission.withdrawals", e, c), 0);
    }
    assert_eq!(
        count(
            "trajectory.withdrawal_history",
            event.request.engagement_id,
            event.request.campaign_id
        ),
        0
    );
    struct Never;
    impl OperationAllocator for Never {
        fn allocate(&mut self) -> Res<Uuid> {
            panic!("no allocation")
        }
    }
    admin
        .batch_execute("REVOKE SELECT ON mission.withdrawals FROM dw_runtime")
        .unwrap();
    let result = PgMissionStore::new(runtime_client()).withdraw(
        &event.request,
        event.operation_id,
        true,
        &mut Never,
    );
    admin
        .batch_execute("GRANT SELECT ON mission.withdrawals TO dw_runtime")
        .unwrap();
    assert_eq!(result, Err(Fail::Store("storage_error")));
}

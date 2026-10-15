#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_native_inference.rs"]
mod native;
use serde_json::{Value, json};
use uuid::Uuid;

fn decision() -> Value {
    json!({"kind":"reasoning_decision","occurred_at_ms":1,"recorded_at_ms":2,
        "source_mode":"blind","provenance":null,"event":Uuid::from_u128(1),
        "engagement":Uuid::from_u128(2),"campaign":Uuid::from_u128(3),"decision":Uuid::from_u128(4),
        "producer":"cognition","version":1,"mission_revision":1,"permission":Uuid::from_u128(5),
        "call_ids":[],"bindings":[],"proposal":null,"disposition":"reference_ineligible",
        "publication_obligation":"trajectory.reasoning_history.v1"})
}

#[test]
fn direct_cognition_publication_refuses_unknown_metadata_atomically() {
    let _guard = db_support::db();
    native::setup();
    let mut c = native::client("DW_COGNITION_DATABASE_URL");
    let event: Uuid = c.query_one("SELECT gen_random_uuid()", &[]).unwrap().get(0);
    let mut x = decision();
    x["event"] = json!(event);
    x["extra_metadata"] = json!(true);
    let result = c.execute(
        "SELECT cognition.publish_reasoning($1,$2)",
        &[&x, &"fixture-input"],
    );
    assert_eq!(
        result.unwrap_err().as_db_error().unwrap().message(),
        "inference_source_refused",
        "unknown metadata must be rejected before identity handling"
    );
    let mut a = native::admin();
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM cognition.reasoning_decisions WHERE event=$1",
            &[&event]
        )
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM cognition.reasoning_outbox WHERE event=$1",
            &[&event]
        )
        .unwrap()
        .get::<_, i64>(0),
        0
    );
}

#[test]
fn direct_history_insert_cannot_forge_accepted_source() {
    let _guard = db_support::db();
    native::setup();
    let mut h = native::client("DW_TRAJECTORY_DATABASE_URL");
    let mut tx = h.transaction().unwrap();
    let result = tx.execute("INSERT INTO trajectory.reasoning_history(engagement_id,campaign_id,operation_id,event_id,status,contract,completed_at) VALUES($1,$2,$3,$4,'accepted',$5,clock_timestamp())",&[&Uuid::from_u128(102),&Uuid::from_u128(103),&Uuid::from_u128(104),&Uuid::from_u128(101),&decision()]);
    tx.rollback().unwrap();
    assert!(
        result.is_err(),
        "direct SQL must not create completed history without committed source"
    );
}

#[test]
fn history_owner_is_narrow_and_cannot_publish_or_mutate_mission() {
    let _guard = db_support::db();
    native::setup();
    let mut h = native::client("DW_TRAJECTORY_DATABASE_URL");
    let row = h.query_one("SELECT pg_has_role(session_user,'dw_m1_trajectory','MEMBER'),pg_has_role(session_user,'dw_runtime','MEMBER'),has_table_privilege(current_user,'trajectory.reasoning_history','INSERT'),has_table_privilege(current_user,(SELECT t.oid FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname='mission' AND t.relname='missions'),'INSERT')",&[]).unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(!row.get::<_, bool>(1));
    assert!(row.get::<_, bool>(2));
    assert!(!row.get::<_, bool>(3));
    assert!(
        h.execute(
            "SELECT cognition.publish_reasoning($1,$2)",
            &[&decision(), &"fixture-input"]
        )
        .is_err()
    );
    assert!(h.batch_execute("SET ROLE dw_runtime").is_err());
    assert!(
        h.batch_execute("UPDATE mission.missions SET owner_revision=owner_revision")
            .is_err()
    );
    assert!(
        h.batch_execute("DELETE FROM trajectory.reasoning_history")
            .is_err()
    );
    assert!(
        h.batch_execute("TRUNCATE cognition.reasoning_outbox")
            .is_err()
    );
}

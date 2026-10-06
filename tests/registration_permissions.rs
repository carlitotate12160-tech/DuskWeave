//! Privilege and durability guardrails against real PostgreSQL: the runtime
//! role is denied mutation/DDL, durability settings stay on, and
//! qualify_runtime rejects unqualified connections before business use.

use duskweave::Fail;
use duskweave::postgres_mission::qualify_runtime;
use postgres::NoTls;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_permission.rs"]
mod policy;

#[test]
fn runtime_role_denied_mutation_and_ddl() {
    let _g = db();
    let (e, c) = scope(0x8100);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let mut rt = runtime_client();
    for sql in [
        "UPDATE trajectory.registration_history SET status='anomaly'",
        "DELETE FROM trajectory.registration_history",
        "TRUNCATE trajectory.registration_history",
        "UPDATE mission.missions SET revision=9",
        "DELETE FROM mission.missions",
        "UPDATE mission.registration_outbox SET contract='{}'",
        "UPDATE mission.planning_assessments SET contract='{}'",
        "DELETE FROM mission.planning_assessments",
        "TRUNCATE mission.planning_assessments",
        "UPDATE trajectory.planning_history SET status='anomaly'",
        "DELETE FROM trajectory.planning_history",
        "TRUNCATE trajectory.planning_history",
        "CREATE TABLE mission.evil(id int)",
        "CREATE TABLE trajectory.evil(id int)",
    ] {
        assert!(
            rt.batch_execute(sql).is_err(),
            "runtime role must be denied: {sql}"
        );
    }
    for privilege in ["SELECT", "INSERT"] {
        let allowed: bool = rt
            .query_one(
                "SELECT has_table_privilege(current_user, 'mission.planning_assessments', $1)",
                &[&privilege],
            )
            .unwrap()
            .get(0);
        assert!(
            allowed,
            "runtime requires {privilege} on assessment publication"
        );
    }
    // Durability settings must not be weakened for this slice.
    for (guc, want) in [
        ("fsync", "on"),
        ("full_page_writes", "on"),
        ("synchronous_commit", "on"),
    ] {
        let got: String = rt.query_one(&format!("SHOW {guc}"), &[]).unwrap().get(0);
        assert_eq!(got, want, "{guc} must stay {want}");
    }
}

#[test]
fn qualify_runtime_accepts_restricted_rejects_privileged() {
    let _g = db();
    // The restricted runtime role qualifies.
    qualify_runtime(&mut runtime_client()).expect("runtime role must qualify");
    // The admin connection is a superuser: must be rejected before use.
    let mut admin = admin_client();
    assert_eq!(
        qualify_runtime(&mut admin),
        Err(Fail::Config("unqualified_runtime"))
    );
}

#[test]
fn qualify_runtime_rejects_ddl_capable_role() {
    let _g = db();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let role = format!("dw_q_{:x}", nanos & 0xFFFF_FFFF);
    assert!(role.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    // Grants target the isolated test database, not the admin maintenance db.
    let mut admin_cfg = dsn("DW_TEST_ADMIN_DATABASE_URL");
    admin_cfg.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = admin_cfg.connect(NoTls).unwrap();
    admin
        .batch_execute(&format!(
            "DROP ROLE IF EXISTS {role}; \
             CREATE ROLE {role} LOGIN PASSWORD 'qualify_probe'; \
             GRANT USAGE ON SCHEMA mission TO {role}; \
             GRANT CREATE ON SCHEMA mission TO {role};"
        ))
        .unwrap();
    let mut cfg = dsn("DW_TEST_DATABASE_URL");
    cfg.user(&role).password("qualify_probe");
    let mut probe = cfg.connect(NoTls).expect("probe connect failed");
    assert_eq!(
        qualify_runtime(&mut probe),
        Err(Fail::Config("unqualified_runtime"))
    );
    drop(probe);
    admin
        .batch_execute(&format!(
            "REVOKE ALL ON SCHEMA mission FROM {role}; \
             REVOKE ALL ON SCHEMA trajectory FROM {role}; \
             DROP ROLE {role};"
        ))
        .unwrap();
}

#[test]
fn restricted_role_cannot_mutate_or_replace_canonical_m1_contracts() {
    use duskweave::registration::{self, MissionStore};
    let _g = db();
    let (e, c) = scope(0x8200);
    let (mut a, mut s, mut t) = ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut s, &mut t, op, &policy::input(e, c)).unwrap();
    let original = s.outbox_event(e, c, op).unwrap().unwrap();
    let canonical = serde_json::to_value(&original).unwrap();
    let mut rt = runtime_client();
    qualify_runtime(&mut rt).unwrap();
    for table in [
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        for sql in [
            format!("UPDATE {table} SET contract='{{}}' WHERE engagement_id=$1 AND campaign_id=$2"),
            format!("DELETE FROM {table} WHERE engagement_id=$1 AND campaign_id=$2"),
        ] {
            assert!(rt.execute(&sql, &[&e.0, &c.0]).is_err());
        }
        assert!(rt.batch_execute(&format!("TRUNCATE {table}")).is_err());
        let stored: serde_json::Value = rt
            .query_one(
                &format!("SELECT contract FROM {table} WHERE engagement_id=$1 AND campaign_id=$2"),
                &[&e.0, &c.0],
            )
            .unwrap()
            .get(0);
        assert_eq!(stored, canonical);
        assert_eq!(count(table, e, c), 1);
    }
    assert!(rt.execute(
        "INSERT INTO mission.registration_outbox (engagement_id,campaign_id,operation_id,event_id,contract) \
         VALUES ($1,$2,$3,$4,$5) ON CONFLICT (engagement_id,campaign_id,operation_id) DO UPDATE SET contract=EXCLUDED.contract",
        &[&e.0, &c.0, &op.0, &original.event_id.0, &serde_json::json!({})],
    ).is_err());
    assert_eq!(s.outbox_event(e, c, op).unwrap().unwrap(), original);
}

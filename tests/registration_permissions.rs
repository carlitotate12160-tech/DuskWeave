//! Privilege and durability guardrails against real PostgreSQL: the runtime
//! role is denied mutation/DDL, durability settings stay on, and
//! qualify_runtime rejects unqualified connections before business use.

use duskweave::Fail;
use duskweave::postgres_mission::qualify_runtime;
use postgres::NoTls;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

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
        "CREATE TABLE mission.evil(id int)",
        "CREATE TABLE trajectory.evil(id int)",
    ] {
        assert!(
            rt.batch_execute(sql).is_err(),
            "runtime role must be denied: {sql}"
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
    let mut admin = admin_client();
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

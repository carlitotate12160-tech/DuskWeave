//! Role-boundary coverage for the pre-send inference reservation slice on the
//! owned disposable database: authenticated-login and Broker-membership
//! checks inside the guarded function, SET ROLE/GUC impersonation denial,
//! direct DML/RLS/TRUNCATE protection under ordinary and temporarily widened
//! grants, and existing runtime qualification on unsafe privilege shapes.
//! Admin access is fixture setup only; runtime assertions use restricted
//! logins. Cluster-role fixtures serialize under db().
//!
//! Requires the packet's authorized disposable PostgreSQL service.

use duskweave::postgres_mission::qualify_runtime;
use uuid::Uuid;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;
use session_support::*;

fn errcode(e: &postgres::Error) -> Option<String> {
    e.code().map(|s| s.code().to_string())
}

/// RAII privilege/ownership restoration: the undo statement also runs while
/// a failed assertion unwinds, so widened fixture state never leaks.
struct Restore(postgres::Client, String);
impl Restore {
    fn install(sql: &str, undo: &str) -> Self {
        let mut admin = session_support::admin_db_client();
        admin.batch_execute(sql).unwrap();
        Self(admin, undo.into())
    }
}
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = self.0.batch_execute(&self.1);
    }
}

#[test]
fn authenticated_login_and_broker_role_cannot_be_impersonated() {
    let _g = db();
    let (q, source) = budget::fixture(0xf001);
    let request = serde_json::to_value(&q).unwrap();
    let source_v = serde_json::to_value(&source).unwrap();
    // The guarded function rejects the runtime login outright: it is not a
    // member of the Broker group.
    let unqualified = runtime_client()
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation($1,$2,$3,$4))",
            &[&"reserve", &Uuid::from_u128(0xf011), &request, &source_v],
        )
        .expect_err("runtime login must be unqualified");
    assert_eq!(errcode(&unqualified), Some("42501".into()));
    // A second Broker member holds its own writer identity.
    let mut other = other_broker_client();
    let record = budget::sql(&mut other, "reserve", &request, budget::op(), &source_v)
        .unwrap()
        .unwrap();
    let other_oid: u32 = runtime_client()
        .query_one(
            "SELECT oid FROM pg_roles WHERE rolname=$1",
            &[&OTHER_BROKER],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        record["writer_oid"].as_str().unwrap(),
        other_oid.to_string(),
        "writer_oid must be the authenticated login's OID"
    );
    // SET ROLE to the group and a forged actor GUC change nothing: the
    // function binds session_user, so the durable writer stays the login.
    let (q2, source2) = budget::fixture(0xf004);
    let mut member = priv_client();
    member
        .batch_execute("SET ROLE dw_m1_broker; SET app.actor='dw_runtime'")
        .unwrap();
    budget::sql(
        &mut member,
        "reserve",
        &serde_json::to_value(&q2).unwrap(),
        budget::op(),
        &serde_json::to_value(&source2).unwrap(),
    )
    .unwrap()
    .unwrap();
    let priv_oid: u32 = runtime_client()
        .query_one("SELECT oid FROM pg_roles WHERE rolname=$1", &[&PRIV_BROKER])
        .unwrap()
        .get(0);
    let last = budget::state(&q2)["ledger"].clone();
    assert_eq!(
        last.as_array().unwrap().last().unwrap()["writer_oid"]
            .as_str()
            .unwrap(),
        priv_oid.to_string()
    );
}

#[test]
fn direct_dml_truncate_and_widened_grants_never_mutate_ledger() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf002);
    budget::run(
        &q,
        budget::op(),
        duskweave::m1_inference_budget::ReservationAction::Reserve,
    )
    .unwrap()
    .unwrap();
    let before = budget::state(&q);
    let mut probe = priv_client();
    // A complete valid row: a missing-column or constraint error cannot mask
    // the boundary this probe must reach.
    let insert = "INSERT INTO execution.inference_reservations \
        (engagement_id,campaign_id,operation_id,generation,record_revision,writer_oid,request,\
         registration_operation_id,registration_event_id,generation_call_id,repair_call_id,\
         capacity,kind) \
        SELECT engagement_id,campaign_id,gen_random_uuid(),generation,record_revision,writer_oid,\
        request,registration_operation_id,registration_event_id,gen_random_uuid(),\
        gen_random_uuid(),capacity,kind FROM execution.inference_reservations \
        WHERE engagement_id=$1 AND campaign_id=$2";
    // Without grants every statement stops at the permission door (42501);
    // an arbitrary error would not prove which boundary refused it.
    for (label, result) in [
        (
            "insert",
            probe.execute(insert, &[&q.engagement_id.0, &q.campaign_id.0]),
        ),
        (
            "update",
            probe.execute(
                "UPDATE execution.inference_reservations SET kind='forged'",
                &[],
            ),
        ),
        (
            "delete",
            probe.execute("DELETE FROM execution.inference_reservations", &[]),
        ),
        (
            "truncate",
            probe.execute("TRUNCATE execution.inference_reservations", &[]),
        ),
    ] {
        assert_eq!(
            errcode(&result.expect_err("ungranted DML must fail")),
            Some("42501".into()),
            "{label} must stop at permission denial"
        );
    }
    // The probe login is an ordinary restricted non-owner; the widened grant
    // must be effective before the boundary assertions run.
    let shape = runtime_client()
        .query_one(
            "SELECT r.rolsuper,r.rolbypassrls,pg_get_userbyid(t.relowner)::text \
             FROM pg_roles r CROSS JOIN pg_class t WHERE r.rolname=$1 \
             AND t.relname='inference_reservations' AND t.relnamespace='execution'::regnamespace",
            &[&PRIV_BROKER],
        )
        .unwrap();
    let (sup, bypass, owner): (bool, bool, String) = (shape.get(0), shape.get(1), shape.get(2));
    assert!(
        !sup && !bypass,
        "probe login must be non-superuser/non-BYPASSRLS"
    );
    assert_ne!(owner, PRIV_BROKER, "probe login must not own the ledger");
    // Temporarily widened grants — including TRUNCATE — still cannot bypass
    // RLS or the shared no-truncate trigger.
    let _restore = Restore::install(
        "GRANT INSERT, UPDATE, DELETE, TRUNCATE ON execution.inference_reservations TO dw_m1_broker_priv",
        "REVOKE INSERT, UPDATE, DELETE, TRUNCATE ON execution.inference_reservations FROM dw_m1_broker_priv",
    );
    let granted: bool = runtime_client()
        .query_one(
            "SELECT has_table_privilege($1,'execution.inference_reservations','TRUNCATE')",
            &[&PRIV_BROKER],
        )
        .unwrap()
        .get(0);
    assert!(granted, "widened TRUNCATE grant must be effective");
    // Granted INSERT reaches row-level security: a permission denial would
    // carry the grant-check message, never the RLS one.
    let insert_err = probe
        .execute(insert, &[&q.engagement_id.0, &q.campaign_id.0])
        .expect_err("granted INSERT must still fail");
    assert_eq!(errcode(&insert_err), Some("42501".into()));
    assert!(
        insert_err
            .as_db_error()
            .is_some_and(|d| d.message().contains("row-level security")),
        "INSERT must stop at RLS, not at the grant check: {insert_err}"
    );
    // Granted UPDATE/DELETE see no rows: there is no mutation policy, so
    // zero affected rows is the RLS boundary's own verdict.
    for sql in [
        "UPDATE execution.inference_reservations SET kind='forged'",
        "DELETE FROM execution.inference_reservations",
    ] {
        assert_eq!(
            probe.execute(sql, &[]).unwrap(),
            0,
            "granted {sql} must affect zero rows under RLS"
        );
    }
    // Granted TRUNCATE reaches the shared trigger itself: P0001 with the
    // inherited guard message, never a permission denial.
    let truncated = probe
        .execute("TRUNCATE execution.inference_reservations", &[])
        .expect_err("granted TRUNCATE must hit the trigger");
    let db_err = truncated
        .as_db_error()
        .expect("trigger refusal is a db error");
    assert_eq!(db_err.code().code(), "P0001");
    assert_eq!(db_err.message(), "session_table_immutable");
    drop(_restore);
    assert!(
        !runtime_client()
            .query_one(
                "SELECT has_table_privilege($1,'execution.inference_reservations','TRUNCATE')",
                &[&PRIV_BROKER],
            )
            .unwrap()
            .get::<_, bool>(0),
        "fixture-owned grant must be fully restored"
    );
    assert_eq!(
        budget::state(&q),
        before,
        "complete scoped state must be unchanged after DML probes"
    );
}

#[test]
fn unsafe_privilege_shapes_fail_existing_runtime_qualification() {
    let _g = db();
    let (q, source) = budget::fixture(0xf003);
    let request = serde_json::to_value(&q).unwrap();
    let source_v = serde_json::to_value(&source).unwrap();
    // TRIGGER privilege on the new ledger disqualifies the runtime role.
    let _trigger = Restore::install(
        "GRANT TRIGGER ON execution.inference_reservations TO dw_runtime",
        "REVOKE TRIGGER ON execution.inference_reservations FROM dw_runtime",
    );
    assert!(
        qualify_runtime(&mut runtime_client()).is_err(),
        "TRIGGER grant on the ledger must fail runtime qualification"
    );
    drop(_trigger);
    qualify_runtime(&mut runtime_client()).unwrap();
    // A Broker login passes the same environment qualification; the guarded
    // function then enforces its own writer identity.
    let mut broker = priv_client();
    qualify_runtime(&mut broker).unwrap();
    budget::sql(&mut broker, "reserve", &request, budget::op(), &source_v)
        .unwrap()
        .unwrap();
    // Execution-writer privilege on the ledger disqualifies the runtime
    // role: INSERT on any execution table is exactly the class of grant the
    // guarded function exists to replace.
    let _writer = Restore::install(
        "GRANT INSERT ON execution.inference_reservations TO dw_runtime",
        "REVOKE INSERT ON execution.inference_reservations FROM dw_runtime",
    );
    assert!(
        qualify_runtime(&mut runtime_client()).is_err(),
        "execution INSERT on the ledger must fail runtime qualification"
    );
    drop(_writer);
    qualify_runtime(&mut runtime_client()).unwrap();
}

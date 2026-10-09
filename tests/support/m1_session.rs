//! Narrow fixture for the prepared/no-effects session slice: two cluster
//! Broker logins (members of dw_runtime and dw_m1_broker), current-window M1
//! registration inputs, the bounded session request document, and scoped
//! durable-state readers over execution.session_fences/session_history.
//! Role fixtures mutate cluster-wide roles: callers hold db_support::db().
//! Included as a module by the real test crates; not a standalone target.
#![allow(dead_code)]

use crate::db_support;
use duskweave::Res;
use duskweave::m1_session::{SessionAction, SessionRequest, session};
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use duskweave::postgres_m1_session::PgSessionStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest};
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::sync::Once;
use uuid::Uuid;

pub const BROKER: &str = "dw_m1_broker_test";
pub const OTHER_BROKER: &str = "dw_m1_broker_other";
pub const OPERATOR: u128 = 0x11;

/// Fixture logins reuse the authorized DW_TEST_DATABASE_URL password; no
/// credential literal ever enters the repository.
fn broker_pass() -> String {
    let rt = db_support::dsn("DW_TEST_DATABASE_URL");
    String::from_utf8(rt.get_password().expect("runtime password").to_vec()).expect("utf8 password")
}

/// Idempotent cluster-role setup: one fixture-owned Broker login plus a
/// second separately authenticated Broker login for impersonation probes.
pub fn ensure_broker_logins() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let pass = broker_pass().replace('\'', "''");
        db_support::admin_client()
            .batch_execute(&format!(
                "DO $$ BEGIN \
                 IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='{BROKER}') THEN \
                 CREATE ROLE {BROKER} LOGIN PASSWORD '{pass}'; \
                 ELSE ALTER ROLE {BROKER} LOGIN PASSWORD '{pass}'; END IF; \
                 IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='{OTHER_BROKER}') THEN \
                 CREATE ROLE {OTHER_BROKER} LOGIN PASSWORD '{pass}'; \
                 ELSE ALTER ROLE {OTHER_BROKER} LOGIN PASSWORD '{pass}'; END IF; \
                 END $$; \
                 GRANT dw_runtime, dw_m1_broker TO {BROKER}; \
                 GRANT dw_runtime, dw_m1_broker TO {OTHER_BROKER};"
            ))
            .unwrap();
    });
}

/// Administrative connection bound to the test database itself — the plain
/// admin DSN targets the maintenance database; table-local grants need the
/// fixture database.
pub fn admin_db_client() -> Client {
    let rt = db_support::dsn("DW_TEST_DATABASE_URL");
    let db = rt.get_dbname().expect("runtime dbname").to_string();
    let mut a = db_support::dsn("DW_TEST_ADMIN_DATABASE_URL");
    a.dbname(&db);
    a.connect(NoTls).expect("admin db connect failed")
}

/// Qualified runtime-and-Broker member connection for the guarded functions.
pub fn broker_client() -> Client {
    let mut cfg = db_support::dsn("DW_TEST_DATABASE_URL");
    cfg.user(BROKER).password(broker_pass());
    cfg.connect(NoTls).expect("broker connect failed")
}

/// A second Broker login: same class, different session_user identity.
pub fn other_broker_client() -> Client {
    let mut cfg = db_support::dsn("DW_TEST_DATABASE_URL");
    cfg.user(OTHER_BROKER).password(broker_pass());
    cfg.connect(NoTls).expect("other broker connect failed")
}

/// libpq keyword/value escaping: quote a value and escape quote/backslash.
fn kw(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Correctly escaped keyword DSN for the given member login and port —
/// real target port for a direct CLI connection, relay port under faults.
pub fn member_dsn(role: &str, password: &str, port: u16) -> String {
    let rt = db_support::dsn("DW_TEST_DATABASE_URL");
    let dbname = rt.get_dbname().expect("runtime dbname").to_string();
    format!(
        "host=127.0.0.1 port={port} user={} password={} dbname={} sslmode=disable",
        kw(role),
        kw(password),
        kw(&dbname)
    )
}

pub fn broker_dsn(port: u16) -> String {
    member_dsn(BROKER, &broker_pass(), port)
}

pub fn other_broker_dsn(port: u16) -> String {
    member_dsn(OTHER_BROKER, &broker_pass(), port)
}

/// The target's real port from the authorized runtime DSN.
pub fn db_port() -> u16 {
    let rt = db_support::dsn("DW_TEST_DATABASE_URL");
    *rt.get_ports().first().unwrap_or(&5432)
}

/// Database clock: every fixture window is anchored on server time.
pub fn now() -> i64 {
    db_support::runtime_client()
        .query_one(
            "SELECT floor(extract(epoch FROM statement_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0)
}

/// Current-window M1 registration document: mission window +-24h, permission
/// window +-12h around server now — wide enough that a transient test-clock
/// skew cannot silently lapse an otherwise valid fixture. `window_shift`
/// expires both windows when nonzero (negative values produce a fully
/// elapsed attachment).
pub fn registration(e: EngagementId, c: CampaignId, now: i64, window_shift: i64) -> Value {
    let mut value = db_support::reg_json(e, c);
    value["starts_at"] = json!(now + window_shift - 86400);
    value["ends_at"] = json!(now + window_shift + 86400);
    value["m1_permission"] = json!({
        "policy_version": 1, "ct_base_domain": "example.invalid",
        "provider_disclosure": "crt_sh", "vantage_ref": Uuid::from_u128(0x31),
        "resolver_ipv4": "192.0.2.53",
        "discovery_rules": [{"label_suffix": "example.invalid"}],
        "contact_rules": [{"asset_ref": Uuid::from_u128(0x21),
            "rule": {"exact": "api.example.invalid"}, "priority": 1}],
        "excluded_names": [{"exact": "excluded.example.invalid"}], "approved_path": "/",
        "starts_at": now + window_shift - 43200, "ends_at": now + window_shift + 43200,
        "campaign_limits": {"episodes": 2, "provider_calls": 3, "dns_questions": 10,
            "dns_followups": 2, "tcp_connections": 5, "head_requests": 2},
        "concurrency": 1
    });
    value
}

/// Bounded session request document; the operation stays a CLI flag.
pub fn request(e: EngagementId, c: CampaignId) -> Value {
    json!({"engagement_id": e, "campaign_id": c,
        "operator_ref": Uuid::from_u128(OPERATOR), "expected_mission_revision": 1})
}

/// Scoped durable-state readers through the restricted runtime login.
pub fn fence(e: EngagementId, c: CampaignId) -> Option<(String, i64, Option<Uuid>, Option<u32>)> {
    db_support::runtime_client()
        .query_opt(
            "SELECT phase, generation, operation_id, writer_oid \
             FROM execution.session_fences WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
}

pub fn history(e: EngagementId, c: CampaignId, op: OperationId) -> Option<Value> {
    db_support::runtime_client()
        .query_opt(
            "SELECT to_jsonb(h) FROM execution.session_history h \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 \
             ORDER BY (kind='released_no_effects') DESC LIMIT 1",
            &[&e.0, &c.0, &op.0],
        )
        .unwrap()
        .map(|row| row.get(0))
}

pub fn history_count(e: EngagementId, c: CampaignId) -> i64 {
    db_support::runtime_client()
        .query_one(
            "SELECT count(*) FROM execution.session_history \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0)
}

/// Bounded session request for direct port-level calls.
pub fn req(e: EngagementId, c: CampaignId) -> SessionRequest {
    SessionRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(OPERATOR)),
        expected_mission_revision: 1,
    }
}

/// Session operations use the same deliberate fresh allocation path as
/// registration operations; deterministic ids would collide across reruns.
pub fn session_op() -> OperationId {
    duskweave::registration::prepare_operation(&mut db_support::ports().0).unwrap()
}

/// Register a current-window M1 scope through the ordinary runtime path.
pub fn register_m1(e: EngagementId, c: CampaignId, shift: i64) -> duskweave::registration::Receipt {
    let raw = serde_json::to_vec(&registration(e, c, now(), shift)).unwrap();
    let input = duskweave::input::parse_register(&raw).unwrap();
    let (mut a, mut s, mut t) = db_support::ports();
    let op = duskweave::registration::prepare_operation(&mut a).unwrap();
    duskweave::registration::register(&mut a, &mut s, &mut t, op, &input).unwrap()
}

/// Broker prepare through the guarded function and the Mission reader port.
pub fn prepare(e: EngagementId, c: CampaignId, op: OperationId) -> Res<Option<Value>> {
    let mut store = PgSessionStore::new(broker_client());
    let mut reader = PgMissionStore::new(db_support::runtime_client());
    session(
        &mut store,
        Some(&mut reader),
        SessionAction::Prepare,
        &req(e, c),
        op,
    )
    .map(|r| r.map(|r| serde_json::to_value(r).unwrap()))
}

/// The fixture Broker login's role OID — writer identity proof.
pub fn broker_oid() -> u32 {
    db_support::runtime_client()
        .query_one("SELECT oid FROM pg_roles WHERE rolname=$1", &[&BROKER])
        .unwrap()
        .get(0)
}

/// Matching withdrawal request for the ordinary writer-fence probes.
pub fn withdraw_req(e: EngagementId, c: CampaignId) -> WithdrawalRequest {
    WithdrawalRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(OPERATOR)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    }
}

/// Bounded poll until the expected durable history row is visible through a
/// fresh restricted connection; never a commit assumption from the writer.
pub fn await_history(e: EngagementId, c: CampaignId, op: OperationId, kind: &str) -> Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(record) = history(e, c, op)
            && record["kind"] == kind
        {
            return record;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "expected durable {kind} record never appeared"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

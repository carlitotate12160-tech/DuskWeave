//! Shared disposable-PostgreSQL fixture for M0A integration tests.
//! Included as a module by the real test crates; not a standalone target.
//! Requires DW_TEST_ADMIN_DATABASE_URL + DW_TEST_DATABASE_URL; absent
//! config fails loudly. Admin is used only for isolated setup; runtime
//! assertions connect as the restricted runtime role.
#![allow(dead_code)]

use duskweave::input::parse_register;
use duskweave::mission::*;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::{Fail, Res};
use postgres::{Client, NoTls};
use std::sync::{Mutex, MutexGuard, Once};
use uuid::Uuid;

pub static INIT: Once = Once::new();
pub static DB: Mutex<()> = Mutex::new(());
pub const MIGRATION: &str = include_str!("../../migrations/0001_mission_registration.sql");
pub const PLANNING_MIGRATION: &str = include_str!("../../migrations/0002_planning_assessment.sql");
pub const HISTORY_MIGRATION: &str =
    include_str!("../../migrations/0003_trajectory_planning_history.sql");
pub const V2_MIGRATION: &str = include_str!("../../migrations/0004_planning_assessment_v2.sql");
pub const WITHDRAWAL_MIGRATION: &str = include_str!("../../migrations/0005_mission_withdrawal.sql");

/// Serializes DB tests (SSI predicate locks intentionally abort racing
/// serializable transactions) and applies one-time admin setup.
pub fn db() -> MutexGuard<'static, ()> {
    let g = DB.lock().unwrap_or_else(|e| e.into_inner());
    setup();
    g
}

pub fn dsn(var: &str) -> postgres::Config {
    std::env::var(var)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| panic!("{var} required"))
        .parse()
        .expect("DSN invalid")
}

pub fn runtime_client() -> Client {
    dsn("DW_TEST_DATABASE_URL")
        .connect(NoTls)
        .expect("runtime connect failed")
}

pub fn admin_client() -> Client {
    dsn("DW_TEST_ADMIN_DATABASE_URL")
        .connect(NoTls)
        .expect("admin connect failed")
}

/// Process-unique engagement/campaign scope so reruns never collide.
pub fn scope(n: u128) -> (EngagementId, CampaignId) {
    static SEED: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    let base = *SEED.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        (nanos ^ ((std::process::id() as u128) << 96)) | 0x100
    });
    let slot = base.wrapping_add(n.wrapping_mul(2));
    (
        EngagementId(Uuid::from_u128(slot)),
        CampaignId(Uuid::from_u128(slot + 1)),
    )
}

/// One-time admin setup: isolated database, migration, runtime login role
/// backed by the DW_TEST_DATABASE_URL password, least privilege via
/// membership in the migration-created NOLOGIN role dw_runtime.
fn setup() {
    INIT.call_once(|| {
        let rt = dsn("DW_TEST_DATABASE_URL");
        let rt_user = rt.get_user().unwrap().to_string();
        assert!(
            rt_user
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "unsafe generated test role identifier"
        );
        let rt_pass = std::str::from_utf8(rt.get_password().unwrap())
            .unwrap()
            .replace('\'', "''");
        let rt_db = rt.get_dbname().unwrap().to_string();
        assert_ne!(rt_db, "postgres", "test DB must be isolated from admin db");
        let mut admin = admin_client();
        if admin
            .query_opt("SELECT 1 FROM pg_database WHERE datname=$1", &[&rt_db])
            .unwrap()
            .is_none()
        {
            admin
                .batch_execute(&format!("CREATE DATABASE {rt_db}"))
                .unwrap();
        }
        let mut a = dsn("DW_TEST_ADMIN_DATABASE_URL");
        a.dbname(&rt_db);
        let mut a = a.connect(NoTls).unwrap();
        a.batch_execute(MIGRATION).unwrap();
        a.batch_execute(PLANNING_MIGRATION).unwrap();
        a.batch_execute(HISTORY_MIGRATION).unwrap();
        a.batch_execute(V2_MIGRATION).unwrap();
        a.batch_execute(WITHDRAWAL_MIGRATION).unwrap();
        a.batch_execute(&format!(
            "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='{rt_user}') \
             THEN CREATE ROLE {rt_user} LOGIN PASSWORD '{rt_pass}'; \
             ELSE ALTER ROLE {rt_user} LOGIN PASSWORD '{rt_pass}'; END IF; END $$; \
             GRANT dw_runtime TO {rt_user};"
        ))
        .unwrap();
    });
}

pub fn reg_json(e: EngagementId, c: CampaignId) -> serde_json::Value {
    serde_json::json!({
        "engagement_id": e.0, "campaign_id": c.0,
        "operator_ref": Uuid::from_u128(0x11), "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 1, "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21), Uuid::from_u128(0x22)],
        "excluded_assets": [Uuid::from_u128(0x23)],
        "exercise_mode": "blind", "starts_at": 1700000000, "ends_at": 1700086400
    })
}

pub fn reg_input(e: EngagementId, c: CampaignId) -> RegistrationInput {
    parse_register(&serde_json::to_vec(&reg_json(e, c)).unwrap()).expect("fixture must parse")
}

pub fn reg(
    a: &mut PgAllocator,
    s: &mut PgMissionStore,
    t: &mut PgTrajectory,
    op: OperationId,
    e: EngagementId,
    c: CampaignId,
) -> Res<registration::Receipt> {
    registration::register(a, s, t, op, &reg_input(e, c))
}

pub fn accepted(
    alloc: &mut PgAllocator,
    store: &mut PgMissionStore,
    traj: &mut PgTrajectory,
    e: EngagementId,
    c: CampaignId,
) -> registration::Receipt {
    let op = registration::prepare_operation(alloc).unwrap();
    registration::register(alloc, store, traj, op, &reg_input(e, c)).unwrap()
}

pub fn ports() -> (PgAllocator, PgMissionStore, PgTrajectory) {
    (
        PgAllocator::new(runtime_client()),
        PgMissionStore::new(runtime_client()),
        PgTrajectory::new(runtime_client()),
    )
}

pub fn count(table: &str, e: EngagementId, cp: CampaignId) -> i64 {
    count_where(table, "", e, cp)
}

pub fn count_where(table: &str, extra: &str, e: EngagementId, cp: CampaignId) -> i64 {
    runtime_client()
        .query_one(
            &format!(
                "SELECT count(*) FROM {table} WHERE engagement_id=$1 AND campaign_id=$2 {extra}"
            ),
            &[&e.0, &cp.0],
        )
        .unwrap()
        .get(0)
}

/// Bounded retry for SSI serialization aborts; deterministic, no sleeps.
pub fn retry<T>(mut f: impl FnMut() -> Res<T>) -> Res<T> {
    let mut last = Err(Fail::Store("unattempted"));
    for _ in 0..20 {
        last = f();
        if !matches!(last, Err(Fail::Store("serialization_retry"))) {
            break;
        }
    }
    last
}

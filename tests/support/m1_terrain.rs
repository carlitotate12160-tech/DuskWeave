//! Shared test support for M1 Terrain DNS intake, PostgreSQL, CLI, and ACK-loss suites.
#![allow(dead_code)]

#[path = "m1_native_inference.rs"]
pub mod native;
#[path = "m1_policy.rs"]
pub mod policy_support;

use crate::db_support;
use duskweave::input::parse_register;
use duskweave::m1_terrain_input::{TerrainDnsRequest, parse_terrain_dns_request};
use duskweave::mission::{CampaignId, EngagementId, MissionRegistered, OperationId};
use duskweave::postgres_m1_terrain::{PgTerrainStore, qualify_terrain};
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_terrain_history::qualify_terrain_trajectory;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::{self, MissionStore};
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;
use std::sync::{MutexGuard, Once};
use uuid::Uuid;

static TERRAIN_INIT: Once = Once::new();

pub fn db() -> MutexGuard<'static, ()> {
    let guard = native::db();
    TERRAIN_INIT.call_once(|| {
        let mut tr = terrain_client();
        qualify_terrain(&mut tr).expect("terrain role qualification");
        let mut tj = trajectory_client();
        qualify_terrain_trajectory(&mut tj).expect("trajectory role qualification");
    });
    guard
}

pub fn runtime_client() -> Client {
    native::client("DW_TEST_DATABASE_URL")
}

pub fn terrain_client() -> Client {
    native::client("DW_TERRAIN_DATABASE_URL")
}

pub fn trajectory_client() -> Client {
    native::client("DW_TRAJECTORY_DATABASE_URL")
}

pub fn broker_client() -> Client {
    native::client("DW_INFERENCE_BROKER_DATABASE_URL")
}

pub fn cognition_client() -> Client {
    native::client("DW_COGNITION_DATABASE_URL")
}

pub fn admin_client() -> Client {
    native::admin()
}

pub fn through_dsn(variable: &str, port: u16) -> String {
    let source = std::env::var(variable).expect("fixture DSN");
    assert_eq!(source.matches("@127.0.0.1:5432/").count(), 1);
    source.replacen("@127.0.0.1:5432/", &format!("@127.0.0.1:{port}/"), 1)
}

pub fn through(variable: &str, port: u16) -> Client {
    through_dsn(variable, port)
        .parse::<postgres::Config>()
        .expect("routed DSN")
        .connect(NoTls)
        .expect("routed connection")
}

pub fn allocate_op() -> OperationId {
    let mut alloc = PgAllocator::new(runtime_client());
    registration::prepare_operation(&mut alloc).expect("allocate operation")
}

pub fn stores() -> (PgMissionStore, PgTerrainStore, PgTrajectory) {
    (
        PgMissionStore::new(runtime_client()),
        PgTerrainStore::new(terrain_client()),
        PgTrajectory::new(trajectory_client()),
    )
}

pub fn register_m1_campaign(slot: u128) -> (EngagementId, CampaignId, MissionRegistered, i64) {
    let (e, c) = db_support::scope(slot);
    let now = policy_support::now();
    let reg_val = policy_support::registration(e, c, now);
    let input = parse_register(&serde_json::to_vec(&reg_val).unwrap()).unwrap();
    let (mut alloc, mut store, mut traj) = db_support::ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let receipt = registration::register(&mut alloc, &mut store, &mut traj, op, &input).unwrap();
    let ev = store
        .outbox_event(e, c, receipt.operation_id)
        .unwrap()
        .unwrap();
    (e, c, ev, now)
}

pub fn dns_request_json(e: EngagementId, c: CampaignId, perceived_at: i64) -> Value {
    json!({
        "version": "m1_terrain_dns_observation_v1",
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "expected_mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "exercise_mode": "blind",
        "correlation_id": Uuid::from_u128(0x701),
        "source_kind": "operator_reviewed_dns",
        "vantage_ref": Uuid::from_u128(0x31),
        "resolver_ipv4": "192.0.2.53",
        "subject_name": "www.example.invalid",
        "asset_ref": Uuid::from_u128(0x21),
        "expected_claim_revision": 0,
        "perceived_at": perceived_at,
        "uncertainty_seconds": 1,
        "transport": "udp",
        "qtype": "A",
        "qclass": "IN",
        "rcode": "noerror",
        "truncated": false,
        "completeness": "complete",
        "limitations": ["single_vantage_only"],
        "answers": [
            {"ipv4": "198.51.100.10", "ttl_seconds": 120}
        ]
    })
}

pub fn parse_req(val: &Value) -> TerrainDnsRequest {
    parse_terrain_dns_request(&serde_json::to_vec(val).unwrap()).expect("valid terrain dns request")
}

/// Exercise the deployed SQL boundary itself, without Rust permission admission.
pub fn sql_accept(
    tx: &mut postgres::Transaction<'_>,
    request: &Value,
    event: &Value,
) -> Result<postgres::Row, postgres::Error> {
    tx.query_one(
        "SELECT terrain.accept_dns_observation($1,$2)",
        &[request, event],
    )
}

pub fn cli_run(
    command: &str,
    input: &Path,
    operation: &str,
    overrides: &[(&str, &str)],
) -> (bool, String, Value) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    child
        .env_clear()
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").expect("runtime DSN"),
        )
        .env(
            "DW_TERRAIN_DATABASE_URL",
            std::env::var("DW_TERRAIN_DATABASE_URL").expect("terrain DSN"),
        )
        .env(
            "DW_TRAJECTORY_DATABASE_URL",
            std::env::var("DW_TRAJECTORY_DATABASE_URL").expect("trajectory DSN"),
        );
    for var in ["SystemRoot", "LLVM_PROFILE_FILE"] {
        if let Some(val) = std::env::var_os(var) {
            child.env(var, val);
        }
    }
    for (k, v) in overrides {
        child.env(k, v);
    }
    let out = child
        .arg(command)
        .arg("--input")
        .arg(input)
        .arg("--operation")
        .arg(operation)
        .output()
        .expect("execute duskweave CLI");
    assert!(out.stderr.is_empty(), "stderr must be empty: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let json_line = out
        .stdout
        .split(|b| *b == b'\n')
        .find(|line| line.first() == Some(&b'{'))
        .expect("JSON receipt line on stdout");
    let receipt: Value = serde_json::from_slice(json_line).expect("parse JSON receipt");
    assert_eq!(receipt["dispatch_granted"], false);
    assert_eq!(receipt["acquisition_qualified"], false);
    assert_eq!(receipt["corroborated"], false);
    (out.status.success(), stdout, receipt)
}

#![allow(dead_code)]

use crate::db_support;
use duskweave::input::parse_register;
use duskweave::m1_policy::{M1PolicyQuery, M1PolicyResult};
use duskweave::m1_policy_input::parse_policy_query;
use duskweave::mission::*;
use duskweave::postgres_mission::{PgMissionStore, qualify_runtime};
use duskweave::registration::{self, MissionStore};
use duskweave::withdrawal::{MissionAuthorityWithdrawn, WithdrawalReason, WithdrawalRequest};
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::process::{Command, Output};
use uuid::Uuid;

pub fn registration(e: EngagementId, c: CampaignId, now: i64) -> Value {
    let mut value = db_support::reg_json(e, c);
    value["authority_revision"] = json!(17);
    value["starts_at"] = json!(now - 7200);
    value["ends_at"] = json!(now + 7200);
    value["m1_permission"] = json!({
        "policy_version": 1, "ct_base_domain": "example.invalid", "provider_disclosure": "crt_sh",
        "vantage_ref": Uuid::from_u128(0x31), "resolver_ipv4": "192.0.2.53",
        "discovery_rules": [{"label_suffix": "example.invalid"}],
        "contact_rules": [
            {"asset_ref": Uuid::from_u128(0x21), "rule": {"exact": "www.example.invalid"}, "priority": 1},
            {"asset_ref": Uuid::from_u128(0x22), "rule": {"label_suffix": "other.invalid"}, "priority": 2}],
        "excluded_names": [{"label_suffix": "excluded.example.invalid"}], "approved_path": "/",
        "starts_at": now - 3600, "ends_at": now + 3600,
        "campaign_limits": {"episodes": 2, "provider_calls": 3, "dns_questions": 10,
            "dns_followups": 2, "tcp_connections": 5, "head_requests": 2}, "concurrency": 1
    });
    value
}

pub fn query_json(e: EngagementId, c: CampaignId, purpose: Value) -> Value {
    json!({"engagement_id": e, "campaign_id": c, "expected_mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13), "exercise_mode": "blind", "purpose": purpose})
}

pub fn discovery(name: &str) -> Value {
    json!({"discovery_disclosure": {"name": name}})
}
pub fn contact(name: &str, asset: u128) -> Value {
    json!({"contact": {"name": name, "asset_ref": Uuid::from_u128(asset)}})
}
pub fn query(e: EngagementId, c: CampaignId, purpose: Value) -> M1PolicyQuery {
    parse_policy_query(&serde_json::to_vec(&query_json(e, c, purpose)).unwrap()).unwrap()
}

pub fn event(value: &Value) -> MissionRegistered {
    let input = parse_register(&serde_json::to_vec(value).unwrap()).unwrap();
    Mission::register(
        &input,
        OperationId(Uuid::from_u128(0x41)),
        EventId(Uuid::from_u128(0x42)),
        100,
    )
    .unwrap()
    .1
}

pub fn now() -> i64 {
    db_support::runtime_client()
        .query_one(
            "SELECT floor(extract(epoch FROM statement_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0)
}

pub fn reader() -> PgMissionStore {
    let mut client = db_support::runtime_client();
    qualify_runtime(&mut client).unwrap();
    PgMissionStore::new(client)
}

pub fn admin() -> Client {
    let mut config = db_support::dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(
        db_support::dsn("DW_TEST_DATABASE_URL")
            .get_dbname()
            .unwrap(),
    );
    config.connect(NoTls).unwrap()
}

pub fn register(value: &Value) -> MissionRegistered {
    let input = parse_register(&serde_json::to_vec(value).unwrap()).unwrap();
    let (mut allocator, mut store, mut history) = db_support::ports();
    let op = registration::prepare_operation(&mut allocator).unwrap();
    let receipt =
        registration::register(&mut allocator, &mut store, &mut history, op, &input).unwrap();
    store
        .outbox_event(input.engagement_id, input.campaign_id, receipt.operation_id)
        .unwrap()
        .unwrap()
}

pub fn marker(event: &MissionRegistered) -> MissionAuthorityWithdrawn {
    MissionAuthorityWithdrawn::new(
        WithdrawalRequest {
            engagement_id: event.engagement_id,
            campaign_id: event.campaign_id,
            operator_ref: OperatorRef(Uuid::from_u128(0x11)),
            expected_mission_revision: 1,
            reason: WithdrawalReason::OperatorRequested,
        },
        OperationId(Uuid::from_u128(0x51)),
        event.operation_id,
        EventId(Uuid::from_u128(0x52)),
        100,
    )
    .unwrap()
}

pub fn insert_marker(client: &mut impl postgres::GenericClient, event: &MissionAuthorityWithdrawn) {
    client.execute("INSERT INTO mission.withdrawals \
        (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract) VALUES ($1,$2,$3,$4,$5,$6)",
        &[&event.request.engagement_id.0, &event.request.campaign_id.0, &event.operation_id.0,
            &event.event_id.0, &event.registration_operation_id.0, &serde_json::to_value(event).unwrap()]).unwrap();
}

pub fn persisted(e: EngagementId, c: CampaignId) -> Vec<Value> {
    let mut client = db_support::runtime_client();
    ["mission.missions", "mission.registration_outbox", "mission.planning_assessments", "mission.withdrawals",
        "trajectory.registration_history", "trajectory.planning_history", "trajectory.withdrawal_history"]
        .iter().map(|table| client.query_one(&format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) \
             FROM {table} t WHERE engagement_id=$1 AND campaign_id=$2"), &[&e.0, &c.0]).unwrap().get(0)).collect()
}

pub fn flags(receipt: &Value) {
    assert_eq!(receipt["check_kind"], "m1_name_policy_snapshot_v1");
    for flag in [
        "current_permission",
        "dispatch_granted",
        "acquisition_qualified",
    ] {
        assert_eq!(receipt[flag], false);
    }
    let text = receipt.to_string();
    assert!(!text.contains(".invalid") && !text.contains("192.0.2.53"));
    assert!(text.len() < 1800);
}

pub fn assert_reason(result: &M1PolicyResult, reason: &str, disposition: &str) {
    let receipt = serde_json::to_value(result).unwrap();
    flags(&receipt);
    assert_eq!(receipt["reason"], reason);
    assert_eq!(receipt["result"], disposition);
}

pub fn cli(args: &[&str], dsn: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    if let Some(dsn) = dsn {
        cmd.env("DW_DATABASE_URL", dsn);
    }
    cmd.args(args).output().unwrap()
}

pub fn file_command(command: &str, raw: &[u8], extra: &[&str], dsn: Option<&str>) -> Output {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("dw-m1-policy-{}-{n}.json", std::process::id()));
    std::fs::write(&path, raw).unwrap();
    let mut args = vec![command, "--input", path.to_str().unwrap()];
    args.extend(extra);
    let output = cli(&args, dsn);
    std::fs::remove_file(path).unwrap();
    output
}

pub fn receipt(output: &Output, reason: &str, disposition: &str) -> Value {
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    let receipt: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    flags(&receipt);
    assert_eq!(receipt["reason"], reason, "{text}");
    assert_eq!(receipt["result"], disposition);
    assert_eq!(
        output.status.success(),
        disposition != "unavailable",
        "{text}"
    );
    assert!(output.stderr.is_empty());
    assert!(!text.contains("SENTINEL") && !text.contains("postgresql://"));
    receipt
}

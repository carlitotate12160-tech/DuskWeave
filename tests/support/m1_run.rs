#![allow(dead_code)]
use crate::{db_support, native, terrain};
use serde_json::{Value, json};
use std::process::Command;
use uuid::Uuid;
pub fn refresh() {
    let mut a = native::admin();
    a.batch_execute(
        &include_str!("../../migrations/0016_autonomous_inference_profile.sql")
            .replace("CREATE FUNCTION ", "CREATE OR REPLACE FUNCTION "),
    )
    .unwrap();
    for source in [
        include_str!("../../migrations/0017_autonomous_fixture_run.sql"),
        include_str!("../../migrations/0018_issued_fixture_terrain.sql"),
    ] {
        let normalized = source.replace("CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION ");
        for function in normalized.split("CREATE FUNCTION ").skip(1) {
            let end = function.find("$$;").unwrap() + 3;
            a.batch_execute(&format!("CREATE OR REPLACE FUNCTION {}", &function[..end]))
                .unwrap();
        }
    }
}
pub fn registered(slot: u128) -> Value {
    registered_with(slot, |_| {})
}
pub fn registered_with(slot: u128, change: impl FnOnce(&mut Value)) -> Value {
    let (e, c) = db_support::scope(slot);
    let now: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let mut raw = terrain::combined_registration_json(e, c, now, true);
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v3");
    p["recipient"] = json!("urn:duskweave:fixture:inference:v1");
    p["profile"] = json!("deterministic-autonomous-v1");
    p["execution_mode"] = json!("controlled_fixture");
    p["run_profile"] = json!("autonomous-dns-fixture-v1");
    p["field_classes"] = json!([
        "mission_question_constraints",
        "scoped_opaque_references",
        "owner_qualified_summaries",
        "admitted_operator_premises",
        "manifest_availability",
        "omissions_unknown",
        "capacity",
        "admitted_repair_fields_codes",
        "admitted_campaign_decision_feedback"
    ]);
    p["campaign_limits"] = json!({"calls":4,"request_bytes":65536,"response_bytes":262144,"output_token_capacity":16384,"wall_time_ms":90000});
    for (k, v) in [
        ("context_bytes", 8192),
        ("decision_wall_time_ms", 45000),
        ("per_call_wall_time_ms", 20000),
        ("shared_wall_time_ms", 40000),
        ("phase_wall_time_ms", 50000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        p["operation_limits"][k] = json!(v);
    }
    raw["included_assets"] = json!([Uuid::from_u128(0x21)]);
    raw["m1_permission"]["contact_rules"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    change(&mut raw);
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut a, mut m, mut h) = db_support::ports();
    let op = duskweave::registration::prepare_operation(&mut a).unwrap();
    duskweave::registration::register(&mut a, &mut m, &mut h, op, &input).unwrap();
    let candidates = raw["included_assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                Uuid::parse_str(v.as_str().unwrap()).unwrap(),
                duskweave::m1_context_input::Priority::High,
            )
        })
        .collect();
    let context = terrain::context_request_for(e, c, &raw, candidates).0;
    let seeds:Vec<_>=raw["m1_permission"]["contact_rules"].as_array().unwrap().iter().map(|r|
        json!({"asset_ref":r["asset_ref"],"name":r["rule"].get("exact").or_else(||r["rule"].get("label_suffix")).unwrap()})).collect();
    json!({"version":"m1_autonomous_run_request_v1","run_id":consent_id(),"run_profile":"autonomous-dns-fixture-v1",
        "execution_mode":"controlled_fixture","deadline_at_ms":now*1000+149000,"context":context,
        "seed_names":seeds})
}
pub fn consent_id() -> Uuid {
    native::client("DW_TEST_DATABASE_URL")
        .query_one("SELECT gen_random_uuid()", &[])
        .unwrap()
        .get(0)
}
pub fn command() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    c.env_clear();
    c.env("DW_DATABASE_CONFIG_MODE", "env-local").env(
        "DW_DATABASE_URL",
        std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    );
    for k in [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(v) = std::env::var_os(k) {
            c.env(k, v);
        }
    }
    c
}
pub fn cli(command: &str, flag: &str, value: &str) -> (bool, Value) {
    let mut c = self::command();
    let o = c.arg(command).arg(flag).arg(value).output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    let receipt = text
        .lines()
        .find(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .unwrap_or_else(|| json!({"diagnostic":text.trim()}));
    (o.status.success(), receipt)
}

pub fn path(q: &Value) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("dw-run-{}.json", q["run_id"].as_str().unwrap()));
    std::fs::write(&path, serde_json::to_vec(q).unwrap()).unwrap();
    path
}
pub fn id(q: &Value) -> Uuid {
    Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap()
}
pub fn release() -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(std::fs::read(env!("CARGO_BIN_EXE_duskweave")).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn start_only(q: &Value) -> duskweave::m1_run::RunRecord {
    let q: duskweave::m1_run::RunRequest = serde_json::from_value(q.clone()).unwrap();
    let mut store = duskweave::postgres_m1_run::PgRunStore::new(native::client(
        "DW_INFERENCE_BROKER_DATABASE_URL",
    ));
    assert!(store.start(&q, &release()).unwrap());
    store.inspect(q.run_id).unwrap()
}

pub fn complete_first(record: &duskweave::m1_run::RunRecord) {
    use duskweave::{m1_run::RunPhase, postgres_m1_run::PgRunStore};
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let r = record.request.run_id;
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    store.start_phase(r, RunPhase::DecisionOne).unwrap();
    let mut child = command()
        .args([
            "m1-run-phase",
            "--run",
            &r.to_string(),
            "--phase",
            "decision_one",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("first phase deadline");
        }
        store.renew(r).unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
    store.complete_phase(r, RunPhase::DecisionOne).unwrap();
}

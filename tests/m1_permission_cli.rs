use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::registration::{self, MissionStore, TrajectoryPort};
use serde_json::{Value, json};
use std::process::{Command, Output};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_permission.rs"]
mod policy;

fn cli(args: &[&str], database: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    if database {
        cmd.env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        );
    }
    cmd.args(args).output().unwrap()
}

fn register_raw(raw: &[u8], operation: OperationId, database: bool) -> Output {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("dw-m1-{}-{serial}.json", std::process::id()));
    std::fs::write(&path, raw).unwrap();
    let output = cli(
        &[
            "register",
            "--operation",
            &operation.to_string(),
            "--input",
            path.to_str().unwrap(),
        ],
        database,
    );
    std::fs::remove_file(path).unwrap();
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn valid_attachment_reaches_database_configuration_boundary() {
    let (e, c) = db_support::scope(1);
    let output = register_raw(
        &serde_json::to_vec(&policy::registration(e, c)).unwrap(),
        OperationId(Uuid::from_u128(1)),
        false,
    );
    assert_eq!(stdout(&output), "error=missing_env\n");
    assert!(output.stderr.is_empty());
}

fn inspect(e: EngagementId, c: CampaignId) -> Output {
    cli(
        &[
            "inspect",
            "--engagement",
            &e.to_string(),
            "--campaign",
            &c.to_string(),
        ],
        true,
    )
}

fn assert_historical(output: &Output, history: &str) {
    let text = stdout(output);
    assert!(output.status.success(), "{text}");
    assert!(output.stderr.is_empty());
    assert!(text.contains("m1 historical_configuration"));
    assert!(text.contains("policy_version=1"));
    assert!(text.contains("current_permission=false acquisition_qualified=false"));
    assert!(text.contains(&format!("history={history}")));
    for field in [
        "episodes=2",
        "provider_calls=3",
        "dns_questions=10",
        "dns_followups=2",
        "tcp_connections=5",
        "head_requests=2",
        "discovery_rules=1",
        "contact_rules=1",
        "excluded_names=1",
    ] {
        assert!(text.contains(field), "missing {field}");
    }
    assert!(!text.contains("example") && !text.contains("192.0.2.53"));
    assert!(text.len() < 1600);
}

#[test]
fn register_retry_fresh_inspect_and_withdrawal_preserve_one_attachment() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0x7701);
    let prepared = cli(
        &[
            "prepare-operation",
            "--engagement",
            &e.to_string(),
            "--campaign",
            &c.to_string(),
        ],
        true,
    );
    assert!(prepared.status.success());
    let op = stdout(&prepared)
        .split_whitespace()
        .find_map(|part| part.strip_prefix("operation="))
        .unwrap()
        .to_string();
    let op = OperationId::parse(&op).unwrap();
    let raw = serde_json::to_vec(&policy::registration(e, c)).unwrap();
    let accepted = register_raw(&raw, op, true);
    assert!(accepted.status.success(), "{}", stdout(&accepted));
    assert!(stdout(&accepted).contains("history=completed"));
    assert_eq!(stdout(&register_raw(&raw, op, true)), stdout(&accepted));
    let inspected = inspect(e, c);
    assert_historical(&inspected, "completed");
    let (_, mut s, _) = db_support::ports();
    let event = s.outbox_event(e, c, op).unwrap().unwrap();
    assert!(stdout(&accepted).contains(&format!("event={}", event.event_id)));
    assert!(stdout(&inspected).contains(&format!("history.event={}", event.event_id)));
    let row: Value = db_support::runtime_client().query_one(
        "SELECT contract FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0],
    ).unwrap().get(0);
    assert_eq!(row, serde_json::to_value(&event).unwrap());
    let withdrawal = json!({"engagement_id": e.0, "campaign_id": c.0,
        "operator_ref": Uuid::from_u128(0x11), "expected_mission_revision": 1, "reason": "operator_requested"});
    let path = std::env::temp_dir().join(format!("dw-m1-withdraw-{}.json", std::process::id()));
    std::fs::write(&path, withdrawal.to_string()).unwrap();
    let withdrawn = cli(
        &[
            "withdraw",
            "--operation",
            &Uuid::from_u128(0x7799).to_string(),
            "--input",
            path.to_str().unwrap(),
            "--recover",
            "false",
        ],
        true,
    );
    std::fs::remove_file(path).unwrap();
    assert!(withdrawn.status.success(), "{}", stdout(&withdrawn));
    let inspected = inspect(e, c);
    assert_historical(&inspected, "completed");
    assert!(stdout(&inspected).contains("mission.revision=2"));
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(db_support::count(table, e, c), 1);
    }
}

#[test]
fn pending_and_anomalous_configuration_is_never_current_permission() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0x7801);
    let (mut a, mut s, mut t) = db_support::ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    let (mission, event) = duskweave::mission::Mission::register(
        &policy::input(e, c),
        op,
        duskweave::mission::EventId(Uuid::from_u128(0x7802)),
        1700000001,
    )
    .unwrap();
    s.commit_registration(&mission, &event).unwrap();
    assert_historical(&inspect(e, c), "pending");
    let reconciled = cli(
        &[
            "reconcile",
            "--engagement",
            &e.to_string(),
            "--campaign",
            &c.to_string(),
            "--operation",
            &op.to_string(),
        ],
        true,
    );
    assert!(reconciled.status.success());
    assert!(stdout(&reconciled).contains("Committed"));
    let mut changed = serde_json::to_value(&event).unwrap();
    changed["fields"]["m1_permission"]["campaign_limits"]["episodes"] = json!(3);
    assert_eq!(
        t.deliver(&serde_json::from_value(changed).unwrap())
            .unwrap(),
        duskweave::trajectory::Delivered::Anomaly
    );
    assert_historical(&inspect(e, c), "anomaly");
    let empty = inspect(e, CampaignId(Uuid::from_u128(0x78ff)));
    assert_eq!(stdout(&empty), "inspect result=empty\n");
}

#[test]
fn rejected_nested_input_never_reaches_durable_rows_or_diagnostics() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0x7901);
    for pointer in [
        "/m1_permission",
        "/m1_permission/contact_rules/0",
        "/m1_permission/campaign_limits",
        "/m1_permission/discovery_rules/0",
    ] {
        let mut raw = policy::registration(e, c);
        raw.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("secret".into(), json!("SENTINEL-M1-FORBIDDEN"));
        let output = register_raw(
            &serde_json::to_vec(&raw).unwrap(),
            OperationId(Uuid::from_u128(0x7902)),
            true,
        );
        assert!(!output.status.success());
        assert!(matches!(
            stdout(&output).as_str(),
            "error=schema_violation\n" | "error=malformed_json\n"
        ));
        assert!(output.stderr.is_empty());
        assert!(!stdout(&output).contains("SENTINEL"));
    }
    for (field, value) in [
        ("m1_permission", Value::Null),
        ("m1_permission", json!({"secret":"SENTINEL-M1-FORBIDDEN"})),
    ] {
        let mut raw = policy::registration(e, c);
        raw[field] = value;
        let output = register_raw(
            &serde_json::to_vec(&raw).unwrap(),
            OperationId(Uuid::from_u128(0x7902)),
            false,
        );
        assert_eq!(stdout(&output), "error=schema_violation\n");
    }
    let output = register_raw(
        &vec![b' '; 16 * 1024 + 1],
        OperationId(Uuid::from_u128(0x7902)),
        false,
    );
    assert_eq!(stdout(&output), "error=size_limit\n");
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(db_support::count(table, e, c), 0);
    }
}

#[test]
fn corrupted_attachment_inspection_is_bounded_not_legacy_fallback() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0x7a01);
    let (mut a, mut s, mut t) = db_support::ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut s, &mut t, op, &policy::input(e, c)).unwrap();
    let mut cfg = db_support::dsn("DW_TEST_ADMIN_DATABASE_URL");
    cfg.dbname(
        db_support::dsn("DW_TEST_DATABASE_URL")
            .get_dbname()
            .unwrap(),
    );
    cfg.connect(postgres::NoTls).unwrap().execute(
        "UPDATE mission.registration_outbox SET contract=jsonb_set(contract,'{fields,m1_permission}', 'null'::jsonb) WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0],
    ).unwrap();
    let output = inspect(e, c);
    assert!(!output.status.success());
    assert_eq!(stdout(&output), "error=contract_decode\n");
    assert!(output.stderr.is_empty());
}

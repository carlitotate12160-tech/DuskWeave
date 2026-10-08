//! End-to-end CLI tests through the real binary. The child process gets an
//! explicit environment allowlist: only DW_DATABASE_URL for database config.

use duskweave::mission::OperationId;
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use uuid::Uuid;

const BIN: &str = env!("CARGO_BIN_EXE_duskweave");
const MIGRATION: &str = include_str!("../migrations/0001_mission_registration.sql");
const PLANNING_MIGRATION: &str = include_str!("../migrations/0002_planning_assessment.sql");
const HISTORY_MIGRATION: &str = include_str!("../migrations/0003_trajectory_planning_history.sql");
const WITHDRAWAL_MIGRATION: &str = include_str!("../migrations/0005_mission_withdrawal.sql");
const ELIGIBILITY_MIGRATION: &str = include_str!("../migrations/0007_planning_eligibility.sql");

static DB: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn admin_client() -> Client {
    std::env::var("DW_TEST_ADMIN_DATABASE_URL")
        .expect("DW_TEST_ADMIN_DATABASE_URL required")
        .parse::<postgres::Config>()
        .unwrap()
        .connect(NoTls)
        .unwrap()
}

fn runtime_dsn() -> String {
    std::env::var("DW_TEST_DATABASE_URL").expect("DW_TEST_DATABASE_URL required")
}

fn withdrawal_row_counts() -> [i64; 7] {
    let mut client = runtime_dsn()
        .parse::<postgres::Config>()
        .unwrap_or_else(|_| panic!("withdrawal snapshot config failed"))
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("withdrawal snapshot connection failed"));
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM mission.missions), \
             (SELECT count(*) FROM mission.registration_outbox), \
             (SELECT count(*) FROM mission.planning_assessments), \
             (SELECT count(*) FROM mission.withdrawals), \
             (SELECT count(*) FROM trajectory.registration_history), \
             (SELECT count(*) FROM trajectory.planning_history), \
             (SELECT count(*) FROM trajectory.withdrawal_history)",
            &[],
        )
        .unwrap_or_else(|_| panic!("withdrawal snapshot read failed"));
    std::array::from_fn(|i| row.get(i))
}

fn ensure_setup() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let rt: postgres::Config = runtime_dsn().parse().unwrap();
        let rt_user = rt.get_user().unwrap().to_string();
        let rt_pass = std::str::from_utf8(rt.get_password().unwrap())
            .unwrap()
            .replace('\'', "''");
        let rt_db = rt.get_dbname().unwrap().to_string();
        let mut admin = admin_client();
        if admin
            .query_opt("SELECT 1 FROM pg_database WHERE datname = $1", &[&rt_db])
            .unwrap()
            .is_none()
        {
            admin
                .batch_execute(&format!("CREATE DATABASE {rt_db}"))
                .unwrap();
        }
        drop(admin);
        let mut cfg: postgres::Config = std::env::var("DW_TEST_ADMIN_DATABASE_URL")
            .unwrap()
            .parse()
            .unwrap();
        cfg.dbname(&rt_db);
        let mut a = cfg.connect(NoTls).unwrap();
        a.batch_execute(MIGRATION).unwrap();
        a.batch_execute(PLANNING_MIGRATION).unwrap();
        a.batch_execute(HISTORY_MIGRATION).unwrap();
        // 0007 is the reapply-safe latest check even after eligible v4 rows;
        // earlier version checks run only in isolated upgrade fixtures.
        a.batch_execute(WITHDRAWAL_MIGRATION).unwrap();
        a.batch_execute(ELIGIBILITY_MIGRATION).unwrap();
        a.batch_execute(&format!(
            "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{rt_user}') \
             THEN CREATE ROLE {rt_user} LOGIN PASSWORD '{rt_pass}'; \
             ELSE ALTER ROLE {rt_user} LOGIN PASSWORD '{rt_pass}'; END IF; END $$; \
             GRANT dw_runtime TO {rt_user};"
        ))
        .unwrap();
    });
}

/// Spawn the CLI with an explicit environment allowlist. Windows child
/// processes additionally need SystemRoot for Winsock/DNS; it is still an
/// allowlist, not the ambient environment.
fn cli(args: &[&str], dsn: Option<&str>) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.env_clear().env("DW_DATABASE_CONFIG_MODE", "env-local");
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    if let Some(p) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", p);
    }
    if let Some(d) = dsn {
        cmd.env("DW_DATABASE_URL", d);
    }
    cmd.args(args).output().expect("spawn failed")
}

fn assess_cli(operation: &str, input: &Path, recover: bool, dsn: Option<&str>) -> Output {
    cli(
        &[
            "assess",
            "--operation",
            operation,
            "--input",
            input.to_str().unwrap(),
            "--recover",
            if recover { "true" } else { "false" },
        ],
        dsn,
    )
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn mission_json(e: &str, c: &str) -> String {
    format!(
        r#"{{"engagement_id":"{e}","campaign_id":"{c}","operator_ref":"{}",
        "authority_ref":"{}","authority_revision":1,"goal_ref":"{}",
        "included_assets":["{}"],"excluded_assets":[],
        "exercise_mode":"blind","starts_at":1700000000,"ends_at":1700086400}}"#,
        Uuid::from_u128(0x11),
        Uuid::from_u128(0x12),
        Uuid::from_u128(0x13),
        Uuid::from_u128(0x21)
    )
}

fn planning_json(e: &str, c: &str, revision: u64, confirmed: bool) -> String {
    serde_json::json!({
        "engagement_id": e, "campaign_id": c,
        "purpose_ref": Uuid::from_u128(0x13),
        "asset_ref": Uuid::from_u128(0x21),
        "expected_mission_revision": revision,
        "current_authority_confirmed": confirmed,
    })
    .to_string()
}

fn fresh_scope() -> (String, String) {
    static SEED: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let base = *SEED.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        (nanos ^ ((std::process::id() as u128) << 96)) | 0x100
    });
    let slot =
        base.wrapping_add(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as u128 * 2);
    (
        Uuid::from_u128(slot).to_string(),
        Uuid::from_u128(slot + 1).to_string(),
    )
}

fn write_input(body: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("dw-m0a-{n}-{}.json", std::process::id()));
    std::fs::write(&path, body).unwrap();
    path
}

fn rows(e: &Uuid, c: &Uuid) -> i64 {
    let cfg: postgres::Config = runtime_dsn().parse().unwrap();
    let db = cfg.get_dbname().unwrap().to_string();
    let mut a: postgres::Config = std::env::var("DW_TEST_ADMIN_DATABASE_URL")
        .unwrap()
        .parse()
        .unwrap();
    a.dbname(&db);
    let mut conn = a.connect(NoTls).unwrap();
    conn.query_one(
        "SELECT (SELECT count(*) FROM mission.missions WHERE engagement_id=$1 AND campaign_id=$2) + \
         (SELECT count(*) FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2)",
        &[e, c],
    )
    .unwrap()
    .get(0)
}

#[test]
fn cli_full_flow_and_fresh_process_inspect() {
    ensure_setup();
    let _guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    let (es, cs) = fresh_scope();
    let dsn = runtime_dsn();

    let o = cli(
        &["prepare-operation", "--engagement", &es, "--campaign", &cs],
        Some(&dsn),
    );
    assert!(o.status.success(), "{}", stdout(&o));
    let op = stdout(&o)
        .split_whitespace()
        .find_map(|t| t.strip_prefix("operation="))
        .expect("operation receipt")
        .to_string();
    OperationId::parse(&op).expect("receipt operation must be a UUID");

    let path = write_input(&mission_json(&es, &cs));
    let o = cli(
        &[
            "register",
            "--operation",
            &op,
            "--input",
            path.to_str().unwrap(),
        ],
        Some(&dsn),
    );
    std::fs::remove_file(&path).ok();
    assert!(o.status.success(), "{}", stdout(&o));
    let out = stdout(&o);
    assert!(out.contains("result=accepted"), "{out}");
    assert!(out.contains("history=completed"), "{out}");

    // Fresh process: inspect preserves accepted registration and history.
    let o = cli(
        &["inspect", "--engagement", &es, "--campaign", &cs],
        Some(&dsn),
    );
    let out = stdout(&o);
    assert!(o.status.success(), "{out}");
    assert!(out.contains("mission.revision=1"), "{out}");
    assert!(out.contains("history.status=completed"), "{out}");

    let o = cli(
        &[
            "reconcile",
            "--engagement",
            &es,
            "--campaign",
            &cs,
            "--operation",
            &op,
        ],
        Some(&dsn),
    );
    let out = stdout(&o);
    assert!(o.status.success(), "{out}");
    assert!(out.contains("Committed"), "{out}");
}

#[test]
fn cli_rejects_without_mutation() {
    ensure_setup();
    let _guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    let (es, cs) = fresh_scope();
    let e = Uuid::parse_str(&es).unwrap();
    let c = Uuid::parse_str(&cs).unwrap();
    let dsn = runtime_dsn();
    let assessment_operation = Uuid::from_u128(0xf005).to_string();

    for (label, args) in [
        ("unknown", vec!["assess", "--engagement", &es]),
        ("withdraw", vec!["withdraw", "--campaign", &cs]),
        ("missing args", vec!["register", "--operation"]),
        (
            "assess missing",
            vec!["assess", "--operation", "not-a-uuid"],
        ),
        ("assess unknown", vec!["assess", "--unknown", "value"]),
        (
            "assess duplicate",
            vec!["assess", "--recover", "true", "--recover", "false"],
        ),
        (
            "assess bad recover",
            vec![
                "assess",
                "--operation",
                &assessment_operation,
                "--input",
                "unused",
                "--recover",
                "maybe",
            ],
        ),
        (
            "bad uuid",
            vec!["inspect", "--engagement", "not-a-uuid", "--campaign", &cs],
        ),
    ] {
        let before = if args.first() == Some(&"withdraw") {
            Some(withdrawal_row_counts())
        } else {
            None
        };
        let o = cli(&args, Some(&dsn));
        let after = before.map(|_| withdrawal_row_counts());
        assert!(!o.status.success(), "{label} must fail: {}", stdout(&o));
        if args.first() == Some(&"withdraw") {
            assert_eq!(before, after, "malformed withdrawal mutated durable rows");
            assert!(o.stderr.is_empty(), "withdrawal emitted stderr");
            let output = stdout(&o);
            let lines: Vec<_> = output.lines().collect();
            assert_eq!(lines.len(), 2, "withdrawal output line count");
            let receipt: Value = serde_json::from_str(lines[0])
                .unwrap_or_else(|_| panic!("withdrawal rejection JSON invalid"));
            assert!(
                receipt
                    == json!({
                        "result": "rejected", "operation": null,
                        "reason": "invalid_args", "action": "reconcile_authority",
                        "current_permission": false, "continuation_blocked": true
                    }),
                "withdrawal rejection receipt mismatch"
            );
            assert!(
                lines[1] == "error=invalid_args",
                "withdrawal category mismatch"
            );
        } else {
            assert!(stdout(&o).starts_with("error="), "{label}: {}", stdout(&o));
        }
    }

    let path =
        write_input(&mission_json(&es, &cs).replace("\"ends_at\":1700086400", "\"ends_at\":1"));
    let o = cli(
        &[
            "register",
            "--operation",
            &Uuid::from_u128(0xf001).to_string(),
            "--input",
            path.to_str().unwrap(),
        ],
        Some(&dsn),
    );
    std::fs::remove_file(&path).ok();
    assert!(!o.status.success());
    assert!(stdout(&o).contains("invalid_window"), "{}", stdout(&o));
    assert_eq!(rows(&e, &c), 0, "rejected commands must not create state");
}

#[test]
fn cli_rejects_oversized_input_before_connecting() {
    let _guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    let body = " ".repeat(16 * 1024 + 8);
    let path = write_input(&body);
    // No DSN at all: the bounded file read must reject before any connection.
    let o = cli(
        &[
            "register",
            "--operation",
            &Uuid::from_u128(0xf003).to_string(),
            "--input",
            path.to_str().unwrap(),
        ],
        None,
    );
    assert!(!o.status.success());
    assert!(stdout(&o).contains("size_limit"), "{}", stdout(&o));
    let o = assess_cli(&Uuid::from_u128(0xf003).to_string(), &path, false, None);
    std::fs::remove_file(&path).ok();
    assert!(stdout(&o).contains("size_limit"), "{}", stdout(&o));
}

#[test]
fn cli_rejects_sensitive_sentinel_cleanly() {
    ensure_setup();
    let _guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    let dsn = runtime_dsn();
    let mut body = mission_json(&fresh_scope().0, &fresh_scope().1);
    body = body.trim_end_matches('}').to_string() + ",\"secret\":\"S3NTINEL-9f3a-live\"}";
    let path = write_input(&body);
    let o = cli(
        &[
            "register",
            "--operation",
            &Uuid::from_u128(0xf002).to_string(),
            "--input",
            path.to_str().unwrap(),
        ],
        Some(&dsn),
    );
    std::fs::remove_file(&path).ok();
    assert!(!o.status.success());
    let all = stdout(&o) + &String::from_utf8_lossy(&o.stderr);
    assert!(!all.contains("S3NTINEL"), "sentinel leaked: {all}");
    assert!(all.contains("schema_violation"), "{all}");
    let (engagement, campaign) = fresh_scope();
    let planning = planning_json(&engagement, &campaign, 1, true);
    let body = format!(
        "{},\"secret\":\"S3NTINEL-B1A\"}}",
        planning.trim_end_matches('}')
    );
    let path = write_input(&body);
    let result = assess_cli(&Uuid::from_u128(0xf004).to_string(), &path, false, None);
    std::fs::remove_file(path).ok();
    let all = stdout(&result) + &String::from_utf8_lossy(&result.stderr);
    assert!(all.contains("schema_violation") && !all.contains("S3NTINEL"));
    assert!(!all.contains("missing_env"));
    let path = write_input("{");
    let result = assess_cli(&Uuid::from_u128(0xf004).to_string(), &path, false, None);
    std::fs::remove_file(path).ok();
    assert!(stdout(&result).contains("malformed_json"));
    let (engagement, campaign) = fresh_scope();
    let valid: serde_json::Value =
        serde_json::from_str(&planning_json(&engagement, &campaign, 1, true)).unwrap();
    for (field, value) in [
        ("engagement_id", serde_json::json!(Uuid::nil())),
        ("expected_mission_revision", serde_json::json!(0)),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        let path = write_input(&invalid.to_string());
        let result = assess_cli(&Uuid::from_u128(0xf004).to_string(), &path, false, None);
        std::fs::remove_file(path).ok();
        assert!(!result.status.success() && !stdout(&result).contains("missing_env"));
    }
}

#[test]
fn cli_refuses_bad_environment_before_connecting() {
    let _guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    let (e, c) = fresh_scope();
    let inspect = [
        "inspect",
        "--engagement",
        e.as_str(),
        "--campaign",
        c.as_str(),
    ];

    let o = cli(&inspect, None);
    assert!(stdout(&o).contains("missing_env"), "{}", stdout(&o));
    let o = cli(&inspect, Some("   "));
    assert!(stdout(&o).contains("missing_env"), "{}", stdout(&o));
    let o = cli(&inspect, Some("not a dsn at all"));
    assert!(stdout(&o).contains("invalid_dsn"), "{}", stdout(&o));
    let o = cli(&inspect, Some("postgres://u:p@10.0.0.9:5432/db"));
    assert!(stdout(&o).contains("non_loopback_host"), "{}", stdout(&o));
    let o = cli(&inspect, Some("postgres://u@127.0.0.1:5432/db"));
    assert!(stdout(&o).contains("missing_credentials"), "{}", stdout(&o));
}

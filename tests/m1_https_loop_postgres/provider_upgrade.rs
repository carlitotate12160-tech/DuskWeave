//! Actual Pro-v2 admission overlay (`0039`) over the delivered `0038` schema:
//! shared fixtures, catalog/ACL disposition, populated upgrade and key failure cases.
use crate::https_support;
use crate::terrain::terrain_support::native;
use postgres::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) const ACTUAL: &str = duskweave::m1_inference_profile::PRO_V2_PROFILE;
pub(crate) const FIXTURE: &str = duskweave::m1_inference_profile::FIXTURE_PRO_V2_PROFILE;
pub(crate) const ACTUAL_URL: &str = "https://api.deepseek.com/responses";
pub(crate) const FIXTURE_URL: &str = "http://127.0.0.1:18083/responses";
pub(crate) const BROKER: &str = "DW_INFERENCE_BROKER_DATABASE_URL";
const MATCHES: &str = "execution.inference_profile_matches";
const RUN_START: &str = "execution.run_start";
const SQL_0037: &str = include_str!("../../migrations/0037_https_loop.sql");
const SQL_0038: &str = include_str!("../../migrations/0038_https_lab.sql");
const SQL_0039: &str = include_str!("../../migrations/0039_https_provider_fixture.sql");

pub(crate) fn actual_pro_v2_registered_with(slot: u128, change: impl FnOnce(&mut Value)) -> Value {
    let mut q = https_support::registered_with(slot, |raw| {
        let p = &mut raw["m1_inference_permission"];
        (p["profile"], p["recipient"]) = (json!(ACTUAL), json!(ACTUAL_URL));
        change(raw);
    });
    q["inference_profile"] = json!(ACTUAL);
    q
}

pub(crate) fn live_binding() -> Value {
    json!({"qualification_ref": Uuid::from_u128(0xe0b1), "release_sha256": "a".repeat(64), "resolver_ipv4": "192.0.2.53"})
}

pub(crate) fn permission_json(profile: &str, recipient: &str) -> Value {
    json!({"version": "m1_https_loop_v1", "profile": profile, "recipient": recipient,
        "run_profile": "m1_https_loop_v1", "execution_mode": "controlled_fixture",
        "field_classes": ["admitted_campaign_decision_feedback", "admitted_repair_fields_codes"],
        "operation_limits": {"request_bytes": 16384, "output_tokens_per_call": 8192,
            "decision_wall_time_ms": 249000, "per_call_wall_time_ms": 122000,
            "soft_completion_ms": 120000, "shared_wall_time_ms": 244000,
            "phase_wall_time_ms": 254000, "cancel_grace_ms": 2000, "accounting_window_ms": 3000}})
}

pub(crate) fn lab_permission(mut p: Value) -> Value {
    p["run_profile"] = json!(duskweave::m1_inference_profile::HTTPS_LAB_RUN_PROFILE);
    p["execution_mode"] = json!("live");
    p["live_binding"] = live_binding();
    p
}

pub(crate) fn sql_matches(c: &mut Client, p: &Value) -> bool {
    let q = "SELECT execution.inference_profile_matches($1)";
    c.query_one(q, &[p]).unwrap().get(0)
}

fn statement(sql: &str, name: &str) -> String {
    let n = sql.replace("\r\n", "\n");
    let head = format!("CREATE OR REPLACE FUNCTION {name}(");
    let start = n.find(&head).unwrap_or_else(|| panic!("missing {name}"));
    let marker = n[start..].find("AS $$").expect("AS $$") + start + 5;
    let end = n[marker..].find("$$;").expect("$$;") + marker;
    n[start..end + 3].to_owned()
}

fn expected(sql: &str) -> [String; 2] {
    [MATCHES, RUN_START].map(|name| {
        let s = statement(sql, name);
        let body = &s[s.find("AS $$").unwrap() + 5..s.len() - 3];
        let digest = Sha256::digest(body.as_bytes());
        digest.iter().map(|b| format!("{b:02x}")).collect()
    })
}

fn catalog_row(a: &mut Client, function: &str) -> postgres::Row {
    let (ns, name) = function.split_once('.').unwrap();
    let sql = "SELECT p.prosecdef, p.provolatile::text, coalesce(p.proconfig, ARRAY[]::text[]), \
               has_function_privilege('public', p.oid, 'EXECUTE'), p.proowner, p.oid, \
               encode(sha256(convert_to(replace(p.prosrc, E'\\r\\n', E'\\n'), 'UTF8')), 'hex'), \
               p.prosrc ~ '(^|[^.])\\mpg_roles\\M' \
               FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
               WHERE n.nspname = $1 AND p.proname = $2";
    a.query_one(sql, &[&ns, &name]).unwrap()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Level {
    V0037,
    V0038,
    V0039,
}

pub(crate) fn level(a: &mut Client) -> Level {
    let q = "SELECT to_regclass('execution.live_https_results') IS NOT NULL";
    if !a.query_one(q, &[]).unwrap().get::<_, bool>(0) {
        return Level::V0037;
    }
    let now = [
        catalog_row(a, MATCHES).get::<_, String>(6),
        catalog_row(a, RUN_START).get::<_, String>(6),
    ];
    if now == expected(SQL_0039) {
        Level::V0039
    } else if now == expected(SQL_0038) {
        Level::V0038
    } else {
        panic!("unrecognized inference_profile_matches/run_start catalog bodies")
    }
}

pub(crate) struct Overlay {
    pub initial: Level,
    pub applied_0038: bool,
    pub applied_0039: bool,
}

pub(crate) fn ensure_0039_overlay() -> Overlay {
    let mut a = native::admin();
    let initial = level(&mut a);
    let applied_0038 = initial == Level::V0037;
    if applied_0038 {
        a.batch_execute(SQL_0038).unwrap();
    }
    let applied_0039 = initial != Level::V0039;
    if applied_0039 {
        a.batch_execute(SQL_0039).unwrap();
    }
    assert_eq!(level(&mut a), Level::V0039);
    Overlay {
        initial,
        applied_0038,
        applied_0039,
    }
}

fn ensure_0038_baseline() {
    let mut a = native::admin();
    match level(&mut a) {
        Level::V0037 => a.batch_execute(SQL_0038).unwrap(),
        Level::V0038 => {}
        Level::V0039 => [MATCHES, RUN_START]
            .into_iter()
            .for_each(|f| a.batch_execute(&statement(SQL_0038, f)).unwrap()),
    }
    assert_eq!(level(&mut a), Level::V0038);
}

fn product_counts(a: &mut Client) -> [i64; 4] {
    [
        "execution.autonomous_runs",
        "execution.https_loop_steps",
        "execution.inference_calls",
        "execution.native_input_bindings",
    ]
    .map(|t| {
        a.query_one(&format!("SELECT count(*) FROM {t}"), &[])
            .unwrap()
            .get(0)
    })
}

fn can_exec(a: &mut Client, role: &str, oid: u32) -> bool {
    let q = "SELECT has_function_privilege($1, $2::oid, 'EXECUTE')";
    a.query_one(q, &[&role, &oid]).unwrap().get(0)
}

#[ignore]
#[test]
fn overlay_0039_upgrade_and_catalog_disposition() {
    let _guard = native::qualified_existing_db();
    let mut a = native::admin();
    let before = product_counts(&mut a);
    let overlay = ensure_0039_overlay();
    let after = product_counts(&mut a);
    println!(
        "OVERLAY_0039_WITNESS initial_schema={:?} applied_0038={} applied_0039={} before={before:?} after={after:?}",
        overlay.initial, overlay.applied_0038, overlay.applied_0039
    );
    assert_eq!(before, after);
    let reference_owner: u32 = catalog_row(&mut a, "execution.run_current").get(4);
    for (name, definer, volatility, idx) in
        [(MATCHES, false, "i", 0usize), (RUN_START, true, "v", 1)]
    {
        let row = catalog_row(&mut a, name);
        let cfg: Vec<String> = row.get(2);
        let sha: String = row.get(6);
        assert_eq!(row.get::<_, bool>(0), definer, "{name} SECURITY DEFINER");
        assert_eq!(row.get::<_, String>(1), volatility, "{name} volatility");
        assert!(cfg.iter().any(|c| c == "search_path=pg_catalog, pg_temp"));
        assert!(!row.get::<_, bool>(3), "{name} PUBLIC EXECUTE");
        assert_eq!(row.get::<_, u32>(4), reference_owner, "{name} owner");
        assert!(!row.get::<_, bool>(7), "{name} unqualified pg_roles");
        assert_eq!(sha, expected(SQL_0039)[idx]);
        assert_ne!(sha, expected(SQL_0038)[idx]);
        assert_ne!(sha, expected(SQL_0037)[idx]);
        let oid: u32 = row.get(5);
        for role in [
            "dw_runtime",
            "dw_m1_cognition",
            "dw_m1_terrain",
            "dw_m1_trajectory",
            "dw_m1_stop_writer",
        ] {
            assert!(!can_exec(&mut a, role, oid), "{role} must not exec {name}");
        }
        assert!(can_exec(&mut a, "dw_m1_broker", oid), "broker exec {name}");
    }
    for var in [
        "DW_TEST_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
    ] {
        let mut c = native::client(var);
        for sql in [
            "SELECT execution.inference_profile_matches('{}'::jsonb)",
            "SELECT execution.run_start('{}'::jsonb, 'x')",
        ] {
            let err = c.query_one(sql, &[]).unwrap_err();
            assert_eq!(err.as_db_error().unwrap().code().code(), "42501", "{var}");
        }
    }
    let mut broker = native::client(BROKER);
    assert!(!sql_matches(&mut broker, &json!({})));
    let err = broker
        .query_one("SELECT execution.run_start('{}'::jsonb, 'x')", &[])
        .unwrap_err();
    assert_eq!(err.as_db_error().unwrap().message(), "run_request_refused");
}

#[derive(PartialEq)]
struct Snapshot {
    counts: [i64; 4],
    steps: Vec<(i32, String, String)>,
    dns: Vec<String>,
    head: Vec<String>,
    charged: Value,
    held: Value,
}

fn cap_pair(a: &mut Client, e: Uuid, c: Uuid) -> (Value, Value) {
    let s: Value = a
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    let h: Value = a
        .query_one(
            "SELECT execution.https_loop_protected_held($1,$2)",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    (s["charged"].clone(), h)
}

fn snapshot(a: &mut Client, run: Uuid, e: Uuid, c: Uuid) -> Snapshot {
    let steps = a
        .query("SELECT step_index, decision_id::text, status FROM execution.https_loop_steps WHERE run_id=$1 ORDER BY step_index", &[&run])
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect();
    let dns = a
        .query("SELECT request->>'source_kind' FROM execution.fixture_dns_results WHERE run_id=$1 ORDER BY step_index, attempt_kind", &[&run])
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    let head = a
        .query("SELECT result_payload->>'source_kind' FROM execution.fixture_https_results WHERE run_id=$1 ORDER BY step_index", &[&run])
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    let (charged, held) = cap_pair(a, e, c);
    Snapshot {
        counts: product_counts(a),
        steps,
        dns,
        head,
        charged,
        held,
    }
}

fn run_ids(q: &Value) -> (Uuid, Uuid, Uuid) {
    let m = &q["context"]["mission"];
    let p = |v: &Value| Uuid::parse_str(v.as_str().unwrap()).unwrap();
    (
        p(&q["run_id"]),
        p(&m["engagement_id"]),
        p(&m["campaign_id"]),
    )
}

#[ignore]
#[test]
fn populated_0038_to_0039_preserves_fixture_run_and_recoverability() {
    let _guard = native::qualified_existing_db();
    ensure_0038_baseline();
    let mut a = native::admin();
    let mut broker = native::client(BROKER);
    let (actual, fixture) = (
        permission_json(ACTUAL, ACTUAL_URL),
        permission_json(FIXTURE, FIXTURE_URL),
    );
    assert!(
        !sql_matches(&mut broker, &actual),
        "0038 must reject actual"
    );
    assert!(sql_matches(&mut broker, &fixture));

    let (_server, reqs) = https_support::start_server(https_support::principal_witness_proposal);
    let q = https_support::registered_with(0xe301, |_| {});
    let (run, e, c) = run_ids(&q);
    let (ok, receipt) = https_support::run_cli(&q);
    assert!(ok, "{receipt}");
    assert_eq!(
        (&receipt["status"], reqs.lock().unwrap().len()),
        (&json!("completed"), 4)
    );
    let id = run.to_string();
    let before = snapshot(&mut a, run, e, c);
    assert_eq!(before.dns, ["fixture_dns_acquisition"]);
    assert_eq!(before.head, ["fixture_https_acquisition"; 2]);
    assert_eq!(
        (&before.charged["calls"], &before.held["calls"]),
        (&json!(8), &json!(0))
    );
    assert_eq!(
        https_support::cli(&["m1-run-inspect", "--run", &id]).1,
        receipt
    );

    a.batch_execute(SQL_0039).unwrap();
    assert_eq!(level(&mut a), Level::V0039);
    assert!(sql_matches(&mut broker, &actual) && sql_matches(&mut broker, &fixture));

    let (ok1, r1) = https_support::cli(&["m1-run-recover", "--run", &id]);
    let (ok2, r2) = https_support::cli(&["m1-run-recover", "--run", &id]);
    let (ok3, r3) = https_support::cli(&["m1-run-inspect", "--run", &id]);
    assert!(ok1 && ok2 && ok3);
    assert_eq!((r1, r2, r3), (receipt.clone(), receipt.clone(), receipt));
    assert_eq!(reqs.lock().unwrap().len(), 4, "recovery makes zero calls");
    assert!(
        snapshot(&mut a, run, e, c) == before,
        "upgrade preserves rows"
    );
}

fn run_keyed(q: &Value, key: Option<&str>) -> (bool, Value) {
    let path = crate::run_support::path(q);
    let mut cmd = https_support::command();
    let args = [
        "m1-run",
        "--input",
        path.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ];
    cmd.args(args);
    if let Some(k) = key {
        cmd.env("DEEPSEEK_API_KEY", k);
    }
    let out = cmd.output().unwrap();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    let receipt = text
        .lines()
        .find(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).unwrap())
        .unwrap_or_else(|| json!({"stdout": text.trim(), "stderr": err.trim()}));
    (out.status.success(), receipt)
}

fn durable_counts(a: &mut Client, e: Uuid, c: Uuid, run: Uuid) -> [i64; 8] {
    let by_ec = [
        "SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",
        "SELECT count(*) FROM execution.inference_calls WHERE e=$1 AND c=$2",
        "SELECT count(*) FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2",
        "SELECT count(*) FROM trajectory.reasoning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
        "SELECT count(*) FROM cognition.reasoning_outbox o JOIN cognition.reasoning_decisions d ON d.event=o.event WHERE d.e=$1 AND d.c=$2",
        "SELECT count(*) FROM execution.inference_closures WHERE e=$1 AND c=$2",
    ]
    .map(|sql| a.query_one(sql, &[&e, &c]).unwrap().get(0));
    let by_run = [
        "execution.fixture_dns_attempts",
        "execution.fixture_https_attempts",
    ]
    .map(|t| {
        a.query_one(
            &format!("SELECT count(*) FROM {t} WHERE run_id=$1"),
            &[&run],
        )
        .unwrap()
        .get(0)
    });
    [
        by_ec[0], by_ec[1], by_ec[2], by_ec[3], by_run[0], by_run[1], by_ec[4], by_ec[5],
    ]
}

#[ignore]
#[test]
fn actual_pro_v2_cli_missing_and_invalid_key_fail_honestly_and_recover_key_free() {
    let _guard = native::qualified_existing_db();
    ensure_0039_overlay();
    let mut a = native::admin();
    for (slot, key, expected) in [
        (0xe401u128, None, "live_key_missing"),
        (0xe402, Some("synthetic-invalid\n"), "live_key_invalid"),
    ] {
        let q = actual_pro_v2_registered_with(slot, |_| {});
        let (run, e, c) = run_ids(&q);
        let (_server, reqs) =
            https_support::start_server(https_support::principal_witness_proposal);
        let (ok, receipt) = run_keyed(&q, key);
        assert!(!ok, "{expected}: m1-run must fail honestly: {receipt}");
        let decision = &receipt["recorded_decisions"][0];
        assert_eq!(decision["disposition"], expected, "{receipt}");
        assert!(decision["proposal"].is_null() && decision["dispatch_granted"] == false);
        assert_eq!(
            (&receipt["dns_effect_count"], &receipt["head_effect_count"]),
            (&json!(0), &json!(0))
        );
        let r = a
            .query_one("SELECT request->>'inference_profile', request->>'run_profile', request->>'execution_mode' FROM execution.autonomous_runs WHERE run_id=$1", &[&run])
            .unwrap();
        assert_eq!(
            (
                r.get::<_, String>(0),
                r.get::<_, String>(1),
                r.get::<_, String>(2)
            ),
            (
                ACTUAL.into(),
                "m1_https_loop_v1".into(),
                "controlled_fixture".into()
            )
        );
        let reserved = durable_counts(&mut a, e, c, run);
        println!("KEY_FAILURE_STATE {expected} durable_and_pending={reserved:?}");
        assert_eq!(reserved, [1, 0, 1, 1, 0, 0, 1, 1], "{expected}: durable");
        let kinds: Vec<String> = a
            .query("SELECT kind FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2 ORDER BY record_revision", &[&e, &c])
            .unwrap()
            .iter()
            .map(|r| r.get(0))
            .collect();
        assert_eq!(kinds, ["reserved_not_sent"]);
        let row = a
            .query_one("SELECT payload->>'disposition', payload->'bindings' FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2", &[&e, &c])
            .unwrap();
        assert_eq!(
            (row.get::<_, String>(0), row.get::<_, Value>(1)),
            (expected.to_owned(), json!([]))
        );
        let (charged, held) = cap_pair(&mut a, e, c);
        assert_eq!((&charged["calls"], &held["calls"]), (&json!(2), &json!(0)));

        let id = run.to_string();
        assert_eq!(
            https_support::cli(&["m1-run-inspect", "--run", &id]).1["run_id"],
            id.as_str()
        );
        let (ok1, r1) = https_support::cli(&["m1-run-recover", "--run", &id]);
        let (ok2, r2) = https_support::cli(&["m1-run-recover", "--run", &id]);
        let (ok3, r3) = https_support::cli(&["m1-run-inspect", "--run", &id]);
        assert!(ok1 && ok2 && ok3, "{r1} {r2} {r3}");
        assert_eq!(
            (&r2, &r3),
            (&r1, &r1),
            "{expected}: recovery must be stable"
        );
        assert_eq!(reqs.lock().unwrap().len(), 0, "{expected}: zero fallback");
        assert_eq!(
            durable_counts(&mut a, e, c, run),
            reserved,
            "{expected}: zero replay"
        );
        assert_eq!(cap_pair(&mut a, e, c), (charged, held));
    }
}

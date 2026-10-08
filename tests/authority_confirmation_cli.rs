//! Real-process controls for the CLI fresh-authority confirmation gate: a
//! saved `current_authority_confirmed=true` request assertion cannot
//! authorize a new assessment by itself. Only a live scoped
//! challenge/response in the current invocation may proceed, and it never
//! overrides Mission guards, revision checks or history semantics.

use duskweave::mission::{CampaignId, EngagementId};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Output};
use uuid::Uuid;

#[path = "support/authority_confirmation_cli.rs"]
mod confirm_support;
#[path = "support/registration_db.rs"]
mod db_support;
use confirm_support::{Finished, STATEMENT};
use db_support::*;

const DENIED: &[u8] = b"error=authority_confirmation_failed\n";

struct InputFile(PathBuf);

impl InputFile {
    fn new(label: &str, value: &Value) -> Self {
        let file = Self(std::env::temp_dir().join(format!("dw-m0c-c0-{label}.json")));
        std::fs::write(&file.0, value.to_string()).unwrap();
        file
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Non-interactive CLI invocation: stdin is at EOF from spawn.
fn cli(args: &[&str], dsn: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear().env("DW_DATABASE_CONFIG_MODE", "env-local");
    if let Some(dsn) = dsn {
        cmd.env("DW_DATABASE_URL", dsn);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.args(args).output().expect("spawn CLI")
}

fn assess_closed(operation: &str, input: &str, recover: bool, dsn: &str) -> Output {
    cli(
        &[
            "assess",
            "--operation",
            operation,
            "--input",
            input,
            "--recover",
            if recover { "true" } else { "false" },
        ],
        Some(dsn),
    )
}

fn history(operation: &str, input: &str, recover: bool, dsn: &str) -> Value {
    let out = cli(
        &[
            "planning-history",
            "--operation",
            operation,
            "--input",
            input,
            "--recover",
            if recover { "true" } else { "false" },
        ],
        Some(dsn),
    );
    assert!(out.status.success(), "planning-history failed");
    assert!(out.stderr.is_empty(), "unexpected CLI stderr");
    serde_json::from_slice(&out.stdout).unwrap()
}

fn prepare(e: &str, c: &str, dsn: &str) -> String {
    let out = cli(
        &["prepare-operation", "--engagement", e, "--campaign", c],
        Some(dsn),
    );
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .find_map(|s| s.strip_prefix("operation=").map(str::to_owned))
        .unwrap()
}

fn register(e: &str, c: &str, dsn: &str, window: Option<(i64, i64)>) {
    let mut body = reg_json(
        EngagementId::parse(e).unwrap(),
        CampaignId::parse(c).unwrap(),
    );
    if let Some((starts_at, ends_at)) = window {
        body["starts_at"] = json!(starts_at);
        body["ends_at"] = json!(ends_at);
    }
    let file = InputFile::new(&format!("reg-{e}"), &body);
    let op = prepare(e, c, dsn);
    let out = cli(
        &["register", "--operation", &op, "--input", file.path()],
        Some(dsn),
    );
    assert!(out.status.success(), "register failed");
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("history=completed")
    );
}

fn request_json(e: &EngagementId, c: &CampaignId, confirmed: bool) -> Value {
    json!({
        "engagement_id": e.0, "campaign_id": c.0,
        "purpose_ref": Uuid::from_u128(0x13),
        "asset_ref": Uuid::from_u128(0x21),
        "expected_mission_revision": 1,
        "current_authority_confirmed": confirmed,
    })
}

fn denied(finished: &Finished) {
    assert!(!finished.status.success());
    assert_eq!(finished.stdout, DENIED);
    assert!(finished.stderr_tail.is_empty(), "stderr tail must be empty");
}

fn no_effects(e: EngagementId, c: CampaignId) {
    assert_eq!(count("mission.planning_assessments", e, c), 0);
    assert_eq!(count("trajectory.planning_history", e, c), 0);
}

#[test]
fn saved_true_without_live_confirmation_fails_without_effects() {
    let _guard = db();
    let (e, c) = scope(0xc0a1);
    let (mut a, mut s, mut t) = ports();
    accepted(&mut a, &mut s, &mut t, e, c);
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let (es, cs) = (e.to_string(), c.to_string());
    let op = prepare(&es, &cs, &dsn);
    let file = InputFile::new(&op, &request_json(&e, &c, true));
    let canary = "C0_REJECTED_RESPONSE_CANARY";

    // Absent response: stdin at EOF before any bytes.
    let out = assess_closed(&op, file.path(), false, &dsn);
    assert!(!out.status.success());
    assert_eq!(out.stdout, DENIED);
    let challenge: Value =
        serde_json::from_str(std::str::from_utf8(&out.stderr).unwrap().trim_end())
            .expect("stderr carries exactly one JSON challenge line");
    assert_eq!(challenge["action"], "confirm_current_authority");
    assert_eq!(challenge["engagement_id"], es);
    assert_eq!(challenge["campaign_id"], cs);
    assert_eq!(challenge["operation_id"], op);
    assert_eq!(challenge["expected_mission_revision"], 1);
    assert_eq!(challenge["statement"], STATEMENT);
    assert_ne!(
        Uuid::parse_str(challenge["challenge_id"].as_str().unwrap()).unwrap(),
        Uuid::nil()
    );
    no_effects(e, c);

    // Every malformed/dishonest response fails the same bounded way: wrong
    // or nil or unparseable challenge id, wrong statement, unknown fields,
    // malformed bytes, an oversized line (>1024 including the line ending),
    // and a non-object body. The canary marks rejected bytes that must never
    // echo into stdout/stderr or storage.
    for payload in [
        json!({"challenge_id": Uuid::from_u128(0xdead), "statement": STATEMENT}).to_string(),
        json!({"challenge_id": Uuid::nil(), "statement": STATEMENT}).to_string(),
        json!({"challenge_id": "not-a-uuid", "statement": STATEMENT}).to_string(),
        json!({"challenge_id": "placeholder", "statement": canary}).to_string(),
        json!({"challenge_id": "placeholder", "statement": STATEMENT, "note": canary}).to_string(),
        format!("{{\"challenge_id\":\"{canary}\""),
        format!(
            "{{\"challenge_id\":\"x\",\"statement\":\"{}\",\"pad\":\"{}\"}}",
            canary,
            "x".repeat(1100)
        ),
        format!("[\"{canary}\"]"),
    ] {
        let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
        let live = session.challenge();
        let body = payload.replace(
            "\"placeholder\"",
            &json!(live["challenge_id"].as_str().unwrap()).to_string(),
        );
        session.respond(body.as_bytes());
        let finished = session.finish();
        denied(&finished);
        for surface in [
            &finished.stdout,
            &finished.challenge_line,
            &finished.stderr_tail,
        ] {
            assert!(
                !String::from_utf8_lossy(surface).contains(canary),
                "rejected bytes echoed"
            );
        }
    }

    // A response missing its line ending is EOF-incomplete, not accepted.
    let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
    let live = session.challenge();
    session.write_raw(
        json!({"challenge_id": live["challenge_id"], "statement": STATEMENT})
            .to_string()
            .as_bytes(),
    );
    denied(&session.finish());
    no_effects(e, c);

    // Failed attempts leave no durable residue: the same operation proceeds
    // under a fresh live confirmation.
    let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
    session.affirm(&es, &cs, &op, 1);
    let finished = session.finish();
    assert!(finished.status.success());
    assert!(finished.stderr_tail.is_empty());
    let receipt: Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_eq!(receipt["result"], "durable");
    assert_eq!(receipt["current_permission"], false);
    assert_eq!(count("mission.planning_assessments", e, c), 1);
}

#[test]
fn live_confirmation_publishes_and_recovers_through_fresh_processes() {
    let _guard = db();
    let (e, c) = scope(0xc0b2);
    let (es, cs) = (e.to_string(), c.to_string());
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    register(&es, &cs, &dsn, None);
    let op = prepare(&es, &cs, &dsn);
    let file = InputFile::new(&op, &request_json(&e, &c, true));

    let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
    session.affirm(&es, &cs, &op, 1);
    let finished = session.finish();
    assert!(finished.status.success());
    assert!(finished.stderr_tail.is_empty());
    let assessed: Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_eq!(assessed["result"], "durable");
    assert_eq!(assessed["contract"]["version"], 2);
    assert_eq!(assessed["contract"]["operation_id"], op);
    assert_eq!(assessed["contract"]["request"], request_json(&e, &c, true));
    assert_eq!(assessed["decision_origin"], "durable_record");
    assert_eq!(assessed["history"], "pending");
    assert_eq!(assessed["history_reason"], "not_published_at_decision");
    assert_eq!(assessed["complete_assessment"], false);
    assert_eq!(assessed["current_permission"], false);
    assert_eq!(assessed["scope"], "matched");
    assert_eq!(assessed["window"], "expired");

    let pending = history(&op, file.path(), true, &dsn);
    assert_eq!(pending["history"], "pending");
    assert_eq!(count("trajectory.planning_history", e, c), 0);
    let completed = history(&op, file.path(), false, &dsn);
    assert_eq!(completed["contract"], assessed["contract"]);
    assert_eq!(completed["history"], "completed");
    assert_eq!(completed["complete_history"], true);
    assert_eq!(completed["complete_assessment"], false);
    assert_eq!(completed["current_permission"], false);
    let restarted = history(&op, file.path(), true, &dsn);
    assert_eq!(restarted, completed);

    // Historic duplicates and recovery need no interaction and return the
    // unchanged producer receipt on a fresh process.
    for recover in [false, true] {
        let out = assess_closed(&op, file.path(), recover, &dsn);
        assert!(out.status.success());
        assert!(out.stderr.is_empty(), "historic path emits no challenge");
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap(),
            assessed
        );
    }
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    assert_eq!(
        count_where("trajectory.planning_history", "AND status='accepted'", e, c),
        1
    );
}

#[test]
fn a_response_from_another_invocation_cannot_authorize_this_one() {
    let _guard = db();
    let (e, c) = scope(0xc0c3);
    let (mut a, mut s, mut t) = ports();
    accepted(&mut a, &mut s, &mut t, e, c);
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let (es, cs) = (e.to_string(), c.to_string());
    let file = InputFile::new("replay", &request_json(&e, &c, true));

    let first_op = prepare(&es, &cs, &dsn);
    let mut first = confirm_support::assess_session(&first_op, file.path(), false, &dsn);
    let first_challenge = first.affirm(&es, &cs, &first_op, 1);
    let finished = first.finish();
    assert!(finished.status.success());
    let original: Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_eq!(original["result"], "durable");

    // A new process and new operation carrying the same saved JSON boolean
    // receives a fresh challenge; the old response cannot satisfy it.
    let second_op = prepare(&es, &cs, &dsn);
    let mut second = confirm_support::assess_session(&second_op, file.path(), false, &dsn);
    let second_challenge = second.challenge();
    assert_eq!(second_challenge["operation_id"], second_op);
    assert_ne!(
        second_challenge["challenge_id"],
        first_challenge["challenge_id"]
    );
    second.respond(
        json!({"challenge_id": first_challenge["challenge_id"], "statement": STATEMENT})
            .to_string()
            .as_bytes(),
    );
    denied(&second.finish());
    assert_eq!(count("mission.planning_assessments", e, c), 1);

    // The same operation may proceed under its own live confirmation.
    let mut retry = confirm_support::assess_session(&second_op, file.path(), false, &dsn);
    retry.affirm(&es, &cs, &second_op, 1);
    let finished = retry.finish();
    assert!(finished.status.success());
    let second_receipt: Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_eq!(second_receipt["result"], "durable");
    assert_eq!(second_receipt["contract"]["operation_id"], second_op);
    assert_eq!(count("mission.planning_assessments", e, c), 2);

    // Both durable originals replay identically with no interaction.
    for (op, receipt) in [(&first_op, &original), (&second_op, &second_receipt)] {
        let out = assess_closed(op, file.path(), false, &dsn);
        assert!(out.status.success());
        assert!(out.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap(),
            *receipt
        );
    }
}

#[test]
fn confirmation_never_overrides_mission_guards_or_receipt_flags() {
    let _guard = db();
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let now: i64 = runtime_client()
        .query_one("SELECT floor(extract(epoch FROM now()))::bigint", &[])
        .unwrap()
        .get(0);

    // Each case gets a confirmed live exchange yet still lands on its
    // nonpositive guard outcome; current_permission stays false.
    let cases = [
        // (window, overrides, decision)
        (
            Some((now - 7_200, now + 7_200)),
            json!({"expected_mission_revision": 2}),
            "refused_revision_mismatch",
        ),
        (
            Some((now - 7_200, now + 7_200)),
            json!({"purpose_ref": Uuid::from_u128(0xc0d4)}),
            "refused_purpose_mismatch",
        ),
        (
            Some((now - 7_200, now + 7_200)),
            json!({"asset_ref": Uuid::from_u128(0x23)}),
            "refused_asset_excluded",
        ),
        (
            Some((now - 7_200, now + 7_200)),
            json!({"asset_ref": Uuid::from_u128(0xc0d5)}),
            "refused_asset_unknown",
        ),
        (
            Some((now - 14_400, now - 7_200)),
            json!({}),
            "refused_expired",
        ),
        (
            Some((now + 7_200, now + 14_400)),
            json!({}),
            "refused_not_yet_valid",
        ),
        // No Mission basis at all: confirmation does not conjure one.
        (None, json!({}), "unresolved_mission_basis"),
    ];
    for (i, (window, overrides, decision)) in cases.iter().enumerate() {
        let (e, c) = scope(0xc0e0 + i as u128);
        let (es, cs) = (e.to_string(), c.to_string());
        if let Some(window) = window {
            register(&es, &cs, &dsn, Some(*window));
        }
        let mut body = request_json(&e, &c, true);
        for (key, value) in overrides.as_object().unwrap() {
            body[key] = value.clone();
        }
        let file = InputFile::new(&format!("guards-{i}"), &body);
        let op = prepare(&es, &cs, &dsn);
        let mut session = confirm_support::assess_session(&op, file.path(), false, &dsn);
        let revision = body["expected_mission_revision"].as_u64().unwrap();
        session.affirm(&es, &cs, &op, revision);
        let finished = session.finish();
        assert!(finished.status.success());
        let receipt: Value = serde_json::from_slice(&finished.stdout).unwrap();
        assert_eq!(receipt["contract"]["decision"], *decision, "case {i}");
        assert_eq!(receipt["complete_assessment"], false);
        assert_eq!(receipt["current_permission"], false);
        assert_eq!(count("mission.planning_assessments", e, c), 1);
    }

    // A saved false keeps the existing unconfirmed path: no challenge is
    // emitted and the guard ordering is unchanged.
    let (e, c) = scope(0xc0f0);
    let (es, cs) = (e.to_string(), c.to_string());
    register(&es, &cs, &dsn, Some((now - 7_200, now + 7_200)));
    let file = InputFile::new("unconfirmed", &request_json(&e, &c, false));
    let op = prepare(&es, &cs, &dsn);
    let out = assess_closed(&op, file.path(), false, &dsn);
    assert!(out.status.success());
    assert!(out.stderr.is_empty(), "false requests emit no challenge");
    let receipt: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        receipt["contract"]["decision"],
        "unresolved_authority_unconfirmed"
    );
    assert_eq!(receipt["current_permission"], false);
}

#[test]
fn configuration_or_store_failures_create_no_effects() {
    let _guard = db();
    let (e, c) = scope(0xc101);
    let (mut a, mut s, mut t) = ports();
    accepted(&mut a, &mut s, &mut t, e, c);
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let (es, cs) = (e.to_string(), c.to_string());
    let op = prepare(&es, &cs, &dsn);
    let file = InputFile::new(&op, &request_json(&e, &c, true));

    // Missing configuration fails before any challenge or effect.
    let out = cli(
        &[
            "assess",
            "--operation",
            &op,
            "--input",
            file.path(),
            "--recover",
            "false",
        ],
        None,
    );
    assert!(!out.status.success());
    assert_eq!(out.stdout, b"error=missing_env\n");
    assert!(out.stderr.is_empty(), "no challenge without configuration");
    no_effects(e, c);

    // A store connection failure likewise fails before the challenge.
    let role = db_support::dsn("DW_TEST_DATABASE_URL")
        .get_user()
        .unwrap()
        .to_owned();
    assert!(
        role.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    );
    let mut administrator = admin_client();
    administrator
        .batch_execute(&format!("ALTER ROLE {role} CONNECTION LIMIT 1"))
        .unwrap();
    let out = assess_closed(&op, file.path(), false, &dsn);
    administrator
        .batch_execute(&format!("ALTER ROLE {role} CONNECTION LIMIT -1"))
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.stdout, b"error=connect_failed\n");
    assert!(out.stderr.is_empty(), "no challenge when a port fails");
    no_effects(e, c);
}

//! Durable, bounded proposal-admission diagnostics through the real CLI.
use super::fixture::*;
use super::run_support::*;
use super::successor_episode::{cli, command, control_root};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

fn snapshot(e: Uuid, c: Uuid) -> Value {
    admin().query_one(
        "SELECT jsonb_build_object('calls',(SELECT jsonb_agg(jsonb_build_array(call,binding,state,usage) ORDER BY call) FROM execution.inference_calls WHERE e=$1 AND c=$2),'decisions',(SELECT jsonb_agg(payload ORDER BY d) FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2),'effects',(SELECT count(*) FROM execution.fixture_dns_results WHERE run_id IN (SELECT run_id FROM execution.autonomous_runs WHERE e=$1 AND c=$2)))",
        &[&e,&c],
    ).unwrap().get(0)
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_proposal_diagnostics_survive_terminal_repair_and_recovery() {
    assert!(worker());
    let (e, c, _) = register_live(0x6e31, false);
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    let request = successor_request(&publication, run, "m1_autonomous_run_request_v3");
    let input = std::env::temp_dir().join(format!("dw-proposal-diagnostic-{run}.json"));
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let calls = AtomicUsize::new(0);
    let server = genai::Server::lab(move |r| {
        let wire: Value = serde_json::from_slice(&r.body).unwrap();
        let pack = genai::pack_in(&wire).unwrap();
        let mut proposal = genai::proposal(&pack);
        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            proposal.as_object_mut().unwrap().remove("hypothesis");
        } else {
            proposal["synthetic-secret-probe"] = json!("synthetic-private-value");
        }
        genai::Reply::Length(
            200,
            serde_json::to_vec(&genai::envelope(&proposal)).unwrap(),
        )
    });
    let root = control_root();
    let output = command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args([
            "m1-run",
            "--input",
            input.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let receipt: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|line| line.starts_with('{'))
            .expect("terminal receipt JSON"),
    )
    .unwrap();
    assert!(
        !output.status.success(),
        "invalid repair must not become success; {}",
        closed_diagnosis(&receipt)
    );
    let decisions = receipt["recorded_decisions"].as_array();
    assert!(
        decisions.is_some_and(|items| items.len() == 1),
        "exactly one recorded decision expected; {}",
        closed_diagnosis(&receipt)
    );
    let decision = &decisions.unwrap()[0];
    assert_eq!(
        decision["disposition"],
        "schema_repair_exhausted",
        "unexpected terminal outcome; {}",
        closed_diagnosis(&receipt)
    );
    assert!(
        decision["proposal"].is_null(),
        "exhausted repair keeps the proposal null; {}",
        closed_diagnosis(&receipt)
    );
    assert_eq!(receipt["effect_count"], 0);
    assert_eq!(server.count(), 2, "generation and one eligible repair only");
    let rows = admin().query(
        "SELECT configuration::jsonb->>'call_role',usage,call,binding FROM execution.inference_calls WHERE e=$1 AND c=$2 ORDER BY configuration::jsonb->>'call_role'",
        &[&e,&c],
    ).unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let role: String = row.get(0);
        let usage: Value = row.get(1);
        let call: Uuid = row.get(2);
        let binding: String = row.get(3);
        let reported = receipt["inference"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["call"] == json!(call))
            .unwrap();
        assert_eq!(
            reported["reported"], usage,
            "receipt lost diagnostic for {role}"
        );
        let provenance: Value = admin().query_one(
            "SELECT payload->'provenance' FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2", &[&e,&c],
        ).unwrap().get(0);
        let recorded = provenance
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["binding"] == binding)
            .unwrap();
        assert_eq!(
            recorded["reported"], usage,
            "provenance lost diagnostic for {role}"
        );
        let d = &usage["proposal_diagnostic"];
        assert_eq!(
            d["version"], "proposal_admission_diagnostic_v1",
            "durable diagnostic missing for {role}"
        );
        assert_eq!(d["reason"], "proposal_shape");
        let expected = if role == "generation" {
            json!({"kind":"missing_field","field":"hypothesis"})
        } else {
            assert_eq!(role, "repair");
            json!({"kind":"unknown_field","field":"unclassified"})
        };
        assert_eq!(d["shape"], expected);
        assert!(!usage.to_string().contains("synthetic-secret-probe"));
        assert!(!usage.to_string().contains("synthetic-private-value"));
    }
    reject_unsafe_metadata(e, c);
    missing_capability_preserves_evidence(e, c);
    let before = snapshot(e, c);
    for op in ["m1-run-inspect", "m1-run-recover"] {
        let (ok, receipt) = cli(op, run);
        assert!(ok, "{op} failed");
        assert_eq!(receipt["run_id"], json!(run));
        assert_eq!(
            snapshot(e, c),
            before,
            "{op} changed durable diagnostic identity"
        );
        assert_eq!(server.count(), 2);
    }
    std::fs::remove_file(input).unwrap();
}

/// Bounded failure diagnosis for the terminal assertions: closed status/count
/// plus validated `PhaseDiagnostic` fields only. Proposal/body fields, raw
/// output and unvalidated records never render; unknown content degrades to
/// closed categories.
pub(super) fn closed_diagnosis(receipt: &Value) -> String {
    let count = receipt["recorded_decisions"]
        .as_array()
        .map(|items| items.len().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "status={} recorded_decisions={} diagnostics={} pending_diagnostics={}",
        closed_token(&receipt["status"]),
        count,
        diagnostic_list(receipt.get("diagnostics")),
        diagnostic_list(receipt.get("pending_diagnostics")),
    )
}

fn closed_token(value: &Value) -> String {
    match value.as_str() {
        Some(text)
            if !text.is_empty()
                && text.len() <= 64
                && text
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') =>
        {
            text.to_string()
        }
        _ => "unavailable".to_string(),
    }
}

fn diagnostic_list(raw: Option<&Value>) -> String {
    match raw {
        None => "absent".to_string(),
        Some(Value::Array(items)) => {
            if items.is_empty() {
                return "[]".to_string();
            }
            let rendered: Vec<String> = items.iter().map(diagnostic_entry).collect();
            format!("[{rendered}]", rendered = rendered.join(";"))
        }
        Some(_) => "malformed".to_string(),
    }
}

fn diagnostic_entry(raw: &Value) -> String {
    let parsed = serde_json::from_value::<duskweave::m1_run::PhaseDiagnostic>(raw.clone());
    match parsed.ok().filter(|d| d.validate().is_ok()) {
        Some(d) => format!(
            "{}:{}:{}:ready={}:elapsed={}:err={}",
            name_of(&d.phase),
            name_of(&d.cause),
            name_of(&d.cause_source),
            d.readiness_observed,
            d.elapsed_ms
                .map(|ms| ms.to_string())
                .unwrap_or_else(|| "-".to_string()),
            d.child_error.as_deref().unwrap_or("-"),
        ),
        None => "unvalidated".to_string(),
    }
}

fn name_of(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unavailable".to_string())
}

#[test]
fn closed_diagnosis_reports_validated_fields_and_degrades_unknown_content() {
    let run = Uuid::new_v4();
    let diagnostic = json!({
        "version":"m1_run_phase_diagnostic_v2","run_id":run,"phase":"decision_one",
        "cause":"readiness_deadline","cause_source":"parent_observed",
        "child_error":null,"exit_code":null,"readiness_observed":false,"elapsed_ms":5022});
    let receipt = json!({"status":"unknown","recorded_decisions":[],
        "diagnostics":[diagnostic,{"version":"m1_run_phase_diagnostic_v2","run_id":run,
            "phase":"decision_one","cause":"child_error","cause_source":"child_report",
            "child_error":"synthetic-private-value","exit_code":null,
            "readiness_observed":false,"elapsed_ms":1}],
        "pending_diagnostics":{"nested":true}});
    let text = closed_diagnosis(&receipt);
    assert!(text.contains("status=unknown"));
    assert!(text.contains("recorded_decisions=0"));
    assert!(text.contains(
        "decision_one:readiness_deadline:parent_observed:ready=false:elapsed=5022:err=-"
    ));
    assert!(text.contains("unvalidated"));
    assert!(text.contains("pending_diagnostics=malformed"));
    assert!(!text.contains("synthetic-private-value"));
}

#[test]
fn closed_diagnosis_tolerates_missing_and_malformed_sections() {
    let receipt = json!({"status":"has unsafe extra text","recorded_decisions":{},
        "diagnostics":[{"unexpected":"shape"}]});
    let text = closed_diagnosis(&receipt);
    assert!(text.contains("status=unavailable"));
    assert!(text.contains("recorded_decisions=unavailable"));
    assert!(text.contains("diagnostics=[unvalidated]"));
    assert!(text.contains("pending_diagnostics=absent"));
    assert!(!text.contains("unsafe extra text"));
    let receipt = json!({"diagnostics":"not-an-array","pending_diagnostics":[]});
    let text = closed_diagnosis(&receipt);
    assert!(text.contains("diagnostics=malformed"));
    assert!(text.contains("pending_diagnostics=[]"));
}

fn reject_unsafe_metadata(e: Uuid, c: Uuid) {
    let row = admin().query_one(
        "SELECT call,usage FROM execution.inference_calls WHERE e=$1 AND c=$2 ORDER BY call LIMIT 1",
        &[&e,&c],
    ).unwrap();
    let call: Uuid = row.get(0);
    let usage: Value = row.get(1);
    let mut mutations = Vec::new();
    let mut q = usage.clone();
    q["proposal_diagnostic"]["raw"] = json!("private-value");
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["version"] = json!("untrusted-version");
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["shape"]["field"] = json!("synthetic-secret-probe");
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["shape"]["kind"] = json!("untrusted-kind");
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["reason"] = json!("json_structure");
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["private_extra"] = json!("private-value");
    mutations.push(("completed", q));
    mutations.push(("usage_unknown", usage.clone()));
    let mut q = usage.clone();
    q["proposal_diagnostic"]
        .as_object_mut()
        .unwrap()
        .remove("reason");
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["shape"] = json!([]);
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["shape"]["kind"] = json!(9);
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["shape"]["field"] = json!("x".repeat(600));
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["proposal_diagnostic"]["shape"] = json!({"kind":"invalid_type","field":"hypothesis"});
    mutations.push(("completed", q));
    let mut q = usage.clone();
    q["input_tokens"] = json!("private-value");
    mutations.push(("completed", q));
    for value in [
        serde_json::Value::Null,
        json!(9),
        json!("synthetic-unknown-model"),
        json!("probe-fixture"),
    ] {
        let mut q = usage.clone();
        q["requested_model"] = value;
        mutations.push(("completed", q));
    }
    let before = snapshot(e, c);
    for (state, q) in mutations {
        let error = broker()
            .query_one(
                "SELECT execution.inference_call_state($1,$2,$3)",
                &[&call, &state, &q],
            )
            .unwrap_err();
        assert_eq!(
            error.code().map(postgres::error::SqlState::code),
            Some("P0001")
        );
        assert_eq!(db_message(&error), "inference_usage_refused");
        assert_eq!(
            snapshot(e, c),
            before,
            "invalid diagnostic changed durable evidence"
        );
    }
    let mut valid_change = usage.clone();
    valid_change["proposal_diagnostic"] = json!({"version":"proposal_admission_diagnostic_v1","reason":"json_structure","shape":null});
    let error = broker()
        .query_one(
            "SELECT execution.complete_inference_proposal($1,$2)",
            &[&call, &valid_change],
        )
        .unwrap_err();
    assert_eq!(db_message(&error), "inference_state_conflict");
    assert_eq!(snapshot(e, c), before, "terminal diagnostic was amended");
    for variable in [
        "DW_TEST_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
    ] {
        let error = client(variable)
            .query_one(
                "SELECT execution.complete_inference_proposal($1,$2)",
                &[&call, &usage],
            )
            .unwrap_err();
        assert_eq!(
            error.code().map(postgres::error::SqlState::code),
            Some("42501")
        );
    }
}

fn missing_capability_preserves_evidence(e: Uuid, c: Uuid) {
    let row = admin()
        .query_one(
            "SELECT call,usage FROM execution.inference_calls WHERE e=$1 AND c=$2 LIMIT 1",
            &[&e, &c],
        )
        .unwrap();
    let call: Uuid = row.get(0);
    let usage: Value = row.get(1);
    let before = snapshot(e, c);
    let mut owner = admin();
    owner.batch_execute("ALTER FUNCTION execution.complete_inference_proposal(uuid,jsonb) RENAME TO diag_capability_hidden").unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        broker().query_one(
            "SELECT execution.complete_inference_proposal($1,$2)",
            &[&call, &usage],
        )
    }));
    owner.batch_execute("ALTER FUNCTION execution.diag_capability_hidden(uuid,jsonb) RENAME TO complete_inference_proposal").unwrap();
    let error = result.expect("capability probe panicked").unwrap_err();
    assert_eq!(
        error.code().map(postgres::error::SqlState::code),
        Some("42883")
    );
    assert_eq!(snapshot(e, c), before, "missing capability wrote evidence");
}

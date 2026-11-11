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
    assert!(
        !output.status.success(),
        "invalid repair must not become success"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let receipt: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|line| line.starts_with('{'))
            .expect("terminal receipt JSON"),
    )
    .unwrap();
    assert_eq!(
        receipt["recorded_decisions"][0]["disposition"],
        "schema_repair_exhausted"
    );
    assert!(receipt["recorded_decisions"][0]["proposal"].is_null());
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

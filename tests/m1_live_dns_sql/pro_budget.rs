//! Schema-0035 Pro-v2 and companion mirror SQL admission, diagnostics and real-CLI end-to-end tests.
#[path = "pro_budget/accounting.rs"]
mod accounting;
#[path = "pro_budget/capacity.rs"]
mod capacity;
#[path = "pro_budget/diagnostic.rs"]
mod diagnostic;
#[path = "pro_budget/live_binding.rs"]
mod live_binding;
use super::fixture::*;
use super::run_support::*;
use super::successor_episode::{cli, command, control_root, run_identities};
use duskweave::m1_inference_profile::{
    FIXTURE_PRO_V2_PROFILE, LAB_RUN_PROFILE, LIVE_RUN_PROFILE, PRO_V2_PROFILE,
    PRO_V2_TIMING_PROFILE, RUN_PROFILE,
};
use duskweave::m1_reasoning::{inference::Broker, inference_intent};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

const MIRROR_ENDPOINT: &str = "http://127.0.0.1:18083/responses";
const PRO_ENDPOINT: &str = "https://api.deepseek.com/responses";
const PRO_MODEL: &str = "deepseek-v4-pro";

fn set_pro_v2_limits(p: &mut Value) {
    let op = p["operation_limits"].as_object_mut().unwrap();
    for (k, v) in [
        ("output_tokens_per_call", 8192),
        ("request_bytes", 16384),
        ("response_bytes", 65536),
        ("soft_completion_ms", 120000),
        ("per_call_wall_time_ms", 122000),
        ("shared_wall_time_ms", 244000),
        ("decision_wall_time_ms", 249000),
        ("phase_wall_time_ms", 254000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        op.insert(k.into(), json!(v));
    }
    let cl = p["campaign_limits"].as_object_mut().unwrap();
    for (k, v) in [
        ("calls", 4),
        ("output_token_capacity", 32768),
        ("wall_time_ms", 498000),
        ("request_bytes", 65536),
        ("response_bytes", 262144),
    ] {
        cl.insert(k.into(), json!(v));
    }
}

fn register_pro_v2(slot: u128, profile: &str) -> (Uuid, Uuid, Uuid) {
    let endpoint = if profile == FIXTURE_PRO_V2_PROFILE {
        MIRROR_ENDPOINT
    } else {
        PRO_ENDPOINT
    };
    register_shaped(slot, |raw| {
        let p = &mut raw["m1_inference_permission"];
        p["profile"] = json!(profile);
        p["recipient"] = json!(endpoint);
        set_pro_v2_limits(p);
    })
}

fn sample_permission(version: &str, profile: &str, run_profile: &str, mode: &str) -> Value {
    let (e, c) = super::db_support::scope(0x73500);
    let mut raw = super::consent::registration(e, c);
    super::genai::configure_permission(&mut raw);
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!(version);
    p["profile"] = json!(profile);
    p["recipient"] = json!(if profile.starts_with("fixture") {
        MIRROR_ENDPOINT
    } else {
        PRO_ENDPOINT
    });
    p["run_profile"] = json!(run_profile);
    p["execution_mode"] = json!(mode);
    set_pro_v2_limits(p);
    if version == "m1_inference_permission_v6" {
        p["live_binding"] = json!({
            "qualification_ref": Uuid::from_u128(0x99),
            "release_sha256": RELEASE,
            "resolver_ipv4": if run_profile == LAB_RUN_PROFILE { "192.0.2.53" } else { "1.1.1.1" }
        });
    }
    p.clone()
}

fn sql_matches(p: &Value) -> bool {
    broker()
        .query_one("SELECT execution.inference_profile_matches($1)", &[p])
        .unwrap()
        .get(0)
}

fn diagnostic_payload(run: Uuid, elapsed_ms: i64) -> Value {
    json!({
        "version": "m1_run_phase_diagnostic_v2",
        "run_id": run,
        "phase": "decision_one",
        "cause": "phase_deadline",
        "cause_source": "parent_observed",
        "child_error": null,
        "exit_code": 124,
        "readiness_observed": false,
        "elapsed_ms": elapsed_ms,
        "reported_guard": "command_watchdog"
    })
}

fn check_sql_admission_and_mixed_matrix() {
    let mirror_v5 = sample_permission(
        "m1_inference_permission_v5",
        FIXTURE_PRO_V2_PROFILE,
        RUN_PROFILE,
        "controlled_fixture",
    );
    let pro_v5 = sample_permission(
        "m1_inference_permission_v5",
        PRO_V2_PROFILE,
        RUN_PROFILE,
        "controlled_fixture",
    );
    let pro_v6_live = sample_permission(
        "m1_inference_permission_v6",
        PRO_V2_PROFILE,
        LIVE_RUN_PROFILE,
        "live",
    );
    assert!(sql_matches(&mirror_v5));
    assert!(sql_matches(&pro_v5));
    assert!(sql_matches(&pro_v6_live));

    let pro_v6_lab = sample_permission(
        "m1_inference_permission_v6",
        PRO_V2_PROFILE,
        LAB_RUN_PROFILE,
        "live",
    );
    let mirror_v6_live = sample_permission(
        "m1_inference_permission_v6",
        FIXTURE_PRO_V2_PROFILE,
        LIVE_RUN_PROFILE,
        "live",
    );
    assert!(!sql_matches(&pro_v6_lab));
    assert!(!sql_matches(&mirror_v6_live));

    for version in [
        "m1_inference_permission_v1",
        "m1_inference_permission_v2",
        "m1_inference_permission_v3",
        "m1_inference_permission_v4",
    ] {
        for profile in [PRO_V2_PROFILE, FIXTURE_PRO_V2_PROFILE] {
            assert!(!sql_matches(&sample_permission(
                version,
                profile,
                RUN_PROFILE,
                "controlled_fixture"
            )));
        }
    }

    let mut mixed_pro = mirror_v5.clone();
    mixed_pro["operation_limits"]["output_tokens_per_call"] = json!(4096);
    assert!(!sql_matches(&mixed_pro));
    let mut mixed_v1 = mirror_v5.clone();
    mixed_v1["profile"] = json!("fixture-genai-responses-v1");
    assert!(!sql_matches(&mixed_v1));
}

fn check_sql_window_intent_and_diagnostics() {
    let (e_v2, c_v2, _) = register_pro_v2(0x73501, FIXTURE_PRO_V2_PROFILE);
    let run_v2 = Uuid::new_v4();
    let pub_v2 = live_publication(e_v2, c_v2, run_v2);
    let mut runs = duskweave::postgres_m1_run::PgRunStore::new(broker());
    let req_v2 = successor_request(&pub_v2, run_v2, "m1_autonomous_run_request_v3");
    assert!(runs.start(&req_v2, RELEASE).unwrap());
    let rec_v2 = runs.inspect(run_v2).unwrap();
    let renewals_v2 = Renewals::start(run_v2);
    let prep_v2 = prepare_decision_one(&mut runs, &rec_v2);
    assert_eq!(prep_v2.window_ms, 244000);

    let mut port = broker_port(
        &rec_v2,
        std::time::Instant::now() + std::time::Duration::from_millis(244000),
    );
    let pack = prep_v2.pack.utf8().as_bytes();
    let res = port.reserve(&prep_v2.scope, pack).unwrap();
    let bound =
        inference_intent::bind(&prep_v2.scope, &res, res.generation_call, pack, None).unwrap();
    let mut tampered = serde_json::from_slice::<Value>(&bound.configuration).unwrap();
    tampered["timing_profile"] = json!("reasoning-bounded-v1");
    tampered.sort_all_objects();
    let cfg = serde_json::to_vec(&tampered).unwrap();
    let digest = inference_intent::digest(pack, &cfg);
    assert!(
        broker()
            .query_one(
                "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
                &[
                    &e_v2,
                    &c_v2,
                    &prep_v2.scope.decision,
                    &res.generation_call,
                    &prep_v2.pack.utf8(),
                    &String::from_utf8(cfg).unwrap(),
                    &"",
                    &digest
                ]
            )
            .is_err()
    );
    assert!(
        broker()
            .query_one(
                "SELECT execution.run_phase_diagnostic($1)",
                &[&diagnostic_payload(run_v2, 650001)]
            )
            .is_err()
    );
    broker()
        .execute(
            "SELECT execution.run_phase_diagnostic($1)",
            &[&diagnostic_payload(run_v2, 650000)],
        )
        .unwrap();
    renewals_v2.retire();
    runs.stop(run_v2).unwrap();

    let (e_v1, c_v1, _) = register_live(0x73502, false);
    let run_v1 = Uuid::new_v4();
    let pub_v1 = live_publication(e_v1, c_v1, run_v1);
    let req_v1 = successor_request(&pub_v1, run_v1, "m1_autonomous_run_request_v3");
    assert!(runs.start(&req_v1, RELEASE).unwrap());
    let rec_v1 = runs.inspect(run_v1).unwrap();
    let renewals_v1 = Renewals::start(run_v1);
    let prep_v1 = prepare_decision_one(&mut runs, &rec_v1);
    assert_eq!(prep_v1.window_ms, 124000);
    assert!(
        broker()
            .query_one(
                "SELECT execution.run_phase_diagnostic($1)",
                &[&diagnostic_payload(run_v1, 150001)]
            )
            .is_err()
    );
    broker()
        .execute(
            "SELECT execution.run_phase_diagnostic($1)",
            &[&diagnostic_payload(run_v1, 150000)],
        )
        .unwrap();
    renewals_v1.retire();
    runs.stop(run_v1).unwrap();
}

#[test]
#[ignore = "dedicated schema-0035 worker only"]
fn sql_live_pro_v2_and_mirror_admission_and_cross_profile_matrix() {
    assert!(worker());
    check_sql_admission_and_mixed_matrix();
    check_sql_window_intent_and_diagnostics();
}

fn check_mirror_wire_contract(wire: &Value) {
    assert_eq!(wire["model"], PRO_MODEL);
    assert_eq!(wire["reasoning"]["effort"], "low");
    assert_eq!(wire["max_output_tokens"], 8192);
    assert_eq!(wire["store"], false);
    assert!(wire["stream"].as_bool().is_none_or(|v| !v));
}

fn verify_stored_pro_v2_calls(e: Uuid, requests: &[super::genai::Incoming]) {
    let rows = admin()
        .query(
            "SELECT p,configuration,body,binding,state FROM execution.inference_calls WHERE e=$1",
            &[&e],
        )
        .unwrap();
    assert_eq!(rows.len(), 3);
    for row in rows {
        let (p, i, b, h, state): (String, String, String, String, String) =
            (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4));
        assert_eq!(state, "completed");
        assert!(b.is_empty());
        assert_eq!(inference_intent::digest(p.as_bytes(), i.as_bytes()), h);
        assert!(p.len() <= 8192 && i.len() <= 16384);
        let intent: Value = serde_json::from_str(&i).unwrap();
        assert_eq!(intent["timing_profile"], PRO_V2_TIMING_PROFILE);
        assert_eq!(intent["settings"]["max_output_tokens"], 8192);
        let wire = requests
            .iter()
            .map(|r| serde_json::from_slice::<Value>(&r.body).unwrap())
            .find(|v| super::genai::pack_in(v).unwrap()["decision_id"] == intent["decision_id"])
            .unwrap();
        assert_eq!(wire["model"], PRO_MODEL);
    }
}

#[test]
#[ignore = "dedicated schema-0035 worker only"]
fn sql_live_fixture_pro_v2_cli_feedback_repair_incomplete_and_key_free_recovery() {
    assert!(worker());
    let (e, c, _) = register_pro_v2(0x73510, FIXTURE_PRO_V2_PROFILE);
    let run = Uuid::new_v4();
    let request = successor_request(
        &live_publication(e, c, run),
        run,
        "m1_autonomous_run_request_v3",
    );
    let input = std::env::temp_dir().join(format!("dw-pro-v2-run-{run}.json"));
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let root = control_root();

    let mismatch = command()
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
    assert!(!mismatch.status.success());

    let seq = AtomicUsize::new(0);
    let server = super::genai::Server::lab(move |r| {
        let wire: Value = serde_json::from_slice(&r.body).unwrap();
        check_mirror_wire_contract(&wire);
        let pack = super::genai::pack_in(&wire).unwrap();
        let n = seq.fetch_add(1, Ordering::SeqCst);
        let mut envelope = if n == 1 {
            let mut e = super::genai::envelope(&json!({}));
            e["output"][0]["content"][0]["text"] = json!("{}");
            e
        } else {
            super::genai::envelope(&super::genai::proposal(&pack))
        };
        envelope["model"] = json!(PRO_MODEL);
        super::genai::Reply::Length(200, serde_json::to_vec(&envelope).unwrap())
    });

    let output = command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args([
            "m1-run",
            "--input",
            input.to_str().unwrap(),
            "--timing-profile",
            PRO_V2_TIMING_PROFILE,
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "m1-run pro-v2 failed: {text}; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(receipt["effect_count"], 1);
    assert_eq!(receipt["terrain_history_count"], 1);
    assert_eq!(server.count(), 3);

    let identities = run_identities(e, c, run);
    for op in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, r) = cli(op, run);
        assert!(ok, "{op}: {r}");
        assert_eq!(run_identities(e, c, run), identities);
    }
    assert_eq!(server.count(), 3);
    verify_stored_pro_v2_calls(e, &server.requests.lock().unwrap());
    drop(server);
    std::fs::remove_file(input).unwrap();

    let (e2, c2, _) = register_pro_v2(0x73511, FIXTURE_PRO_V2_PROFILE);
    let run2 = Uuid::new_v4();
    let req2 = successor_request(
        &live_publication(e2, c2, run2),
        run2,
        "m1_autonomous_run_request_v3",
    );
    let input2 = std::env::temp_dir().join(format!("dw-pro-v2-inc-{run2}.json"));
    std::fs::write(&input2, serde_json::to_vec(&req2).unwrap()).unwrap();
    let server2 = super::genai::Server::lab(|r| {
        let wire: Value = serde_json::from_slice(&r.body).unwrap();
        let pack = super::genai::pack_in(&wire).unwrap();
        let mut envelope = super::genai::envelope(&super::genai::proposal(&pack));
        envelope["model"] = json!(PRO_MODEL);
        envelope["status"] = json!("incomplete");
        envelope["incomplete_details"] = json!({"reason": "max_output_tokens"});
        super::genai::Reply::Length(200, serde_json::to_vec(&envelope).unwrap())
    });
    let out2 = command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args([
            "m1-run",
            "--input",
            input2.to_str().unwrap(),
            "--timing-profile",
            PRO_V2_TIMING_PROFILE,
        ])
        .output()
        .unwrap();
    assert!(!out2.status.success());
    assert_eq!(server2.count(), 1);
    let row = admin()
        .query_one(
            "SELECT state,usage->>'requested_model' FROM execution.inference_calls WHERE e=$1",
            &[&e2],
        )
        .unwrap();
    assert_eq!(row.get::<_, String>(1), PRO_MODEL);
    assert_ne!(row.get::<_, String>(0), "completed");
    let _ = cli("m1-run-recover", run2);
    assert_eq!(server2.count(), 1);
    std::fs::remove_file(input2).unwrap();
}

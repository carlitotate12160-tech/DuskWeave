//! SDK episode qualification through the existing native fixture.
use super::*;

pub(super) fn sdk_real_cli_completes_feedback_and_fresh_key_free_recovery() {
    assert_sdk_real_cli(0x2a100, |_| {});
}
pub(super) fn sdk_eight_seed_producer_preserves_real_entity_and_intent_capacity() {
    assert_sdk_real_cli(0x2a102, wire::eight_seeds);
}
fn assert_sdk_real_cli(
    slot: u128,
    configure: impl FnOnce(&mut serde_json::Value),
) -> Vec<serde_json::Value> {
    let _guard = native::db();
    refresh();
    let server = fixture::Server::start("127.0.0.1:18083", |r| {
        let wire: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        let pack = fixture::pack_in(&wire).expect("real SDK must carry immutable P");
        assert_eq!(wire["model"], "deepseek-flash");
        assert_eq!(wire["reasoning"]["effort"], "low");
        assert_eq!(wire["max_output_tokens"], 4096);
        assert_eq!(wire["store"], false);
        assert!(wire["stream"].as_bool().is_none_or(|v| !v));
        for field in [
            "temperature",
            "top_p",
            "previous_response_id",
            "conversation",
        ] {
            assert!(wire.get(field).is_none());
        }
        assert!(wire["tools"].as_array().is_none_or(|v| v.is_empty()));

        fixture::Reply::Length(
            200,
            serde_json::to_vec(&fixture::envelope(&fixture::proposal(&pack))).unwrap(),
        )
    });
    let q = registered_with(slot, configure);
    let path = run_support::path(&q);
    let root = run_support::control_root();
    let output = run_support::command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args([
            "m1-run",
            "--input",
            path.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "safe CLI receipt: {text}; sends={} request_sizes={:?} wire={:?}",
        server.count(),
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.body.len())
            .collect::<Vec<_>>(),
        server.wire.lock().unwrap()
    );
    let receipt: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(receipt["decision_history_count"], 2);
    assert_eq!(receipt["effect_count"], 1);
    assert_eq!(receipt["terrain_history_count"], 1);
    assert_ne!(
        receipt["recorded_decisions"][0]["proposal"]["hypothesis"],
        receipt["recorded_decisions"][1]["proposal"]["hypothesis"]
    );
    assert_eq!(
        receipt["recorded_decisions"][1]["proposal"]["next_intent"]["category"],
        "retain"
    );
    assert_eq!(server.count(), 2, "D1 and D2 must each send exactly once");
    for operation in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, r) = run_support::cli(operation, "--run", q["run_id"].as_str().unwrap());
        assert!(ok, "{r}");
        assert_eq!(r["effect_count"], 1);
        assert_eq!(r["decision_count"], 2);
        assert_eq!(
            server.count(),
            2,
            "fresh recovery must never contact provider"
        );
    }
    eprintln!(
        "synthetic_provider_dns_fixture_episode run={} decisions={} decision_history={} effects={} terrain_history={} observed_provider_requests={} d1_intent={} d2_intent={}",
        q["run_id"],
        receipt["decision_count"],
        receipt["decision_history_count"],
        receipt["effect_count"],
        receipt["terrain_history_count"],
        server.count(),
        receipt["recorded_decisions"][0]["proposal"]["next_intent"]["category"],
        receipt["recorded_decisions"][1]["proposal"]["next_intent"]["category"]
    );
    let rows = native::admin()
        .query(
            "SELECT p,configuration,body,binding,usage FROM execution.inference_calls WHERE e=$1",
            &[
                &uuid::Uuid::parse_str(q["context"]["mission"]["engagement_id"].as_str().unwrap())
                    .unwrap(),
            ],
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    let mut observed_packs = Vec::new();
    for row in rows {
        let (p, i, b, h): (String, String, String, String) =
            (row.get(0), row.get(1), row.get(2), row.get(3));
        observed_packs.push(serde_json::from_str(&p).unwrap());
        assert!(b.is_empty());
        assert_eq!(
            duskweave::m1_reasoning::inference_intent::digest(p.as_bytes(), i.as_bytes()),
            h
        );
        assert!(!i.contains("synthetic-sdk-fixture-key"));
        let intent: serde_json::Value = serde_json::from_str(&i).unwrap();
        let requests = server.requests.lock().unwrap();
        let wire = requests
            .iter()
            .map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).unwrap())
            .find(|v| fixture::pack_in(v).unwrap()["decision_id"] == intent["decision_id"])
            .unwrap();
        assert!(
            wire["text"]["format"]["schema"] == intent["schema"],
            "SDK schema must match the bound intent"
        );
        let request_bytes = requests
            .iter()
            .find(|r| {
                fixture::pack_in(&serde_json::from_slice(&r.body).unwrap()).unwrap()["decision_id"]
                    == intent["decision_id"]
            })
            .unwrap()
            .body
            .len();
        let usage: serde_json::Value = row.get(4);
        eprintln!(
            "safe_sdk_sizes case={slot} pack={} intent={} request_entity={} response_entity={} context_margin={} intent_margin={} request_margin={}",
            p.len(),
            i.len(),
            request_bytes,
            usage["response_bytes"],
            8192 - p.len(),
            16384 - i.len(),
            16384 - request_bytes
        );
    }
    std::fs::remove_file(path).unwrap();
    observed_packs
}

pub(super) fn sdk_d2_omits_optional_summary_under_admitted_cap_before_removing_candidates() {
    let packs = assert_sdk_real_cli(0x2a103, |raw| {
        wire::eight_seeds(raw);
        raw["m1_inference_permission"]["operation_limits"]["context_bytes"] =
            serde_json::json!(7520);
    });
    let feedback = packs
        .iter()
        .find(|p| p["predecessor"]["availability"] == "committed")
        .unwrap();
    assert_eq!(
        feedback["candidates"].as_array().unwrap().len(),
        8,
        "omit optional summary before discarding admitted candidates"
    );
    assert!(feedback["predecessor"].get("hypothesis_summary").is_none());
    assert_eq!(
        feedback["predecessor"]["hypothesis_summary_omission"],
        "byte_bound"
    );
    assert_eq!(
        feedback["predecessor"]["outcome"],
        "owner_qualified_observed"
    );
    assert!(feedback["predecessor"]["terrain_publication_ref"].is_string());
    assert!(
        feedback["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "previous_decision")
    );
}

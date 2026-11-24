#![allow(dead_code)]
//! Privileged Linux ARM64 native HTTPS LAB failure & zero-replay recovery matrix (`#[ignore]`).
#[cfg(target_os = "linux")]
use super::support::linux_host::NativeHost;
#[cfg(target_os = "linux")]
use super::support::{
    CORE_BIN, build_native_run_bundle, extract_https_context_pack, start_loopback_sdk,
};
#[cfg(target_os = "linux")]
use crate::lab_support::{lab_proposal_for_pack, sanitize_inspect_receipt};
#[cfg(target_os = "linux")]
use crate::sdk_fixture::{FixtureChild, Reply, Server, envelope};
#[cfg(target_os = "linux")]
use serde_json::{Value, json};
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(target_os = "linux")]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

pub const NEGATIVE_SCENARIOS: &[(&str, &str, u64, u64)] = &[
    ("connection_close", "connection_failed", 1, 1),
    ("wrong_hostname", "tls_failed", 0, 0),
    ("wrong_root", "tls_failed", 0, 0),
    ("expired", "tls_failed", 0, 0),
    ("http2", "execution_refused", 1, 0),
    ("oversize_header", "execution_refused", 1, 1),
    ("timeout", "timed_out", 1, 1),
];

#[cfg(target_os = "linux")]
fn assert_inspect_recover_parity(
    host: &mut NativeHost,
    label: &str,
    run_id: uuid::Uuid,
    expected_status: &str,
    expected_dns: u64,
    expected_head: u64,
) {
    let hc = host.https_counters();
    let insp = host.cli(&["m1-run-inspect", "--run", &run_id.to_string()]);
    assert!(
        insp.status.success(),
        "m1-run-inspect failed on {label}: {}",
        String::from_utf8_lossy(&insp.stderr)
    );
    let rec = host.cli(&["m1-run-recover", "--run", &run_id.to_string()]);
    assert!(
        rec.status.success(),
        "m1-run-recover failed on {label}: {}",
        String::from_utf8_lossy(&rec.stderr)
    );
    let insp_san = sanitize_inspect_receipt(&NativeHost::receipt(&insp)).unwrap();
    let rec_san = sanitize_inspect_receipt(&NativeHost::receipt(&rec)).unwrap();
    assert_eq!(insp_san, rec_san);
    assert_eq!(insp_san["status"], expected_status);
    let hc_after = host.https_counters();
    assert_eq!(hc_after, hc, "m1-run-recover replayed I/O on {label}");
    assert_eq!(host.dns_count("dns_answered"), expected_dns);
    assert_eq!(hc_after["head_requests"], expected_head);
    host.save_inspect_artifact(
        label,
        run_id,
        &String::from_utf8_lossy(&insp.stdout),
        &insp_san,
        &rec_san,
    );
}

#[cfg(target_os = "linux")]
fn prepare_run_input(
    host: &mut NativeHost,
    run_id: uuid::Uuid,
    mutate: impl FnOnce(&mut Value),
) -> std::path::PathBuf {
    let (e, c, now_s) = (host.id(), host.id(), host.db_now_s());
    let (reg, mut req) =
        build_native_run_bundle(e, c, run_id, &host.manifest, now_s, now_s * 1000 + 400_000);
    mutate(&mut req);
    let op = host.id();
    let reg_file = host.write_input(&reg);
    let reg_out = host.cli(&[
        "register",
        "--operation",
        &op.to_string(),
        "--input",
        reg_file.to_str().unwrap(),
    ]);
    assert!(reg_out.status.success(), "register failed for {run_id}");
    host.write_input(&req)
}

#[cfg(target_os = "linux")]
fn run_acquisition_failure(scenario: &str, exp_outcome: &str, exp_tls: u64, exp_head: u64) {
    let mut host = NativeHost::new();
    let run_id = host.id();
    host.start_topology_for(scenario, run_id, 30, 30);
    let sdk = start_loopback_sdk("none");
    let run_file = prepare_run_input(&mut host, run_id, |_| {});
    let run_out = host.cli(&[
        "m1-run",
        "--input",
        run_file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ]);
    assert!(
        run_out.status.success(),
        "bounded acquisition failure {scenario} must retain cleanly: {}",
        String::from_utf8_lossy(&run_out.stderr)
    );
    let receipt = NativeHost::receipt(&run_out);
    assert_eq!(receipt["status"], "retained");
    assert_eq!(sdk.count(), 3);

    let row = host
        .admin
        .query_one(
            "SELECT s.status, s.head_outcome, lr.result_payload->>'outcome',
                    lr.result_payload->>'status_code'
             FROM execution.https_loop_steps s
             JOIN execution.live_https_results lr
               ON (lr.run_id, lr.step_index, lr.attempt, lr.result)
                = (s.run_id, s.step_index, s.head_attempt_id, s.head_result_id)
             WHERE s.run_id = $1 AND s.step_index = 2",
            &[&run_id],
        )
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "completed");
    assert_eq!(row.get::<_, String>(1), exp_outcome);
    assert_eq!(row.get::<_, String>(2), exp_outcome);
    assert!(row.get::<_, Option<String>>(3).is_none());

    let hc = host.https_counters();
    assert_eq!(hc["run_id"], run_id.to_string());
    assert_eq!(hc["dns_answered"], 1);
    assert_eq!(hc["connections"], 1);
    assert_eq!(hc["tls_handshakes"], exp_tls);
    assert_eq!(hc["head_requests"], exp_head);
    assert_inspect_recover_parity(
        &mut host,
        &format!("failure-{scenario}"),
        run_id,
        "retained",
        1,
        exp_head,
    );
}

#[cfg(target_os = "linux")]
fn run_preadmission_refusal(label: &str, mutate_req: impl FnOnce(&mut Value)) {
    let mut host = NativeHost::new();
    let run_id = host.id();
    host.start_topology_for("200", run_id, 30, 30);
    let _sdk = start_loopback_sdk("none");
    let run_file = prepare_run_input(&mut host, run_id, mutate_req);
    let run_out = host.cli(&[
        "m1-run",
        "--input",
        run_file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ]);
    assert!(
        !run_out.status.success(),
        "{label} must refuse before admission"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&run_out.stdout),
        String::from_utf8_lossy(&run_out.stderr)
    );
    assert!(!combined.trim().is_empty(), "{label} must explain refusal");

    let run_rows: i64 = host
        .admin
        .query_one(
            "SELECT count(*) FROM execution.autonomous_runs WHERE run_id = $1",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(run_rows, 0, "{label} must not persist a run");
    let hc = host.https_counters();
    assert_eq!(hc["dns_queries"], 0);
    assert_eq!(hc["dns_answered"], 0);
    assert_eq!(hc["connections"], 0);
    assert_eq!(hc["tls_handshakes"], 0);
    assert_eq!(hc["head_requests"], 0);
    assert!(
        !host
            .cli(&["m1-run-inspect", "--run", &run_id.to_string()])
            .status
            .success()
    );
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_https_lab_negative_matrix_and_zero_replay_recovery() {
    for &(scenario, exp_outcome, exp_tls, exp_head) in NEGATIVE_SCENARIOS {
        run_acquisition_failure(scenario, exp_outcome, exp_tls, exp_head);
    }
    run_preadmission_refusal("bad_profile", |req| {
        req["run_profile"] = json!("autonomous-dns-genai-live-v1");
    });
    run_preadmission_refusal("bad_release", |req| {
        req["live_binding"]["release_sha256"] = json!("b".repeat(64));
    });
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_stop_and_control_loss_prevent_further_effects() {
    let mut host = NativeHost::new();
    let run_id = host.id();
    host.start_topology_for("200", run_id, 30, 30);
    let release = Arc::new(AtomicBool::new(false));
    let rel_srv = release.clone();
    let _sdk = Server::lab(move |inc| {
        let req: Value = serde_json::from_slice(&inc.body).unwrap();
        let pack = extract_https_context_pack(&req).unwrap();
        if pack["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "endpoint_0")
        {
            return Reply::Hold(rel_srv.clone());
        }
        Reply::Length(
            200,
            serde_json::to_vec(&envelope(&lab_proposal_for_pack(&pack, "none"))).unwrap(),
        )
    });
    let run_file = prepare_run_input(&mut host, run_id, |_| {});
    let mut parent = FixtureChild::new(
        Command::new("/usr/sbin/runuser")
            .args([
                "-u",
                "duskweave-core",
                "--",
                CORE_BIN,
                "m1-run",
                "--input",
                run_file.to_str().unwrap(),
                "--timing-profile",
                "reasoning-bounded-pro-v2",
            ])
            .env_clear()
            .envs(&host.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let wait_end = Instant::now() + Duration::from_secs(8);
    while host.dns_count("dns_answered") == 0 && Instant::now() < wait_end {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(host.dns_count("dns_answered"), 1);
    let stopped = host.cli(&["m1-run-stop", "--run", &run_id.to_string()]);
    assert!(stopped.status.success());
    let stop_val = NativeHost::receipt(&stopped);
    assert_eq!(stop_val["local_stop"]["stop_latched"], true);
    release.store(true, Ordering::Release);
    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(10))
            .is_some()
    );
    let hc = host.https_counters();
    assert_eq!(hc["connections"], 0);
    assert_eq!(hc["tls_handshakes"], 0);
    assert_eq!(hc["head_requests"], 0);
    assert_inspect_recover_parity(&mut host, "stop-d2-hold", run_id, "stopped", 1, 0);
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_unknown_outcome_blocks_replay_on_recover() {
    let mut host = NativeHost::new();
    let run_id = host.id();
    host.start_topology_for("200", run_id, 30, 30);
    let _sdk = Server::lab(|inc| {
        let req: Value = serde_json::from_slice(&inc.body).unwrap();
        let pack = extract_https_context_pack(&req).unwrap();
        if pack["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "endpoint_0")
        {
            return Reply::Length(400, br#"{"error":"synthetic_control_loss"}"#.to_vec());
        }
        Reply::Length(
            200,
            serde_json::to_vec(&envelope(&lab_proposal_for_pack(&pack, "none"))).unwrap(),
        )
    });
    let run_file = prepare_run_input(&mut host, run_id, |_| {});
    let run_out = host.cli(&[
        "m1-run",
        "--input",
        run_file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ]);
    assert!(!run_out.status.success());
    let hc = host.https_counters();
    assert_eq!(hc["dns_answered"], 1);
    assert_eq!(hc["connections"], 0);
    assert_eq!(hc["head_requests"], 0);
    assert_inspect_recover_parity(&mut host, "unknown-d2-loss", run_id, "unknown", 1, 0);
}

#[cfg(test)]
mod tests {
    use super::super::support::{extract_https_context_pack, start_loopback_sdk_at};
    use crate::lab_support::{
        BASELINE_COMMIT, BASELINE_TREE, lab_proposal_for_pack, lab_registration_json,
        lab_run_request_json, sample_deployment_manifest, sanitize_inspect_receipt, sha256_hex,
        validate_deployment_manifest, verify_responder_provenance_bytes,
    };
    use duskweave::input::parse_register;
    use duskweave::m1_https::VERSION_HTTPS_LOOP;
    use duskweave::m1_inference_profile::HTTPS_LAB_PURPOSE;
    use duskweave::m1_run::RunRequest;
    use genai::adapter::AdapterKind;
    use genai::chat::ChatRequest;
    use genai::resolver::{AuthData, Endpoint};
    use genai::{Client, ModelIden, ModelSpec, ServiceTarget};
    use serde_json::{Value, json};
    use std::time::Duration;
    use uuid::Uuid;

    fn sample_elf() -> Vec<u8> {
        let mut b = vec![0u8; 64];
        b[0..4].copy_from_slice(b"\x7fELF");
        b[4] = 2;
        b[5] = 1;
        b[18..20].copy_from_slice(&183u16.to_le_bytes());
        b
    }

    #[test]
    fn responder_provenance_and_manifest_validation_reject_swaps_and_tampering() {
        let bin = sample_elf();
        let good = json!({
            "schema_version": 3, "purpose": HTTPS_LAB_PURPOSE, "binary": "httpslab-arm64",
            "size": bin.len(), "sha256": sha256_hex(&bin),
            "release_baseline_commit": BASELINE_COMMIT, "release_baseline_tree": BASELINE_TREE,
            "build_source_commit": "1".repeat(40), "build_source_tree": "2".repeat(40),
            "driver_commit": "1".repeat(40), "driver_tree": "2".repeat(40),
        });
        assert!(verify_responder_provenance_bytes(good.to_string().as_bytes(), &bin).is_ok());
        for (k, v) in [
            ("schema_version", json!(2)),
            ("purpose", json!("LIVE")),
            ("build_source_commit", json!(BASELINE_COMMIT)),
            ("driver_tree", json!("1".repeat(40))),
        ] {
            let mut bad = good.clone();
            bad[k] = v;
            assert!(verify_responder_provenance_bytes(bad.to_string().as_bytes(), &bin).is_err());
        }
        assert!(
            verify_responder_provenance_bytes(good.to_string().as_bytes(), b"not-elf").is_err()
        );

        let (mfst, _, _) = sample_deployment_manifest(Uuid::new_v4(), 1000, 2000, 1001);
        assert!(validate_deployment_manifest(&mfst).is_ok());
        let mut bad_m = mfst.clone();
        bad_m["purpose"] = json!("LIVE");
        assert!(validate_deployment_manifest(&bad_m).is_err());
    }

    #[test]
    fn proposal_progression_and_inspect_sanitization_preserve_lab_contract() {
        let p1 = lab_proposal_for_pack(&json!({"references": [{"id": "asset_0"}]}), "none");
        assert_eq!(p1["next_intent"]["manifest_reference"], "dns_udp");
        let p2 = lab_proposal_for_pack(
            &json!({"references": [{"id": "asset_0"}, {"id": "endpoint_0"}]}),
            "dns_once_if_stale",
        );
        assert_eq!(p2["next_intent"]["manifest_reference"], "https-head-v1");
        let p3 = lab_proposal_for_pack(
            &json!({"references": [{"id": "asset_0"}, {"id": "https_0"}]}),
            "none",
        );
        assert_eq!(p3["next_intent"]["category"], "retain");
        let ok = json!({
            "run_id": Uuid::new_v4(), "status": "retained", "live_qualified": true,
            "decision_count": 3, "dns_effect_count": 1, "head_effect_count": 1,
            "decision_history_count": 3, "terrain_history_count": 2, "release_sha256": "a".repeat(64)
        });
        assert!(sanitize_inspect_receipt(&ok).is_ok());
        assert!(sanitize_inspect_receipt(&json!({"d": "postgresql://u:pw@h/db"})).is_err());
    }

    #[test]
    fn lab_registration_and_run_request_parse_through_production_entrypoints() {
        let qref = Uuid::new_v4();
        let (mfst, _, _) = sample_deployment_manifest(qref, 1000, 2000, 1001);
        let rel = mfst["release_sha256"].as_str().unwrap();
        let reg = lab_registration_json(Uuid::new_v4(), Uuid::new_v4(), qref, rel, 1005);
        assert!(parse_register(reg.to_string().as_bytes()).is_ok());

        let run_id = Uuid::new_v4();
        let req = lab_run_request_json(&reg, run_id, qref, rel, 1_405_000);
        let parsed = RunRequest::parse(req.to_string().as_bytes()).unwrap();
        assert_eq!(parsed.run_id, run_id);
        assert_eq!(parsed.context.mission.expected_mission_revision, 1);

        for mutate in [
            (|r: &mut Value| r["run_profile"] = json!("autonomous-dns-genai-live-v1"))
                as fn(&mut Value),
            |r| r["context"]["mission"]["expected_mission_revision"] = Value::Null,
            |r| r["context"]["candidates"][0]["priority"] = json!("urgent"),
            |r| r["context"]["mission"]["mission_revision"] = json!(1),
        ] {
            let mut bad = req.clone();
            mutate(&mut bad);
            assert!(RunRequest::parse(bad.to_string().as_bytes()).is_err());
        }
    }

    #[test]
    fn loopback_sdk_extracts_actual_sdk_wire_and_progresses_dns_head_retain() {
        assert!(extract_https_context_pack(&json!({})).is_err());
        assert!(
            extract_https_context_pack(&json!({
                "input": [{"role": "user", "content": "{\"version\":\"m1_context_pack_autonomous_v4\",\"references\":[]}"}]
            }))
            .is_err()
        );

        let srv = start_loopback_sdk_at("127.0.0.1:0", "dns_once_if_stale", Duration::ZERO);
        let client = Client::new().unwrap();
        let target = ServiceTarget {
            endpoint: Endpoint::from_owned(format!("http://{}/", srv.address)),
            auth: AuthData::from_single("synthetic-key"),
            model: ModelIden::new(AdapterKind::OpenAIResp, "deepseek-v4-pro"),
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let call = |pack: Value| {
            rt.block_on(client.exec_chat_observed(
                ModelSpec::Target(target.clone()),
                ChatRequest::from_user(pack.to_string()).with_system("sys"),
                None,
            ))
        };

        let p1: Value = serde_json::from_str(
            call(json!({"version": VERSION_HTTPS_LOOP, "references": [{"id": "asset_0"}]}))
                .result
                .unwrap()
                .first_text()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(p1["next_intent"]["manifest_reference"], "dns_udp");

        let p2: Value = serde_json::from_str(
            call(json!({"version": VERSION_HTTPS_LOOP, "references": [{"id": "asset_0"}, {"id": "endpoint_0"}]}))
                .result
                .unwrap()
                .first_text()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(p2["next_intent"]["manifest_reference"], "https-head-v1");
        assert_eq!(p2["next_intent"]["dependency_refresh"], "dns_once_if_stale");

        let p3: Value = serde_json::from_str(
            call(json!({"version": VERSION_HTTPS_LOOP, "references": [{"id": "asset_0"}, {"id": "https_0"}]}))
                .result
                .unwrap()
                .first_text()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(p3["next_intent"]["category"], "retain");

        let bad = call(json!({"version": "m1_context_pack_autonomous_v4", "references": []}));
        assert!(bad.result.is_err());
    }
}

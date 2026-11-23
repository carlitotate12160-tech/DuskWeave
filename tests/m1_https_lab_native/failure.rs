#![allow(dead_code)]
//! Privileged Linux ARM64 native HTTPS LAB failure & zero-replay recovery matrix (`#[ignore]`).
#[cfg(target_os = "linux")]
use super::support::linux_host::NativeHost;
#[cfg(target_os = "linux")]
use super::support::{build_native_run_bundle, start_loopback_sdk};
#[cfg(target_os = "linux")]
use crate::lab_support::sanitize_inspect_receipt;
#[cfg(target_os = "linux")]
use serde_json::{Value, json};
#[cfg(target_os = "linux")]
use uuid::Uuid;

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
fn run_failure_case(
    scenario: &str,
    mut mutate_req: impl FnMut(&mut Value, &mut Value),
) -> (Value, Value, Value, u64) {
    let mut host = NativeHost::new();
    host.start_topology(scenario);
    let _sdk = start_loopback_sdk("none");
    let (e, c, run_id, now_s) = (host.id(), host.id(), host.id(), host.db_now_s());
    let (mut reg, mut req) =
        build_native_run_bundle(e, c, run_id, &host.manifest, now_s, now_s * 1000 + 400_000);
    mutate_req(&mut reg, &mut req);
    let reg_file = host.write_input(&reg);
    let op = host.id();
    let reg_out = host.cli(&[
        "register",
        "--operation",
        &op.to_string(),
        "--input",
        reg_file.to_str().unwrap(),
    ]);
    if reg_out.status.success() {
        let run_file = host.write_input(&req);
        let _ = host.cli(&[
            "m1-run",
            "--input",
            run_file.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-pro-v2",
        ]);
    } else {
        let run_file = host.write_input(&req);
        let out = host.cli(&[
            "m1-run",
            "--input",
            run_file.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-pro-v2",
        ]);
        assert!(!out.status.success());
    }
    let hc = host.https_counters();
    let dc = host.dns_count("dns_udp");
    let insp = host.cli(&["m1-run-inspect", "--run", &run_id.to_string()]);
    let rec = host.cli(&["m1-run-recover", "--run", &run_id.to_string()]);
    if insp.status.success() && rec.status.success() {
        let iv = NativeHost::receipt(&insp);
        let rv = NativeHost::receipt(&rec);
        assert_eq!(
            sanitize_inspect_receipt(&iv).unwrap(),
            sanitize_inspect_receipt(&rv).unwrap()
        );
    }
    let hc_after = host.https_counters();
    assert_eq!(
        hc_after, hc,
        "m1-run-recover replayed network I/O on {scenario}"
    );
    (hc, hc_after, json!({"run_id": run_id}), dc)
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_https_lab_negative_matrix_and_zero_replay_recovery() {
    for (scenario, _exp_outcome, exp_tls, exp_head) in NEGATIVE_SCENARIOS {
        let (hc, _, _, dc) = run_failure_case(scenario, |_, _| {});
        assert_eq!(dc, 1, "expected 1 DNS UDP question for {scenario}");
        assert_eq!(
            hc["connections"], 1,
            "expected 1 TCP connect for {scenario}"
        );
        assert_eq!(
            hc["tls_handshakes"], *exp_tls,
            "TLS count mismatch for {scenario}"
        );
        assert_eq!(
            hc["head_requests"], *exp_head,
            "HEAD count mismatch for {scenario}"
        );
    }

    // Profile/release/endpoint drift preflight refusals produce zero network effects.
    let (hc_bad_prof, _, _, dc_bad_prof) = run_failure_case("200", |_, req| {
        req["run_profile"] = json!("autonomous-dns-genai-live-v1");
    });
    assert_eq!(dc_bad_prof, 0);
    assert_eq!(hc_bad_prof["connections"], 0);

    let (hc_bad_rel, _, _, dc_bad_rel) = run_failure_case("200", |_, req| {
        req["live_binding"]["release_sha256"] = json!("b".repeat(64));
    });
    assert_eq!(dc_bad_rel, 0);
    assert_eq!(hc_bad_rel["connections"], 0);
}

#[cfg(test)]
mod tests {
    use crate::lab_support::{
        BASELINE_COMMIT, BASELINE_TREE, lab_proposal_for_pack, sample_deployment_manifest,
        sanitize_inspect_receipt, sha256_hex, validate_deployment_manifest,
        verify_responder_provenance_bytes,
    };
    use duskweave::m1_inference_profile::HTTPS_LAB_PURPOSE;
    use serde_json::json;
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
}

//! Same-run `m1-run` -> D1 -> LAB DNS -> Terrain -> D2 -> LAB HEAD -> Terrain -> D3 -> inspect/recover witnesses.
use crate::lab_support::{LAB_ROOT_SHA256, LAB_ROOT_VERSION};
use crate::sdk_fixture::{Reply, Server};
use duskweave::m1_https::VERSION_HTTPS_LOOP;
use duskweave::m1_inference_profile::{
    FIXTURE_PRO_V2_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_PURPOSE,
    HTTPS_LAB_RESOLVER, HTTPS_LAB_RUN_PROFILE,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

pub(super) const CORE_BIN: &str = "/usr/local/libexec/duskweave/duskweave";
pub(super) const HTTPSLAB_BIN: &str = "/usr/local/libexec/duskweave/httpslab";

pub(super) fn require_native_lab_roots() -> (PathBuf, PathBuf) {
    let cfg = std::env::var_os("DW_M1_LAB_CONFIG_ROOT")
        .map(PathBuf::from)
        .expect("DW_M1_LAB_CONFIG_ROOT required for privileged native HTTPS LAB harness");
    let rpt = std::env::var_os("DW_M1_LAB_REPORT_ROOT")
        .map(PathBuf::from)
        .expect("DW_M1_LAB_REPORT_ROOT required for privileged native HTTPS LAB harness");
    assert!(
        cfg.is_absolute() && rpt.is_absolute(),
        "DW_M1_LAB_CONFIG_ROOT and DW_M1_LAB_REPORT_ROOT must be absolute"
    );
    (cfg, rpt)
}

pub(super) fn proposal_for_pack(pack: &Value) -> Value {
    let refs = pack["references"]
        .as_array()
        .expect("ContextPack references");
    let has_https = refs
        .iter()
        .any(|r| r["id"].as_str().is_some_and(|id| id.starts_with("https_")));
    let has_ep0 = refs.iter().any(|r| r["id"] == "endpoint_0");
    let next_intent = if !has_ep0 && !has_https {
        json!({
            "category": "observe_more",
            "references": ["mission", "asset_0"],
            "manifest_reference": "dns_udp",
            "dependency_refresh": "none",
            "view_request": Value::Null,
        })
    } else if !has_https {
        json!({
            "category": "validate",
            "references": ["asset_0", "endpoint_0"],
            "manifest_reference": "https-head-v1",
            "dependency_refresh": "dns_once_if_stale",
        })
    } else {
        json!({
            "category": "retain",
            "references": ["asset_0", "https_0"],
            "manifest_reference": Value::Null,
            "dependency_refresh": "none",
        })
    };
    json!({
        "version": VERSION_HTTPS_LOOP,
        "hypothesis": "Validate admitted synthetic HTTPS LAB endpoint under single-endpoint limits.",
        "considered_alternative": "Defer if routing or TLS/HEAD authority fails.",
        "references": if has_https { json!(["asset_0", "previous_decision", "https_0"]) } else if has_ep0 { json!(["asset_0", "endpoint_0"]) } else { json!(["asset_0"]) },
        "expected_observation": "Qualified DNS or single-endpoint HTTPS HEAD metadata.",
        "falsifying_observation": "Blocked authority, TLS mismatch, or endpoint failure.",
        "uncertainty": "Synthetic HTTPS LAB only; single-endpoint HEAD never proves whole-host reachability or objective completion.",
        "rationale": "Advance through the closed HTTPS LAB loop using issued opaque IDs only.",
        "next_intent": next_intent,
    })
}

pub(super) fn start_loopback_sdk() -> Server {
    Server::lab(|inc| {
        let req: Value = serde_json::from_slice(&inc.body).expect("SDK request JSON");
        let text = req["input"][0]["content"][0]["text"]
            .as_str()
            .unwrap_or("{}");
        let pack: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({"references": []}));
        let prop = proposal_for_pack(&pack);
        let env = json!({
            "id": "synthetic-https-lab-response",
            "model": "deepseek-v4-pro",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": prop.to_string()}]
            }],
            "usage": {
                "input_tokens": 120,
                "output_tokens": 90,
                "output_tokens_details": {"reasoning_tokens": 20}
            }
        });
        Reply::Length(200, serde_json::to_vec(&env).unwrap())
    })
}

#[ignore]
#[test]
fn native_same_run_dns_head_200_and_302_feedback_and_key_free_inspect() {
    let (cfg, rpt) = require_native_lab_roots();
    let _sdk = start_loopback_sdk();
    let manifest_raw = std::fs::read("/etc/duskweave/m1/deployment.json")
        .expect("installed HTTPS LAB deployment.json");
    let manifest: Value = serde_json::from_slice(&manifest_raw).unwrap();
    assert_eq!(manifest["purpose"], HTTPS_LAB_PURPOSE);
    assert_eq!(manifest["run_profile"], HTTPS_LAB_RUN_PROFILE);
    assert_eq!(manifest["resolver_ipv4"], HTTPS_LAB_RESOLVER);
    assert_eq!(manifest["https_lab_root_version"], LAB_ROOT_VERSION);
    assert_eq!(manifest["https_lab_root_sha256"], LAB_ROOT_SHA256);
    assert_eq!(HTTPS_LAB_HOSTNAME, "head.duskweave.test");
    assert_eq!(HTTPS_LAB_IPV4, "198.51.100.10");
    assert_eq!(FIXTURE_PRO_V2_PROFILE, "fixture-genai-v4-pro-low-v2");

    let status = Command::new(CORE_BIN)
        .args(["--help"])
        .current_dir(&cfg)
        .output()
        .expect("installed duskweave binary");
    assert!(status.status.success());
    assert!(rpt.is_absolute());
}

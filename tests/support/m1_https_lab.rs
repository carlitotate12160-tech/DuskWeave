#![allow(dead_code)]
//! Shared test fixtures and builders for `m1_https_lab` and `m1_https_lab_postgres`.

use duskweave::m1_dns::{DnsRequest, PROFILE as DNS_PROFILE, SCHEMA as DNS_SCHEMA};
use duskweave::m1_https::ipc::{HttpsIpcRequest, HttpsLimits, HttpsOutcome, HttpsSafeResult};
use duskweave::m1_https::result::LAB_SCOPE_LIMITATION;
use duskweave::m1_https::{
    HTTPS_METHOD_HEAD, HTTPS_PORT, RedirectDisposition, VERSION_HTTPS_IPC, VERSION_HTTPS_LOOP,
};
use duskweave::m1_inference_profile::{
    FIXTURE_PRO_V2_PROFILE, HTTPS_HEAD_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4,
    HTTPS_LAB_PURPOSE, HTTPS_LAB_RESOLVER, HTTPS_LAB_RUN_PROFILE,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const LAB_ROOT_VERSION: &str = "duskweave-https-lab-root-v1";
pub const LAB_ROOT_SHA256: &str =
    "af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc";

pub fn uid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sample_release_sha() -> String {
    "a".repeat(64)
}

pub fn sample_live_binding(qref: Uuid) -> Value {
    json!({
        "release_sha256": sample_release_sha(),
        "qualification_ref": qref,
        "resolver_ipv4": HTTPS_LAB_RESOLVER,
    })
}

pub fn sample_deployment_manifest(
    qref: Uuid,
    valid_from_ms: i64,
    valid_until_ms: i64,
    core_uid: u32,
) -> (Value, Vec<u8>, String) {
    let h = sample_release_sha();
    let manifest = json!({
        "boot_id": uid(0xbeef_0001),
        "capture_sha256": h,
        "core_uid": core_uid,
        "exporter_sha256": h,
        "https_lab_root_sha256": LAB_ROOT_SHA256,
        "https_lab_root_version": LAB_ROOT_VERSION,
        "image_digest": format!("sha256:{h}"),
        "launcher_sha256": h,
        "purpose": HTTPS_LAB_PURPOSE,
        "qualification_ref": qref,
        "release_sha256": h,
        "resolver_ipv4": HTTPS_LAB_RESOLVER,
        "run_profile": HTTPS_LAB_RUN_PROFILE,
        "valid_from_ms": valid_from_ms,
        "valid_until_ms": valid_until_ms,
    });
    let mut encoded = serde_json::to_vec(&manifest).unwrap();
    encoded.push(b'\n');
    let digest = sha256_hex(&encoded);
    (manifest, encoded, digest)
}

pub fn sample_lab_context_pack(eng: Uuid, cmp: Uuid, dec: Uuid, perm: Uuid, qref: Uuid) -> Value {
    json!({
        "version": VERSION_HTTPS_LOOP,
        "run_profile": HTTPS_LAB_RUN_PROFILE,
        "execution_mode": "live",
        "inference_profile": FIXTURE_PRO_V2_PROFILE,
        "qualification_purpose": HTTPS_LAB_PURPOSE,
        "live_binding": sample_live_binding(qref),
        "mission_revision": 1,
        "engagement_id": eng,
        "campaign_id": cmp,
        "decision_id": dec,
        "permission_id": perm,
        "question": "Select next authorized candidate for dns_udp or single-endpoint https-head-v1 (endpoint_N), or retain/defer/stop/capability_gap. Use issued IDs only; HEAD tests one endpoint and never proves whole-host reachability or objective completion.",
        "references": [
            {"id": "mission", "owner": "mission"},
            {"id": "asset_0", "owner": "mission"},
            {"id": "endpoint_0", "owner": "cyber_terrain"}
        ],
        "manifests": [
            {"reference": "dns_udp", "availability": "qualified"},
            {"reference": "https-head-v1", "availability": "qualified"}
        ]
    })
}

pub fn sample_dns_request(now_ms: i64) -> DnsRequest {
    DnsRequest {
        schema: DNS_SCHEMA.into(),
        kind: "REQUEST".into(),
        attempt: uid(0x701),
        session_generation: 1,
        sequence: 0,
        engagement_id: uid(0x702),
        campaign_id: uid(0x703),
        decision_id: uid(0x704),
        profile: DNS_PROFILE.into(),
        execution_mode: "live".into(),
        run_profile: HTTPS_LAB_RUN_PROFILE.into(),
        release_sha256: sample_release_sha(),
        qualification_ref: uid(0x705),
        name: HTTPS_LAB_HOSTNAME.into(),
        resolver_ipv4: HTTPS_LAB_RESOLVER.into(),
        transport: "udp".into(),
        query_id: 17,
        allowed_names: vec![HTTPS_LAB_HOSTNAME.into()],
        deadline_at_ms: now_ms + 2000,
        max_duration_ms: 2000,
    }
}

pub fn sample_https_ipc_request(now_ms: i64) -> HttpsIpcRequest {
    HttpsIpcRequest {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "REQUEST".into(),
        attempt: uid(0x801),
        session_generation: 1,
        sequence: 0,
        engagement_id: uid(0x802),
        campaign_id: uid(0x803),
        decision_id: uid(0x804),
        run_id: uid(0x805),
        step_index: 2,
        attempt_kind: "head".into(),
        result_id: uid(0x806),
        profile: HTTPS_HEAD_PROFILE.into(),
        execution_mode: "live".into(),
        run_profile: HTTPS_LAB_RUN_PROFILE.into(),
        release_sha256: sample_release_sha(),
        qualification_ref: uid(0x807),
        asset_ref: uid(0x21),
        endpoint_ref: "endpoint_0".into(),
        hostname: HTTPS_LAB_HOSTNAME.into(),
        ipv4: HTTPS_LAB_IPV4.into(),
        port: HTTPS_PORT,
        approved_path: "/".into(),
        method: HTTPS_METHOD_HEAD.into(),
        parent_claim_id: uid(0x808),
        parent_claim_revision: 1,
        resolver_ipv4: HTTPS_LAB_RESOLVER.into(),
        vantage_ref: uid(0x809),
        permission_id: uid(0x80a),
        expected_mission_revision: 1,
        authority_evaluated_at_ms: now_ms - 100,
        claim_valid_until_s: (now_ms / 1000) + 60,
        episode_deadline_at_ms: now_ms + 55_000,
        deadline_at_ms: now_ms + 5000,
        max_duration_ms: 5000,
        limits: HttpsLimits::fixed(),
    }
}

pub fn sample_https_safe_result(
    req: &HttpsIpcRequest,
    outcome: HttpsOutcome,
    observed_at_ms: i64,
    status_code: u16,
    tls_category: &str,
    subcode: Option<&str>,
) -> HttpsSafeResult {
    let second = match outcome {
        HttpsOutcome::HeadResponse => "head_response_does_not_prove_access_or_objective_completion",
        HttpsOutcome::ConnectionFailed => "single_endpoint_connection_failed_not_whole_host",
        HttpsOutcome::TlsFailed => "single_endpoint_tls_failed_not_whole_host",
        HttpsOutcome::TimedOut => "single_endpoint_timeout_not_whole_host",
        HttpsOutcome::ExecutionRefused => "single_endpoint_execution_refused_not_whole_host",
    };
    let mut limitations = vec![LAB_SCOPE_LIMITATION.into(), second.into()];
    if let Some(code) = subcode {
        limitations.push(code.into());
    }
    let redirect = if outcome == HttpsOutcome::HeadResponse && (300..=399).contains(&status_code) {
        RedirectDisposition::RefusedRedirect
    } else {
        RedirectDisposition::None
    };
    let (network_bytes, header_bytes) = if outcome == HttpsOutcome::HeadResponse {
        (512, 128)
    } else {
        (64, 0)
    };
    HttpsSafeResult {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "RESULT".into(),
        attempt: req.attempt,
        session_generation: req.session_generation,
        sequence: 7,
        outcome,
        observed_at_ms,
        network_bytes,
        header_bytes,
        status_code: if outcome == HttpsOutcome::HeadResponse {
            status_code
        } else {
            0
        },
        tls_category: tls_category.into(),
        redirect,
        limitations,
    }
}

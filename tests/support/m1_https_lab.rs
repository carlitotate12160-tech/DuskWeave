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

pub fn lab_registration_json(eng: Uuid, cmp: Uuid, qref: Uuid, rel: &str, now_s: i64) -> Value {
    let (op_ref, auth_ref, goal_ref, asset) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let (vantage_ref, perm_id, terms_ref) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let op_limits = json!({
        "context_bytes": 8192, "candidates": 8, "request_bytes": 16384,
        "configuration_bytes": 16384, "response_bytes": 65536, "calls_per_decision": 2,
        "output_tokens_per_call": 8192, "decision_wall_time_ms": 249000,
        "per_call_wall_time_ms": 122000, "soft_completion_ms": 120000,
        "shared_wall_time_ms": 244000, "phase_wall_time_ms": 254000,
        "connect_time_ms": 5000, "proposal_bytes": 8192, "repair_bytes": 2048,
        "text_field_bytes": 1024, "repair_error_codes": 8, "json_depth": 32,
        "collection_entries": 64, "concurrency": 1, "cancel_grace_ms": 2000,
        "accounting_window_ms": 3000
    });
    let m1_perm = json!({
        "policy_version": 1, "ct_base_domain": "duskweave.test",
        "provider_disclosure": "crt_sh", "vantage_ref": vantage_ref,
        "resolver_ipv4": HTTPS_LAB_RESOLVER,
        "discovery_rules": [{"label_suffix": "duskweave.test"}],
        "contact_rules": [{"asset_ref": asset, "rule": {"exact": HTTPS_LAB_HOSTNAME}, "priority": 1}],
        "excluded_names": [], "approved_path": "/",
        "starts_at": now_s - 60, "ends_at": now_s + 3600,
        "campaign_limits": {
            "episodes": 3, "provider_calls": 8, "dns_questions": 3,
            "dns_followups": 1, "tcp_connections": 2, "head_requests": 2
        },
        "concurrency": 1
    });
    let inf_perm = json!({
        "version": VERSION_HTTPS_LOOP, "permission_id": perm_id,
        "engagement_id": eng, "campaign_id": cmp, "operator_ref": op_ref,
        "authority_ref": auth_ref, "authority_revision": 1, "goal_ref": goal_ref,
        "exercise_mode": "blind", "mission_revision": 1,
        "profile": FIXTURE_PRO_V2_PROFILE, "recipient": "http://127.0.0.1:18083/responses",
        "execution_mode": "live", "run_profile": HTTPS_LAB_RUN_PROFILE,
        "live_binding": {
            "release_sha256": rel, "qualification_ref": qref, "resolver_ipv4": HTTPS_LAB_RESOLVER
        },
        "provider_terms_approved": true, "provider_terms_ref": terms_ref,
        "starts_at": now_s - 60, "ends_at": now_s + 3600,
        "field_classes": [
            "mission_question_constraints", "scoped_opaque_references",
            "owner_qualified_summaries", "admitted_operator_premises",
            "manifest_availability", "omissions_unknown", "capacity",
            "admitted_repair_fields_codes", "admitted_campaign_decision_feedback"
        ],
        "campaign_limits": {
            "calls": 8, "output_token_capacity": 65536, "wall_time_ms": 996000,
            "request_bytes": 131072, "response_bytes": 524288
        },
        "operation_limits": op_limits
    });
    json!({
        "engagement_id": eng, "campaign_id": cmp, "operator_ref": op_ref,
        "authority_ref": auth_ref, "authority_revision": 1, "goal_ref": goal_ref,
        "exercise_mode": "blind", "starts_at": now_s - 60, "ends_at": now_s + 3600,
        "included_assets": [asset], "excluded_assets": [],
        "m1_permission": m1_perm,
        "m1_inference_permission": inf_perm
    })
}

pub fn lab_run_request_json(
    reg: &Value,
    run_id: Uuid,
    qref: Uuid,
    rel: &str,
    deadline_ms: i64,
) -> Value {
    let asset = &reg["included_assets"][0];
    let inf = &reg["m1_inference_permission"];
    json!({
        "version": VERSION_HTTPS_LOOP, "run_id": run_id, "execution_mode": "live",
        "run_profile": HTTPS_LAB_RUN_PROFILE, "inference_profile": FIXTURE_PRO_V2_PROFILE,
        "live_binding": {
            "release_sha256": rel, "qualification_ref": qref, "resolver_ipv4": HTTPS_LAB_RESOLVER
        },
        "context": {
            "version": "m1_context_prepare_request_v1",
            "mission": {
                "version": "m1_inference_reservation_request_v1",
                "engagement_id": reg["engagement_id"], "campaign_id": reg["campaign_id"],
                "operator_ref": reg["operator_ref"], "expected_mission_revision": 1,
                "permission_id": inf["permission_id"], "goal_ref": reg["goal_ref"],
                "exercise_mode": reg["exercise_mode"]
            },
            "candidates": [{"asset_ref": asset, "priority": "high"}]
        },
        "seed_names": [{"asset_ref": asset, "name": HTTPS_LAB_HOSTNAME}],
        "deadline_at_ms": deadline_ms
    })
}

pub const BASELINE_COMMIT: &str = "c2077f0e77ad99fa5f9ce1daa22b12dfb4d4a2b2";
pub const BASELINE_TREE: &str = "a421fef312e18e06ae3276db82bd6e3a38bed9ac";

pub fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub fn is_elf64_aarch64(b: &[u8]) -> bool {
    b.len() >= 20
        && &b[0..4] == b"\x7fELF"
        && b[4] == 2
        && b[5] == 1
        && u16::from_le_bytes([b[18], b[19]]) == 183
}

pub fn verify_responder_provenance_bytes(prov: &[u8], bin: &[u8]) -> Result<Value, &'static str> {
    if !is_elf64_aarch64(bin) {
        return Err("responder_not_elf64_aarch64");
    }
    let obj: Value = serde_json::from_slice(prov).map_err(|_| "provenance_malformed")?;
    if obj["schema_version"].as_i64() != Some(3)
        || obj["purpose"].as_str() != Some(HTTPS_LAB_PURPOSE)
        || obj["binary"].as_str() != Some("httpslab-arm64")
        || obj["release_baseline_commit"].as_str() != Some(BASELINE_COMMIT)
        || obj["release_baseline_tree"].as_str() != Some(BASELINE_TREE)
    {
        return Err("provenance_scope_refused");
    }
    let (bc, bt, dc, dt) = (
        obj["build_source_commit"].as_str().unwrap_or(""),
        obj["build_source_tree"].as_str().unwrap_or(""),
        obj["driver_commit"].as_str().unwrap_or(""),
        obj["driver_tree"].as_str().unwrap_or(""),
    );
    if !is_hex(bc, 40)
        || !is_hex(bt, 40)
        || !is_hex(dc, 40)
        || !is_hex(dt, 40)
        || bc == BASELINE_COMMIT
        || dc == BASELINE_COMMIT
        || bt == BASELINE_TREE
        || dt == BASELINE_TREE
        || bc == bt
        || dc == dt
        || bc == dt
        || dc == bt
    {
        return Err("provenance_pin_refused");
    }
    if obj["size"].as_u64() != Some(bin.len() as u64)
        || obj["sha256"].as_str() != Some(sha256_hex(bin).as_str())
    {
        return Err("provenance_bytes_mismatch");
    }
    Ok(obj)
}

pub fn validate_deployment_manifest(m: &Value) -> Result<(), &'static str> {
    if m["purpose"].as_str() != Some(HTTPS_LAB_PURPOSE)
        || m["run_profile"].as_str() != Some(HTTPS_LAB_RUN_PROFILE)
        || m["resolver_ipv4"].as_str() != Some(HTTPS_LAB_RESOLVER)
        || m["https_lab_root_version"].as_str() != Some(LAB_ROOT_VERSION)
        || m["https_lab_root_sha256"].as_str() != Some(LAB_ROOT_SHA256)
        || !m["release_sha256"].as_str().is_some_and(|s| is_hex(s, 64))
        || m["core_uid"].as_u64().unwrap_or(0) == 0
    {
        return Err("manifest_binding_refused");
    }
    let qref = m["qualification_ref"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or("manifest_qref_invalid")?;
    if qref.is_nil()
        || m["valid_from_ms"].as_i64().unwrap_or(0) >= m["valid_until_ms"].as_i64().unwrap_or(0)
    {
        return Err("manifest_window_invalid");
    }
    Ok(())
}

pub fn sanitize_inspect_receipt(v: &Value) -> Result<Value, &'static str> {
    let raw = v.to_string();
    if raw.contains("://") || raw.contains("password=") || raw.contains("DW_") {
        return Err("inspect_secret_or_dsn_leak");
    }
    Ok(json!({
        "run_id": v["run_id"], "status": v["status"], "live_qualified": v["live_qualified"],
        "decision_count": v["decision_count"], "dns_effect_count": v["dns_effect_count"],
        "head_effect_count": v["head_effect_count"], "decision_history_count": v["decision_history_count"],
        "terrain_history_count": v["terrain_history_count"], "release_sha256": v["release_sha256"],
    }))
}

pub fn lab_proposal_for_pack(pack: &Value, dep_refresh: &str) -> Value {
    let refs = pack["references"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let has_https = refs
        .iter()
        .any(|r| r["id"].as_str().is_some_and(|id| id.starts_with("https_")));
    let has_ep0 = refs.iter().any(|r| r["id"] == "endpoint_0");
    let (prop_refs, next) = if !has_ep0 && !has_https {
        (
            json!(["mission", "asset_0"]),
            json!({"category": "observe_more", "references": ["mission", "asset_0"],
         "manifest_reference": "dns_udp", "dependency_refresh": "none", "view_request": Value::Null}),
        )
    } else if !has_https {
        (
            json!(["asset_0", "endpoint_0"]),
            json!({"category": "validate", "references": ["asset_0", "endpoint_0"],
         "manifest_reference": "https-head-v1", "dependency_refresh": dep_refresh}),
        )
    } else {
        (
            json!(["asset_0", "previous_decision", "https_0"]),
            json!({"category": "retain",
         "references": ["asset_0", "https_0"], "manifest_reference": Value::Null, "dependency_refresh": "none"}),
        )
    };
    json!({
        "version": VERSION_HTTPS_LOOP, "hypothesis": "Validate admitted synthetic HTTPS LAB endpoint.",
        "considered_alternative": "Defer if routing or TLS/HEAD authority fails.", "references": prop_refs,
        "expected_observation": "Qualified DNS or single-endpoint HTTPS HEAD metadata.",
        "falsifying_observation": "Blocked authority, TLS mismatch, or endpoint failure.",
        "uncertainty": "Synthetic HTTPS LAB only; single-endpoint HEAD never proves whole-host reachability.",
        "rationale": "Advance through closed HTTPS LAB loop using issued opaque IDs only.", "next_intent": next,
    })
}

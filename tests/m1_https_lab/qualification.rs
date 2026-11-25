use crate::lab_support::{
    LAB_ROOT_SHA256, LAB_ROOT_VERSION, sample_deployment_manifest, sample_live_binding,
    sample_release_sha, sha256_hex, uid,
};
use duskweave::m1_https::VERSION_HTTPS_LOOP;
use duskweave::m1_inference_permission::LiveBinding;
use duskweave::m1_inference_profile::{
    FIXTURE_PRO_V2_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_PURPOSE, HTTPS_LAB_RESOLVER,
    HTTPS_LAB_RUN_PROFILE, LAB_RUN_PROFILE, LIVE_RUN_PROFILE, live_binding, resolver_binding,
    sdk_versions,
};
use duskweave::m1_run::RunRequest;
use serde_json::json;

#[test]
fn https_lab_profile_bindings_and_committed_root_ca_identity() {
    let pem = std::fs::read("boundary/fixtures/https-lab-root-v1.pem").unwrap();
    assert_eq!(pem.len(), 473);
    assert_eq!(sha256_hex(&pem), LAB_ROOT_SHA256);

    let v = sdk_versions(HTTPS_LAB_RUN_PROFILE).unwrap();
    assert_eq!(v.permission, VERSION_HTTPS_LOOP);
    assert_eq!(v.run, VERSION_HTTPS_LOOP);
    assert_eq!(v.pack, VERSION_HTTPS_LOOP);
    assert_eq!(v.qualification_purpose, Some(HTTPS_LAB_PURPOSE));

    assert!(resolver_binding(HTTPS_LAB_RUN_PROFILE, HTTPS_LAB_RESOLVER));
    assert!(!resolver_binding(HTTPS_LAB_RUN_PROFILE, "1.1.1.1"));
    assert!(live_binding(
        HTTPS_LAB_RUN_PROFILE,
        "live",
        FIXTURE_PRO_V2_PROFILE
    ));
    assert!(!live_binding(
        HTTPS_LAB_RUN_PROFILE,
        "controlled_fixture",
        FIXTURE_PRO_V2_PROFILE
    ));
    assert!(!live_binding(
        HTTPS_LAB_RUN_PROFILE,
        "live",
        "deepseek-genai-v4-pro-low-v2"
    ));

    let qref = uid(0x44);
    let lb: LiveBinding = serde_json::from_value(sample_live_binding(qref)).unwrap();
    lb.validate_for(HTTPS_LAB_RUN_PROFILE).unwrap();
    lb.validate_for(LAB_RUN_PROFILE).unwrap();
    assert!(lb.validate_for(LIVE_RUN_PROFILE).is_err());
    assert!(lb.validate_for(VERSION_HTTPS_LOOP).is_err());

    let (manifest, encoded, digest) =
        sample_deployment_manifest(qref, 1_000, 4_000_000_000_000, 1001);
    assert_eq!(manifest.as_object().unwrap().len(), 15);
    assert_eq!(manifest["purpose"], HTTPS_LAB_PURPOSE);
    assert_eq!(manifest["run_profile"], HTTPS_LAB_RUN_PROFILE);
    assert_eq!(manifest["https_lab_root_version"], LAB_ROOT_VERSION);
    assert_eq!(manifest["https_lab_root_sha256"], LAB_ROOT_SHA256);
    assert!(encoded.len() <= 16384);
    assert_eq!(digest.len(), 64);
}

#[test]
fn https_lab_run_request_requires_exact_tuple_and_singleton_seed() {
    let qref = uid(0x55);
    let id = uid(0x21);
    let base = json!({
        "version": VERSION_HTTPS_LOOP,
        "run_id": id,
        "run_profile": HTTPS_LAB_RUN_PROFILE,
        "execution_mode": "live",
        "inference_profile": FIXTURE_PRO_V2_PROFILE,
        "live_binding": {
            "release_sha256": sample_release_sha(),
            "qualification_ref": qref,
            "resolver_ipv4": HTTPS_LAB_RESOLVER,
        },
        "deadline_at_ms": 1_700_000_600_000i64,
        "context": {
            "version": "m1_context_prepare_request_v1",
            "mission": {
                "version": "m1_inference_reservation_request_v1",
                "engagement_id": id,
                "campaign_id": id,
                "operator_ref": id,
                "expected_mission_revision": 1,
                "permission_id": id,
                "goal_ref": id,
                "exercise_mode": "blind"
            },
            "candidates": [{"asset_ref": id, "priority": "high"}]
        },
        "seed_names": [{"asset_ref": id, "name": HTTPS_LAB_HOSTNAME}]
    });
    let parsed: RunRequest = serde_json::from_value(base.clone()).unwrap();
    parsed.validate().unwrap();

    for bad_mut in [
        {
            let mut v = base.clone();
            v["execution_mode"] = json!("controlled_fixture");
            v
        },
        {
            let mut v = base.clone();
            v["seed_names"][0]["name"] = json!("other.duskweave.test");
            v
        },
        {
            let mut v = base.clone();
            v["seed_names"] = json!([
                {"asset_ref": id, "name": HTTPS_LAB_HOSTNAME},
                {"asset_ref": id, "name": "extra.duskweave.test"}
            ]);
            v
        },
        {
            let mut v = base.clone();
            v.as_object_mut().unwrap().remove("live_binding");
            v
        },
    ] {
        let bad_req: Result<RunRequest, _> = serde_json::from_value(bad_mut);
        if let Ok(r) = bad_req {
            assert!(r.validate().is_err());
        }
    }
}

#[test]
fn https_loop_inference_permission_validates_lab_and_rejects_drift() {
    use duskweave::input::parse_register;
    let qref = uid(0x66);
    let (e, c, op, auth, goal, asset) = (
        uid(0x60),
        uid(0x63),
        uid(0x64),
        uid(0x65),
        uid(0x67),
        uid(0x68),
    );
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
    let mut base = json!({
        "engagement_id": e, "campaign_id": c, "operator_ref": op,
        "authority_ref": auth, "authority_revision": 1, "goal_ref": goal,
        "included_assets": [asset], "excluded_assets": [], "exercise_mode": "blind",
        "starts_at": 50, "ends_at": 250,
        "m1_inference_permission": {
            "version": VERSION_HTTPS_LOOP, "permission_id": uid(0x61),
            "engagement_id": e, "campaign_id": c, "operator_ref": op,
            "authority_ref": auth, "authority_revision": 1, "mission_revision": 1,
            "goal_ref": goal, "exercise_mode": "blind",
            "run_profile": HTTPS_LAB_RUN_PROFILE, "execution_mode": "live",
            "profile": FIXTURE_PRO_V2_PROFILE, "recipient": "http://127.0.0.1:18083/responses",
            "starts_at": 100, "ends_at": 200, "provider_terms_approved": true,
            "provider_terms_ref": uid(0x62), "live_binding": sample_live_binding(qref),
            "field_classes": [
                "mission_question_constraints", "scoped_opaque_references", "owner_qualified_summaries",
                "admitted_operator_premises", "manifest_availability", "omissions_unknown",
                "capacity", "admitted_repair_fields_codes", "admitted_campaign_decision_feedback"
            ],
            "campaign_limits": {
                "calls": 8, "output_token_capacity": 65536, "wall_time_ms": 996000,
                "request_bytes": 131072, "response_bytes": 524288
            },
            "operation_limits": op_limits
        }
    });
    parse_register(&serde_json::to_vec(&base).unwrap()).unwrap();

    for edit in [
        |v: &mut serde_json::Value| v["recipient"] = json!("https://example.test/responses"),
        |v: &mut serde_json::Value| v["operation_limits"]["request_bytes"] = json!(8192),
        |v: &mut serde_json::Value| v["execution_mode"] = json!("controlled_fixture"),
        |v: &mut serde_json::Value| {
            v.as_object_mut().unwrap().remove("live_binding");
        },
        |v: &mut serde_json::Value| v["field_classes"] = json!(["mission_question_constraints"]),
    ] {
        let mut mutated = base.clone();
        edit(&mut mutated["m1_inference_permission"]);
        assert!(parse_register(&serde_json::to_vec(&mutated).unwrap()).is_err());
    }
    base["m1_inference_permission"]["run_profile"] =
        json!(duskweave::m1_inference_profile::HTTPS_LOOP_RUN_PROFILE);
    base["m1_inference_permission"]["execution_mode"] = json!("controlled_fixture");
    base["m1_inference_permission"]
        .as_object_mut()
        .unwrap()
        .remove("live_binding");
    parse_register(&serde_json::to_vec(&base).unwrap()).unwrap();
}

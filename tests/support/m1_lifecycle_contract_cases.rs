//! Existing pure successor contracts; no database or CLI fixture ownership.
use crate::consent;
use duskweave::input::parse_register;
use duskweave::mission::{CampaignId, EngagementId};
use serde_json::{Value, json};
use uuid::Uuid;

#[test]
fn t7_reported_guard_is_optional_closed_evidence_separate_from_parent_cause() {
    let raw = json!({"version":"m1_run_phase_diagnostic_v2","run_id":Uuid::from_u128(17),
        "phase":"decision_one","cause":"child_exit","cause_source":"parent_observed",
        "child_error":null,"exit_code":124,"readiness_observed":true,"elapsed_ms":61950,
        "reported_guard":"transport_hard_deadline"});
    let diagnostic: duskweave::m1_run::PhaseDiagnostic = serde_json::from_value(raw.clone())
        .expect("a received closed report must survive the diagnostic boundary");
    diagnostic.validate().unwrap();
    for value in [
        json!("timeout"),
        json!("synthetic-sensitive-sentinel"),
        json!(3),
        Value::Null,
    ] {
        let mut bad = raw.clone();
        bad["reported_guard"] = value;
        assert!(serde_json::from_value::<duskweave::m1_run::PhaseDiagnostic>(bad).is_err());
    }
    let mut absent = raw;
    absent.as_object_mut().unwrap().remove("reported_guard");
    serde_json::from_value::<duskweave::m1_run::PhaseDiagnostic>(absent)
        .unwrap()
        .validate()
        .unwrap();
}

fn successor(profile: &str, recipient: &str) -> Value {
    let mut raw = consent::registration(
        EngagementId(Uuid::from_u128(1)),
        CampaignId(Uuid::from_u128(2)),
    );
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v4");
    p["profile"] = json!(profile);
    p["recipient"] = json!(recipient);
    p["execution_mode"] = json!("controlled_fixture");
    p["run_profile"] = json!("autonomous-dns-fixture-v2");
    p["field_classes"] = json!([
        "mission_question_constraints",
        "scoped_opaque_references",
        "owner_qualified_summaries",
        "admitted_operator_premises",
        "manifest_availability",
        "omissions_unknown",
        "capacity",
        "admitted_repair_fields_codes",
        "admitted_campaign_decision_feedback"
    ]);
    p["campaign_limits"] = json!({"calls":4,"request_bytes":65536,"response_bytes":262144,"output_token_capacity":16384,"wall_time_ms":258000});
    for (name, value) in [
        ("context_bytes", 8192),
        ("json_depth", 32),
        ("decision_wall_time_ms", 129000),
        ("per_call_wall_time_ms", 62000),
        ("soft_completion_ms", 60000),
        ("shared_wall_time_ms", 124000),
        ("phase_wall_time_ms", 134000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        p["operation_limits"][name] = json!(value);
    }
    raw
}

#[test]
fn successor_permission_admits_only_the_three_explicit_bound_profiles() {
    for (profile, recipient) in [
        ("fixture-responses-v1", "urn:duskweave:fixture:responses:v1"),
        (
            "fixture-chat-completions-v1",
            "urn:duskweave:fixture:chat-completions:v1",
        ),
        (
            "deepseek-responses-reasoning-v1",
            "https://api.deepseek.com/responses",
        ),
    ] {
        let raw = successor(profile, recipient);
        assert!(
            parse_register(&serde_json::to_vec(&raw).unwrap()).is_ok(),
            "a fresh v4 permission with the selected finite allocation must register: {profile}"
        );
        for (field, value) in [
            ("profile", json!("unknown")),
            ("recipient", json!("https://unapproved.invalid")),
            ("run_profile", json!("autonomous-dns-fixture-v1")),
            ("version", json!("m1_inference_permission_v3")),
        ] {
            let mut bad = raw.clone();
            bad["m1_inference_permission"][field] = value;
            assert!(
                parse_register(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "{field}"
            );
        }
    }
}

#[test]
fn successor_permission_refuses_missing_or_rebound_timer_values() {
    let raw = successor("fixture-responses-v1", "urn:duskweave:fixture:responses:v1");
    assert!(parse_register(&serde_json::to_vec(&raw).unwrap()).is_ok());
    for name in [
        "soft_completion_ms",
        "per_call_wall_time_ms",
        "shared_wall_time_ms",
        "decision_wall_time_ms",
        "phase_wall_time_ms",
        "cancel_grace_ms",
        "accounting_window_ms",
    ] {
        for value in [json!(0), json!(-1), json!(1e300), Value::Null, json!(60001)] {
            let mut bad = raw.clone();
            bad["m1_inference_permission"]["operation_limits"][name] = value;
            assert!(
                parse_register(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "{name}"
            );
        }
        let mut bad = raw.clone();
        bad["m1_inference_permission"]["operation_limits"]
            .as_object_mut()
            .unwrap()
            .remove(name);
        assert!(
            parse_register(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "missing {name}"
        );
    }
}

#[test]
fn successor_run_requires_explicit_profile_and_preserves_it_in_custody() {
    let id = Uuid::from_u128(1);
    let raw = json!({"version":"m1_autonomous_run_request_v2","run_id":id,
        "run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
        "inference_profile":"fixture-chat-completions-v1","deadline_at_ms":1700000400000i64,
        "context":{"version":"m1_context_prepare_request_v1","mission":{
            "version":"m1_inference_reservation_request_v1","engagement_id":id,"campaign_id":id,
            "operator_ref":id,"expected_mission_revision":1,"permission_id":id,"goal_ref":id,
            "exercise_mode":"blind"},"candidates":[{"asset_ref":id,"priority":"high"}]},
        "seed_names":[{"asset_ref":id,"name":"www.example.invalid"}]});
    let parsed = duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&raw).unwrap());
    assert!(
        parsed.is_ok(),
        "v2 run must admit the explicit selected profile"
    );
    assert_eq!(serde_json::to_value(parsed.unwrap()).unwrap(), raw);
    let mut missing = raw.clone();
    missing.as_object_mut().unwrap().remove("inference_profile");
    assert!(duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&missing).unwrap()).is_err());
    let mut old = raw;
    old["version"] = json!("m1_autonomous_run_request_v1");
    assert!(duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&old).unwrap()).is_err());
}

#[test]
fn both_successor_protocols_bind_the_same_pack_and_conditional_repair() {
    use duskweave::m1_reasoning::request;
    for profile in ["fixture-responses-v1", "fixture-chat-completions-v1"] {
        let p = json!({"version":"m1_context_pack_autonomous_v2","run_profile":"autonomous-dns-fixture-v2",
            "execution_mode":"controlled_fixture","inference_profile":profile,
            "references":[{"id":"asset_0"}],"manifests":[{"reference":"dns_udp"}]});
        let pack = serde_json::to_string(&p).unwrap();
        for repair in [None, Some(&["proposal_shape"][..])] {
            let bound = request::bind(pack.as_bytes(), repair).unwrap();
            assert!(
                request::verify_binding(
                    &pack,
                    std::str::from_utf8(&bound.configuration).unwrap(),
                    std::str::from_utf8(&bound.body).unwrap(),
                    &bound.digest,
                    repair.is_some()
                )
                .is_ok(),
                "both real codecs must verify the bound request: {profile}"
            );
            let mut config: Value = serde_json::from_slice(&bound.configuration).unwrap();
            config["recipient"] = json!("urn:unapproved");
            let forged = serde_json::to_string(&config).unwrap();
            let forged_digest =
                request::compute_binding(pack.as_bytes(), forged.as_bytes(), &bound.body);
            assert!(
                request::verify_binding(
                    &pack,
                    &forged,
                    std::str::from_utf8(&bound.body).unwrap(),
                    &forged_digest,
                    repair.is_some()
                )
                .is_err(),
                "a recomputed hash grants no profile rebinding"
            );
        }
    }
}

use duskweave::input::parse_register;
use duskweave::mission::{CampaignId, EngagementId};
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;

fn fixture() -> Value {
    let mut r = consent::registration(
        EngagementId(Uuid::from_u128(1)),
        CampaignId(Uuid::from_u128(2)),
    );
    let p = &mut r["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v3");
    p["recipient"] = json!("urn:duskweave:fixture:inference:v1");
    p["profile"] = json!("deterministic-autonomous-v1");
    p["execution_mode"] = json!("controlled_fixture");
    p["run_profile"] = json!("autonomous-dns-fixture-v1");
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
    let l = &mut p["operation_limits"];
    for (k, v) in [
        ("context_bytes", 8192),
        ("json_depth", 32),
        ("decision_wall_time_ms", 45000),
        ("per_call_wall_time_ms", 20000),
        ("shared_wall_time_ms", 40000),
        ("phase_wall_time_ms", 50000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        l[k] = json!(v);
    }
    r
}
fn accepted(r: &Value) -> bool {
    parse_register(&serde_json::to_vec(r).unwrap()).is_ok()
}

#[test]
fn v3_fixture_requires_exact_mode_recipient_feedback_and_timers() {
    let r = fixture();
    assert!(accepted(&r), "fresh fixture v3 must register");
    for (k, v) in [
        ("execution_mode", json!("live")),
        ("recipient", json!("https://api.deepseek.com/responses")),
        ("run_profile", json!("autonomous-dns-v1")),
        ("profile", json!("deepseek-responses-envelope32-v1")),
    ] {
        let mut bad = r.clone();
        bad["m1_inference_permission"][k] = v;
        assert!(!accepted(&bad), "{k}");
    }
    let fields = r["m1_inference_permission"]["operation_limits"]
        .as_object()
        .unwrap();
    for k in [
        "per_call_wall_time_ms",
        "shared_wall_time_ms",
        "phase_wall_time_ms",
        "cancel_grace_ms",
        "accounting_window_ms",
        "decision_wall_time_ms",
    ] {
        let mut bad = r.clone();
        bad["m1_inference_permission"]["operation_limits"][k] =
            json!(fields[k].as_i64().unwrap() + 1);
        assert!(!accepted(&bad), "{k}");
        bad["m1_inference_permission"]["operation_limits"]
            .as_object_mut()
            .unwrap()
            .remove(k);
        assert!(!accepted(&bad), "missing {k}");
    }
    let mut missing = r.clone();
    missing["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(!accepted(&missing));
}

#[test]
fn legacy_consent_cannot_import_autonomous_fields_or_feedback() {
    let original = consent::registration(
        EngagementId(Uuid::from_u128(1)),
        CampaignId(Uuid::from_u128(2)),
    );
    assert!(accepted(&original));
    for k in ["execution_mode", "run_profile"] {
        let mut bad = original.clone();
        bad["m1_inference_permission"][k] = fixture()["m1_inference_permission"][k].clone();
        assert!(!accepted(&bad));
    }
    let mut bad = original;
    bad["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_campaign_decision_feedback"));
    assert!(!accepted(&bad));
}

use duskweave::input::{MAX_INPUT_BYTES, parse_register};
use duskweave::mission::*;
use duskweave::trajectory::check_event;
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/m1_permission.rs"]
mod ct;
#[path = "support/registration_db.rs"]
mod db_support;

fn scope() -> (EngagementId, CampaignId) {
    (
        EngagementId(Uuid::from_u128(1)),
        CampaignId(Uuid::from_u128(2)),
    )
}
fn parsed(value: &Value) -> duskweave::Res<RegistrationInput> {
    parse_register(&serde_json::to_vec(value).unwrap())
}
fn event(value: &Value) -> MissionRegistered {
    Mission::register(
        &parsed(value).unwrap(),
        OperationId(Uuid::from_u128(3)),
        EventId(Uuid::from_u128(4)),
        5,
    )
    .unwrap()
    .1
}

#[test]
fn initial_inference_only_registration_is_v3_and_round_trips() {
    let (e, c) = scope();
    let raw = consent::registration(e, c);
    assert!(
        parsed(&raw).is_ok(),
        "new optional inference attachment must parse"
    );
    let ev = event(&raw);
    assert_eq!(ev.version, 3);
    assert_eq!(check_event(&ev), Ok(()));
    let wire = serde_json::to_value(&ev).unwrap();
    assert_eq!(
        wire["fields"]["m1_inference_permission"],
        raw["m1_inference_permission"]
    );
    assert!(wire["fields"].get("m1_permission").is_none());
    let restored: MissionRegistered = serde_json::from_value(wire).unwrap();
    assert_eq!(restored, ev);
}

#[test]
fn every_new_field_is_required_nonnull_and_objects_are_closed() {
    let (e, c) = scope();
    let good = consent::registration(e, c);
    assert!(parsed(&good).is_ok());
    for path in [
        "/m1_inference_permission",
        "/m1_inference_permission/campaign_limits",
        "/m1_inference_permission/operation_limits",
    ] {
        let fields: Vec<_> = good
            .pointer(path)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for field in fields {
            let mut missing = good.clone();
            missing
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&field);
            assert!(parsed(&missing).is_err(), "missing {path}/{field}");
            let mut null = good.clone();
            null.pointer_mut(path).unwrap()[&field] = Value::Null;
            assert!(parsed(&null).is_err(), "null {path}/{field}");
        }
        let mut extra = good.clone();
        extra.pointer_mut(path).unwrap()["secret"] = json!("SYNTHETIC_SECRET_SENTINEL");
        let error = format!("{:?}", parsed(&extra).unwrap_err());
        assert!(!error.contains("SYNTHETIC_SECRET_SENTINEL"));
    }
    let mut null = good.clone();
    null["m1_inference_permission"] = Value::Null;
    assert!(parsed(&null).is_err());
    let text = serde_json::to_string(&good).unwrap();
    for (needle, duplicate) in [
        ("\"calls\":10", "\"calls\":10,\"calls\":10"),
        (
            "\"mission_revision\":1",
            "\"mission_revision\":1,\"mission_revision\":1",
        ),
        ("\"concurrency\":1", "\"concurrency\":1,\"concurrency\":1"),
    ] {
        assert!(text.contains(needle));
        assert!(parse_register(text.replace(needle, duplicate).as_bytes()).is_err());
    }
    let mut padded = text.into_bytes();
    padded.resize(MAX_INPUT_BYTES, b' ');
    assert!(parse_register(&padded).is_ok());
    padded.push(b' ');
    assert!(parse_register(&padded).is_err());
}

#[test]
fn scope_basis_profile_terms_and_window_reject_unsupported_values() {
    let (e, c) = scope();
    let good = consent::registration(e, c);
    for (field, value) in [
        ("version", json!("m1_inference_permission_v2")),
        ("permission_id", json!(Uuid::nil())),
        ("provider_terms_ref", json!(Uuid::nil())),
        ("engagement_id", json!(Uuid::nil())),
        ("campaign_id", json!(Uuid::nil())),
        ("engagement_id", json!(Uuid::from_u128(99))),
        ("campaign_id", json!(Uuid::from_u128(99))),
        ("operator_ref", json!(Uuid::from_u128(99))),
        ("authority_ref", json!(Uuid::from_u128(99))),
        ("authority_revision", json!(2)),
        ("authority_revision", json!(-1)),
        ("mission_revision", json!(0)),
        ("mission_revision", json!(2)),
        ("goal_ref", json!(Uuid::from_u128(99))),
        ("exercise_mode", json!("defender_informed")),
        ("recipient", json!("https://other.invalid")),
        ("profile", json!("other")),
        ("provider_terms_approved", json!(false)),
        ("starts_at", json!(1699999999)),
        ("ends_at", json!(1700086401)),
        ("ends_at", json!(1700000001)),
        ("field_classes", json!([])),
        ("field_classes", json!(["raw_content"])),
        ("field_classes", json!(["capacity", "capacity"])),
        ("field_classes", json!(vec!["capacity"; 9])),
    ] {
        let mut changed = good.clone();
        changed["m1_inference_permission"][field] = value;
        assert!(parsed(&changed).is_err(), "{field}");
    }
    let mut subset = good.clone();
    subset["m1_inference_permission"]["field_classes"] = json!(["capacity"]);
    assert!(parsed(&subset).is_ok());
    subset["m1_inference_permission"]["field_classes"] = json!([
        "mission_question_constraints",
        "scoped_opaque_references",
        "owner_qualified_summaries",
        "admitted_operator_premises",
        "manifest_availability",
        "omissions_unknown",
        "capacity",
        "admitted_repair_fields_codes"
    ]);
    subset["m1_inference_permission"]["starts_at"] = subset["starts_at"].clone();
    subset["m1_inference_permission"]["ends_at"] = subset["ends_at"].clone();
    assert!(parsed(&subset).is_ok());
}

#[test]
fn each_campaign_and_operation_limit_enforces_its_boundary() {
    let (e, c) = scope();
    let good = consent::registration(e, c);
    for field in [
        "calls",
        "request_bytes",
        "response_bytes",
        "output_token_capacity",
        "wall_time_ms",
    ] {
        for value in [json!(0), json!(-1), json!(u64::MAX), json!(1.5), json!("1")] {
            let mut changed = good.clone();
            changed["m1_inference_permission"]["campaign_limits"][field] = value;
            assert!(parsed(&changed).is_err(), "campaign {field}");
        }
        for valid in [1, i64::MAX] {
            let mut changed = good.clone();
            changed["m1_inference_permission"]["campaign_limits"][field] = json!(valid);
            assert!(parsed(&changed).is_ok());
        }
    }
    for (field, cap) in [
        ("context_bytes", 4096),
        ("candidates", 8),
        ("request_bytes", 16384),
        ("configuration_bytes", 16384),
        ("response_bytes", 65536),
        ("decision_wall_time_ms", 20000),
        ("connect_time_ms", 5000),
        ("proposal_bytes", 8192),
        ("repair_bytes", 2048),
        ("text_field_bytes", 1024),
        ("repair_error_codes", 8),
        ("json_depth", 8),
        ("collection_entries", 64),
    ] {
        for invalid in [json!(0), json!(-1), json!(cap + 1), json!(u64::MAX)] {
            let mut changed = good.clone();
            changed["m1_inference_permission"]["operation_limits"][field] = invalid;
            assert!(parsed(&changed).is_err(), "operation {field}");
        }
        let mut smaller = good.clone();
        smaller["m1_inference_permission"]["operation_limits"][field] = json!(1);
        if field == "decision_wall_time_ms" {
            smaller["m1_inference_permission"]["operation_limits"]["connect_time_ms"] = json!(1);
        }
        assert!(parsed(&smaller).is_ok(), "smaller {field}");
    }
    for (field, fixed) in [
        ("calls_per_decision", 2),
        ("output_tokens_per_call", 4096),
        ("concurrency", 1),
    ] {
        for invalid in [0, -1, fixed - 1, fixed + 1] {
            let mut changed = good.clone();
            changed["m1_inference_permission"]["operation_limits"][field] = json!(invalid);
            assert!(parsed(&changed).is_err(), "fixed {field}");
        }
    }
    let mut crossed = good.clone();
    crossed["m1_inference_permission"]["operation_limits"]["decision_wall_time_ms"] = json!(4999);
    assert!(parsed(&crossed).is_err());
}

#[test]
fn legacy_bytes_omit_inference_and_dual_consent_uses_v3() {
    let (e, c) = scope();
    for (raw, version) in [(db_support::reg_json(e, c), 1), (ct::registration(e, c), 2)] {
        let ev = event(&raw);
        assert_eq!(ev.version, version);
        let bytes = serde_json::to_vec(&ev).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("m1_inference_permission"));
        let restored: MissionRegistered = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
        assert_eq!(check_event(&restored), Ok(()));
    }
    let mut both = consent::registration(e, c);
    both["m1_permission"] = ct::permission();
    let ev = event(&both);
    assert_eq!(ev.version, 3);
    assert_eq!(check_event(&ev), Ok(()));
    let wire = serde_json::to_value(&ev).unwrap();
    assert_eq!(wire["fields"]["m1_permission"], both["m1_permission"]);
    assert_eq!(
        wire["fields"]["m1_inference_permission"],
        both["m1_inference_permission"]
    );
}

#[test]
fn construction_and_event_decoding_reestablish_owner_bindings() {
    let (e, c) = scope();
    let raw = consent::registration(e, c);
    let mut input = parsed(&raw).unwrap();
    input.campaign_id = CampaignId(Uuid::from_u128(99));
    assert!(
        Mission::register(
            &input,
            OperationId(Uuid::from_u128(3)),
            EventId(Uuid::from_u128(4)),
            5
        )
        .is_err()
    );
    let ev = event(&raw);
    let wire = serde_json::to_value(&ev).unwrap();
    for (pointer, value) in [
        ("/version", json!(1)),
        ("/version", json!(2)),
        ("/version", json!(4)),
        ("/producer", json!("cognition")),
        ("/kind", json!("reasoning_decision")),
        ("/owner_revision", json!(2)),
        ("/affected_entity", json!(Uuid::from_u128(99))),
        ("/causation_id", json!(Uuid::from_u128(99))),
        ("/correlation_id", json!(Uuid::from_u128(99))),
        ("/event_id", json!(Uuid::nil())),
        ("/operation_id", json!(Uuid::nil())),
        ("/engagement_id", json!(Uuid::nil())),
        ("/campaign_id", json!(Uuid::nil())),
        ("/fields/m1_inference_permission", Value::Null),
        ("/fields/m1_inference_permission/mission_revision", json!(2)),
        (
            "/fields/m1_inference_permission/operator_ref",
            json!(Uuid::nil()),
        ),
        (
            "/fields/m1_inference_permission/authority_revision",
            json!(0),
        ),
        (
            "/fields/m1_inference_permission/permission_id",
            json!(Uuid::nil()),
        ),
        (
            "/fields/m1_inference_permission/campaign_limits/calls",
            json!(0),
        ),
        (
            "/fields/m1_inference_permission/operation_limits/concurrency",
            json!(2),
        ),
    ] {
        let mut invalid = wire.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        if let Ok(decoded) = serde_json::from_value::<MissionRegistered>(invalid) {
            assert!(check_event(&decoded).is_err(), "{pointer}");
        }
    }
}

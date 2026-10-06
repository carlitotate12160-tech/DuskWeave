use duskweave::input::parse_register;
use duskweave::mission::*;
use duskweave::trajectory::check_event;
use duskweave::{Fail, Res};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_permission.rs"]
mod policy;

#[test]
fn valid_attachment_is_admitted_and_emits_version_two() {
    let (e, c) = db_support::scope(1);
    let raw = policy::registration(e, c);
    let parsed = parse_register(&serde_json::to_vec(&raw).unwrap());
    assert!(parsed.is_ok(), "valid M1 attachment rejected: {parsed:?}");
    let (_, event) = Mission::register(
        &parsed.unwrap(),
        OperationId(Uuid::from_u128(1)),
        EventId(Uuid::from_u128(2)),
        1700000001,
    )
    .unwrap();
    assert_eq!(event.version, 2);
    assert_eq!(check_event(&event), Ok(()));
    assert_eq!(
        serde_json::to_value(event).unwrap()["fields"]["m1_permission"],
        raw["m1_permission"]
    );
}

fn parse(value: &Value) -> Res<RegistrationInput> {
    parse_register(&serde_json::to_vec(value).unwrap())
}

fn fixture() -> Value {
    policy::registration(
        EngagementId(Uuid::from_u128(10)),
        CampaignId(Uuid::from_u128(11)),
    )
}

#[test]
fn legacy_representation_and_supported_version_shape_pairs() {
    let (e, c) = db_support::scope(2);
    let input = db_support::reg_input(e, c);
    let (_, legacy) = Mission::register(
        &input,
        OperationId(Uuid::from_u128(1)),
        EventId(Uuid::from_u128(2)),
        3,
    )
    .unwrap();
    let value = serde_json::to_value(&legacy).unwrap();
    assert_eq!(legacy.version, CONTRACT_VERSION);
    assert!(value["fields"].get("m1_permission").is_none());
    assert_eq!(check_event(&legacy), Ok(()));
    let mut absent = legacy.clone();
    absent.version = 2;
    assert_eq!(check_event(&absent), Err("unsupported_contract"));
    let (_, mut m1) = Mission::register(
        &policy::input(e, c),
        legacy.operation_id,
        legacy.event_id,
        3,
    )
    .unwrap();
    for version in [0, 1, 3, u32::MAX] {
        m1.version = version;
        assert_eq!(check_event(&m1), Err("unsupported_contract"));
    }
    let decoded: MissionRegistered = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
}

#[test]
fn closed_schema_rejects_missing_null_unknown_and_duplicate_fields() {
    let base = fixture();
    for field in base["m1_permission"].as_object().unwrap().keys() {
        let mut invalid = base.clone();
        invalid["m1_permission"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            parse(&invalid),
            Err(Fail::Input("schema_violation")),
            "{field}"
        );
        invalid = base.clone();
        invalid["m1_permission"][field] = Value::Null;
        assert!(parse(&invalid).is_err(), "null {field} admitted");
    }
    for pointer in [
        "",
        "/m1_permission",
        "/m1_permission/campaign_limits",
        "/m1_permission/contact_rules/0",
        "/m1_permission/contact_rules/0/rule",
        "/m1_permission/discovery_rules/0",
        "/m1_permission/excluded_names/0",
    ] {
        let mut invalid = base.clone();
        invalid
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("secret".into(), json!("SENTINEL-M1"));
        assert!(
            matches!(
                parse(&invalid),
                Err(Fail::Input("schema_violation" | "malformed_json"))
            ),
            "{pointer}"
        );
    }
    let mut invalid = base.clone();
    invalid["m1_permission"] = Value::Null;
    assert_eq!(parse(&invalid), Err(Fail::Input("schema_violation")));
    for (old, new) in [
        (
            "\"policy_version\":1",
            "\"policy_version\":1,\"policy_version\":1",
        ),
        ("\"episodes\":2", "\"episodes\":2,\"episodes\":2"),
        ("\"priority\":1", "\"priority\":1,\"priority\":1"),
        (
            "\"label_suffix\":\"example.com\"",
            "\"label_suffix\":\"example.com\",\"label_suffix\":\"example.com\"",
        ),
    ] {
        let raw = base.to_string();
        assert!(raw.contains(old));
        assert!(matches!(
            parse_register(raw.replacen(old, new, 1).as_bytes()),
            Err(Fail::Input("schema_violation" | "malformed_json"))
        ));
    }
    let raw = base.to_string();
    let attachment = base["m1_permission"].to_string();
    let duplicate = raw.replacen(
        &format!("\"m1_permission\":{attachment}"),
        &format!("\"m1_permission\":{attachment},\"m1_permission\":{attachment}"),
        1,
    );
    assert_eq!(
        parse_register(duplicate.as_bytes()),
        Err(Fail::Input("schema_violation"))
    );
}

#[test]
fn finite_totals_and_fixed_configuration_are_required() {
    let base = fixture();
    for field in base["m1_permission"]["campaign_limits"]
        .as_object()
        .unwrap()
        .keys()
    {
        for value in [
            json!(0),
            json!(-1),
            json!(9223372036854775808_u64),
            json!(1.5),
            Value::Null,
        ] {
            let mut invalid = base.clone();
            invalid["m1_permission"]["campaign_limits"][field] = value;
            assert!(parse(&invalid).is_err(), "{field}");
        }
        let mut invalid = base.clone();
        invalid["m1_permission"]["campaign_limits"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(parse(&invalid).is_err());
        let mut valid = base.clone();
        valid["m1_permission"]["campaign_limits"][field] = json!(i64::MAX);
        assert!(parse(&valid).is_ok());
    }
    for (field, values) in [
        ("policy_version", vec![json!(0), json!(2), json!(-1)]),
        ("concurrency", vec![json!(0), json!(2), json!(256)]),
        (
            "provider_disclosure",
            vec![json!("other"), json!("https://crt.sh")],
        ),
        (
            "approved_path",
            vec![json!(""), json!("/a"), json!("/?secret=x"), json!("/\n")],
        ),
        (
            "resolver_ipv4",
            vec![
                json!("resolver.example.com"),
                json!("::1"),
                json!("http://192.0.2.53"),
                json!("u:p@192.0.2.53"),
                json!("999.1.1.1"),
            ],
        ),
        ("vantage_ref", vec![json!(Uuid::nil()), json!("vantage")]),
    ] {
        for value in values {
            let mut invalid = base.clone();
            invalid["m1_permission"][field] = value;
            assert!(parse(&invalid).is_err(), "{field}");
        }
    }
}

#[test]
fn canonical_names_label_boundaries_exclusions_and_separate_purposes() {
    let base = fixture();
    for name in [
        "",
        "Example.com",
        ".example.com",
        "example.com.",
        "x..com",
        "-x.com",
        "x-.com",
        "*.example.com",
        "https://example.com",
        "u@example.com",
        "example.com:443",
        "a/b",
        "é.com",
        "a\n.com",
    ] {
        let mut invalid = base.clone();
        invalid["m1_permission"]["discovery_rules"] = json!([{"label_suffix": name}]);
        assert!(parse(&invalid).is_err(), "invalid canonical rule admitted");
    }
    for name in [
        format!("{}.com", "a".repeat(64)),
        format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(63)
        ),
    ] {
        let mut invalid = base.clone();
        invalid["m1_permission"]["excluded_names"] = json!([{"exact": name}]);
        assert!(parse(&invalid).is_err());
    }
    for name in ["example.com", "child.example.com", "deep.child.example.com"] {
        let mut valid = base.clone();
        valid["m1_permission"]["ct_base_domain"] = json!(name);
        assert!(parse(&valid).is_ok());
    }
    let mut invalid = base.clone();
    invalid["m1_permission"]["ct_base_domain"] = json!("notexample.com");
    assert_eq!(parse(&invalid), Err(Fail::Input("m1_invalid_disclosure")));
    invalid["m1_permission"]["ct_base_domain"] = json!("excluded.example.com");
    assert_eq!(parse(&invalid), Err(Fail::Input("m1_invalid_disclosure")));
    let mut valid = base.clone();
    valid["m1_permission"]["discovery_rules"] = json!([{"exact": "example.com"}]);
    valid["m1_permission"]["excluded_names"] = json!([{"exact": "api.example.net"}]);
    assert!(parse(&valid).is_ok(), "overlap is not widened authority");
    valid["m1_permission"]["contact_rules"] = json!([]);
    assert!(parse(&valid).is_ok(), "discovery-only attachment is valid");
    valid["m1_permission"]["discovery_rules"] = json!([{"exact": "child.example.com"}]);
    assert_eq!(parse(&valid), Err(Fail::Input("m1_invalid_disclosure")));
}

#[test]
fn rule_collections_priorities_assets_and_parent_window_are_bounded() {
    let base = fixture();
    for field in ["discovery_rules", "contact_rules", "excluded_names"] {
        let mut invalid = base.clone();
        let rule = invalid["m1_permission"][field][0].clone();
        invalid["m1_permission"][field] = json!([rule, rule]);
        assert!(parse(&invalid).is_err(), "duplicate {field}");
        invalid["m1_permission"][field] = json!(vec![rule; 9]);
        assert_eq!(parse(&invalid), Err(Fail::Input("m1_rule_limit")));
    }
    let mut invalid = base.clone();
    invalid["m1_permission"]["discovery_rules"] = json!([]);
    assert_eq!(parse(&invalid), Err(Fail::Input("m1_rule_limit")));
    for asset in [Uuid::nil(), Uuid::from_u128(0x23), Uuid::from_u128(0x99)] {
        let mut invalid = base.clone();
        invalid["m1_permission"]["contact_rules"][0]["asset_ref"] = json!(asset);
        assert_eq!(parse(&invalid), Err(Fail::Input("m1_invalid_contact")));
    }
    let mut invalid = base.clone();
    invalid["included_assets"]
        .as_array_mut()
        .unwrap()
        .push(json!(Uuid::from_u128(0x23)));
    invalid["m1_permission"]["contact_rules"][0]["asset_ref"] = json!(Uuid::from_u128(0x23));
    assert_eq!(parse(&invalid), Err(Fail::Input("m1_invalid_contact")));
    let mut duplicate = base.clone();
    let mut other = duplicate["m1_permission"]["contact_rules"][0].clone();
    other["rule"] = json!({"exact": "other.example.net"});
    duplicate["m1_permission"]["contact_rules"]
        .as_array_mut()
        .unwrap()
        .push(other);
    assert_eq!(parse(&duplicate), Err(Fail::Input("m1_invalid_contact")));
    for priority in [0, -1, 256] {
        let mut invalid = base.clone();
        invalid["m1_permission"]["contact_rules"][0]["priority"] = json!(priority);
        assert!(parse(&invalid).is_err());
    }
    for (start, end) in [
        (1699999999, 1700000002),
        (1700000001, 1700086401),
        (1700000001, 1700000001),
        (1700000002, 1700000001),
    ] {
        let mut invalid = base.clone();
        invalid["m1_permission"]["starts_at"] = json!(start);
        invalid["m1_permission"]["ends_at"] = json!(end);
        assert_eq!(parse(&invalid), Err(Fail::Input("m1_invalid_window")));
    }
    let mut valid = base.clone();
    valid["m1_permission"]["starts_at"] = valid["starts_at"].clone();
    valid["m1_permission"]["ends_at"] = valid["ends_at"].clone();
    valid["m1_permission"]["contact_rules"][0]["priority"] = json!(255);
    assert!(parse(&valid).is_ok());
}

#[test]
fn decoded_attachment_revalidates_owner_constraints() {
    let input = parse(&fixture()).unwrap();
    let (_, event) = Mission::register(
        &input,
        OperationId(Uuid::from_u128(1)),
        EventId(Uuid::from_u128(2)),
        3,
    )
    .unwrap();
    for pointer in [
        "/fields/m1_permission/campaign_limits/episodes",
        "/fields/m1_permission/concurrency",
        "/fields/m1_permission/contact_rules/0/priority",
    ] {
        let mut value = serde_json::to_value(&event).unwrap();
        *value.pointer_mut(pointer).unwrap() = json!(0);
        let decoded: MissionRegistered = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(check_event(&decoded), Err("invalid_fields"));
        let fields: RegistrationFields = serde_json::from_value(value["fields"].clone()).unwrap();
        let forged = RegistrationInput {
            fields,
            ..input.clone()
        };
        assert!(Mission::register(&forged, event.operation_id, event.event_id, 3).is_err());
    }
    let mut oversized = fixture().to_string();
    oversized.push_str(&" ".repeat(16 * 1024));
    assert_eq!(
        parse_register(oversized.as_bytes()),
        Err(Fail::Input("size_limit"))
    );
}

#[test]
fn self_consistent_nil_m1_identities_cannot_be_admitted_history() {
    let input = parse(&fixture()).unwrap();
    let (_, event) = Mission::register(
        &input,
        OperationId(Uuid::from_u128(1)),
        EventId(Uuid::from_u128(2)),
        3,
    )
    .unwrap();
    for field in ["event_id", "engagement_id", "campaign_id", "operation_id"] {
        let mut value = serde_json::to_value(&event).unwrap();
        value[field] = json!(Uuid::nil());
        if field == "campaign_id" {
            value["affected_entity"] = json!(Uuid::nil());
        }
        if field == "operation_id" {
            value["causation_id"] = json!(Uuid::nil());
            value["correlation_id"] = json!(Uuid::nil());
        }
        let invalid: MissionRegistered = serde_json::from_value(value).unwrap();
        assert_eq!(check_event(&invalid), Err("scope_violation"), "{field}");
    }
}

#[test]
fn maximum_rules_and_name_length_preserve_declared_order() {
    let mut raw = fixture();
    let long_name = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );
    assert_eq!(long_name.len(), 253);
    raw["m1_permission"]["ct_base_domain"] = json!(long_name);
    let mut discoveries = vec![json!({"exact": long_name})];
    let contacts: Vec<_> = (0..8).map(|i| json!({"asset_ref": Uuid::from_u128(0x100+i), "rule": {"exact": format!("c{i}.example.net")}, "priority": 8-i})).collect();
    discoveries.extend((0..7).map(|i| json!({"exact": format!("d{i}.example.com")})));
    raw["included_assets"] = json!(
        (0..8)
            .map(|i| Uuid::from_u128(0x100 + i))
            .collect::<Vec<_>>()
    );
    raw["m1_permission"]["discovery_rules"] = json!(discoveries);
    raw["m1_permission"]["contact_rules"] = json!(contacts);
    raw["m1_permission"]["excluded_names"] = json!(
        (0..8)
            .map(|i| json!({"label_suffix": format!("x{i}.example.com")}))
            .collect::<Vec<_>>()
    );
    let input = parse(&raw).unwrap();
    let (_, event) = Mission::register(
        &input,
        OperationId(Uuid::from_u128(1)),
        EventId(Uuid::from_u128(2)),
        3,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(event).unwrap()["fields"]["m1_permission"],
        raw["m1_permission"]
    );
}

use duskweave::Fail;
use duskweave::input::MAX_INPUT_BYTES;
use duskweave::mission::*;
use duskweave::withdrawal::*;
use duskweave::withdrawal_input::{parse_withdrawal, read_withdrawal_file};
use serde_json::{Value, json};
use uuid::Uuid;

fn request() -> Value {
    json!({"engagement_id": Uuid::from_u128(1), "campaign_id": Uuid::from_u128(2),
        "operator_ref": Uuid::from_u128(3), "expected_mission_revision": 1,
        "reason": "operator_requested"})
}

#[test]
fn ingress_is_closed_typed_and_bounded() {
    for reason in ["operator_requested", "authorization_ended", "scope_concern"] {
        let mut value = request();
        value["reason"] = reason.into();
        let parsed = parse_withdrawal(value.to_string().as_bytes()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }
    for key in ["engagement_id", "campaign_id", "operator_ref"] {
        let mut value = request();
        value[key] = Uuid::nil().to_string().into();
        assert_eq!(
            parse_withdrawal(value.to_string().as_bytes()),
            Err(Fail::Input("nil_reference"))
        );
    }
    for revision in [json!(0), json!(-1), json!(1.5), json!("1"), json!(u64::MAX)] {
        let mut value = request();
        value["expected_mission_revision"] = revision;
        assert!(parse_withdrawal(value.to_string().as_bytes()).is_err());
    }
    let mut value = request();
    value["expected_mission_revision"] = json!(i64::MAX);
    assert!(parse_withdrawal(value.to_string().as_bytes()).is_ok());
    for key in ["reason", "operator_ref"] {
        let mut value = request();
        value[key] = "SYNTHETIC_SECRET_SENTINEL".into();
        let error = parse_withdrawal(value.to_string().as_bytes()).unwrap_err();
        assert!(!format!("{error:?}").contains("SYNTHETIC_SECRET_SENTINEL"));
    }
    let mut unknown = request();
    unknown["secret"] = "SYNTHETIC_SECRET_SENTINEL".into();
    assert_eq!(
        parse_withdrawal(unknown.to_string().as_bytes()),
        Err(Fail::Input("schema_violation"))
    );
    for raw in [b"".as_slice(), b"{", b"null", b"{}", b"[]", b"false"] {
        assert!(parse_withdrawal(raw).is_err());
    }
    let valid = request().to_string();
    assert!(parse_withdrawal(format!("{valid} {{}}").as_bytes()).is_err());
    let duplicate = valid.replacen('{', "{\"reason\":\"scope_concern\",", 1);
    assert_eq!(
        parse_withdrawal(duplicate.as_bytes()),
        Err(Fail::Input("schema_violation"))
    );
    let mut limit = valid.into_bytes();
    limit.resize(MAX_INPUT_BYTES, b' ');
    assert!(parse_withdrawal(&limit).is_ok());
    limit.push(b' ');
    assert_eq!(parse_withdrawal(&limit), Err(Fail::Input("size_limit")));
    let path = std::env::temp_dir().join(format!("dw-withdraw-input-{}.json", std::process::id()));
    std::fs::write(&path, &limit).unwrap();
    assert_eq!(read_withdrawal_file(&path), Err(Fail::Input("size_limit")));
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        read_withdrawal_file(&path),
        Err(Fail::Input("unreadable_input"))
    );
}

#[test]
fn immutable_event_rejects_each_invalid_header_or_transition() {
    let request = parse_withdrawal(request().to_string().as_bytes()).unwrap();
    let event = MissionAuthorityWithdrawn::new(
        request,
        OperationId(Uuid::from_u128(4)),
        OperationId(Uuid::from_u128(5)),
        EventId(Uuid::from_u128(6)),
        123,
    )
    .unwrap();
    assert_eq!(event.occurred_at, event.recorded_at);
    let raw = serde_json::to_value(&event).unwrap();
    for (field, replacement) in [
        ("event_id", json!(Uuid::nil())),
        ("operation_id", json!(Uuid::nil())),
        ("registration_operation_id", json!(Uuid::nil())),
        ("producer", json!("trajectory")),
        ("kind", json!("mission_registered")),
        ("version", json!(2)),
        ("owner_revision", json!(1)),
        ("affected_entity", json!(Uuid::from_u128(99))),
        ("causation_id", json!(Uuid::nil())),
        ("correlation_id", json!(Uuid::nil())),
        ("recorded_at", json!(124)),
    ] {
        let mut changed = raw.clone();
        changed[field] = replacement;
        let parsed: MissionAuthorityWithdrawn = serde_json::from_value(changed).unwrap();
        assert!(parsed.validate().is_err(), "accepted invalid {field}");
    }
    let mut stale = event.clone();
    stale.request.expected_mission_revision = 2;
    assert_eq!(stale.validate(), Err(Fail::State("unsupported_contract")));
    let mut unknown = raw;
    unknown["extra"] = json!("SYNTHETIC_SECRET_SENTINEL");
    assert!(serde_json::from_value::<MissionAuthorityWithdrawn>(unknown).is_err());
    let mut nil = event.request;
    nil.operator_ref = OperatorRef(Uuid::nil());
    assert!(
        MissionAuthorityWithdrawn::new(
            nil,
            event.operation_id,
            event.registration_operation_id,
            event.event_id,
            123
        )
        .is_err()
    );
}

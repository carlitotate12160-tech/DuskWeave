//! Boundary counterexamples for mutants that survived the deterministic
//! suite in the M0 parser-mutation qualification. Each assertion protects a
//! requirement of the strict input boundary: the declared byte bound is a
//! fixed limit (not self-adjusting), exact-limit input still parses,
//! over-limit bytes are rejected before deserialization with `size_limit`,
//! and truncated bytes are `malformed_json`, not `schema_violation`.

use duskweave::Fail;
use duskweave::input::parse_register;
use duskweave::planning_input::parse_planning;
use duskweave::withdrawal_input::parse_withdrawal;
use serde_json::{Value, json};
use uuid::Uuid;

/// The declared ingress bound: `MAX_INPUT_BYTES` in the production source.
/// Spelled as a literal so a mutant shrinking the constant cannot also move
/// this test's target size.
const DECLARED_BOUND: usize = 16 * 1024;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n + 1)
}

fn register_json() -> Value {
    json!({
        "engagement_id": id(1), "campaign_id": id(2), "operator_ref": id(10),
        "authority_ref": id(11), "authority_revision": 1, "goal_ref": id(12),
        "included_assets": [id(20)], "excluded_assets": [id(30)],
        "exercise_mode": "blind", "starts_at": 100, "ends_at": 200
    })
}

fn planning_json() -> Value {
    json!({
        "engagement_id": id(1), "campaign_id": id(2), "purpose_ref": id(3),
        "asset_ref": id(4), "expected_mission_revision": 1,
        "current_authority_confirmed": true
    })
}

fn withdrawal_json() -> Value {
    json!({
        "engagement_id": id(1), "campaign_id": id(2), "operator_ref": id(3),
        "expected_mission_revision": 1, "reason": "operator_requested"
    })
}

/// Serialize `value` and pad with trailing spaces to exactly `n` bytes;
/// serde_json accepts trailing whitespace, so only the length exercises the
/// byte bound and the document stays valid.
fn padded(value: &Value, n: usize) -> Vec<u8> {
    let mut raw = value.to_string().into_bytes();
    assert!(raw.len() <= n, "fixture exceeds requested size");
    raw.resize(n, b' ');
    raw
}

#[test]
fn parsers_accept_input_at_the_declared_bound() {
    // Kills `16 * 1024 -> 16 + 1024` (cap shrinks to 1040) and `> -> >=` /
    // `> -> ==` at each size guard: exactly-boundary bytes must still parse.
    // A mid-range document also fails the shrunk-cap mutant.
    for size in [8 * 1024, DECLARED_BOUND] {
        assert!(parse_register(&padded(&register_json(), size)).is_ok());
        assert!(parse_planning(&padded(&planning_json(), size)).is_ok());
        assert!(parse_withdrawal(&padded(&withdrawal_json(), size)).is_ok());
    }
}

#[test]
fn planning_parses_a_valid_request() {
    // Before this qualification, no deterministic test parsed a valid
    // planning request through the public byte boundary, so mutations
    // rejecting ordinary sizes survived.
    let request = parse_planning(&planning_json().to_string().into_bytes()).unwrap();
    assert_eq!(request.expected_mission_revision, 1);
    assert!(request.current_authority_confirmed);
    assert_eq!(request.asset_ref.0, id(4));
}

#[test]
fn over_limit_bytes_stop_before_deserialization() {
    // Kills `|| -> &&` and `> -> ==` at each size guard: oversized bytes must
    // not reach serde, so the safe category is `size_limit` even when the
    // bytes themselves are well-formed.
    for (size, document) in [
        (DECLARED_BOUND + 1, planning_json()),
        (DECLARED_BOUND + 1, withdrawal_json()),
        (DECLARED_BOUND + 1, register_json()),
    ] {
        let raw = padded(&document, size);
        assert_eq!(
            parse_planning(&raw),
            Err(Fail::Input("size_limit")),
            "planning accepted or misclassified over-limit bytes"
        );
        assert_eq!(
            parse_withdrawal(&raw),
            Err(Fail::Input("size_limit")),
            "withdrawal accepted or misclassified over-limit bytes"
        );
        assert_eq!(
            parse_register(&raw),
            Err(Fail::Input("size_limit")),
            "register accepted or misclassified over-limit bytes"
        );
    }
    assert_eq!(parse_planning(b""), Err(Fail::Input("size_limit")));
}

#[test]
fn truncated_bytes_are_malformed_not_schema_violations() {
    // Kills `is_syntax || is_eof -> &&`: a truncated document ends at EOF,
    // which is malformed input; schema_violation is reserved for well-formed
    // JSON with the wrong shape.
    let planning_prefix = planning_json().to_string();
    for truncated in [
        b"{".as_slice(),
        b"{\"engagement_id\":".as_slice(),
        &planning_prefix.as_bytes()[..40],
    ] {
        assert_eq!(
            parse_planning(truncated),
            Err(Fail::Input("malformed_json")),
            "planning misclassified truncated bytes"
        );
    }
    let withdrawal_prefix = withdrawal_json().to_string();
    for truncated in [
        b"{".as_slice(),
        b"{\"engagement_id\":".as_slice(),
        &withdrawal_prefix.as_bytes()[..40],
    ] {
        assert_eq!(
            parse_withdrawal(truncated),
            Err(Fail::Input("malformed_json")),
            "withdrawal misclassified truncated bytes"
        );
    }
    assert_eq!(
        parse_planning(b"{}"),
        Err(Fail::Input("schema_violation")),
        "well-formed but wrong-shape JSON must remain a schema violation"
    );
    assert_eq!(
        parse_withdrawal(b"{}"),
        Err(Fail::Input("schema_violation")),
        "well-formed but wrong-shape JSON must remain a schema violation"
    );
}

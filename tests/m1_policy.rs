use duskweave::m1_policy::*;
use duskweave::m1_policy_input::*;
use duskweave::mission::*;
use duskweave::{Fail, Res};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_policy.rs"]
mod support;

struct Reader(M1PolicySnapshot);
impl M1PolicyReader for Reader {
    fn read_policy_snapshot(&mut self, _: EngagementId, _: CampaignId) -> Res<M1PolicySnapshot> {
        Ok(self.0.clone())
    }
}

fn evaluate(value: &Value, query: &M1PolicyQuery, time: i64, withdrawn: bool) -> M1PolicyResult {
    let event = support::event(value);
    let marker = withdrawn.then(|| support::marker(&event));
    let snapshot = M1PolicySnapshot::new(
        event.engagement_id,
        event.campaign_id,
        Some(event),
        marker,
        time,
    )
    .unwrap();
    check_name_policy(&mut Reader(snapshot), query).unwrap()
}

#[test]
fn rule_families_assets_exclusions_and_label_boundaries_do_not_confer_other_rights() {
    let (e, c) = db_support::scope(1);
    let value = support::registration(e, c, 10000);
    for (purpose, matches) in [
        (support::discovery("example.invalid"), true),
        (support::discovery("www.example.invalid"), true),
        (support::discovery("a.b.example.invalid"), true),
        (support::discovery("badexample.invalid"), false),
        (support::discovery("example.invalid.sibling"), false),
        (support::discovery("excluded.example.invalid"), false),
        (support::discovery("a.excluded.example.invalid"), false),
        (support::discovery("other.invalid"), false),
        (support::contact("hint.example.invalid", 0x21), false),
        (support::contact("www.example.invalid", 0x21), true),
        (support::contact("www.example.invalid", 0x22), false),
        (support::contact("www.example.invalid", 0x23), false),
        (support::contact("other.invalid", 0x22), true),
        (support::contact("a.other.invalid", 0x22), true),
        (support::contact("a.other.invalid", 0x21), false),
        (support::contact("www.www.example.invalid", 0x21), false),
    ] {
        let result = evaluate(&value, &support::query(e, c, purpose), 10000, false);
        support::assert_reason(
            &result,
            if matches {
                "matches_name_policy"
            } else {
                "name_not_permitted"
            },
            if matches {
                "policy_match"
            } else {
                "policy_no_match"
            },
        );
    }
    let mut excluded = value.clone();
    excluded["m1_permission"]["excluded_names"] = json!([{"exact":"www.example.invalid"}]);
    let q = support::query(e, c, support::contact("www.example.invalid", 0x21));
    support::assert_reason(
        &evaluate(&excluded, &q, 10000, false),
        "name_not_permitted",
        "policy_no_match",
    );
}

#[test]
fn ordered_identity_policy_and_both_half_open_windows_are_distinct() {
    let (e, c) = db_support::scope(2);
    let value = support::registration(e, c, 10000);
    let q = support::query(e, c, support::discovery("www.example.invalid"));
    for (time, reason) in [
        (2799, "outside_operating_window"),
        (2800, "outside_operating_window"),
        (6399, "outside_operating_window"),
        (6400, "matches_name_policy"),
        (13599, "matches_name_policy"),
        (13600, "outside_operating_window"),
        (17200, "outside_operating_window"),
    ] {
        support::assert_reason(
            &evaluate(&value, &q, time, false),
            reason,
            if reason == "matches_name_policy" {
                "policy_match"
            } else {
                "policy_no_match"
            },
        );
    }
    let mut equal_windows = value.clone();
    equal_windows["m1_permission"]["starts_at"] = json!(2800);
    equal_windows["m1_permission"]["ends_at"] = json!(17200);
    for (time, matches) in [(2799, false), (2800, true), (17199, true), (17200, false)] {
        assert_eq!(
            evaluate(&equal_windows, &q, time, false).result == PolicyDisposition::PolicyMatch,
            matches
        );
    }
    let mut changed = q.clone();
    changed.expected_mission_revision = 17;
    support::assert_reason(
        &evaluate(&value, &changed, 10000, false),
        "stale_revision",
        "policy_no_match",
    );
    changed.expected_mission_revision = 1;
    changed.goal_ref = GoalRef(Uuid::from_u128(0x14));
    changed.exercise_mode = ExerciseMode::DefenderInformed;
    support::assert_reason(
        &evaluate(&value, &changed, 10000, false),
        "purpose_mismatch",
        "policy_no_match",
    );
    changed.goal_ref = q.goal_ref;
    support::assert_reason(
        &evaluate(&value, &changed, 10000, false),
        "mode_mismatch",
        "policy_no_match",
    );
    let mut informed = value.clone();
    informed["exercise_mode"] = json!("defender_informed");
    support::assert_reason(
        &evaluate(&informed, &q, 10000, false),
        "mode_mismatch",
        "policy_no_match",
    );
    support::assert_reason(
        &evaluate(&informed, &changed, 10000, false),
        "matches_name_policy",
        "policy_match",
    );
    let mut legacy = value.clone();
    legacy.as_object_mut().unwrap().remove("m1_permission");
    support::assert_reason(
        &evaluate(&legacy, &q, 10000, false),
        "m1_policy_absent",
        "policy_no_match",
    );
    for revision in [1, 2, 17] {
        changed.expected_mission_revision = revision;
        let result = evaluate(&value, &changed, 0, true);
        support::assert_reason(&result, "authority_withdrawn", "policy_no_match");
        assert_eq!(result.snapshot.unwrap().effective_mission_revision, 2);
    }
    let result = evaluate(&value, &q, 10000, false);
    let provenance = result.snapshot.unwrap();
    assert_eq!(provenance.authority_revision, 17);
    assert_eq!(provenance.effective_mission_revision, 1);
    assert_eq!(provenance.policy_version, Some(1));
    assert_eq!(result.checked_at, Some(10000));
}

#[test]
fn parser_is_closed_exclusive_canonical_and_bounded_without_normalization() {
    let (e, c) = db_support::scope(3);
    let valid = support::query_json(e, c, support::discovery("www.example.invalid"));
    for name in [
        "",
        "WWW.example.invalid",
        "www.example.invalid.",
        ".example.invalid",
        "a..invalid",
        "-a.invalid",
        "a-.invalid",
        "a_b.invalid",
        "é.invalid",
        "https://www.example.invalid",
        "127.0.0.1",
        "001.002.003.004",
        "999.999.999.999",
        "::1",
        "[::1]",
    ] {
        let raw = support::query_json(e, c, support::discovery(name));
        assert!(
            parse_policy_query(raw.to_string().as_bytes()).is_err(),
            "{name}"
        );
    }
    for name in [
        format!("{}.invalid", "a".repeat(64)),
        "a.".repeat(126) + "aa",
    ] {
        let raw = support::query_json(e, c, support::discovery(&name));
        assert!(parse_policy_query(raw.to_string().as_bytes()).is_err());
    }
    let longest = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "a".repeat(63),
        "a".repeat(63),
        "a".repeat(61)
    );
    assert_eq!(longest.len(), 253);
    assert!(
        parse_policy_query(
            support::query_json(e, c, support::discovery(&longest))
                .to_string()
                .as_bytes()
        )
        .is_ok()
    );
    for (key, value) in [
        ("expected_mission_revision", json!(0)),
        ("expected_mission_revision", json!(-1)),
        ("expected_mission_revision", json!(u64::MAX)),
        ("expected_mission_revision", json!(1.5)),
        ("exercise_mode", json!("unsupported")),
        ("purpose", json!({"unknown":{"name":"a.invalid"}})),
        (
            "purpose",
            json!({"discovery_disclosure":{"name":"a.invalid"},"contact":{"name":"a.invalid","asset_ref":Uuid::from_u128(1)}}),
        ),
        ("purpose", support::contact("a.invalid", 0)),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert!(parse_policy_query(bad.to_string().as_bytes()).is_err());
    }
    for key in ["engagement_id", "campaign_id", "goal_ref"] {
        let mut bad = valid.clone();
        bad[key] = json!(Uuid::nil());
        assert!(parse_policy_query(bad.to_string().as_bytes()).is_err());
    }
    for key in valid.as_object().unwrap().keys() {
        let mut bad = valid.clone();
        bad[key] = Value::Null;
        assert!(parse_policy_query(bad.to_string().as_bytes()).is_err());
        bad.as_object_mut().unwrap().remove(key);
        assert!(parse_policy_query(bad.to_string().as_bytes()).is_err());
    }
    for purpose in [
        support::discovery("a.invalid"),
        support::contact("a.invalid", 1),
    ] {
        let good = support::query_json(e, c, purpose);
        let variant = good["purpose"].as_object().unwrap().keys().next().unwrap();
        for pointer in [
            "".to_string(),
            "/purpose".to_string(),
            format!("/purpose/{variant}"),
        ] {
            let mut bad = good.clone();
            bad.pointer_mut(&pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("secret".into(), json!("SENTINEL"));
            assert!(parse_policy_query(bad.to_string().as_bytes()).is_err());
        }
        for field in good["purpose"][variant].as_object().unwrap().keys() {
            let mut bad = good.clone();
            bad["purpose"][variant][field] = Value::Null;
            assert!(parse_policy_query(bad.to_string().as_bytes()).is_err());
            bad["purpose"][variant]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(parse_policy_query(bad.to_string().as_bytes()).is_err());
            let raw = good.to_string().replacen(
                &format!("\"{field}\":"),
                &format!(
                    "\"{field}\":{},\"{field}\":",
                    good["purpose"][variant][field]
                ),
                1,
            );
            assert!(parse_policy_query(raw.as_bytes()).is_err());
        }
    }
    for field in valid.as_object().unwrap().keys() {
        let raw = valid.to_string().replacen(
            &format!("\"{field}\":"),
            &format!("\"{field}\":{},\"{field}\":", valid[field]),
            1,
        );
        assert!(parse_policy_query(raw.as_bytes()).is_err());
    }
    for raw in [valid.to_string() + " {}", "{".into()] {
        assert!(parse_policy_query(raw.as_bytes()).is_err());
    }
    let mut bytes = valid.to_string().into_bytes();
    bytes.resize(MAX_INPUT_BYTES, b' ');
    assert!(parse_policy_query(&bytes).is_ok());
    bytes.push(b' ');
    assert_eq!(parse_policy_query(&bytes), Err(Fail::Input("size_limit")));
    assert_eq!(parse_policy_query(&[]), Err(Fail::Input("size_limit")));
}

#[test]
fn public_use_case_and_snapshot_construction_reject_forged_or_cross_scope_inputs() {
    struct NeverRead;
    impl M1PolicyReader for NeverRead {
        fn read_policy_snapshot(
            &mut self,
            _: EngagementId,
            _: CampaignId,
        ) -> Res<M1PolicySnapshot> {
            panic!("invalid query must not reach reader")
        }
    }
    let (e, c) = db_support::scope(4);
    let value = support::registration(e, c, 10000);
    let q = support::query(e, c, support::discovery("www.example.invalid"));
    let mut bad = q.clone();
    bad.engagement_id = EngagementId(Uuid::nil());
    assert!(check_name_policy(&mut NeverRead, &bad).is_err());
    bad = q.clone();
    bad.purpose = NamePurpose::DiscoveryDisclosure {
        name: "UPPER.invalid".into(),
    };
    assert!(check_name_policy(&mut NeverRead, &bad).is_err());
    bad = q.clone();
    bad.expected_mission_revision = 0;
    assert!(check_name_policy(&mut NeverRead, &bad).is_err());
    let event = support::event(&value);
    let mut marker = support::marker(&event);
    marker.registration_operation_id = OperationId(Uuid::from_u128(0xff));
    assert!(M1PolicySnapshot::new(e, c, Some(event.clone()), Some(marker), 10000).is_err());
    assert!(M1PolicySnapshot::new(e, c, None, Some(support::marker(&event)), 10000).is_err());
    let mut marker = support::marker(&event);
    marker.version = 99;
    assert!(M1PolicySnapshot::new(e, c, Some(event.clone()), Some(marker), 10000).is_err());
    for (field, changed) in [
        ("version", json!(99)),
        ("owner_revision", json!(0)),
        ("event_id", json!(Uuid::nil())),
        ("campaign_id", json!(Uuid::from_u128(33))),
    ] {
        let mut bad = serde_json::to_value(&event).unwrap();
        bad[field] = changed;
        assert!(
            M1PolicySnapshot::new(
                e,
                c,
                Some(serde_json::from_value(bad).unwrap()),
                None,
                10000
            )
            .is_err()
        );
    }
    let mut legacy = value.clone();
    legacy.as_object_mut().unwrap().remove("m1_permission");
    let mut nil_legacy = support::event(&legacy);
    nil_legacy.event_id = EventId(Uuid::nil());
    assert!(M1PolicySnapshot::new(e, c, Some(nil_legacy), None, 10000).is_err());
    let other_c = CampaignId(Uuid::from_u128(1));
    let snapshot = M1PolicySnapshot::new(e, other_c, None, None, 10000).unwrap();
    assert_eq!(
        check_name_policy(&mut Reader(snapshot), &q),
        Err(Fail::Store("contract_decode"))
    );
    let absent = M1PolicySnapshot::new(e, c, None, None, 10000).unwrap();
    support::assert_reason(
        &check_name_policy(&mut Reader(absent), &q).unwrap(),
        "mission_missing",
        "unavailable",
    );
    for failure in [
        Fail::Input("secret"),
        Fail::Config("secret"),
        Fail::Store("contract_decode"),
        Fail::Store("secret"),
    ] {
        support::flags(
            &serde_json::to_value(M1PolicyResult::unavailable(failure, Some(&q))).unwrap(),
        );
    }
}

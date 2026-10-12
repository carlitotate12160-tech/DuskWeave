//! Stored-contract and limits boundary coverage for the pre-send inference
//! reservation slice on the owned disposable database: the closed
//! nineteen-field consent is fully validated, window containment and
//! shape/cardinality doors hold, all sixteen operation limits and five
//! campaign totals refuse outside their declared intervals, and accepted
//! lower/upper bounds reserve exactly their derived capacity. Admin access
//! is fixture setup and deliberate stored-contract tamper only; runtime
//! assertions use the restricted logins. Cluster-role fixtures serialize
//! under db().
//!
//! Requires the packet's authorized disposable PostgreSQL service.

use duskweave::Fail;
use duskweave::m1_inference_budget::{ReservationAction::*, ReservationRequest};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;
use session_support::*;

#[test]
fn every_missing_or_replaced_consent_field_is_rejected_directly() {
    let _g = db();
    let (q, source) = budget::fixture(0xf006);
    let qv = serde_json::to_value(&q).unwrap();
    let original = serde_json::to_value(&source).unwrap();
    let names: Vec<String> = original["fields"]["m1_inference_permission"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(names.len(), 19, "closed consent contract has 19 keys");
    let mut admin = consent::admin();
    for name in names {
        for replaced in [false, true] {
            budget::tamper_refuse(
                &mut admin,
                &q,
                &qv,
                &original,
                "inference_consent_refused",
                |f| {
                    f["fields"]["m1_inference_permission"]
                        .as_object_mut()
                        .unwrap()
                        .remove(&name);
                    if replaced {
                        f["fields"]["m1_inference_permission"]["unknown"] = json!(1);
                    }
                },
            );
        }
    }
    // Tampered values refuse at consent validation, not only missing keys.
    for (field, value) in [
        ("version", json!("m1_inference_permission_v2")),
        ("provider_terms_approved", json!(false)),
        ("provider_terms_approved", json!("yes")),
        ("profile", json!("other-provider-v9")),
        ("recipient", json!("https://attacker.example/collect")),
        ("field_classes", json!(["capacity", "capacity"])),
        ("field_classes", json!(["raw_response"])),
        ("starts_at", json!("0")),
    ] {
        budget::tamper_refuse(
            &mut admin,
            &q,
            &qv,
            &original,
            "inference_consent_refused",
            |f| {
                f["fields"]["m1_inference_permission"][field] = value.clone();
            },
        );
    }
    // Consent windows outside the mission window refuse at containment.
    let consent = &original["fields"]["m1_inference_permission"];
    for (starts, ends) in [
        (
            json!(session_support::now() + 90000),
            consent["ends_at"].clone(),
        ),
        (
            consent["starts_at"].clone(),
            json!(original["fields"]["ends_at"].as_i64().unwrap() + 10),
        ),
        (
            json!(original["fields"]["starts_at"].as_i64().unwrap() - 10),
            consent["ends_at"].clone(),
        ),
    ] {
        budget::tamper_refuse(
            &mut admin,
            &q,
            &qv,
            &original,
            "inference_consent_refused",
            |f| {
                f["fields"]["m1_inference_permission"]["starts_at"] = starts.clone();
                f["fields"]["m1_inference_permission"]["ends_at"] = ends.clone();
            },
        );
    }
    // Restore the real contract; the same door admits the valid call.
    budget::tamper(&mut admin, &q, &original);
    assert!(budget::run(&q, budget::op(), Reserve).unwrap().is_some());
}

#[test]
fn absent_consent_and_expired_windows_never_allocate() {
    let _g = db();
    session_support::ensure_broker_logins();
    let now = session_support::now();
    // Registrations without usable inference consent (v2: permission only;
    // v1: neither attachment) refuse at the Rust boundary before the store.
    for (n, remove) in [
        (0xf010u128, vec!["m1_inference_permission"]),
        (0xf011, vec!["m1_permission", "m1_inference_permission"]),
    ] {
        let (e, c) = scope(n);
        let mut raw = budget::registration_value(e, c, now);
        for field in remove {
            raw.as_object_mut().unwrap().remove(field);
        }
        budget::register(e, c, &raw);
        let q: ReservationRequest = serde_json::from_value(json!({
            "version":"m1_inference_reservation_request_v1",
            "engagement_id":e,"campaign_id":c,"operator_ref":raw["operator_ref"],
            "expected_mission_revision":1,"permission_id":Uuid::from_u128(n),
            "goal_ref":raw["goal_ref"],"exercise_mode":"blind"}))
        .unwrap();
        assert_eq!(
            budget::run(&q, budget::op(), Reserve),
            Err(Fail::State("inference_consent_absent"))
        );
    }
    // Expired or displaced windows refuse at the SQL boundary: registration
    // validation rejects them, so they arrive only through contract tampering.
    let (q, source) = budget::fixture(0xf012);
    let qv = serde_json::to_value(&q).unwrap();
    let original = serde_json::to_value(&source).unwrap();
    let mut admin = consent::admin();
    budget::tamper_refuse(
        &mut admin,
        &q,
        &qv,
        &original,
        "inference_consent_refused",
        |f| {
            f["fields"]["m1_inference_permission"]["ends_at"] = json!(now - 10);
        },
    );
    budget::tamper_refuse(
        &mut admin,
        &q,
        &qv,
        &original,
        "inference_consent_refused",
        |f| {
            f["fields"]["m1_inference_permission"]["starts_at"] = json!(now + 100);
        },
    );
    // An elapsed mission window refuses even with consistent contract and row.
    let mut forged = original.clone();
    forged["fields"]["ends_at"] = json!(now - 10);
    admin
        .execute(
            "UPDATE mission.missions SET ends_at=$1 \
             WHERE engagement_id=$2 AND campaign_id=$3",
            &[&(now - 10), &q.engagement_id.0, &q.campaign_id.0],
        )
        .unwrap();
    budget::tamper(&mut admin, &q, &forged);
    budget::refused(budget::sql(
        &mut broker_client(),
        "reserve",
        &qv,
        budget::op(),
        &forged,
    ));
    assert!(budget::state(&q)["ledger"].as_array().unwrap().is_empty());
}

#[test]
fn operation_and_campaign_limits_are_validated_at_the_sql_boundary() {
    let _g = db();
    let (q, source) = budget::fixture(0xf030);
    let qv = serde_json::to_value(&q).unwrap();
    let original = serde_json::to_value(&source).unwrap();
    let mut admin = consent::admin();
    // Every named operation field must be an integer inside its accepted
    // interval; a missing key closes the 16-field object at the shape door.
    for &(field, lo, hi) in budget::LIMIT_BOUNDS {
        for case in [
            json!(lo - 1),
            json!(hi + 1),
            json!(hi.to_string()),
            json!("not-an-integer"),
            json!(hi as f64 + 0.5),
            json!(9.0e30),
            Value::Null,
        ] {
            budget::tamper_refuse(
                &mut admin,
                &q,
                &qv,
                &original,
                "inference_limits_refused",
                |f| {
                    f["fields"]["m1_inference_permission"]["operation_limits"][field] =
                        case.clone();
                },
            );
        }
        budget::tamper_refuse(
            &mut admin,
            &q,
            &qv,
            &original,
            "inference_consent_refused",
            |f| {
                f["fields"]["m1_inference_permission"]["operation_limits"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
            },
        );
    }
    // connect_time_ms may not exceed the shared decision_wall_time_ms.
    budget::tamper_refuse(
        &mut admin,
        &q,
        &qv,
        &original,
        "inference_limits_refused",
        |f| {
            let limits = &mut f["fields"]["m1_inference_permission"]["operation_limits"];
            limits["decision_wall_time_ms"] = json!(4000);
            limits["connect_time_ms"] = json!(5000);
        },
    );
    // Both limit objects are closed: an extra key refuses at the shape door.
    for nested in ["operation_limits", "campaign_limits"] {
        budget::tamper_refuse(
            &mut admin,
            &q,
            &qv,
            &original,
            "inference_consent_refused",
            |f| {
                f["fields"]["m1_inference_permission"][nested]["extra"] = json!(1);
            },
        );
    }
    // Campaign totals must be positive integers covering the complete hold;
    // a missing key closes the 5-field object at the shape door.
    let caps =
        budget::capacity_for(&original["fields"]["m1_inference_permission"]["operation_limits"]);
    for field in [
        "calls",
        "request_bytes",
        "response_bytes",
        "output_token_capacity",
        "wall_time_ms",
    ] {
        for case in [
            json!(0),
            json!(-4),
            json!("5"),
            json!(1.5),
            Value::Null,
            json!(9223372036854775808u64),
            json!(caps[field].as_i64().unwrap() - 1),
        ] {
            budget::tamper_refuse(
                &mut admin,
                &q,
                &qv,
                &original,
                "inference_capacity_exhausted",
                |f| {
                    f["fields"]["m1_inference_permission"]["campaign_limits"][field] = case.clone();
                },
            );
        }
        budget::tamper_refuse(
            &mut admin,
            &q,
            &qv,
            &original,
            "inference_consent_refused",
            |f| {
                f["fields"]["m1_inference_permission"]["campaign_limits"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
            },
        );
    }
    assert!(budget::state(&q)["ledger"].as_array().unwrap().is_empty());
    // Restore the real contract; the stored limits yield the derived capacity.
    budget::tamper(&mut admin, &q, &original);
    let record = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&record.capacity).unwrap(),
        budget::capacity_for(&original["fields"]["m1_inference_permission"]["operation_limits"])
    );
    // A forged tighter source is tampering, not a negotiation: the stored
    // contract still governs and the caller's copy must match it.
    let mut tighter = original.clone();
    tighter["fields"]["m1_inference_permission"]["operation_limits"]["request_bytes"] = json!(1);
    budget::refused(budget::sql(
        &mut broker_client(),
        "reserve",
        &qv,
        budget::op(),
        &tighter,
    ));
}

#[test]
fn plan_modes_refuse_malformed_clock_limits_without_mutation() {
    let _g = db();
    for (n, mode) in ["force_custom_plan", "force_generic_plan", "auto"]
        .into_iter()
        .enumerate()
    {
        let (q, source) = budget::fixture(0xf060 + n as u128);
        let original = serde_json::to_value(&source).unwrap();
        let before = budget::state(&q);
        let mut admin = consent::admin();
        let mut broker = broker_client();
        broker
            .batch_execute(&format!("SET plan_cache_mode='{mode}'"))
            .unwrap();
        assert_eq!(
            broker
                .query_one("SHOW plan_cache_mode", &[])
                .unwrap()
                .get::<_, String>(0),
            mode
        );
        for (field, hi) in [("connect_time_ms", 5000), ("decision_wall_time_ms", 20000)] {
            for case in [
                json!(1.5),
                json!(9.0e30),
                Value::Null,
                json!(hi.to_string()),
                json!("not-an-integer"),
                json!(0),
                json!(-1),
                json!(hi + 1),
                json!(9223372036854775808u64),
            ] {
                let mut forged = original.clone();
                forged["fields"]["m1_inference_permission"]["operation_limits"][field] = case;
                budget::tamper(&mut admin, &q, &forged);
                for _ in 0..if mode == "auto" { 6 } else { 1 } {
                    budget::assert_refused_unchanged(
                        &mut broker,
                        &q,
                        &forged,
                        "inference_limits_refused",
                    );
                }
                budget::tamper(&mut admin, &q, &original);
            }
        }
        let mut forged = original.clone();
        forged["fields"]["m1_inference_permission"]["operation_limits"]["decision_wall_time_ms"] =
            json!(4000);
        forged["fields"]["m1_inference_permission"]["operation_limits"]["connect_time_ms"] =
            json!(5000);
        budget::tamper(&mut admin, &q, &forged);
        budget::assert_refused_unchanged(&mut broker, &q, &forged, "inference_limits_refused");
        budget::tamper(&mut admin, &q, &original);
        broker.batch_execute("RESET plan_cache_mode").unwrap();
        assert_eq!(budget::state(&q), before);
    }
}

#[test]
fn boundary_limits_reserve_exactly_their_derived_capacity() {
    let _g = db();
    session_support::ensure_broker_logins();
    // Lower and upper accepted bounds register through the real path, each
    // with campaign totals exactly equal to the resulting reservation.
    for (i, mode) in ["force_custom_plan", "force_generic_plan", "auto"]
        .into_iter()
        .enumerate()
    {
        for (n, high) in [(0xf040u128, false), (0xf041, true)] {
            let (e, c) = scope(n + i as u128 * 0x40);
            let ops = budget::limits_at(high);
            let mut raw = budget::registration_value(e, c, session_support::now());
            raw["m1_inference_permission"]["operation_limits"] = ops.clone();
            raw["m1_inference_permission"]["campaign_limits"] = budget::capacity_for(&ops);
            let (_, source) = budget::register(e, c, &raw);
            let q = budget::request(e, c, &raw);
            let id = budget::op();
            let mut broker = broker_client();
            broker
                .batch_execute(&format!("SET plan_cache_mode='{mode}'"))
                .unwrap();
            let held = budget::sql(
                &mut broker,
                "reserve",
                &serde_json::to_value(&q).unwrap(),
                id,
                &serde_json::to_value(&source).unwrap(),
            )
            .unwrap()
            .unwrap();
            // Two stable call slots, two request/response allowances, 8192
            // output tokens and one shared wall-time envelope — never a
            // silently downshifted fixed field.
            assert_eq!(held["capacity"], budget::capacity_for(&ops));
            let rows = budget::state(&q)["ledger"].clone();
            assert_eq!(rows.as_array().unwrap().len(), 1);
            budget::assert_full_record(&rows[0], &q, id, "reserved_not_sent", 1, 1, &source);
            broker.batch_execute("RESET plan_cache_mode").unwrap();
        }
    }
}

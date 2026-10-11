//! Real role/lifecycle coverage for the pre-send inference reservation slice
//! on the owned disposable database: durable reserve/replay/cancel
//! generations, writer identity, direct guarded-function probes, consent and
//! capacity boundary refusals, plus upgrade compatibility. Admin access is
//! fixture setup only; runtime assertions use restricted logins.
//! Cluster-role fixtures serialize under db().
//!
//! Requires the packet's authorized disposable PostgreSQL service.

use duskweave::Fail;
use duskweave::m1_inference_budget::{ReservationAction::*, ReservationRequest, reservation};
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_mission::PgMissionStore;
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

/// The guarded function refuses every malformed call with one SQLSTATE.
fn refused(result: Result<Option<Value>, postgres::Error>) {
    let error = result.expect_err("guarded function must refuse");
    assert_eq!(
        error.code().map(postgres::error::SqlState::code),
        Some("P0001"),
        "{error}"
    );
}

/// Direct guarded-function call over the broker login.
fn sql_call(action: &str, qv: &Value, sv: &Value) -> Result<Option<Value>, postgres::Error> {
    budget::sql(&mut broker_client(), action, qv, budget::op(), sv)
}

/// Direct guarded-function reserve probe over the broker login.
fn probe(qv: &Value, sv: &Value) {
    refused(sql_call("reserve", qv, sv));
}

/// Overwrite the stored publication; the guarded function then sees the
/// tampered contract as the authoritative source.
fn tamper(admin: &mut postgres::Client, q: &ReservationRequest, contract: &Value) {
    admin
        .execute(
            "UPDATE mission.registration_outbox SET contract=$1 \
             WHERE engagement_id=$2 AND campaign_id=$3",
            &[&contract, &q.engagement_id.0, &q.campaign_id.0],
        )
        .unwrap();
}

/// Tamper the stored contract and prove the same call refuses at the door.
fn tamper_refuse(
    admin: &mut postgres::Client,
    q: &ReservationRequest,
    qv: &Value,
    original: &Value,
    mutate: impl Fn(&mut Value),
) {
    let mut forged = original.clone();
    mutate(&mut forged);
    tamper(admin, q, &forged);
    refused(sql_call("reserve", qv, &forged));
}

#[test]
fn durable_lifecycle_replays_and_old_cancel_cannot_release_new_hold() {
    let _g = db();
    let (q, _source) = budget::fixture(0xf000);
    let id = budget::op();
    let held = budget::run(&q, id, Reserve).unwrap().unwrap();
    assert_eq!(
        (held.generation, held.record_revision, held.kind.as_str()),
        (1, 1, "reserved_not_sent")
    );
    assert_eq!(
        (
            held.capacity.calls,
            held.capacity.output_token_capacity,
            held.capacity.request_bytes,
            held.capacity.response_bytes,
            held.capacity.wall_time_ms
        ),
        (2, 8192, 32768, 131072, 20000)
    );
    let reserved = budget::state(&q);
    // Recover and replayed reserve return the same durable row, unchanged.
    assert_eq!(budget::run(&q, id, Recover).unwrap().unwrap(), held);
    assert_eq!(budget::run(&q, id, Reserve).unwrap().unwrap(), held);
    assert_eq!(budget::state(&q), reserved);
    // A competing operation cannot reserve while a hold is active.
    assert_eq!(
        budget::run(&q, budget::op(), Reserve),
        Err(Fail::State("inference_reservation_refused"))
    );
    assert_eq!(budget::state(&q), reserved);
    let cancelled = budget::run(&q, id, Cancel).unwrap().unwrap();
    assert_eq!(
        (
            cancelled.generation,
            cancelled.record_revision,
            cancelled.kind.as_str()
        ),
        (1, 2, "cancelled_never_sent")
    );
    assert_eq!(cancelled.generation_call_id, held.generation_call_id);
    assert_eq!(cancelled.repair_call_id, held.repair_call_id);
    assert_eq!(cancelled.capacity, held.capacity);
    // The next hold is a new generation; the stale cancel cannot touch it.
    let next = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    assert_eq!(next.generation, 2);
    let active = budget::state(&q);
    assert_eq!(budget::run(&q, id, Cancel).unwrap().unwrap(), cancelled);
    assert_eq!(budget::run(&q, id, Reserve).unwrap().unwrap(), cancelled);
    assert_eq!(budget::state(&q), active);
    // Reservations never move the shared session fence out of idle.
    assert_eq!(
        budget::state(&q)["fence"]["phase"].as_str().unwrap(),
        "idle"
    );
}

#[test]
fn authenticated_writer_and_impersonation_cannot_cross_the_boundary() {
    let _g = db();
    let (q, source) = budget::fixture(0xf001);
    let id = budget::op();
    budget::run(&q, id, Reserve).unwrap();
    let before = budget::state(&q);
    // A different broker login is not the writer: cancel conflicts on
    // writer_oid, not on caller-supplied identity.
    let mut other = PgInferenceReservationStore::new(other_broker_client());
    assert!(reservation(&mut other, None::<&mut PgMissionStore>, Cancel, &q, id).is_err());
    // SET ROLE plus a forged actor setting cannot impersonate the broker:
    // session_user is the authenticated login, never a GUC. dw_runtime holds
    // no EXECUTE grant on the guarded function.
    let mut rt = runtime_client();
    rt.batch_execute("SET ROLE dw_runtime; SET app.actor='dw_m1_broker_test'")
        .unwrap();
    assert!(
        budget::sql(
            &mut rt,
            "reserve",
            &serde_json::to_value(&q).unwrap(),
            budget::op(),
            &serde_json::to_value(&source).unwrap(),
        )
        .is_err()
    );
    assert_eq!(budget::state(&q), before);
}

#[test]
fn raw_sql_boundary_refuses_malformed_forged_and_mismatched_input() {
    let _g = db();
    let (q, source) = budget::fixture(0xf002);
    let qv = serde_json::to_value(&q).unwrap();
    let sv = serde_json::to_value(&source).unwrap();
    let before = budget::state(&q);
    // Null/corrupt request and action arguments refuse at the shape door.
    for (action, request) in [
        ("reserve", Value::Null),
        ("hold", qv.clone()),
        ("reserve", json!({"nonsense": true})),
    ] {
        refused(sql_call(action, &request, &sv));
    }
    // An unknown request field refuses.
    let mut extra = qv.clone();
    extra["extra_field"] = json!(1);
    probe(&extra, &sv);
    // Every request identity must match the publication and the contract.
    for (field, value) in [
        ("engagement_id", json!(Uuid::from_u128(0xdead))),
        ("campaign_id", json!(Uuid::from_u128(0xdead))),
        ("operator_ref", json!(Uuid::from_u128(0xdead))),
        ("permission_id", json!(Uuid::from_u128(0xdead))),
        ("goal_ref", json!(Uuid::from_u128(0xdead))),
        ("expected_mission_revision", json!(2)),
        ("exercise_mode", json!("defender_informed")),
    ] {
        let mut forged = qv.clone();
        forged[field] = value;
        probe(&forged, &sv);
    }
    // Absent, malformed or tampered source publications refuse: the guarded
    // function compares against the stored contract, never the caller's copy.
    probe(&qv, &Value::Null);
    probe(&qv, &json!({"nonsense": true}));
    for mutate in [
        |f: &mut Value| f["event_id"] = json!(Uuid::from_u128(0xfeed)),
        |f: &mut Value| {
            f.as_object_mut().unwrap().remove("event_id");
        },
        |f: &mut Value| f["operation_id"] = json!(Uuid::from_u128(0x8888)),
        |f: &mut Value| f["version"] = json!(2),
        |f: &mut Value| f["fields"]["campaign_id"] = json!(Uuid::from_u128(0x9999)),
    ] {
        let mut forged = sv.clone();
        mutate(&mut forged);
        probe(&qv, &forged);
    }
    assert_eq!(
        budget::state(&q),
        before,
        "every refusal must leave durable state untouched"
    );
    // The same door admits a well-formed call.
    let record = sql_call("reserve", &qv, &sv).unwrap().unwrap();
    assert_eq!(record["kind"], "reserved_not_sent");
}

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
            tamper_refuse(&mut admin, &q, &qv, &original, |f| {
                f["fields"]["m1_inference_permission"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&name);
                if replaced {
                    f["fields"]["m1_inference_permission"]["unknown"] = json!(1);
                }
            });
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
        tamper_refuse(&mut admin, &q, &qv, &original, |f| {
            f["fields"]["m1_inference_permission"][field] = value.clone();
        });
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
        tamper_refuse(&mut admin, &q, &qv, &original, |f| {
            f["fields"]["m1_inference_permission"]["starts_at"] = starts.clone();
            f["fields"]["m1_inference_permission"]["ends_at"] = ends.clone();
        });
    }
    // Restore the real contract; the same door admits the valid call.
    tamper(&mut admin, &q, &original);
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
    tamper_refuse(&mut admin, &q, &qv, &original, |f| {
        f["fields"]["m1_inference_permission"]["ends_at"] = json!(now - 10);
    });
    tamper_refuse(&mut admin, &q, &qv, &original, |f| {
        f["fields"]["m1_inference_permission"]["starts_at"] = json!(now + 100);
    });
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
    tamper(&mut admin, &q, &forged);
    refused(budget::sql(
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
    // Every operation limit must be an integer inside its declared bound.
    for (field, cases) in [
        (
            "request_bytes",
            [
                json!(0),
                json!(16385),
                json!("16384"),
                json!(1.5),
                Value::Null,
            ]
            .as_slice(),
        ),
        ("response_bytes", [json!(0), json!(65537)].as_slice()),
        ("decision_wall_time_ms", [json!(0), json!(20001)].as_slice()),
        ("connect_time_ms", [json!(0), json!(5001)].as_slice()),
        ("calls_per_decision", [json!(1), json!(3)].as_slice()),
        (
            "output_tokens_per_call",
            [json!(4095), json!(4097)].as_slice(),
        ),
        ("concurrency", [json!(0), json!(2)].as_slice()),
    ] {
        for case in cases {
            tamper_refuse(&mut admin, &q, &qv, &original, |f| {
                f["fields"]["m1_inference_permission"]["operation_limits"][field] = case.clone();
            });
        }
    }
    // connect_time_ms may not exceed decision_wall_time_ms.
    tamper_refuse(&mut admin, &q, &qv, &original, |f| {
        let limits = &mut f["fields"]["m1_inference_permission"]["operation_limits"];
        limits["decision_wall_time_ms"] = json!(4000);
        limits["connect_time_ms"] = json!(5000);
    });
    // Cardinality is closed: a missing or extra limit key refuses.
    tamper_refuse(&mut admin, &q, &qv, &original, |f| {
        f["fields"]["m1_inference_permission"]["operation_limits"]
            .as_object_mut()
            .unwrap()
            .remove("candidates");
    });
    tamper_refuse(&mut admin, &q, &qv, &original, |f| {
        f["fields"]["m1_inference_permission"]["operation_limits"]["extra"] = json!(1);
    });
    tamper_refuse(&mut admin, &q, &qv, &original, |f| {
        f["fields"]["m1_inference_permission"]["campaign_limits"]["extra"] = json!(1);
    });
    // Campaign totals must be positive integers covering the hold's cost.
    for (field, cases) in [
        ("calls", [json!(1), json!(0), json!("2")].as_slice()),
        ("request_bytes", [json!(32767), json!(-4)].as_slice()),
        ("response_bytes", [json!(131071), Value::Null].as_slice()),
        (
            "output_token_capacity",
            [json!(8191), json!(1.5)].as_slice(),
        ),
        ("wall_time_ms", [json!(19999), json!("20000")].as_slice()),
    ] {
        for case in cases {
            tamper_refuse(&mut admin, &q, &qv, &original, |f| {
                f["fields"]["m1_inference_permission"]["campaign_limits"][field] = case.clone();
            });
        }
    }
    assert!(budget::state(&q)["ledger"].as_array().unwrap().is_empty());
    // Restore the real contract; the stored limits yield the derived capacity.
    tamper(&mut admin, &q, &original);
    let record = budget::run(&q, budget::op(), Reserve).unwrap().unwrap();
    assert_eq!(
        (
            record.capacity.calls,
            record.capacity.output_token_capacity,
            record.capacity.request_bytes,
            record.capacity.response_bytes,
        ),
        (2, 8192, 32768, 131072)
    );
    // A forged tighter source is tampering, not a negotiation: the stored
    // contract still governs and the caller's copy must match it.
    let mut tighter = original.clone();
    tighter["fields"]["m1_inference_permission"]["operation_limits"]["request_bytes"] = json!(1);
    refused(budget::sql(
        &mut broker_client(),
        "reserve",
        &qv,
        budget::op(),
        &tighter,
    ));
}

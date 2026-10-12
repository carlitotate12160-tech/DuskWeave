//! Real role/lifecycle coverage for the pre-send inference reservation slice
//! on the owned disposable database: durable reserve/replay/cancel
//! generations, writer identity and direct guarded-function boundary probes.
//! Admin access is fixture setup only; runtime assertions use restricted
//! logins. Cluster-role fixtures serialize under db().
//!
//! Requires the packet's authorized disposable PostgreSQL service.

use duskweave::Fail;
use duskweave::m1_inference_budget::{ReservationAction::*, reservation};
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

/// Direct guarded-function reserve probe over the broker login.
fn probe(qv: &Value, sv: &Value) {
    budget::refused(budget::sql_call("reserve", qv, sv));
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
        budget::refused(budget::sql_call(action, &request, &sv));
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
    let record = budget::sql_call("reserve", &qv, &sv).unwrap().unwrap();
    assert_eq!(record["kind"], "reserved_not_sent");
}

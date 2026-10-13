//! PostgreSQL integration tests for m1-context-prepare:
//! Broker privilege checks, Repeatable Read snapshot, and state immutability.
use duskweave::m1_context::{ContextReader, prepare};
use duskweave::m1_context_input::{Candidate, ContextRequest, Priority};
use duskweave::m1_inference_budget::ReservationAction;
use duskweave::postgres_mission::PgContextReader;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/m1_context_prepare.rs"]
mod context_support;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;

#[test]
fn broker_privilege_required_runtime_without_broker_refuses() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9101);
    let op = budget::op();
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![],
    };

    let mut runtime_reader = PgContextReader::new(db_support::runtime_client());
    let res_runtime = runtime_reader.read(&q, op);
    assert!(
        res_runtime.is_err(),
        "runtime without broker role must refuse capacity snapshot"
    );

    let mut broker_reader = PgContextReader::new(session_support::broker_client());
    let res_broker = broker_reader.read(&q, op);
    assert!(
        res_broker.is_ok(),
        "broker member must succeed in reading context basis"
    );
}

#[test]
fn function_acl_strictly_enforced() {
    let _g = db_support::db();
    let mut admin = session_support::admin_db_client();

    let public_exec: bool = admin
        .query_one(
            "SELECT has_function_privilege('public', 'execution.inference_capacity_snapshot(uuid,uuid)', 'execute')",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(
        !public_exec,
        "PUBLIC must not have execute privilege on inference_capacity_snapshot"
    );

    let broker_exec: bool = admin
        .query_one(
            "SELECT has_function_privilege('dw_m1_broker', 'execution.inference_capacity_snapshot(uuid,uuid)', 'execute')",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(
        broker_exec,
        "dw_m1_broker must have execute privilege on inference_capacity_snapshot"
    );
}

#[test]
fn capacity_snapshot_tracks_ledger_holds_and_cancelled_release() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9201);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let mut reader = PgContextReader::new(session_support::broker_client());
    let basis = reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis.capacity.active_holds, 0);
    assert_eq!(basis.capacity.charged.calls, 0);
    assert!(!basis.capacity.session_claimed);

    let hold_op = budget::op();
    let hold = budget::run(&req, hold_op, ReservationAction::Reserve).unwrap();
    assert!(hold.is_some(), "reservation hold must succeed");

    let basis_held = reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis_held.capacity.active_holds, 1);
    assert_eq!(basis_held.capacity.charged.calls, 2);
    assert_eq!(basis_held.capacity.charged.output_token_capacity, 8192);

    let cancel = budget::run(&req, hold_op, ReservationAction::Cancel).unwrap();
    assert!(cancel.is_some(), "cancellation must succeed");

    let basis_cancelled = reader.read(&q, budget::op()).unwrap();
    assert_eq!(basis_cancelled.capacity.active_holds, 0);
    assert_eq!(basis_cancelled.capacity.charged.calls, 0);
}

#[test]
fn preparation_leaves_scoped_state_completely_unchanged() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9301);

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let before_state = budget::state(&req);

    let op = budget::op();
    let mut reader = PgContextReader::new(session_support::broker_client());
    let prepared = prepare(&mut reader, &q, op).expect("prepare must succeed");
    assert!(!prepared.utf8().is_empty());

    let after_state = budget::state(&req);
    assert_eq!(
        before_state, after_state,
        "prepare is read-only and must not change any scoped persistence or ledger state"
    );
}

#[test]
fn preparation_with_admitted_premises_leaves_state_unchanged() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9302);
    let mut raw = budget::registration_value(e, c, session_support::now());
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("admitted_operator_premises"));
    let (_, source) = budget::register(e, c, &raw);
    let req = budget::request(e, c, &raw);
    let inc = source.fields.included_assets();
    let a1 = inc[0];

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![Candidate {
            asset_ref: a1,
            priority: Priority::High,
        }],
    };

    let before_state = budget::state(&req);

    let op = budget::op();
    let mut reader = PgContextReader::new(session_support::broker_client());
    let prepared =
        prepare(&mut reader, &q, op).expect("prepare with operator premises must succeed");
    let pack: serde_json::Value = serde_json::from_str(prepared.utf8()).unwrap();
    assert_eq!(pack["candidates"].as_array().unwrap().len(), 1);

    let after_state = budget::state(&req);
    assert_eq!(
        before_state, after_state,
        "prepare with admitted premises is read-only and preserves exact scoped state"
    );
}

#[test]
fn repeated_sequential_preparation_is_consistent() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9401);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![],
    };

    let op1 = budget::op();
    let op2 = budget::op();
    let mut reader = PgContextReader::new(session_support::broker_client());
    let prep1 = prepare(&mut reader, &q, op1).unwrap();
    let prep2 = prepare(&mut reader, &q, op2).unwrap();

    let p1: serde_json::Value = serde_json::from_str(prep1.utf8()).unwrap();
    let p2: serde_json::Value = serde_json::from_str(prep2.utf8()).unwrap();

    assert_eq!(p1["version"], p2["version"]);
    assert_eq!(p1["question"], p2["question"]);
    assert_eq!(p1["remaining_capacity"], p2["remaining_capacity"]);
    assert_eq!(
        p1["owner_views"]["mission"]["status"],
        p2["owner_views"]["mission"]["status"]
    );
    assert_eq!(
        p1["owner_views"]["mission"]["revision"],
        p2["owner_views"]["mission"]["revision"]
    );
    assert_eq!(
        p1["owner_views"]["mission"]["lineage"],
        p2["owner_views"]["mission"]["lineage"]
    );
    assert_ne!(p1["decision_id"], p2["decision_id"]);
}

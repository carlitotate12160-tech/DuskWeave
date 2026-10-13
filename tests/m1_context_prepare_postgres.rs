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
        p1["owner_views"][0]["revision"],
        p2["owner_views"][0]["revision"]
    );
    assert_eq!(
        p1["owner_views"][0]["frontier"],
        p2["owner_views"][0]["frontier"]
    );
    assert_eq!(p1["references"][0]["status"], p2["references"][0]["status"]);
    assert_eq!(
        p1["references"][0]["lineage"],
        p2["references"][0]["lineage"]
    );
    assert_ne!(p1["decision_id"], p2["decision_id"]);
}

#[test]
fn owner_withdrawal_followed_by_preparation_preserves_scoped_state() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9903);
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };
    let normal_before = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    context_support::prepare_pack(&q);
    assert_eq!(
        normal_before,
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
    );

    use duskweave::withdrawal::WithdrawalStore;

    let mut store = duskweave::postgres_mission::PgMissionStore::new(db_support::runtime_client());
    let mut alloc = duskweave::postgres_mission::PgAllocator::new(db_support::runtime_client());
    let w_req = duskweave::withdrawal::WithdrawalRequest {
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        operator_ref: req.operator_ref,
        expected_mission_revision: 1,
        reason: duskweave::withdrawal::WithdrawalReason::OperatorRequested,
    };
    store
        .withdraw(&w_req, budget::op(), false, &mut alloc)
        .unwrap();

    let state_before = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    let mut reader = PgContextReader::new(session_support::broker_client());
    assert_eq!(
        prepare(&mut reader, &q, budget::op()).err(),
        Some(duskweave::Fail::State("context_mission_unavailable"))
    );

    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let input = wait_db::InputFile::new(
        &context_support::request(&serde_json::to_value(&req).unwrap(), &[]),
        "post_withdraw",
    );
    let out = context_support::run(
        &[
            "m1-context-prepare".into(),
            "--input".into(),
            input.path().into(),
        ],
        &broker_dsn,
    );
    assert!(!out.status.success());
    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "deferred");
    assert!(r["pack_utf8"].is_null());
    context_support::assert_false_flags(&r);

    let state_after = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(state_before, state_after);
}

#[test]
fn consent_missing_required_disclosure_classes_in_database_defers() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let now = session_support::now();
    let required = [
        "mission_question_constraints",
        "scoped_opaque_references",
        "omissions_unknown",
        "capacity",
    ];
    for (i, missing) in required.iter().enumerate() {
        let (e, c) = db_support::scope(0x9910 + i as u128);
        let mut raw = budget::registration_value(e, c, now);
        let classes = raw["m1_inference_permission"]["field_classes"]
            .as_array_mut()
            .unwrap();
        classes.retain(|v| v.as_str() != Some(*missing));
        let (_, source) = budget::register(e, c, &raw);
        assert_eq!(source.version, 3);
        assert!(source.fields.m1_inference_permission().is_some());
        let req = budget::request(e, c, &raw);
        let q = ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: req.clone(),
            candidates: vec![],
        };
        let before_state = context_support::scoped_state(e.0, c.0);
        let mut reader = PgContextReader::new(session_support::broker_client());
        assert_eq!(
            prepare(&mut reader, &q, budget::op()).err(),
            Some(duskweave::Fail::State("context_consent_deferred"))
        );
        let after_state = context_support::scoped_state(e.0, c.0);
        assert_eq!(before_state, after_state);
    }

    let (e_op, c_op) = db_support::scope(0x9920);
    let raw_op = budget::registration_value(e_op, c_op, now);
    let (_, src_op) = budget::register(e_op, c_op, &raw_op);
    let req_op = budget::request(e_op, c_op, &raw_op);
    let optional_before = context_support::scoped_state(e_op.0, c_op.0);
    let mut r1 = PgContextReader::new(session_support::broker_client());
    let q_zero = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_op.clone(),
        candidates: vec![],
    };
    assert!(prepare(&mut r1, &q_zero, budget::op()).is_ok());
    assert_eq!(
        optional_before,
        context_support::scoped_state(e_op.0, c_op.0)
    );

    let mut r2 = PgContextReader::new(session_support::broker_client());
    let q_cand = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_op,
        candidates: vec![Candidate {
            asset_ref: src_op.fields.included_assets()[0],
            priority: Priority::High,
        }],
    };
    assert_eq!(
        prepare(&mut r2, &q_cand, budget::op()).err(),
        Some(duskweave::Fail::State("context_disclosure_deferred"))
    );
    assert_eq!(
        optional_before,
        context_support::scoped_state(e_op.0, c_op.0)
    );
}

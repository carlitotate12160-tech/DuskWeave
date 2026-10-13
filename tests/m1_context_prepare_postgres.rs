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
fn legacy_v1_and_crtsh_v2_in_database_refuse_preparation() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let now = session_support::now();
    for (n, remove_both) in [(0x9901, true), (0x9902, false)] {
        let (e, c) = db_support::scope(n);
        let mut raw = budget::registration_value(e, c, now);
        if remove_both {
            raw.as_object_mut().unwrap().remove("m1_permission");
        }
        raw.as_object_mut()
            .unwrap()
            .remove("m1_inference_permission");
        let (_, _) = budget::register(e, c, &raw);
        let op_ref = duskweave::mission::OperatorRef(
            serde_json::from_value(raw["operator_ref"].clone()).unwrap(),
        );
        let goal_ref =
            duskweave::mission::GoalRef(serde_json::from_value(raw["goal_ref"].clone()).unwrap());
        let req = duskweave::m1_inference_budget::ReservationRequest {
            version: "m1_inference_reservation_request_v1".into(),
            engagement_id: e,
            campaign_id: c,
            operator_ref: op_ref,
            expected_mission_revision: 1,
            permission_id: uuid::Uuid::from_u128(n),
            goal_ref,
            exercise_mode: duskweave::mission::ExerciseMode::Blind,
        };
        let q = ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: req,
            candidates: vec![],
        };
        let mut reader = PgContextReader::new(session_support::broker_client());
        assert!(prepare(&mut reader, &q, budget::op()).is_err());
    }
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
    assert!(prepare(&mut reader, &q, budget::op()).is_err());

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
        let (_, _) = budget::register(e, c, &raw);
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
    let mut r1 = PgContextReader::new(session_support::broker_client());
    let q_zero = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_op.clone(),
        candidates: vec![],
    };
    assert!(prepare(&mut r1, &q_zero, budget::op()).is_ok());

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
}

#[test]
fn stored_publication_tampering_matrix_refuses_and_restores() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9930);
    let mut admin = session_support::admin_db_client();
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let orig_contract: serde_json::Value = admin
        .query_one(
            "SELECT contract FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    let orig_event_id: uuid::Uuid = admin
        .query_one(
            "SELECT event_id FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    let orig_goal_ref: uuid::Uuid = admin
        .query_one(
            "SELECT goal_ref FROM mission.missions WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);

    let foreign_uuid = uuid::Uuid::from_u128(0xbeef_cafe);

    let test_tamper = |admin: &mut postgres::Client,
                       tamper_sql: &str,
                       params: &[&(dyn postgres::types::ToSql + Sync)],
                       restore_sql: &str,
                       restore_params: &[&(dyn postgres::types::ToSql + Sync)],
                       label: &str| {
        let before_tamper = context_support::scoped_state(e, c);
        admin.execute(tamper_sql, params).unwrap();
        let state_after_tamper = context_support::scoped_state(e, c);
        assert_ne!(
            before_tamper, state_after_tamper,
            "{label}: tamper must alter state"
        );

        let mut reader = PgContextReader::new(session_support::broker_client());
        let res = prepare(&mut reader, &q, budget::op());
        assert!(res.is_err(), "{label}: tampered publication must refuse");

        let state_after_attempt = context_support::scoped_state(e, c);
        assert_eq!(
            state_after_tamper, state_after_attempt,
            "{label}: attempt must leave tampered state untouched"
        );

        admin.execute(restore_sql, restore_params).unwrap();
        let state_restored = context_support::scoped_state(e, c);
        assert_eq!(
            before_tamper, state_restored,
            "{label}: restore must recover baseline"
        );

        let mut reader_ok = PgContextReader::new(session_support::broker_client());
        assert!(
            prepare(&mut reader_ok, &q, budget::op()).is_ok(),
            "{label}: normal preparation must succeed after restore"
        );
    };

    let upd_contract = "UPDATE mission.registration_outbox SET contract=$1 WHERE engagement_id=$2 AND campaign_id=$3";
    let upd_event = "UPDATE mission.registration_outbox SET event_id=$1 WHERE engagement_id=$2 AND campaign_id=$3";
    let upd_goal =
        "UPDATE mission.missions SET goal_ref=$1 WHERE engagement_id=$2 AND campaign_id=$3";

    let test_contract_tamper =
        |admin: &mut postgres::Client, tampered: &serde_json::Value, label: &str| {
            test_tamper(
                admin,
                upd_contract,
                &[tampered, &e, &c],
                upd_contract,
                &[&orig_contract, &e, &c],
                label,
            );
        };

    let mut t1 = orig_contract.clone();
    t1["forged_outer"] = serde_json::json!("unauthorized");
    test_contract_tamper(&mut admin, &t1, "unknown_outer");

    let mut t2 = orig_contract.clone();
    t2["fields"]["m1_inference_permission"]["forged_consent"] = serde_json::json!("bad");
    test_contract_tamper(&mut admin, &t2, "unknown_consent_field");

    test_tamper(
        &mut admin,
        upd_event,
        &[&foreign_uuid, &e, &c],
        upd_event,
        &[&orig_event_id, &e, &c],
        "mismatched_event_id",
    );
    test_tamper(
        &mut admin,
        upd_goal,
        &[&foreign_uuid, &e, &c],
        upd_goal,
        &[&orig_goal_ref, &e, &c],
        "mismatched_mission_goal",
    );

    let mut t5 = orig_contract.clone();
    t5["campaign_id"] = serde_json::json!(foreign_uuid);
    test_contract_tamper(&mut admin, &t5, "forged_cross_campaign");

    let mut t6 = orig_contract.clone();
    t6["fields"]["m1_inference_permission"]["terms_approved"] = serde_json::json!(false);
    test_contract_tamper(&mut admin, &t6, "unapproved_terms");
}

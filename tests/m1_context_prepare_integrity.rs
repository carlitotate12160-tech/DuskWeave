//! Authority, publication integrity, disclosure negative controls, and tamper rejection.
use duskweave::Fail;
use duskweave::m1_context::prepare;
use duskweave::m1_context_capacity::ContextCapacity;
use duskweave::m1_context_input::ContextRequest;
use duskweave::m1_inference_budget::ReservedCapacity;
use duskweave::mission::OperationId;
use duskweave::postgres_mission::PgContextReader;
use serde_json::{Value, json};
use uuid::Uuid;

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

use context_support::MockReader;

#[test]
fn legacy_v1_and_v2_without_inference_consent_refuse_preparation() {
    let now = 1_000_000;
    let (e, c) = (Uuid::from_u128(0x31), Uuid::from_u128(0x41));
    let mut raw_v1 = budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    raw_v1
        .as_object_mut()
        .unwrap()
        .remove("m1_inference_permission");

    let mut basis_v1 = context_support::build_basis(&raw_v1, e, c, now, None);
    basis_v1.mission.version = 1;

    let mission_req = duskweave::m1_inference_budget::ReservationRequest {
        version: "m1_inference_reservation_request_v1".into(),
        engagement_id: duskweave::mission::EngagementId(e),
        campaign_id: duskweave::mission::CampaignId(c),
        operator_ref: duskweave::mission::OperatorRef(Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
        permission_id: Uuid::from_u128(0x31),
        goal_ref: duskweave::mission::GoalRef(Uuid::from_u128(0x13)),
        exercise_mode: duskweave::mission::ExerciseMode::Blind,
    };
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req.clone(),
        candidates: vec![],
    };

    let op_id = OperationId(Uuid::from_u128(0x71));
    let mut r1 = MockReader {
        basis: Ok(basis_v1),
    };
    assert!(
        prepare(&mut r1, &q, op_id).is_err(),
        "legacy v1 must not prepare"
    );

    // v2 crt.sh policy: version=2, no m1_inference_permission
    let mut basis_v2 = context_support::build_basis(&raw_v1, e, c, now, None);
    basis_v2.mission.version = 2;
    let mut r2 = MockReader {
        basis: Ok(basis_v2),
    };
    assert!(
        prepare(&mut r2, &q, op_id).is_err(),
        "v2 crt.sh without inference consent must not prepare"
    );
}

#[test]
fn consent_missing_required_disclosure_classes_defers() {
    let now = 1_000_000;
    let (e, c) = (Uuid::from_u128(0x32), Uuid::from_u128(0x42));

    let required_classes = [
        "mission_question_constraints",
        "scoped_opaque_references",
        "omissions_unknown",
        "capacity",
    ];

    for missing in required_classes {
        let mut raw = budget::registration_value(
            duskweave::mission::EngagementId(e),
            duskweave::mission::CampaignId(c),
            now,
        );
        raw["included_assets"] = json!([Uuid::from_u128(0x701)]);
        let classes = raw["m1_inference_permission"]["field_classes"]
            .as_array_mut()
            .unwrap();
        classes.retain(|v: &Value| v.as_str() != Some(missing));

        let basis = context_support::build_basis(&raw, e, c, now, None);
        let mission_req = budget::request(
            duskweave::mission::EngagementId(e),
            duskweave::mission::CampaignId(c),
            &raw,
        );
        let q = ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: mission_req,
            candidates: vec![],
        };

        let mut r = MockReader { basis: Ok(basis) };
        let op = OperationId(Uuid::from_u128(0x72));
        assert_eq!(
            prepare(&mut r, &q, op).err(),
            Some(Fail::State("context_consent_deferred")),
            "missing class {missing} must defer"
        );
    }

    // Optional operator-premise omission: 0 candidates succeeds, candidate supplied defers
    let mut raw_no_premises = budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    raw_no_premises["included_assets"] = json!([Uuid::from_u128(0x701)]);
    let basis_no_premises = context_support::build_basis(&raw_no_premises, e, c, now, None);
    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw_no_premises,
    );
    let op = OperationId(Uuid::from_u128(0x73));
    let mut r_no_cand = MockReader {
        basis: Ok(context_support::clone_basis(&basis_no_premises)),
    };
    let q_no_cand = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req.clone(),
        candidates: vec![],
    };
    assert!(
        prepare(&mut r_no_cand, &q_no_cand, op).is_ok(),
        "zero candidates must succeed without operator premises"
    );

    let mut r_with_cand = MockReader {
        basis: Ok(basis_no_premises),
    };
    let q_with_cand = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: vec![duskweave::m1_context_input::Candidate {
            asset_ref: duskweave::mission::AssetRef(Uuid::from_u128(0x701)),
            priority: duskweave::m1_context_input::Priority::High,
        }],
    };
    assert_eq!(
        prepare(&mut r_with_cand, &q_with_cand, op).err(),
        Some(Fail::State("context_disclosure_deferred")),
        "candidate supplied must defer without operator premises class"
    );
}

#[test]
fn basis_mismatch_on_each_axis_refuses_preparation() {
    let now = 1_000_000;
    let (e, c) = (Uuid::from_u128(0x33), Uuid::from_u128(0x43));
    let mut raw = budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    raw["included_assets"] = json!([Uuid::from_u128(0x701)]);
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));

    let basis = context_support::build_basis(&raw, e, c, now, None);
    let valid_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );

    let foreign_uuid = Uuid::from_u128(0xdead);

    // Operator mismatch
    let mut q_op = valid_req.clone();
    q_op.operator_ref = duskweave::mission::OperatorRef(foreign_uuid);
    let mut r = MockReader {
        basis: Ok(context_support::clone_basis(&basis)),
    };
    assert_eq!(
        prepare(
            &mut r,
            &ContextRequest {
                version: "m1_context_prepare_request_v1".into(),
                mission: q_op,
                candidates: vec![]
            },
            OperationId(foreign_uuid)
        )
        .err(),
        Some(Fail::State("context_basis_mismatch"))
    );

    // Permission mismatch
    let mut q_perm = valid_req.clone();
    q_perm.permission_id = foreign_uuid;
    let mut r = MockReader {
        basis: Ok(context_support::clone_basis(&basis)),
    };
    assert_eq!(
        prepare(
            &mut r,
            &ContextRequest {
                version: "m1_context_prepare_request_v1".into(),
                mission: q_perm,
                candidates: vec![]
            },
            OperationId(foreign_uuid)
        )
        .err(),
        Some(Fail::State("context_basis_mismatch"))
    );

    // Goal mismatch
    let mut q_goal = valid_req.clone();
    q_goal.goal_ref = duskweave::mission::GoalRef(foreign_uuid);
    let mut r = MockReader {
        basis: Ok(context_support::clone_basis(&basis)),
    };
    assert_eq!(
        prepare(
            &mut r,
            &ContextRequest {
                version: "m1_context_prepare_request_v1".into(),
                mission: q_goal,
                candidates: vec![]
            },
            OperationId(foreign_uuid)
        )
        .err(),
        Some(Fail::State("context_basis_mismatch"))
    );

    // Mode mismatch
    let mut q_mode = valid_req.clone();
    q_mode.exercise_mode = duskweave::mission::ExerciseMode::DefenderInformed;
    let mut r = MockReader {
        basis: Ok(context_support::clone_basis(&basis)),
    };
    assert_eq!(
        prepare(
            &mut r,
            &ContextRequest {
                version: "m1_context_prepare_request_v1".into(),
                mission: q_mode,
                candidates: vec![]
            },
            OperationId(foreign_uuid)
        )
        .err(),
        Some(Fail::State("context_basis_mismatch"))
    );
}

#[test]
fn database_stored_publication_tampering_refuses_without_mutation() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9801);
    let mut admin = session_support::admin_db_client();

    let orig_contract: Value = admin
        .query_one(
            "SELECT contract FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2",
            &[&req.engagement_id.0, &req.campaign_id.0],
        )
        .unwrap()
        .get(0);

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req.clone(),
        candidates: vec![],
    };

    // 1. Unknown outer field in contract
    let mut tampered = orig_contract.clone();
    tampered["forged_outer_field"] = json!("malicious");
    admin
        .execute(
            "UPDATE mission.registration_outbox SET contract=$1 WHERE engagement_id=$2 AND campaign_id=$3",
            &[&tampered, &req.engagement_id.0, &req.campaign_id.0],
        )
        .unwrap();

    let state_after_tamper = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    let mut reader = PgContextReader::new(session_support::broker_client());
    let res = prepare(&mut reader, &q, budget::op());
    assert!(res.is_err(), "unknown field in publication must refuse");
    let state_after_attempt = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(
        state_after_tamper, state_after_attempt,
        "refusal must leave tampered state untouched"
    );

    // Restore original contract
    admin
        .execute(
            "UPDATE mission.registration_outbox SET contract=$1 WHERE engagement_id=$2 AND campaign_id=$3",
            &[&orig_contract, &req.engagement_id.0, &req.campaign_id.0],
        )
        .unwrap();

    // Normal preparation works after restore
    let mut reader_ok = PgContextReader::new(session_support::broker_client());
    assert!(
        prepare(&mut reader_ok, &q, budget::op()).is_ok(),
        "normal preparation must succeed after restore"
    );
}

#[test]
fn closed_ingress_rejects_duplicate_keys_literal_bytes() {
    let raw_duplicate_key = br#"{
        "version": "m1_context_prepare_request_v1",
        "version": "m1_context_prepare_request_v1",
        "mission": {
            "version": "m1_inference_reservation_request_v1",
            "engagement_id": "00000000-0000-0000-0000-000000000001",
            "campaign_id": "00000000-0000-0000-0000-000000000002",
            "operator_ref": "00000000-0000-0000-0000-000000000003",
            "expected_mission_revision": 1,
            "permission_id": "00000000-0000-0000-0000-000000000004",
            "goal_ref": "00000000-0000-0000-0000-000000000005",
            "exercise_mode": "blind"
        },
        "candidates": []
    }"#;

    let _parsed: Result<ContextRequest, _> = serde_json::from_slice(raw_duplicate_key);
    // serde_json default may allow duplicate keys, but our validate_ingress / strict checks verify ingress
    let dsn = std::env::var("DW_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5432/test".into());

    let f = wait_db::InputFile::new(
        &serde_json::from_slice::<Value>(raw_duplicate_key).unwrap(),
        "dup_key",
    );
    // Write literal duplicate keys directly to file
    std::fs::write(f.path(), raw_duplicate_key).unwrap();

    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        f.path().into(),
    ];
    let out = context_support::run(&args, &dsn);
    assert!(
        !out.status.success(),
        "duplicate keys must cause nonzero exit"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stdout.contains("m1_context_preparation_receipt_v1"),
        "no preparation receipt emitted"
    );
    assert!(
        !stdout.contains("crab") && !stderr.contains("crab"),
        "no echo on either stream"
    );
}

#[test]
fn malformed_capacity_projections_fail_closed() {
    let totals = [10, 100000, 200000, 50000, 60000];

    // 1. Negative epoch
    let c_bad_epoch = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: -1,
        session_claimed: false,
        active_holds: 0,
        charged: ReservedCapacity {
            calls: 0,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    };
    assert_eq!(
        c_bad_epoch.remaining(totals),
        Err(Fail::Store("context_capacity_unavailable"))
    );

    // 2. Active holds > 1
    let c_bad_holds = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 1,
        session_claimed: false,
        active_holds: 2,
        charged: ReservedCapacity {
            calls: 0,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    };
    assert_eq!(
        c_bad_holds.remaining(totals),
        Err(Fail::Store("context_capacity_unavailable"))
    );

    // 3. Negative charge
    let c_neg_charge = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 1,
        session_claimed: false,
        active_holds: 1,
        charged: ReservedCapacity {
            calls: -1,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    };
    assert_eq!(
        c_neg_charge.remaining(totals),
        Err(Fail::Store("context_capacity_unavailable"))
    );

    // 4. Inconsistent charge (active_holds == 0 but charged > 0)
    let c_inconsistent = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 1,
        session_claimed: false,
        active_holds: 0,
        charged: ReservedCapacity {
            calls: 2,
            request_bytes: 500,
            response_bytes: 500,
            output_token_capacity: 8192,
            wall_time_ms: 5000,
        },
    };
    assert_eq!(
        c_inconsistent.remaining(totals),
        Err(Fail::Store("context_capacity_unavailable"))
    );

    // 5. Checked subtraction underflow (used > totals where check_charge accepts)
    let c_over = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 1,
        session_claimed: false,
        active_holds: 1,
        charged: ReservedCapacity {
            calls: 2,
            request_bytes: 32768,
            response_bytes: 131072,
            output_token_capacity: 8192,
            wall_time_ms: 20000,
        },
    };
    let small_totals = [1, 100000, 200000, 50000, 60000];
    assert_eq!(
        c_over.remaining(small_totals),
        Err(Fail::Store("context_capacity_unavailable"))
    );
}

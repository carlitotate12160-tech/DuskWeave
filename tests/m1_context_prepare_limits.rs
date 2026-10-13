//! Parameterized count, byte limit, omission, and exact byte carrier assertions.
use duskweave::m1_context;
use duskweave::m1_context_input::{Candidate, ContextRequest, Priority};
use duskweave::mission::{AssetRef, OperationId};
use duskweave::{Fail, Res};
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

fn asset_uuid(i: u128) -> Uuid {
    Uuid::from_u128(0x7000 + i)
}

#[test]
fn input_candidate_counts_admit_up_to_64_and_reject_at_65() {
    let (e, c) = (Uuid::from_u128(0x10), Uuid::from_u128(0x20));
    let now = 1_000_000;
    let mut raw = budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    let all_assets: Vec<Uuid> = (0..64).map(asset_uuid).collect();
    raw["included_assets"] = json!(all_assets);
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));

    let basis = context_support::build_basis(&raw, e, c, now, None);
    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );

    for count in [0usize, 1, 8, 9, 64] {
        let candidates: Vec<Candidate> = (0..count)
            .map(|i| Candidate {
                asset_ref: AssetRef(asset_uuid(i as u128)),
                priority: if i % 2 == 0 {
                    Priority::High
                } else {
                    Priority::Low
                },
            })
            .collect();
        let q = ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: mission_req.clone(),
            candidates,
        };
        let op = OperationId(Uuid::from_u128(0x300));
        let mut reader = MockReader {
            basis: Ok(context_support::clone_basis(&basis)),
        };
        let res = m1_context::prepare(&mut reader, &q, op);
        assert!(res.is_ok(), "count {count} must be accepted");
        let prep = res.unwrap();
        let pack: Value = serde_json::from_str(prep.utf8()).unwrap();
        let shown = pack["candidates"].as_array().unwrap().len();
        let omitted = pack["omissions"]["omitted_candidates"].as_i64().unwrap() as usize;
        let supplied = pack["omissions"]["supplied_candidates"].as_i64().unwrap() as usize;
        assert_eq!(shown + omitted, count);
        assert_eq!(supplied, count);
        assert!(shown <= 8);
    }

    // 65 candidates must reject at deserialization/validation
    let candidates_65: Vec<Value> = (0..65)
        .map(|i| json!({"asset_ref": asset_uuid(i), "priority": "high"}))
        .collect();
    let val_65 =
        context_support::request(&serde_json::to_value(&mission_req).unwrap(), &candidates_65);
    let raw_65 = serde_json::to_vec(&val_65).unwrap();
    let parsed: Res<ContextRequest> =
        serde_json::from_slice(&raw_65).map_err(|_| Fail::Input("invalid_context_input"));
    let valid = parsed.and_then(|q| q.validate_ingress());
    assert!(valid.is_err(), "65 candidates must be rejected");
}

#[test]
fn consent_candidate_limits_1_4_8_enforce_shown_bounds_and_order() {
    let (e, c) = (Uuid::from_u128(0x11), Uuid::from_u128(0x21));
    let now = 1_000_000;
    let mut raw = budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    let all_assets: Vec<Uuid> = (0..10).map(asset_uuid).collect();
    raw["included_assets"] = json!(all_assets);
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));

    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );

    let input_candidates: Vec<Candidate> = (0..10)
        .map(|i| Candidate {
            asset_ref: AssetRef(asset_uuid(i)),
            priority: if i == 0 {
                Priority::High
            } else if i == 1 {
                Priority::Medium
            } else {
                Priority::Low
            },
        })
        .collect();

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: input_candidates,
    };

    for limit in [1, 4, 8] {
        let mut r = raw.clone();
        r["m1_inference_permission"]["operation_limits"]["candidates"] = json!(limit);
        let b = context_support::build_basis(&r, e, c, now, None);
        let mut reader = MockReader { basis: Ok(b) };
        let op = OperationId(Uuid::from_u128(0x400));
        let prep = m1_context::prepare(&mut reader, &q, op).unwrap();
        let pack: Value = serde_json::from_str(prep.utf8()).unwrap();
        let shown_arr = pack["candidates"].as_array().unwrap();
        let shown = shown_arr.len();
        assert!(shown <= limit && limit <= 8, "shown <= limit <= 8");
        let omitted = pack["omissions"]["omitted_candidates"].as_i64().unwrap() as usize;
        assert_eq!(shown + omitted, 10, "supplied = shown + omitted");
        assert_eq!(pack["omissions"]["shown_candidates"], json!(shown));
        assert_eq!(pack["omissions"]["omitted_candidates"], json!(omitted));
        assert_eq!(pack["omissions"]["supplied_candidates"], json!(10));
        assert_eq!(
            pack["omissions"]["selection"],
            "input_order_prefix_with_count_and_byte_bounds"
        );

        let refs = pack["references"].as_array().unwrap();
        // Check order, priority, and references match the shown prefix
        for (idx, item) in shown_arr.iter().enumerate() {
            assert_eq!(item["reference"], format!("asset_{idx}"));
            assert_eq!(item["status"], "operator_premise");
            if idx == 0 {
                assert_eq!(item["priority"], "high");
            } else if idx == 1 {
                assert_eq!(item["priority"], "medium");
            } else {
                assert_eq!(item["priority"], "low");
            }
            let ref_item = refs
                .iter()
                .find(|r| r["id"] == format!("asset_{idx}"))
                .expect("matching reference in pack");
            assert_eq!(ref_item["entity"], json!(asset_uuid(idx as u128)));
            assert_eq!(ref_item["source"], "caller_declared_premise");
        }
    }
}

#[test]
fn forbidden_candidate_beyond_consent_limit_still_refuses() {
    let (e, c) = (Uuid::from_u128(0x12), Uuid::from_u128(0x22));
    let now = 1_000_000;
    let mut raw = budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    raw["included_assets"] = json!([asset_uuid(0)]);
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));
    raw["m1_inference_permission"]["operation_limits"]["candidates"] = json!(1);

    let basis = context_support::build_basis(&raw, e, c, now, None);
    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );

    // Provide 2 candidates: first is in-scope, second is out-of-scope
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: vec![
            Candidate {
                asset_ref: AssetRef(asset_uuid(0)),
                priority: Priority::High,
            },
            Candidate {
                asset_ref: AssetRef(asset_uuid(1)),
                priority: Priority::Low,
            },
        ],
    };

    let mut reader = MockReader { basis: Ok(basis) };
    let op = OperationId(Uuid::from_u128(0x500));
    let res = m1_context::prepare(&mut reader, &q, op);
    assert_eq!(
        res.err(),
        Some(Fail::Input("context_scope_refused")),
        "forbidden candidate beyond displayed limit must not be laundered"
    );
}

#[test]
fn byte_limit_exact_boundary_reduction_and_metadata_deferral() {
    let (e, c) = (Uuid::from_u128(0x13), Uuid::from_u128(0x23));
    let now = 1_000_000;
    let mut raw = budget::registration_value(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now,
    );
    raw["included_assets"] = json!([asset_uuid(0), asset_uuid(1), asset_uuid(2)]);
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));

    let basis = context_support::build_basis(&raw, e, c, now, None);
    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );

    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: vec![
            Candidate {
                asset_ref: AssetRef(asset_uuid(0)),
                priority: Priority::High,
            },
            Candidate {
                asset_ref: AssetRef(asset_uuid(1)),
                priority: Priority::Medium,
            },
            Candidate {
                asset_ref: AssetRef(asset_uuid(2)),
                priority: Priority::Low,
            },
        ],
    };

    // First obtain exact length with plenty of room (4096 bytes)
    let op = OperationId(Uuid::from_u128(0x600));
    let mut reader = MockReader {
        basis: Ok(context_support::clone_basis(&basis)),
    };
    let full_prep = m1_context::prepare(&mut reader, &q, op).unwrap();
    let exact_len = full_prep.utf8().len();

    // Exactly equal to exact_len accepts all 3
    let mut raw_exact = raw.clone();
    raw_exact["m1_inference_permission"]["operation_limits"]["context_bytes"] = json!(exact_len);
    let b_exact = context_support::build_basis(&raw_exact, e, c, now, None);
    let mut reader_exact = MockReader { basis: Ok(b_exact) };
    let prep_exact = m1_context::prepare(&mut reader_exact, &q, op).unwrap();
    let p_exact: Value = serde_json::from_str(prep_exact.utf8()).unwrap();
    assert_eq!(p_exact["candidates"].as_array().unwrap().len(), 3);

    // 1 byte below forces omission of at least 1 candidate
    let mut raw_minus_1 = raw.clone();
    raw_minus_1["m1_inference_permission"]["operation_limits"]["context_bytes"] =
        json!(exact_len - 1);
    let b_minus_1 = context_support::build_basis(&raw_minus_1, e, c, now, None);
    let mut reader_minus = MockReader {
        basis: Ok(b_minus_1),
    };
    let prep_minus = m1_context::prepare(&mut reader_minus, &q, op).unwrap();
    let p_minus: Value = serde_json::from_str(prep_minus.utf8()).unwrap();
    let shown = p_minus["candidates"].as_array().unwrap().len();
    assert!(shown < 3);
    assert_eq!(
        p_minus["omissions"]["omitted_candidates"].as_i64().unwrap(),
        (3 - shown) as i64
    );

    // Insufficient limit that cannot even fit metadata yields context_size_deferred
    let mut raw_tiny = raw.clone();
    raw_tiny["m1_inference_permission"]["operation_limits"]["context_bytes"] = json!(50);
    let b_tiny = context_support::build_basis(&raw_tiny, e, c, now, None);
    let mut reader_tiny = MockReader { basis: Ok(b_tiny) };
    let res_tiny = m1_context::prepare(&mut reader_tiny, &q, op);
    assert_eq!(res_tiny.err(), Some(Fail::State("context_size_deferred")));
}

#[test]
fn cli_exact_byte_carrier_matches_reconstructed_core_output() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x9510);
    let mut raw = budget::registration_value(e, c, session_support::now());
    let a1 = Uuid::from_u128(0x21);
    raw["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_operator_premises"));
    let (_reg_op, source) = budget::register(e, c, &raw);
    let mission_req = budget::request(e, c, &raw);

    let candidates = vec![context_support::candidate(a1, "high")];
    let req_val =
        context_support::request(&serde_json::to_value(&mission_req).unwrap(), &candidates);
    let broker_dsn = session_support::broker_dsn(session_support::db_port());
    let input = wait_db::InputFile::new(&req_val, "exact_carrier");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let state_before = context_support::scoped_state(e.0, c.0);
    let out = context_support::run(&args, &broker_dsn);
    assert!(out.status.success(), "CLI must succeed: {out:?}");
    let state_after = context_support::scoped_state(e.0, c.0);
    assert_eq!(
        state_before, state_after,
        "state must be stable before and after prepare"
    );

    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "prepared");
    let receipt_decision_id = r["decision_id"].as_str().unwrap();
    let receipt_op = OperationId(Uuid::parse_str(receipt_decision_id).unwrap());
    let receipt_pack_str = r["pack_utf8"].as_str().unwrap();

    // Verify receipt carries complete and valid parseable JSON matching the schema
    let parsed_pack: Value = serde_json::from_str(receipt_pack_str).unwrap();
    assert_eq!(parsed_pack["version"], "m1_context_pack_v1");
    assert_eq!(parsed_pack["question"], duskweave::m1_context::QUESTION);
    assert_eq!(parsed_pack["engagement_id"], json!(e.0));
    assert_eq!(parsed_pack["campaign_id"], json!(c.0));
    assert_eq!(parsed_pack["decision_id"], r["decision_id"]);
    assert_eq!(parsed_pack["goal_ref"], json!(mission_req.goal_ref.0));
    assert_eq!(parsed_pack["exercise_mode"], "blind");

    // Reconstruct expected PreparedContext through core API m1_context::prepare
    let evaluated_at = parsed_pack["evaluated_at"].as_i64().unwrap();
    let epoch = parsed_pack["remaining_capacity"]["snapshot_epoch"]
        .as_i64()
        .unwrap();
    let capacity = duskweave::m1_context_capacity::ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch,
        session_claimed: false,
        active_holds: 0,
        charged: duskweave::m1_inference_budget::ReservedCapacity {
            calls: 0,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    };
    let reconstructed_basis = duskweave::m1_context::ContextBasis {
        mission: source,
        capacity,
        evaluated_at,
    };
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: vec![Candidate {
            asset_ref: AssetRef(a1),
            priority: Priority::High,
        }],
    };
    let mut reader = MockReader {
        basis: Ok(reconstructed_basis),
    };
    let expected_prep = m1_context::prepare(&mut reader, &q, receipt_op).unwrap();

    // Byte-for-byte exact equality between reconstructed pack and receipt pack_utf8
    assert_eq!(receipt_pack_str, expected_prep.utf8());
    assert_eq!(receipt_pack_str.as_bytes(), expected_prep.utf8().as_bytes());

    let carrier_bytes = receipt_pack_str.as_bytes();
    assert!(!carrier_bytes.is_empty());
    assert!(carrier_bytes.len() <= 4096);
}

#[test]
fn rejected_unknown_unicode_sentinel_never_leaks_to_streams() {
    let dsn = std::env::var("DW_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5432/test".into());

    let sentinel = "SENTINEL_CRAB_🦀_RAW_CONTENT";
    let bad_val = json!({
        "version": "m1_context_prepare_request_v1",
        "mission": {
            "version": "m1_inference_reservation_request_v1",
            "engagement_id": Uuid::nil(),
            "campaign_id": Uuid::nil(),
            "operator_ref": Uuid::nil(),
            "expected_mission_revision": 1,
            "permission_id": Uuid::nil(),
            "goal_ref": Uuid::nil(),
            "exercise_mode": "blind"
        },
        "candidates": [],
        "extra_unknown_field": sentinel
    });

    let input = wait_db::InputFile::new(&bad_val, "sentinel_leak");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];
    let out = context_support::run(&args, &dsn);
    assert!(!out.status.success());

    let stdout_str = String::from_utf8_lossy(&out.stdout);
    let stderr_str = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stdout_str.contains(sentinel),
        "sentinel must not leak in stdout"
    );
    assert!(
        !stderr_str.contains(sentinel),
        "sentinel must not leak in stderr"
    );
}

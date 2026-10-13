//! Behavioral unit and invariant tests for m1_context and ingress parsing.
use duskweave::Fail;
use duskweave::m1_context::{self, QUESTION};
use duskweave::m1_context_capacity::ContextCapacity;
use duskweave::m1_context_input::{Candidate, ContextRequest, Priority};
use duskweave::m1_inference_budget::ReservedCapacity;
use duskweave::mission::{AssetRef, OperationId};
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

use context_support::{MockReader, sample_basis};

#[test]
fn context_pack_successful_assembly_and_byte_equivalence() {
    let now = 1700000000;
    let (e, c) = (Uuid::from_u128(0x101), Uuid::from_u128(0x102));
    let (basis, raw) = sample_basis(now, e, c, true);
    let op = OperationId(Uuid::from_u128(0x201));
    let a1 = AssetRef(Uuid::from_u128(0x701));
    let a2 = AssetRef(Uuid::from_u128(0x702));

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
                asset_ref: a1,
                priority: Priority::High,
            },
            Candidate {
                asset_ref: a2,
                priority: Priority::Low,
            },
        ],
    };

    let mut reader = MockReader { basis: Ok(basis) };
    let prepared = m1_context::prepare(&mut reader, &q, op).expect("prepare must succeed");
    let utf8 = prepared.utf8();

    let parsed: Value = serde_json::from_str(utf8).expect("pack is valid JSON");
    assert_eq!(parsed["version"], "m1_context_pack_v1");
    assert_eq!(parsed["question"], QUESTION);
    assert_eq!(parsed["decision_id"], op.to_string());
    assert_eq!(parsed["engagement_id"], e.to_string());
    assert_eq!(parsed["campaign_id"], c.to_string());
    assert_eq!(parsed["mission_revision"], 1);

    let views = parsed["owner_views"].as_array().expect("views array");
    assert_eq!(views.len(), 6);
    assert_eq!(views[0]["owner"], "mission");
    assert_eq!(views[0]["availability"], "available");
    for v in &views[1..] {
        assert_eq!(v["availability"], "unavailable");
        assert_eq!(v["reason"], "not_loaded_by_this_profile");
    }

    assert_eq!(parsed["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["omissions"]["supplied_candidates"], 2);
    assert_eq!(parsed["omissions"]["shown_candidates"], 2);
    assert_eq!(parsed["omissions"]["omitted_candidates"], 0);
    assert_eq!(parsed["omissions"]["manifests"], "catalog_unavailable");
    assert_eq!(parsed["manifests"].as_array().unwrap().len(), 0);

    let (basis2, _) = sample_basis(now, e, c, true);
    let mut reader2 = MockReader { basis: Ok(basis2) };
    let prepared2 = m1_context::prepare(&mut reader2, &q, op).expect("prepare must succeed");
    assert_eq!(prepared.utf8().as_bytes(), prepared2.utf8().as_bytes());
}

#[test]
fn ingress_bounds_and_unknown_fields_rejected() {
    let now = 1700000000;
    let (e, c) = (Uuid::from_u128(0x301), Uuid::from_u128(0x302));
    let (_, raw) = sample_basis(now, e, c, true);
    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );

    let bad_json = json!({
        "version": "m1_context_prepare_request_v1",
        "mission": mission_req,
        "candidates": [],
        "unknown_extra": "rejected"
    });
    let res: Result<ContextRequest, _> = serde_json::from_value(bad_json);
    assert!(
        res.is_err(),
        "unknown field in ContextRequest must be denied"
    );

    let a = AssetRef(Uuid::from_u128(0x401));
    let mut list64 = Vec::new();
    for _ in 0..64 {
        list64.push(json!({"asset_ref": a.0, "priority": "high"}));
    }
    let mut list65 = list64.clone();
    list65.push(json!({"asset_ref": a.0, "priority": "high"}));

    let req64 = json!({
        "version": "m1_context_prepare_request_v1",
        "mission": mission_req,
        "candidates": list64
    });
    let parsed64: Result<ContextRequest, _> = serde_json::from_value(req64);
    assert!(parsed64.is_ok(), "64 candidates must deserialize");

    let req65 = json!({
        "version": "m1_context_prepare_request_v1",
        "mission": mission_req,
        "candidates": list65
    });
    let parsed65: Result<ContextRequest, _> = serde_json::from_value(req65);
    assert!(
        parsed65.is_err(),
        "65 candidates must be rejected by bounded_candidates visitor"
    );

    let q64 = parsed64.unwrap();
    assert_eq!(
        q64.validate_ingress(),
        Err(Fail::Input("invalid_context_candidates"))
    );

    let nil_cand = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: vec![Candidate {
            asset_ref: AssetRef(Uuid::nil()),
            priority: Priority::High,
        }],
    };
    assert_eq!(
        nil_cand.validate_ingress(),
        Err(Fail::Input("invalid_context_candidates"))
    );
}

#[test]
fn window_boundary_inclusive_starts_exclusive_ends() {
    let now = 1700000000;
    let (e, c) = (Uuid::from_u128(0x501), Uuid::from_u128(0x502));
    let (mut basis, raw) = sample_basis(now, e, c, false);
    let op = OperationId(Uuid::from_u128(0x503));
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

    let p = raw["m1_inference_permission"].clone();
    let starts = p["starts_at"].as_i64().unwrap();
    let ends = p["ends_at"].as_i64().unwrap();

    basis.evaluated_at = starts;
    let mut reader = MockReader { basis: Ok(basis) };
    assert!(m1_context::prepare(&mut reader, &q, op).is_ok());

    let (mut basis_early, _) = sample_basis(now, e, c, false);
    basis_early.evaluated_at = starts - 1;
    let mut reader = MockReader {
        basis: Ok(basis_early),
    };
    assert_eq!(
        m1_context::prepare(&mut reader, &q, op).err(),
        Some(Fail::State("context_consent_deferred"))
    );

    let (mut basis_late, _) = sample_basis(now, e, c, false);
    basis_late.evaluated_at = ends;
    let mut reader = MockReader {
        basis: Ok(basis_late),
    };
    assert_eq!(
        m1_context::prepare(&mut reader, &q, op).err(),
        Some(Fail::State("context_consent_deferred"))
    );
}

#[test]
fn out_of_scope_assets_refused() {
    let now = 1700000000;
    let (e, c) = (Uuid::from_u128(0x601), Uuid::from_u128(0x602));
    let (basis, raw) = sample_basis(now, e, c, true);
    let op = OperationId(Uuid::from_u128(0x603));
    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );
    let q_uninc = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req.clone(),
        candidates: vec![Candidate {
            asset_ref: AssetRef(Uuid::from_u128(0x699)),
            priority: Priority::High,
        }],
    };
    let mut reader = MockReader { basis: Ok(basis) };
    assert_eq!(
        m1_context::prepare(&mut reader, &q_uninc, op).err(),
        Some(Fail::Input("context_scope_refused"))
    );

    let a_exc = AssetRef(Uuid::from_u128(0x703));
    let (basis2, _) = sample_basis(now, e, c, true);
    let q_exc = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: vec![Candidate {
            asset_ref: a_exc,
            priority: Priority::High,
        }],
    };
    let mut reader2 = MockReader { basis: Ok(basis2) };
    assert_eq!(
        m1_context::prepare(&mut reader2, &q_exc, op).err(),
        Some(Fail::Input("context_scope_refused"))
    );
}

#[test]
fn candidates_deferred_when_consent_omits_operator_premises() {
    let now = 1700000000;
    let (e, c) = (Uuid::from_u128(0x801), Uuid::from_u128(0x802));
    let (basis, raw) = sample_basis(now, e, c, false); // without operator premises
    let op = OperationId(Uuid::from_u128(0x803));
    let mission_req = budget::request(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        &raw,
    );
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: mission_req,
        candidates: vec![Candidate {
            asset_ref: AssetRef(Uuid::from_u128(0x701)),
            priority: Priority::High,
        }],
    };
    let mut reader = MockReader { basis: Ok(basis) };
    assert_eq!(
        m1_context::prepare(&mut reader, &q, op).err(),
        Some(Fail::State("context_disclosure_deferred"))
    );
}

#[test]
fn capacity_remainder_and_validation() {
    let c = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 0,
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
    let totals = [10, 100000, 200000, 50000, 60000];
    let remain = c
        .remaining(totals)
        .expect("capacity remainder must compute");
    assert_eq!(remain.calls, 10);
    assert_eq!(remain.request_bytes, 100000);
    assert_eq!(remain.response_bytes, 200000);
    assert_eq!(remain.output_token_capacity, 50000);
    assert_eq!(remain.wall_time_ms, 60000);

    let c_over = ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 0,
        session_claimed: false,
        active_holds: 1,
        charged: ReservedCapacity {
            calls: 2,
            request_bytes: 5000,
            response_bytes: 5000,
            output_token_capacity: 8192,
            wall_time_ms: 5000,
        },
    };
    let small_totals = [1, 5000, 5000, 8192, 5000];
    assert_eq!(
        c_over.remaining(small_totals),
        Err(Fail::Store("context_capacity_unavailable"))
    );
}

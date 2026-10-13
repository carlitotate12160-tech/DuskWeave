use duskweave::Fail;
use duskweave::m1_context::prepare;
use duskweave::m1_inference_budget::ReservationRequest;
use duskweave::mission::{CampaignId, EngagementId, ExerciseMode, OperatorRef};
use duskweave::postgres_mission::PgContextReader;
use serde_json::json;
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

fn legacy_case(seed: u128, remove_both: bool, version: u32) {
    let _g = db_support::db();
    let (normal, _) = budget::fixture(seed);
    let normal_state = context_support::scoped_state(normal.engagement_id.0, normal.campaign_id.0);
    context_support::prepare_pack(&context_support::context_request(&normal));
    let (e, c) = db_support::scope(seed + 1);
    let mut raw = budget::registration_value(e, c, session_support::now());
    let req = budget::request(e, c, &raw);
    if remove_both {
        raw.as_object_mut().unwrap().remove("m1_permission");
    }
    raw.as_object_mut()
        .unwrap()
        .remove("m1_inference_permission");
    let (_, source) = budget::register(e, c, &raw);
    assert_eq!(source.version, version);
    assert!(source.fields.m1_inference_permission().is_none());
    let q = context_support::context_request(&req);
    q.validate_ingress().unwrap();
    let before = context_support::scoped_state(e.0, c.0);
    let mut reader = PgContextReader::new(session_support::broker_client());
    assert_eq!(
        prepare(&mut reader, &q, budget::op()).err(),
        Some(Fail::State("context_mission_unavailable"))
    );
    assert_eq!(before, context_support::scoped_state(e.0, c.0));
    let out = context_support::run_request(&q, "legacy_authority");
    context_support::assert_failed_receipt(&out, "deferred");
    assert_eq!(before, context_support::scoped_state(e.0, c.0));
    assert_eq!(
        normal_state,
        context_support::scoped_state(normal.engagement_id.0, normal.campaign_id.0)
    );
}

#[test]
fn actual_legacy_v1_publication_gate_and_cli_defer_without_mutation() {
    legacy_case(0x9901, true, 1);
}

#[test]
fn actual_crtsh_v2_publication_gate_and_cli_defer_without_mutation() {
    legacy_case(0x99a1, false, 2);
}

fn alternate(e: EngagementId, c: CampaignId, permission: u128) -> ReservationRequest {
    let mut raw = budget::registration_value(e, c, session_support::now());
    raw["m1_inference_permission"]["permission_id"] = json!(Uuid::from_u128(permission));
    let (_, source) = budget::register(e, c, &raw);
    assert_eq!(source.version, 3);
    budget::request(e, c, &raw)
}

#[test]
fn actual_owner_basis_matrix_distinguishes_publication_ingress_and_basis_boundaries() {
    let _g = db_support::db();
    let (original, source) = budget::fixture(0x99b0);
    let (foreign_e, foreign_c) = db_support::scope(0x99b1);
    let (disconnected_e, disconnected_c) = db_support::scope(0x99b2);
    let engagement = alternate(foreign_e, original.campaign_id, 0x99b3);
    let campaign = alternate(original.engagement_id, foreign_c, 0x99b4);
    let disconnected = alternate(disconnected_e, disconnected_c, 0x99b5);
    let scopes = [&original, &engagement, &campaign, &disconnected];
    let states: Vec<_> = scopes
        .iter()
        .map(|q| context_support::scoped_state(q.engagement_id.0, q.campaign_id.0))
        .collect();
    let q = context_support::context_request(&original);
    context_support::assert_mission_frontier(&context_support::prepare_pack(&q), &source);
    for foreign in scopes.iter().skip(1) {
        let p = context_support::prepare_pack(&context_support::context_request(foreign));
        assert_eq!(p["campaign_id"], json!(foreign.campaign_id));
        assert_eq!(p["permission_id"], json!(foreign.permission_id));
    }
    let unavailable = Fail::State("context_mission_unavailable");
    let mismatch = Fail::State("context_basis_mismatch");
    let mut variations = Vec::new();
    for (axis, value, expected) in [
        ("engagement_id", json!(foreign_e), mismatch),
        ("campaign_id", json!(foreign_c), mismatch),
        ("operator_ref", json!(Uuid::from_u128(0x99b6)), mismatch),
        ("permission_id", json!(engagement.permission_id), mismatch),
        (
            "expected_mission_revision",
            json!(2),
            Fail::Input("invalid_inference_reservation"),
        ),
        ("goal_ref", json!(Uuid::from_u128(0x99b7)), mismatch),
        (
            "exercise_mode",
            json!(ExerciseMode::DefenderInformed),
            mismatch,
        ),
        ("engagement_id", json!(disconnected_e), unavailable),
        ("campaign_id", json!(disconnected_c), unavailable),
    ] {
        let mut raw = serde_json::to_value(&original).unwrap();
        raw[axis] = value;
        variations.push((
            axis,
            serde_json::from_value::<ReservationRequest>(raw).unwrap(),
            expected,
        ));
    }
    for (axis, mission, expected) in variations {
        let q = context_support::context_request(&mission);
        if axis == "expected_mission_revision" {
            assert_eq!(q.validate_ingress(), Err(expected));
        } else {
            q.validate_ingress().unwrap();
        }
        let requested_before =
            context_support::scoped_state(mission.engagement_id.0, mission.campaign_id.0);
        let mut reader = PgContextReader::new(session_support::broker_client());
        assert_eq!(
            prepare(&mut reader, &q, budget::op()).err(),
            Some(expected),
            "{axis}"
        );
        assert_eq!(
            requested_before,
            context_support::scoped_state(mission.engagement_id.0, mission.campaign_id.0)
        );
        for (scope, before) in scopes.iter().zip(&states) {
            assert_eq!(
                *before,
                context_support::scoped_state(scope.engagement_id.0, scope.campaign_id.0),
                "{axis}"
            );
        }
    }
}

#[test]
fn cli_wrong_operator_and_mode_are_post_allocation_refused_and_read_only() {
    let _g = db_support::db();
    let (req, _) = budget::fixture(0x99c0);
    let before = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    context_support::prepare_pack(&context_support::context_request(&req));
    let mut operator = req.clone();
    operator.operator_ref = OperatorRef(Uuid::from_u128(0x99c1));
    let mut mode = req.clone();
    mode.exercise_mode = ExerciseMode::DefenderInformed;
    for wrong in [operator, mode] {
        let q = context_support::context_request(&wrong);
        let mut reader = PgContextReader::new(session_support::broker_client());
        assert_eq!(
            prepare(&mut reader, &q, budget::op()).err(),
            Some(Fail::State("context_basis_mismatch"))
        );
        let out = context_support::run_request(&q, "wrong_basis_authority");
        context_support::assert_failed_receipt(&out, "refused");
        assert!(String::from_utf8_lossy(&out.stdout).contains("error=context_basis_mismatch"));
        assert_eq!(
            before,
            context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
        );
    }
}

#[test]
fn revision_two_is_rejected_at_ingress_before_cli_allocation_or_receipt() {
    let _g = db_support::db();
    let (mut req, _) = budget::fixture(0x99d0);
    let before = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    context_support::prepare_pack(&context_support::context_request(&req));
    req.expected_mission_revision = 2;
    let q = context_support::context_request(&req);
    assert_eq!(
        q.validate_ingress(),
        Err(Fail::Input("invalid_inference_reservation"))
    );
    let out = context_support::run_request(&q, "revision_ingress");
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "error=invalid_inference_reservation"
    );
    assert!(out.stderr.is_empty());
    assert_eq!(
        before,
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0)
    );
}

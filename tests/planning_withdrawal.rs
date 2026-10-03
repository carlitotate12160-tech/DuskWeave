//! Version-3 recorded-withdrawal refusal: pure contract assertions. No
//! database; the v3 constructor, source validation, wire shape and the v1/v2
//! exclusion of any withdrawal basis are exercised here.

use duskweave::Fail;
use duskweave::mission::*;
use duskweave::planning::{
    MissionBasis, MissionScope, NonpositiveDecision, PlanningAssessed, PlanningRequest,
    WithdrawalBasis,
};
use duskweave::withdrawal::{MissionAuthorityWithdrawn, WithdrawalReason, WithdrawalRequest};
use uuid::Uuid;

fn request(e: EngagementId, c: CampaignId, revision: u64) -> PlanningRequest {
    PlanningRequest {
        engagement_id: e,
        campaign_id: c,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: revision,
        current_authority_confirmed: true,
    }
}

fn basis(reg_op: OperationId) -> MissionBasis {
    MissionBasis {
        registration_operation_id: reg_op,
        revision: 1,
        exercise_mode: ExerciseMode::Blind,
        starts_at: 100,
        ends_at: 200,
        scope: Some(MissionScope {
            goal_ref: GoalRef(Uuid::from_u128(0x13)),
            included_assets: vec![AssetRef(Uuid::from_u128(0x21))],
            excluded_assets: vec![],
        }),
    }
}

fn withdrawal(
    e: EngagementId,
    c: CampaignId,
    reg_op: OperationId,
    op: OperationId,
    event: EventId,
) -> MissionAuthorityWithdrawn {
    MissionAuthorityWithdrawn::new(
        WithdrawalRequest {
            engagement_id: e,
            campaign_id: c,
            operator_ref: OperatorRef(Uuid::from_u128(99)),
            expected_mission_revision: 1,
            reason: WithdrawalReason::OperatorRequested,
        },
        op,
        reg_op,
        event,
        150,
    )
    .unwrap()
}

fn ids(
    base: u128,
) -> (
    EngagementId,
    CampaignId,
    OperationId,
    OperationId,
    EventId,
    EventId,
) {
    (
        EngagementId(Uuid::from_u128(base)),
        CampaignId(Uuid::from_u128(base + 1)),
        OperationId(Uuid::from_u128(base + 2)),
        OperationId(Uuid::from_u128(base + 3)),
        EventId(Uuid::from_u128(base + 4)),
        EventId(Uuid::from_u128(base + 5)),
    )
}

#[test]
fn v3_refusal_constructs_validates_and_round_trips_with_withdrawal_basis() {
    let (e, c, reg_op, w_op, w_ev, a_ev) = ids(0xa300);
    let event = PlanningAssessed::new_withdrawn(
        request(e, c, 1),
        basis(reg_op),
        withdrawal(e, c, reg_op, w_op, w_ev),
        w_op,
        a_ev,
        150,
    )
    .unwrap();
    assert_eq!(event.version, 3);
    assert_eq!(event.owner_revision, 2);
    assert_eq!(
        event.decision,
        NonpositiveDecision::RefusedAuthorityWithdrawn
    );
    assert_eq!(
        event.withdrawal,
        Some(WithdrawalBasis {
            operation_id: w_op,
            event_id: w_ev,
        })
    );
    event.validate().unwrap();
    assert_eq!(
        event.assessment_labels(),
        ("not_evaluated", "not_evaluated")
    );
    // Wire shape keeps the withdrawal basis and never emits it as absent.
    let wire = serde_json::to_value(&event).unwrap();
    assert_eq!(wire["version"], 3);
    assert_eq!(wire["owner_revision"], 2);
    assert_eq!(wire["withdrawal"]["operation_id"], w_op.0.to_string());
    assert_eq!(wire["withdrawal"]["event_id"], w_ev.0.to_string());
    assert_eq!(wire["decision"], "refused_authority_withdrawn");
    let round: PlanningAssessed = serde_json::from_value(wire).unwrap();
    assert_eq!(round, event);
}

#[test]
fn v3_guard_dominates_stale_revision_scope_and_window() {
    let (e, c, reg_op, w_op, w_ev, a_ev) = ids(0xa310);
    // A stale expected revision still yields the sole withdrawal decision: the
    // v3 guard dominates confirmation, revision, scope and window checks.
    let event = PlanningAssessed::new_withdrawn(
        request(e, c, 99),
        basis(reg_op),
        withdrawal(e, c, reg_op, w_op, w_ev),
        w_op,
        a_ev,
        150,
    )
    .unwrap();
    assert_eq!(
        event.decision,
        NonpositiveDecision::RefusedAuthorityWithdrawn
    );
    event.validate().unwrap();
}

#[test]
fn v3_constructor_rejects_unbound_withdrawal_scope_or_registration() {
    let (e, c, reg_op, w_op, w_ev, a_ev) = ids(0xa320);
    let (other_e, other_c) = (
        EngagementId(Uuid::from_u128(0xa330)),
        CampaignId(Uuid::from_u128(0xa331)),
    );
    // Marker engagement mismatch.
    let mismatched_engagement = withdrawal(other_e, c, reg_op, w_op, w_ev);
    assert_eq!(
        PlanningAssessed::new_withdrawn(
            request(e, c, 1),
            basis(reg_op),
            mismatched_engagement,
            w_op,
            a_ev,
            150,
        )
        .err(),
        Some(Fail::Unresolved("scope_violation"))
    );
    // Marker campaign mismatch.
    let mismatched_campaign = withdrawal(e, other_c, reg_op, w_op, w_ev);
    assert_eq!(
        PlanningAssessed::new_withdrawn(
            request(e, c, 1),
            basis(reg_op),
            mismatched_campaign,
            w_op,
            a_ev,
            150,
        )
        .err(),
        Some(Fail::Unresolved("scope_violation"))
    );
    // Registration-operation mismatch between marker and basis.
    let wrong_registration = withdrawal(e, c, OperationId(Uuid::from_u128(0xa332)), w_op, w_ev);
    assert_eq!(
        PlanningAssessed::new_withdrawn(
            request(e, c, 1),
            basis(reg_op),
            wrong_registration,
            w_op,
            a_ev,
            150,
        )
        .err(),
        Some(Fail::Unresolved("scope_violation"))
    );
}

#[test]
fn v1_v2_reject_any_withdrawal_basis_or_withdrawn_decision() {
    let (e, c, reg_op, w_op, w_ev, a_ev) = ids(0xa340);
    let valid = PlanningAssessed::new_withdrawn(
        request(e, c, 1),
        basis(reg_op),
        withdrawal(e, c, reg_op, w_op, w_ev),
        w_op,
        a_ev,
        150,
    )
    .unwrap();
    // v2 with a populated withdrawal basis is unsupported.
    let mut v2_with_withdrawal = valid.clone();
    v2_with_withdrawal.version = 2;
    v2_with_withdrawal.owner_revision = 1;
    assert_eq!(
        v2_with_withdrawal.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    // v2 with the withdrawn decision but no withdrawal basis is invalid:
    // the decision can never be computed by the v2 guards.
    let mut v2_decision_only = valid.clone();
    v2_decision_only.version = 2;
    v2_decision_only.owner_revision = 1;
    v2_decision_only.withdrawal = None;
    assert_eq!(
        v2_decision_only.validate(),
        Err(Fail::Unresolved("invalid_assessment"))
    );
}

#[test]
fn v3_shape_requires_withdrawn_decision_basis_and_non_nil_withdrawal() {
    let (e, c, reg_op, w_op, w_ev, a_ev) = ids(0xa350);
    let valid = PlanningAssessed::new_withdrawn(
        request(e, c, 1),
        basis(reg_op),
        withdrawal(e, c, reg_op, w_op, w_ev),
        w_op,
        a_ev,
        150,
    )
    .unwrap();
    // Missing basis cannot ground a v3 refusal.
    let mut no_basis = valid.clone();
    no_basis.basis = None;
    assert_eq!(
        no_basis.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    // Unscoped basis cannot ground a v3 refusal.
    let mut unscoped = valid.clone();
    unscoped.basis.as_mut().unwrap().scope = None;
    assert_eq!(
        unscoped.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    // Nil withdrawal references are unsupported.
    let mut nil_withdrawal = valid.clone();
    nil_withdrawal.withdrawal = Some(WithdrawalBasis {
        operation_id: OperationId(Uuid::nil()),
        event_id: EventId(Uuid::nil()),
    });
    assert_eq!(
        nil_withdrawal.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    // Wrong owner revision is unsupported.
    let mut wrong_revision = valid.clone();
    wrong_revision.owner_revision = 1;
    assert_eq!(
        wrong_revision.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    // A different decision is unsupported under v3.
    let mut other_decision = valid.clone();
    other_decision.decision = NonpositiveDecision::RefusedExpired;
    assert_eq!(
        other_decision.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
}

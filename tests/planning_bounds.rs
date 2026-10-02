//! Pure contract tests for v2 scope/time planning decisions.
//! Boundaries are exercised through injected constructor timestamps; no
//! production clock override exists.

use duskweave::Fail;
use duskweave::mission::{
    AssetRef, CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, OperationId,
};
use duskweave::planning::{
    MissionBasis, MissionScope, NonpositiveDecision, PlanningAssessed, PlanningRequest,
};
use uuid::Uuid;

fn request() -> PlanningRequest {
    PlanningRequest {
        engagement_id: EngagementId(Uuid::from_u128(1)),
        campaign_id: CampaignId(Uuid::from_u128(2)),
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}

fn scope() -> MissionScope {
    MissionScope {
        goal_ref: GoalRef(Uuid::from_u128(0x13)),
        included_assets: vec![
            AssetRef(Uuid::from_u128(0x21)),
            AssetRef(Uuid::from_u128(0x22)),
        ],
        excluded_assets: vec![AssetRef(Uuid::from_u128(0x23))],
    }
}

fn basis() -> MissionBasis {
    MissionBasis {
        registration_operation_id: OperationId(Uuid::from_u128(9)),
        revision: 1,
        exercise_mode: ExerciseMode::Blind,
        starts_at: 100,
        ends_at: 200,
        scope: Some(scope()),
    }
}

fn decide(request: &PlanningRequest, basis: Option<&MissionBasis>, at: i64) -> NonpositiveDecision {
    NonpositiveDecision::for_request(2, request, basis, at).unwrap()
}

fn event(
    request: &PlanningRequest,
    basis: Option<MissionBasis>,
    at: i64,
) -> duskweave::Res<PlanningAssessed> {
    PlanningAssessed::new(
        request.clone(),
        basis,
        OperationId(Uuid::from_u128(5)),
        EventId(Uuid::from_u128(6)),
        at,
    )
}

#[test]
fn v2_precedence_rows_cover_every_nonpositive_decision() {
    let request = request();
    let basis = basis();
    assert_eq!(
        decide(&request, None, 150),
        NonpositiveDecision::UnresolvedMissionBasis
    );
    // Guard order: revision, confirmation, purpose, exclusion, inclusion,
    // not-yet-valid, expired; earlier guards dominate later mismatches.
    let mut changed = request.clone();
    changed.expected_mission_revision = 2;
    changed.purpose_ref = GoalRef(Uuid::from_u128(0x99));
    changed.asset_ref = AssetRef(Uuid::from_u128(0x23));
    assert_eq!(
        decide(&changed, Some(&basis), 50),
        NonpositiveDecision::RefusedRevisionMismatch
    );
    changed = request.clone();
    changed.current_authority_confirmed = false;
    changed.purpose_ref = GoalRef(Uuid::from_u128(0x99));
    assert_eq!(
        decide(&changed, Some(&basis), 50),
        NonpositiveDecision::UnresolvedAuthorityUnconfirmed
    );
    changed = request.clone();
    changed.purpose_ref = GoalRef(Uuid::from_u128(0x99));
    changed.asset_ref = AssetRef(Uuid::from_u128(0x23));
    assert_eq!(
        decide(&changed, Some(&basis), 50),
        NonpositiveDecision::RefusedPurposeMismatch
    );
    changed = request.clone();
    changed.asset_ref = AssetRef(Uuid::from_u128(0x23));
    assert_eq!(
        decide(&changed, Some(&basis), 50),
        NonpositiveDecision::RefusedAssetExcluded
    );
    changed.asset_ref = AssetRef(Uuid::from_u128(0x99));
    assert_eq!(
        decide(&changed, Some(&basis), 50),
        NonpositiveDecision::RefusedAssetUnknown
    );
    assert_eq!(
        decide(&request, Some(&basis), 99),
        NonpositiveDecision::RefusedNotYetValid
    );
    assert_eq!(
        decide(&request, Some(&basis), 200),
        NonpositiveDecision::RefusedExpired
    );
    assert_eq!(
        decide(&request, Some(&basis), 150),
        NonpositiveDecision::UnresolvedEvaluationIncomplete
    );
}

#[test]
fn exclusion_wins_over_inclusion_overlap() {
    let mut scope = scope();
    scope.included_assets.push(AssetRef(Uuid::from_u128(0x24)));
    scope.excluded_assets.push(AssetRef(Uuid::from_u128(0x24)));
    let mut basis = basis();
    basis.scope = Some(scope);
    let mut request = request();
    request.asset_ref = AssetRef(Uuid::from_u128(0x24));
    assert_eq!(
        decide(&request, Some(&basis), 150),
        NonpositiveDecision::RefusedAssetExcluded
    );
}

#[test]
fn exact_window_boundaries_use_half_open_interval() {
    let basis = basis();
    let request = request();
    for (at, expected) in [
        (99, NonpositiveDecision::RefusedNotYetValid),
        (100, NonpositiveDecision::UnresolvedEvaluationIncomplete),
        (199, NonpositiveDecision::UnresolvedEvaluationIncomplete),
        (200, NonpositiveDecision::RefusedExpired),
    ] {
        let produced = event(&request, Some(basis.clone()), at).unwrap();
        assert_eq!(produced.decision, expected, "evaluated_at={at}");
        assert_eq!(produced.evaluated_at, at);
        assert_eq!(produced.version, 2);
    }
}

#[test]
fn v2_basis_requires_valid_scope_snapshot() {
    let request = request();
    for broken in [
        {
            let mut s = scope();
            s.goal_ref = GoalRef(Uuid::nil());
            s
        },
        {
            let mut s = scope();
            s.included_assets = Vec::new();
            s
        },
        {
            let mut s = scope();
            s.included_assets[0] = AssetRef(Uuid::nil());
            s
        },
        {
            let mut s = scope();
            s.included_assets = (0..65)
                .map(|i| AssetRef(Uuid::from_u128(0x100 + i)))
                .collect();
            s
        },
        {
            let mut s = scope();
            s.excluded_assets = (0..65)
                .map(|i| AssetRef(Uuid::from_u128(0x200 + i)))
                .collect();
            s
        },
    ] {
        let mut basis = basis();
        basis.scope = Some(broken);
        assert_eq!(
            event(&request, Some(basis), 150),
            Err(Fail::Store("unsupported_basis"))
        );
    }
    // Version 2 without a scope snapshot is not a valid owner basis either.
    let mut unscoped = basis();
    unscoped.scope = None;
    assert_eq!(
        event(&request, Some(unscoped), 150),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    // An absent Mission is still a legal version-2 nonpositive outcome.
    let produced = event(&request, None, 150).unwrap();
    assert_eq!(
        produced.decision,
        NonpositiveDecision::UnresolvedMissionBasis
    );
    assert_eq!(produced.version, 2);
}

#[test]
fn unsupported_versions_and_v1_semantic_masquerade_fail() {
    let request = request();
    assert_eq!(
        NonpositiveDecision::for_request(3, &request, Some(&basis()), 150),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    let mut produced = event(&request, Some(basis()), 150).unwrap();
    produced.version = 3;
    assert_eq!(
        produced.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    // A version-1 event must not carry the scope snapshot or a v2-only decision.
    let mut v1 = produced.clone();
    v1.version = 1;
    assert_eq!(v1.validate(), Err(Fail::Unresolved("unsupported_contract")));
    v1.basis.as_mut().unwrap().scope = None;
    assert!(v1.validate().is_ok());
    v1.decision = NonpositiveDecision::RefusedExpired;
    assert_eq!(v1.validate(), Err(Fail::Unresolved("invalid_assessment")));
}

#[test]
fn historical_labels_follow_version_and_original_decision() {
    let request = request();
    let basis = basis();
    for (mutate, at, scope_label, window_label) in [
        (0u8, 150, "matched", "within_window"),
        (1, 150, "purpose_mismatch", "not_evaluated"),
        (2, 150, "excluded", "not_evaluated"),
        (3, 150, "unknown", "not_evaluated"),
        (4, 99, "matched", "not_yet_valid"),
        (5, 250, "matched", "expired"),
        (6, 150, "not_evaluated", "not_evaluated"),
    ] {
        let mut changed = request.clone();
        let used = match mutate {
            1 => {
                changed.purpose_ref = GoalRef(Uuid::from_u128(0x99));
                Some(basis.clone())
            }
            2 => {
                changed.asset_ref = AssetRef(Uuid::from_u128(0x23));
                Some(basis.clone())
            }
            3 => {
                changed.asset_ref = AssetRef(Uuid::from_u128(0x99));
                Some(basis.clone())
            }
            4 | 5 => Some(basis.clone()),
            6 => {
                changed.current_authority_confirmed = false;
                Some(basis.clone())
            }
            _ => Some(basis.clone()),
        };
        let produced = event(&changed, used, at).unwrap();
        assert_eq!(
            produced.assessment_labels(),
            (scope_label, window_label),
            "case {mutate}"
        );
    }
    let absent = event(&request, None, 150).unwrap();
    assert_eq!(
        absent.assessment_labels(),
        ("not_evaluated", "not_evaluated")
    );
    let mut legacy = event(&request, Some(basis.clone()), 150).unwrap();
    legacy.version = 1;
    legacy.basis.as_mut().unwrap().scope = None;
    assert_eq!(
        legacy.assessment_labels(),
        ("not_evaluated", "not_evaluated")
    );
}

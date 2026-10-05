use duskweave::Fail;
use duskweave::mission::*;
use duskweave::planning::NonpositiveDecision::RefusedExpired;
use duskweave::planning::{
    MissionBasis, MissionScope, NonpositiveDecision, PlanningAssessed, PlanningDecision,
    PlanningRequest,
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

fn basis() -> MissionBasis {
    MissionBasis {
        registration_operation_id: OperationId(Uuid::from_u128(3)),
        revision: 1,
        exercise_mode: ExerciseMode::Blind,
        starts_at: 100,
        ends_at: 200,
        scope: Some(MissionScope {
            goal_ref: GoalRef(Uuid::from_u128(0x13)),
            included_assets: vec![AssetRef(Uuid::from_u128(0x21))],
            excluded_assets: vec![
                AssetRef(Uuid::from_u128(0x21)),
                AssetRef(Uuid::from_u128(0x23)),
            ],
        }),
    }
}

fn current(
    request: PlanningRequest,
    basis: Option<MissionBasis>,
    at: i64,
) -> duskweave::Res<PlanningAssessed> {
    PlanningAssessed::new_current(
        request,
        basis,
        OperationId(Uuid::from_u128(4)),
        EventId(Uuid::from_u128(5)),
        at,
    )
}

#[test]
fn v4_only_records_valid_half_open_window_and_legacy_v2_is_unchanged() {
    let mut scoped = basis();
    scoped.scope.as_mut().unwrap().excluded_assets.remove(0);
    for (at, expected) in [
        (99, PlanningDecision::RefusedNotYetValid),
        (100, PlanningDecision::Eligible),
        (199, PlanningDecision::Eligible),
        (200, PlanningDecision::RefusedExpired),
    ] {
        let event = current(request(), Some(scoped.clone()), at).unwrap();
        assert_eq!(event.decision, expected);
        assert_eq!(
            event.version,
            if expected == PlanningDecision::Eligible {
                4
            } else {
                2
            }
        );
        assert_eq!(
            event.recorded_eligible(),
            expected == PlanningDecision::Eligible
        );
        assert_eq!(event.evaluated_at, at);
        assert_eq!(
            event.assessment_labels(),
            if expected == PlanningDecision::Eligible {
                ("matched", "within_window")
            } else if at == 99 {
                ("matched", "not_yet_valid")
            } else {
                ("matched", "expired")
            }
        );
    }
    let legacy = PlanningAssessed::new(
        request(),
        Some(scoped),
        OperationId(Uuid::from_u128(4)),
        EventId(Uuid::from_u128(5)),
        150,
    )
    .unwrap();
    assert_eq!(legacy.version, 2);
    assert_eq!(
        legacy.decision,
        NonpositiveDecision::UnresolvedEvaluationIncomplete
    );
    assert_eq!(
        serde_json::to_value(&legacy).unwrap()["decision"],
        "unresolved_evaluation_incomplete"
    );
    assert_eq!(legacy.assessment_labels(), ("matched", "within_window"));
    assert!(!legacy.recorded_eligible());
    let mut forged = legacy;
    forged.decision = PlanningDecision::Eligible;
    assert_eq!(
        forged.validate(),
        Err(Fail::Unresolved("invalid_assessment"))
    );
    forged.version = 1;
    forged.basis.as_mut().unwrap().scope = None;
    assert_eq!(
        forged.validate(),
        Err(Fail::Unresolved("invalid_assessment"))
    );
    forged.version = 3;
    assert_eq!(
        forged.validate(),
        Err(Fail::Unresolved("unsupported_contract"))
    );
}

#[test]
fn guard_precedence_keeps_every_fresh_nonpositive_outcome_on_v2() {
    let request = request();
    let scoped = basis();
    let cases = [
        (
            request.clone(),
            None,
            PlanningDecision::UnresolvedMissionBasis,
        ),
        (
            {
                let mut r = request.clone();
                r.expected_mission_revision = 2;
                r
            },
            Some(scoped.clone()),
            PlanningDecision::RefusedRevisionMismatch,
        ),
        (
            {
                let mut r = request.clone();
                r.current_authority_confirmed = false;
                r
            },
            Some(scoped.clone()),
            PlanningDecision::UnresolvedAuthorityUnconfirmed,
        ),
        (
            {
                let mut r = request.clone();
                r.purpose_ref = GoalRef(Uuid::from_u128(10));
                r
            },
            Some(scoped.clone()),
            PlanningDecision::RefusedPurposeMismatch,
        ),
        (
            request.clone(),
            Some(scoped.clone()),
            PlanningDecision::RefusedAssetExcluded,
        ),
        (
            {
                let mut r = request.clone();
                r.asset_ref = AssetRef(Uuid::from_u128(10));
                r
            },
            Some(scoped.clone()),
            PlanningDecision::RefusedAssetUnknown,
        ),
    ];
    for (request, basis, expected) in cases {
        let event = current(request, basis, 150).unwrap();
        assert_eq!(event.version, 2);
        assert_eq!(event.decision, expected);
        assert!(!event.recorded_eligible());
    }
}

#[test]
fn v4_cannot_be_forged_from_stale_or_malformed_source() {
    let mut scoped = basis();
    scoped.scope.as_mut().unwrap().excluded_assets.clear();
    let valid = current(request(), Some(scoped), 150).unwrap();
    assert_eq!(valid.version, 4);
    let mut variants = Vec::new();
    let mut changed = valid.clone();
    changed.basis = None;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.basis.as_mut().unwrap().scope = None;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.basis.as_mut().unwrap().revision = 2;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.request.expected_mission_revision = 2;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.request.current_authority_confirmed = false;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.request.purpose_ref = GoalRef(Uuid::from_u128(99));
    variants.push(changed);
    let mut changed = valid.clone();
    changed.request.asset_ref = AssetRef(Uuid::from_u128(99));
    variants.push(changed);
    let mut changed = valid.clone();
    changed
        .basis
        .as_mut()
        .unwrap()
        .scope
        .as_mut()
        .unwrap()
        .excluded_assets
        .push(valid.request.asset_ref);
    variants.push(changed);
    let mut changed = valid.clone();
    changed.evaluated_at = 200;
    changed.occurred_at = 200;
    changed.recorded_at = 200;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.withdrawal = Some(duskweave::planning::WithdrawalBasis {
        operation_id: valid.operation_id,
        event_id: valid.event_id,
    });
    variants.push(changed);
    let mut changed = valid.clone();
    changed.owner_revision = 2;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.producer = "other".into();
    variants.push(changed);
    let mut changed = valid.clone();
    changed.kind = "other".into();
    variants.push(changed);
    let mut changed = valid.clone();
    changed.causation_id = OperationId(Uuid::from_u128(99));
    variants.push(changed);
    let mut changed = valid.clone();
    changed.recorded_at += 1;
    variants.push(changed);
    let mut changed = valid.clone();
    changed.decision = PlanningDecision::UnresolvedEvaluationIncomplete;
    variants.push(changed);
    for forged in variants {
        assert!(forged.validate().is_err(), "forgery: {forged:?}");
        assert!(!forged.recorded_eligible());
    }
    let round: PlanningAssessed =
        serde_json::from_value(serde_json::to_value(&valid).unwrap()).unwrap();
    assert_eq!(round, valid);
    assert!(round.recorded_eligible());
}

#[test]
fn legacy_variant_import_assigns_to_canonical_decision() {
    let imported: PlanningDecision = RefusedExpired;
    assert_eq!(imported, NonpositiveDecision::RefusedExpired);
    assert_eq!(serde_json::to_value(imported).unwrap(), "refused_expired");
}

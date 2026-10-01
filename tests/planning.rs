use duskweave::mission::{
    AssetRef, CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, OperationId,
};
use duskweave::planning::{MissionBasis, NonpositiveDecision, PlanningAssessed, PlanningRequest};
use duskweave::planning_assessment::{self, PlanningStore};
use duskweave::planning_input::parse_planning;
use duskweave::registration::OperationAllocator;
use duskweave::{Fail, Res};
use uuid::Uuid;

fn request() -> PlanningRequest {
    PlanningRequest {
        engagement_id: EngagementId(Uuid::from_u128(1)),
        campaign_id: CampaignId(Uuid::from_u128(2)),
        purpose_ref: GoalRef(Uuid::from_u128(3)),
        asset_ref: AssetRef(Uuid::from_u128(4)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}

fn basis() -> MissionBasis {
    MissionBasis {
        registration_operation_id: OperationId(Uuid::from_u128(9)),
        revision: 1,
        exercise_mode: ExerciseMode::Blind,
        starts_at: 10,
        ends_at: 20,
    }
}

struct NeverAllocate;

impl OperationAllocator for NeverAllocate {
    fn allocate(&mut self) -> Res<Uuid> {
        panic!("allocator must not be called")
    }
}

struct FixedStore {
    event: Option<PlanningAssessed>,
    called: bool,
}

impl PlanningStore for FixedStore {
    fn assess(
        &mut self,
        _request: &PlanningRequest,
        _operation: OperationId,
        _recover: bool,
        _allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<PlanningAssessed>> {
        self.called = true;
        Ok(self.event.clone())
    }
}

#[test]
fn absent_basis_is_nonpositive_and_strict_input_rejects_unknown_fields() {
    let request = request();
    let event = PlanningAssessed::new(
        request.clone(),
        None,
        OperationId(Uuid::from_u128(5)),
        EventId(Uuid::from_u128(6)),
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(event.decision, NonpositiveDecision::UnresolvedMissionBasis);
    assert_eq!(event.evaluated_at, event.occurred_at);
    assert_eq!(event.occurred_at, event.recorded_at);
    let mut raw = serde_json::to_value(&request).unwrap();
    raw["unexpected"] = serde_json::json!("sensitive-sentinel");
    assert!(parse_planning(&serde_json::to_vec(&raw).unwrap()).is_err());
}

#[test]
fn four_nonpositive_outcomes_follow_owner_precedence() {
    let mut request = request();
    assert_eq!(
        NonpositiveDecision::for_request(&request, None),
        NonpositiveDecision::UnresolvedMissionBasis
    );
    request.expected_mission_revision = 2;
    request.current_authority_confirmed = false;
    assert_eq!(
        NonpositiveDecision::for_request(&request, Some(&basis())),
        NonpositiveDecision::RefusedRevisionMismatch
    );
    request.expected_mission_revision = 1;
    assert_eq!(
        NonpositiveDecision::for_request(&request, Some(&basis())),
        NonpositiveDecision::UnresolvedAuthorityUnconfirmed
    );
    request.current_authority_confirmed = true;
    assert_eq!(
        NonpositiveDecision::for_request(&request, Some(&basis())),
        NonpositiveDecision::UnresolvedEvaluationIncomplete
    );
}

#[test]
fn application_rejects_valid_but_wrong_returned_intent() {
    let request = request();
    let operation = OperationId(Uuid::from_u128(5));
    let mut other_request = request.clone();
    other_request.asset_ref = AssetRef(Uuid::from_u128(99));
    let event = PlanningAssessed::new(
        other_request,
        Some(basis()),
        operation,
        EventId(Uuid::from_u128(6)),
        15,
    )
    .unwrap();
    let mut store = FixedStore {
        event: Some(event),
        called: false,
    };
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut store, &request, operation, true),
        Err(Fail::Conflict("integrity_conflict"))
    );
    let event = PlanningAssessed::new(
        request.clone(),
        Some(basis()),
        OperationId(Uuid::from_u128(7)),
        EventId(Uuid::from_u128(6)),
        15,
    )
    .unwrap();
    store.event = Some(event);
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut store, &request, operation, true),
        Err(Fail::Conflict("integrity_conflict"))
    );
}

#[test]
fn invalid_request_or_operation_never_reaches_store() {
    let mut request = request();
    let mut store = FixedStore {
        event: None,
        called: false,
    };
    request.expected_mission_revision = 0;
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut store,
            &request,
            OperationId(Uuid::from_u128(5)),
            false
        ),
        Err(Fail::Input("invalid_revision"))
    );
    assert!(!store.called);
    request.expected_mission_revision = 1;
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut store,
            &request,
            OperationId(Uuid::nil()),
            true
        ),
        Err(Fail::Input("nil_identity"))
    );
    assert!(!store.called);
}

#[test]
fn contract_rejects_corrupted_identity_basis_and_time() {
    let request = request();
    let event = PlanningAssessed::new(
        request,
        Some(basis()),
        OperationId(Uuid::from_u128(5)),
        EventId(Uuid::from_u128(6)),
        15,
    )
    .unwrap();
    let mut wrong = event.clone();
    wrong.correlation_id = OperationId(Uuid::from_u128(8));
    assert!(wrong.validate().is_err());
    wrong = event.clone();
    wrong.basis.as_mut().unwrap().starts_at = 20;
    assert_eq!(wrong.validate(), Err(Fail::Store("unsupported_basis")));
    wrong = event.clone();
    wrong.recorded_at += 1;
    assert!(wrong.validate().is_err());
    wrong = event.clone();
    wrong.kind = "other".into();
    assert!(wrong.validate().is_err());
    wrong = event;
    wrong.event_id = EventId(Uuid::nil());
    assert!(wrong.validate().is_err());
}

#[test]
fn bounded_strict_input_rejects_nil_revision_and_unrecognized_data() {
    let request = request();
    let mut raw = serde_json::to_value(request).unwrap();
    raw["engagement_id"] = serde_json::json!(Uuid::nil());
    assert!(parse_planning(&serde_json::to_vec(&raw).unwrap()).is_err());
    raw["engagement_id"] = serde_json::json!(Uuid::from_u128(1));
    raw["expected_mission_revision"] = serde_json::json!(i64::MAX as u64 + 1);
    assert!(parse_planning(&serde_json::to_vec(&raw).unwrap()).is_err());
    assert!(parse_planning(&vec![b' '; 16 * 1024 + 1]).is_err());
    assert!(parse_planning(b"{").is_err());
}

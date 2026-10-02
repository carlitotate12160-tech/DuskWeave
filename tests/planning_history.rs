use duskweave::mission::{AssetRef, CampaignId, EngagementId, EventId, GoalRef, OperationId};
use duskweave::planning::{PlanningAssessed, PlanningRequest};
use duskweave::planning_history::{PlanningHistoryPort, history_view};
use duskweave::trajectory::Delivered;
use duskweave::{Fail, Res};
use uuid::Uuid;

fn event() -> PlanningAssessed {
    PlanningAssessed::new(
        PlanningRequest {
            engagement_id: EngagementId(Uuid::from_u128(1)),
            campaign_id: CampaignId(Uuid::from_u128(2)),
            purpose_ref: GoalRef(Uuid::from_u128(3)),
            asset_ref: AssetRef(Uuid::from_u128(4)),
            expected_mission_revision: 1,
            current_authority_confirmed: true,
        },
        None,
        OperationId(Uuid::from_u128(5)),
        EventId(Uuid::from_u128(6)),
        7,
    )
    .unwrap()
}

struct Consumer(Res<Delivered>);

impl PlanningHistoryPort for Consumer {
    fn publish(&mut self, _: &PlanningAssessed) -> Res<Delivered> {
        self.0
    }
    fn inspect(&mut self, _: &PlanningAssessed) -> Res<Delivered> {
        self.0
    }
}

#[test]
fn unknown_acknowledgment_never_reports_completed_or_pending() {
    let view = history_view(
        &mut Consumer(Err(Fail::Store("commit_unknown"))),
        &event(),
        false,
    );
    assert_eq!(view.state, "unknown");
    assert_eq!(view.reason, "consumer_commit_unknown");
    assert!(!view.complete);
}

#[test]
fn statuses_are_explicit_and_only_matching_durable_history_is_complete() {
    for (outcome, state, complete) in [
        (Delivered::Completed, "completed", true),
        (Delivered::Duplicate, "completed", true),
        (Delivered::Anomaly, "anomaly", false),
        (
            Delivered::Unresolved("missing_predecessor"),
            "pending",
            false,
        ),
    ] {
        let view = history_view(&mut Consumer(Ok(outcome)), &event(), false);
        assert_eq!((view.state, view.complete), (state, complete));
    }
}

#[test]
fn read_only_recovery_never_calls_publication_and_invalid_event_never_calls_either() {
    struct ReadOnly;
    impl PlanningHistoryPort for ReadOnly {
        fn publish(&mut self, _: &PlanningAssessed) -> Res<Delivered> {
            panic!("recovery published");
        }
        fn inspect(&mut self, _: &PlanningAssessed) -> Res<Delivered> {
            Ok(Delivered::Completed)
        }
    }
    assert!(history_view(&mut ReadOnly, &event(), true).complete);
    let mut invalid = event();
    invalid.version = 99;
    let view = history_view(&mut ReadOnly, &invalid, false);
    assert_eq!((view.state, view.complete), ("unknown", false));
    for failure in [
        Fail::Store("serialization_retry"),
        Fail::Config("connect_failed"),
    ] {
        let view = history_view(&mut Consumer(Err(failure)), &event(), true);
        assert_eq!((view.state, view.complete), ("unknown", false));
    }
}

#[test]
fn producer_read_uses_recovery_and_has_no_allocation_capability() {
    use duskweave::planning_assessment::PlanningStore;
    use duskweave::planning_history::read_decision;
    use duskweave::registration::OperationAllocator;
    struct RecoverOnly;
    impl PlanningStore for RecoverOnly {
        fn assess(
            &mut self,
            _: &PlanningRequest,
            _: OperationId,
            recover: bool,
            allocator: &mut dyn OperationAllocator,
        ) -> Res<Option<PlanningAssessed>> {
            assert!(recover);
            assert_eq!(
                allocator.allocate(),
                Err(Fail::State("recovery_allocation_forbidden"))
            );
            Ok(Some(event()))
        }
    }
    let original = event();
    assert_eq!(
        read_decision(&mut RecoverOnly, &original.request, original.operation_id).unwrap(),
        Some(original)
    );
}

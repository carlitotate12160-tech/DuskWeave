//! Deterministic domain/application tests for M0A registration.
//! In-memory port fakes only; no database, sleeps or external targets.

use duskweave::input::{MAX_INPUT_BYTES, parse_register};
use duskweave::mission::*;
use duskweave::registration::{
    CommitEffect, MissionStore, MissionView, OperationAllocator, ReconcileOutcome, TrajectoryPort,
    inspect, prepare_operation, reconcile, register,
};
use duskweave::trajectory::{self, Delivered, HistoryStatus};
use duskweave::{Fail, Res};
use uuid::Uuid;

fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n + 1)
}

fn input() -> RegistrationInput {
    parse_register(&serde_json::to_vec(&dto_json()).unwrap()).unwrap()
}

fn alt_input() -> RegistrationInput {
    let mut j = dto_json();
    j["goal_ref"] = serde_json::json!(uuid(95));
    parse_register(&serde_json::to_vec(&j).unwrap()).unwrap()
}

fn dto_json() -> serde_json::Value {
    serde_json::json!({
        "engagement_id": uuid(1), "campaign_id": uuid(2), "operator_ref": uuid(10),
        "authority_ref": uuid(11), "authority_revision": 1, "goal_ref": uuid(12),
        "included_assets": [uuid(20), uuid(21)], "excluded_assets": [uuid(30)],
        "exercise_mode": "blind", "starts_at": 100, "ends_at": 200
    })
}

struct FakeAlloc {
    allocated: Vec<Uuid>,
}
impl OperationAllocator for FakeAlloc {
    fn allocate(&mut self) -> Res<Uuid> {
        let id = uuid(77 + self.allocated.len() as u128);
        self.allocated.push(id);
        Ok(id)
    }
}

struct FakeStore {
    missions: Vec<MissionRegistered>,
}

impl MissionStore for FakeStore {
    fn commit_registration(&mut self, m: &Mission, ev: &MissionRegistered) -> Res<CommitEffect> {
        if let Some(old) = self
            .missions
            .iter()
            .find(|e2| e2.operation_id == m.operation_id())
        {
            return if old.fields == ev.fields {
                Ok(CommitEffect::Existing(Box::new(old.clone())))
            } else {
                Err(Fail::Conflict("integrity_conflict"))
            };
        }
        self.missions.push(ev.clone());
        Ok(CommitEffect::Fresh)
    }
    fn mission_view(&mut self, e: EngagementId, c: CampaignId) -> Res<Option<MissionView>> {
        Ok(self
            .missions
            .iter()
            .find(|ev| ev.engagement_id == e && ev.campaign_id == c)
            .map(|ev| MissionView {
                operation_id: ev.operation_id,
                revision: ev.owner_revision,
                exercise_mode: ev.fields.exercise_mode(),
                starts_at: ev.fields.starts_at(),
                ends_at: ev.fields.ends_at(),
            }))
    }
    fn outbox_event(
        &mut self,
        e: EngagementId,
        c: CampaignId,
        op: OperationId,
    ) -> Res<Option<MissionRegistered>> {
        Ok(self
            .missions
            .iter()
            .find(|ev| ev.engagement_id == e && ev.campaign_id == c && ev.operation_id == op)
            .cloned())
    }
}

struct FakeTraj {
    deliveries: usize,
    outcome: Delivered,
    status: HistoryStatus,
    last_delivered: Option<EventId>,
    deliver_error: Option<Fail>,
    status_error: Option<Fail>,
}
impl TrajectoryPort for FakeTraj {
    fn deliver(&mut self, ev: &MissionRegistered) -> Res<Delivered> {
        self.deliveries += 1;
        self.last_delivered = Some(ev.event_id);
        if let Some(e) = &self.deliver_error {
            return Err(*e);
        }
        Ok(self.outcome)
    }
    fn status(&mut self, _: EngagementId, _: CampaignId, _: EventId) -> Res<HistoryStatus> {
        if let Some(e) = &self.status_error {
            return Err(*e);
        }
        Ok(self.status)
    }
}

fn fixture() -> (FakeAlloc, FakeStore, FakeTraj) {
    (
        FakeAlloc { allocated: vec![] },
        FakeStore { missions: vec![] },
        FakeTraj {
            deliveries: 0,
            outcome: Delivered::Completed,
            status: HistoryStatus::Completed,
            last_delivered: None,
            deliver_error: None,
            status_error: None,
        },
    )
}

#[test]
fn parse_valid_dto_canonicalizes() {
    let mut j = dto_json();
    j["included_assets"] = serde_json::json!([uuid(21), uuid(20), uuid(21)]);
    let got = parse_register(&serde_json::to_vec(&j).unwrap()).unwrap();
    assert_eq!(
        got.fields.included_assets(),
        [AssetRef(uuid(20)), AssetRef(uuid(21))]
    );
}

#[test]
fn parse_rejects_bad_input_without_content() {
    let j = dto_json();
    let mut bad = vec![serde_json::json!({})];
    for (key, val) in [
        ("operator_ref", serde_json::json!(Uuid::nil())),
        ("authority_revision", serde_json::json!(0)),
        ("included_assets", serde_json::json!([])),
        ("included_assets", serde_json::json!(vec![uuid(1); 65])),
        ("starts_at", serde_json::json!(200)),
        ("exercise_mode", serde_json::json!("stealth")),
        ("secret", serde_json::json!("S3NTINEL-9f3a")),
    ] {
        let mut c = j.clone();
        c[key] = val;
        bad.push(c);
    }
    for case in &bad {
        assert!(parse_register(&serde_json::to_vec(case).unwrap()).is_err());
    }
    assert_eq!(
        parse_register(b"not-json{"),
        Err(Fail::Input("malformed_json"))
    );
    assert_eq!(
        parse_register(&vec![b' '; MAX_INPUT_BYTES + 1]),
        Err(Fail::Input("size_limit"))
    );
    let err = parse_register(&serde_json::to_vec(&bad[7]).unwrap()).unwrap_err();
    assert!(
        !format!("{err:?}").contains("S3NTINEL"),
        "rejected content leaked: {err:?}"
    );
}

#[test]
fn register_builds_bounded_contract() {
    let (m, ev) =
        Mission::register(&input(), OperationId(uuid(50)), EventId(uuid(60)), 99).unwrap();
    assert_eq!(m.revision(), 1);
    assert_eq!(ev.producer, PRODUCER);
    assert_eq!(ev.kind, CONTRACT_KIND);
    assert_eq!(ev.version, CONTRACT_VERSION);
    assert_eq!(ev.owner_revision, 1);
    assert_eq!(ev.causation_id, OperationId(uuid(50)));
    assert_eq!(ev.affected_entity, ev.campaign_id);
    assert!(trajectory::check_event(&ev).is_ok());
}

#[test]
fn contract_checks_reject_unsupported() {
    type Mutation = (fn(&mut MissionRegistered), &'static str);
    let (_, base) =
        Mission::register(&input(), OperationId(uuid(50)), EventId(uuid(60)), 9).unwrap();
    let cases: Vec<Mutation> = vec![
        (|e| e.producer = "rogue".into(), "unsupported_contract"),
        (|e| e.version = 99, "unsupported_contract"),
        (|e| e.owner_revision = 7, "unsupported_contract"),
        (
            |e| e.causation_id = OperationId(uuid(90)),
            "scope_violation",
        ),
    ];
    for (f, cat) in cases {
        let mut ev = base.clone();
        f(&mut ev);
        assert_eq!(trajectory::check_event(&ev), Err(cat));
    }
}

#[test]
fn register_usecase_accepted_and_idempotent() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    let r1 = register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    assert_eq!(r1.history, HistoryStatus::Completed);
    let r2 = register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    assert_eq!(
        r1.event_id, r2.event_id,
        "redelivery must reuse committed event id"
    );
    assert_eq!(store.missions.len(), 1);
    assert_eq!(traj.deliveries, 2);
    // The second call allocated a fresh event id, yet the stored original
    // is what was committed and delivered.
    assert_eq!(alloc.allocated.len(), 3);
    assert_ne!(alloc.allocated[2], r1.event_id.0);
    assert_eq!(traj.last_delivered, Some(r1.event_id));
}

#[test]
fn register_conflicting_operation_is_conflict() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    let alt = alt_input();
    assert_eq!(
        register(&mut alloc, &mut store, &mut traj, op, &alt),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(traj.deliveries, 1, "conflict must not be republished");
}

#[test]
fn register_pending_when_delivery_fails() {
    let (mut alloc, mut store, mut traj) = fixture();
    traj.outcome = Delivered::Unresolved("unsupported_contract");
    let op = prepare_operation(&mut alloc).unwrap();
    let r = register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    assert_eq!(r.history, HistoryStatus::Pending);
}

#[test]
fn inspect_labels_views_and_wrong_scope_empty() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    let v = inspect(
        &mut store,
        &mut traj,
        EngagementId(uuid(1)),
        CampaignId(uuid(2)),
    )
    .unwrap()
    .unwrap();
    assert_eq!(v.mission.revision, 1);
    assert_eq!(v.history, HistoryStatus::Completed);
    assert!(v.event_id.is_some());
    assert!(
        inspect(
            &mut store,
            &mut traj,
            EngagementId(uuid(9)),
            CampaignId(uuid(9))
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn reconcile_not_committed_pending_completes_once() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    let (e, c) = (EngagementId(uuid(1)), CampaignId(uuid(2)));
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::NotCommitted)
    );
    register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    traj.status = HistoryStatus::Pending;
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
}

#[test]
fn reconcile_surfaces_anomaly() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    traj.status = HistoryStatus::Anomaly;
    let (e, c) = (EngagementId(uuid(1)), CampaignId(uuid(2)));
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Conflicted)
    );
}

#[test]
fn register_rejects_nil_identities() {
    // Owner/API boundary negative controls: each of the four identities
    // must be non-nil even when construction bypasses the CLI parser.
    let (op, ev) = (OperationId(uuid(50)), EventId(uuid(60)));
    let mut i = input();
    i.engagement_id = EngagementId(Uuid::nil());
    assert!(matches!(
        Mission::register(&i, op, ev, 9),
        Err(Fail::Input(_))
    ));
    let mut i = input();
    i.campaign_id = CampaignId(Uuid::nil());
    assert!(matches!(
        Mission::register(&i, op, ev, 9),
        Err(Fail::Input(_))
    ));
    assert!(matches!(
        Mission::register(&input(), OperationId(Uuid::nil()), ev, 9),
        Err(Fail::Input(_))
    ));
    assert!(matches!(
        Mission::register(&input(), op, EventId(Uuid::nil()), 9),
        Err(Fail::Input(_))
    ));
}

#[test]
fn register_various_delivery_outcomes() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();

    // Duplicate -> Completed
    traj.outcome = Delivered::Duplicate;
    let r = register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    assert_eq!(r.history, HistoryStatus::Completed);

    // Anomaly -> Anomaly
    traj.outcome = Delivered::Anomaly;
    let r = register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    assert_eq!(r.history, HistoryStatus::Anomaly);

    // Err -> Pending
    traj.deliver_error = Some(Fail::State("network_down"));
    let r = register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();
    assert_eq!(r.history, HistoryStatus::Pending);
}

#[test]
fn register_producer_error_prevents_delivery() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();

    let ev = register(&mut alloc, &mut store, &mut traj, op, &input())
        .unwrap()
        .event_id;

    // Now if we try to register with same op but alt input, store errors:
    let r = register(&mut alloc, &mut store, &mut traj, op, &alt_input());
    assert!(r.is_err());
    assert_eq!(traj.deliveries, 1, "producer error must prevent delivery");
    assert_eq!(traj.last_delivered, Some(ev));
}

#[test]
fn reconcile_various_pending_outcomes() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    let ev = register(&mut alloc, &mut store, &mut traj, op, &input())
        .unwrap()
        .event_id;

    let (e, c) = (EngagementId(uuid(1)), CampaignId(uuid(2)));

    traj.status = HistoryStatus::Pending;

    // Pending -> Duplicate -> Committed
    traj.outcome = Delivered::Duplicate;
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    assert_eq!(traj.deliveries, 2);
    assert_eq!(traj.last_delivered, Some(ev));

    // Pending -> Anomaly -> Conflicted
    traj.outcome = Delivered::Anomaly;
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Conflicted)
    );
    assert_eq!(traj.deliveries, 3);
    assert_eq!(traj.last_delivered, Some(ev));

    // Pending -> Unresolved -> Err
    traj.outcome = Delivered::Unresolved("timeout");
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Err(Fail::Unresolved("timeout"))
    );
    assert_eq!(traj.deliveries, 4);
    assert_eq!(traj.last_delivered, Some(ev));

    // Pending -> Delivery error -> Err
    traj.deliver_error = Some(Fail::State("network_error"));
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Err(Fail::State("network_error"))
    );
    assert_eq!(traj.deliveries, 5);
    assert_eq!(traj.last_delivered, Some(ev));
}

#[test]
fn reconcile_status_read_error_propagation() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();

    let (e, c) = (EngagementId(uuid(1)), CampaignId(uuid(2)));

    traj.status_error = Some(Fail::State("db_offline"));
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Err(Fail::State("db_offline"))
    );
    assert_eq!(traj.deliveries, 1, "status read error must not deliver");
}

#[test]
fn reconcile_completed_performs_no_delivery() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();
    register(&mut alloc, &mut store, &mut traj, op, &input()).unwrap();

    let (e, c) = (EngagementId(uuid(1)), CampaignId(uuid(2)));

    let pre = traj.deliveries;
    traj.status = HistoryStatus::Completed;
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    assert_eq!(traj.deliveries, pre, "Completed must not deliver");

    traj.status = HistoryStatus::Anomaly;
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::Conflicted)
    );
    assert_eq!(traj.deliveries, pre, "Anomaly must not deliver");
}

#[test]
fn reconcile_absent_outbox_performs_no_delivery() {
    let (mut alloc, mut store, mut traj) = fixture();
    let op = prepare_operation(&mut alloc).unwrap();

    let (e, c) = (EngagementId(uuid(1)), CampaignId(uuid(2)));

    let pre = traj.deliveries;
    assert_eq!(
        reconcile(&mut store, &mut traj, e, c, op),
        Ok(ReconcileOutcome::NotCommitted)
    );
    assert_eq!(traj.deliveries, pre, "absent outbox must not deliver");
    assert!(traj.last_delivered.is_none());
}

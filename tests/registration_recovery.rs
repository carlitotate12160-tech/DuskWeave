//! Recovery semantics against real PostgreSQL: producer/consumer boundary
//! faults are injected at the port boundary (deterministic, no sleeps, no
//! network sabotage); fresh connections reconcile the durable records.

use duskweave::mission::*;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::{self, CommitEffect, MissionStore, ReconcileOutcome, TrajectoryPort};
use duskweave::trajectory::{Delivered, HistoryStatus};
use duskweave::{Fail, Res};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

/// Deterministic consumer-boundary fault: fails the first `deliver` call
/// with a storage error, then delegates to the real PostgreSQL adapter.
struct FaultOnceTraj {
    inner: PgTrajectory,
    armed: bool,
}

impl TrajectoryPort for FaultOnceTraj {
    fn deliver(&mut self, ev: &MissionRegistered) -> Res<Delivered> {
        if self.armed {
            self.armed = false;
            return Err(Fail::Store("commit_unknown"));
        }
        self.inner.deliver(ev)
    }
    fn status(&mut self, e: EngagementId, c: CampaignId, ev: EventId) -> Res<HistoryStatus> {
        self.inner.status(e, c, ev)
    }
}

/// Deterministic producer-boundary fault: the first commit attempt reports
/// an unknown outcome (lost acknowledgment) without delegating.
struct FaultOnceStore<S> {
    inner: S,
    armed: bool,
}

impl<S: MissionStore> MissionStore for FaultOnceStore<S> {
    fn commit_registration(&mut self, m: &Mission, ev: &MissionRegistered) -> Res<CommitEffect> {
        if self.armed {
            self.armed = false;
            return Err(Fail::Store("commit_unknown"));
        }
        self.inner.commit_registration(m, ev)
    }
    fn mission_view(
        &mut self,
        e: EngagementId,
        c: CampaignId,
    ) -> Res<Option<registration::MissionView>> {
        self.inner.mission_view(e, c)
    }
    fn outbox_event(
        &mut self,
        e: EngagementId,
        c: CampaignId,
        op: OperationId,
    ) -> Res<Option<MissionRegistered>> {
        self.inner.outbox_event(e, c, op)
    }
}

#[test]
fn consumer_fault_after_commit_reconciles_with_one_effect() {
    let _g = db();
    let (e, c) = scope(0x5100);
    let (mut alloc, mut store, _) = ports();
    let mut traj = FaultOnceTraj {
        inner: PgTrajectory::new(runtime_client()),
        armed: true,
    };
    let op = registration::prepare_operation(&mut alloc).unwrap();
    // Producer commits; the single delivery attempt fails at the boundary.
    let r =
        registration::register(&mut alloc, &mut store, &mut traj, op, &reg_input(e, c)).unwrap();
    assert_eq!(
        r.history,
        HistoryStatus::Pending,
        "lost delivery must be pending"
    );
    assert_eq!(count("trajectory.registration_history", e, c), 0);
    // Fresh connections reconcile the durable event to completion.
    let (mut _a, mut s2, mut t2) = ports();
    assert_eq!(
        registration::inspect(&mut s2, &mut t2, e, c)
            .unwrap()
            .unwrap()
            .history,
        HistoryStatus::Pending
    );
    assert_eq!(
        registration::reconcile(&mut s2, &mut t2, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    // Redelivery is deduplicated: exactly one history effect.
    let ev = store.outbox_event(e, c, op).unwrap().unwrap();
    assert_eq!(t2.deliver(&ev).unwrap(), Delivered::Duplicate);
    assert_eq!(count("trajectory.registration_history", e, c), 1);
}

#[test]
fn producer_unknown_ack_then_verified_absence_retry() {
    let _g = db();
    let (e, c) = scope(0x5200);
    let (mut alloc, store, mut traj) = ports();
    let mut store = FaultOnceStore {
        inner: store,
        armed: true,
    };
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let input = reg_input(e, c);
    let got = registration::register(&mut alloc, &mut store, &mut traj, op, &input);
    assert_eq!(got, Err(Fail::Store("commit_unknown")));
    // Verified absence on fresh connections permits unchanged-intent retry.
    let (mut a2, mut s2, mut t2) = ports();
    assert_eq!(
        registration::reconcile(&mut s2, &mut t2, e, c, op),
        Ok(ReconcileOutcome::NotCommitted)
    );
    let r = registration::register(&mut a2, &mut s2, &mut t2, op, &input).unwrap();
    assert_eq!(r.operation_id, op);
    assert_eq!(count("mission.missions", e, c), 1);
    // A never-committed operation reconciles as NotCommitted.
    let never = OperationId(Uuid::from_u128(0xe099));
    assert_eq!(
        registration::reconcile(&mut s2, &mut t2, e, c, never),
        Ok(ReconcileOutcome::NotCommitted)
    );
}

#[test]
fn real_producer_commit_then_interrupted_delivery() {
    let _g = db();
    let (e, c) = scope(0x5300);
    let (mut _a, mut store, mut _traj) = ports();
    let op = OperationId(Uuid::from_u128(0xe101));
    let input = reg_input(e, c);
    let (mission, ev) =
        Mission::register(&input, op, EventId(Uuid::from_u128(0xe001)), 1_700_000_000).unwrap();
    store.commit_registration(&mission, &ev).unwrap();
    // Real committed state; consumer never ran: pending, recoverable.
    let (mut _a2, mut s2, mut t2) = ports();
    assert_eq!(
        registration::inspect(&mut s2, &mut t2, e, c)
            .unwrap()
            .unwrap()
            .history,
        HistoryStatus::Pending
    );
    assert_eq!(
        registration::reconcile(&mut s2, &mut t2, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    // Original-ID redelivery by the real consumer is a single effect.
    assert_eq!(t2.deliver(&ev).unwrap(), Delivered::Duplicate);
    assert_eq!(count("trajectory.registration_history", e, c), 1);
    // Retry with the same operation reuses the committed event identity.
    let (mut a3, mut s3, mut t3) = ports();
    let r = registration::register(&mut a3, &mut s3, &mut t3, op, &input).unwrap();
    assert_eq!(r.event_id, ev.event_id);
    assert_eq!(count("mission.missions", e, c), 1);
}

#[test]
fn producer_rollback_and_conflicting_commit_leave_nothing() {
    let _g = db();
    let (e, c) = scope(0x5400);
    // Explicit real rollback of the producer statements.
    let mut tx_client = runtime_client();
    let mut tx = tx_client.transaction().unwrap();
    tx.execute(
        "INSERT INTO mission.missions \
         (engagement_id, campaign_id, operator_ref, authority_ref, authority_revision, \
          goal_ref, included_assets, excluded_assets, exercise_mode, starts_at, ends_at, \
          revision, operation_id) \
         VALUES ($1,$2,$3,$4,1,$5,'[]','[]','blind',1,2,1,$6)",
        &[
            &e.0,
            &c.0,
            &Uuid::from_u128(0x11),
            &Uuid::from_u128(0x12),
            &Uuid::from_u128(0x13),
            &Uuid::from_u128(0xe201),
        ],
    )
    .unwrap();
    tx.rollback().unwrap();
    assert_eq!(count("mission.missions", e, c), 0);
    assert_eq!(count("mission.registration_outbox", e, c), 0);
    // Failed commit attempt (occupied scope via mission PK) leaves no
    // partial outbox row either.
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let op2 = registration::prepare_operation(&mut alloc).unwrap();
    assert!(reg(&mut alloc, &mut store, &mut traj, op2, e, c).is_err());
    assert_eq!(count("mission.registration_outbox", e, c), 1);
    assert_eq!(count("mission.missions", e, c), 1);
}

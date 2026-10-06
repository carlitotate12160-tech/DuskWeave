//! Lost-commit-acknowledgment coverage: the ACK is dropped only AFTER a
//! real durable commit, proving recovery reconciles committed state. The
//! decorators are test-only port wrappers over the real PG17 adapters;
//! no network sabotage, sleeps or fake storage.

use duskweave::mission::*;
use duskweave::registration::{self, CommitEffect, MissionStore, ReconcileOutcome, TrajectoryPort};
use duskweave::trajectory::{Delivered, HistoryStatus};
use duskweave::{Fail, Res};

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_permission.rs"]
mod policy;

/// Producer-boundary fault: calls the real commit first; only after it
/// succeeds does it discard the result once and report an unknown ack.
/// Inner failures pass through — no commit is assumed on failure.
struct FaultAfterCommitStore<S> {
    inner: S,
    armed: bool,
}

impl<S: MissionStore> MissionStore for FaultAfterCommitStore<S> {
    fn commit_registration(&mut self, m: &Mission, ev: &MissionRegistered) -> Res<CommitEffect> {
        let got = self.inner.commit_registration(m, ev)?;
        if self.armed {
            self.armed = false;
            return Err(Fail::Store("commit_unknown"));
        }
        Ok(got)
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

/// Consumer-boundary fault: calls the real delivery first; only after a
/// real Completed result does it discard that ack once as unknown.
struct FaultAfterCommitTrajectory<P> {
    inner: P,
    armed: bool,
}

impl<P: TrajectoryPort> TrajectoryPort for FaultAfterCommitTrajectory<P> {
    fn deliver(&mut self, ev: &MissionRegistered) -> Res<Delivered> {
        let got = self.inner.deliver(ev)?;
        if self.armed && got == Delivered::Completed {
            self.armed = false;
            return Err(Fail::Store("commit_unknown"));
        }
        Ok(got)
    }
    fn status(&mut self, e: EngagementId, c: CampaignId, ev: EventId) -> Res<HistoryStatus> {
        self.inner.status(e, c, ev)
    }
}

#[test]
fn producer_ack_lost_after_real_commit_recovers_original_event() {
    let _g = db();
    let (e, c) = scope(0x6100);
    let (mut alloc, store, mut traj) = ports();
    let mut store = FaultAfterCommitStore {
        inner: store,
        armed: true,
    };
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let input = reg_input(e, c);
    // Caller sees an unknown outcome even though the commit was durable.
    assert_eq!(
        registration::register(&mut alloc, &mut store, &mut traj, op, &input),
        Err(Fail::Store("commit_unknown"))
    );
    // Proof of a real committed state before any recovery.
    assert_eq!(count("mission.missions", e, c), 1);
    assert_eq!(count("mission.registration_outbox", e, c), 1);
    assert_eq!(count("trajectory.registration_history", e, c), 0);
    let original = store.outbox_event(e, c, op).unwrap().unwrap().event_id;
    // Fresh connections reconcile the durable commit to completion.
    let (mut a2, mut s2, mut t2) = ports();
    assert_eq!(
        registration::reconcile(&mut s2, &mut t2, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    assert_eq!(count("trajectory.registration_history", e, c), 1);
    // Retry with unchanged intent reuses the original event identity.
    let r = registration::register(&mut a2, &mut s2, &mut t2, op, &input).unwrap();
    assert_eq!(r.event_id, original);
    let ev = store.outbox_event(e, c, op).unwrap().unwrap();
    assert_eq!(t2.deliver(&ev).unwrap(), Delivered::Duplicate);
    assert_eq!(count("mission.missions", e, c), 1);
    assert_eq!(count("mission.registration_outbox", e, c), 1);
    assert_eq!(count("trajectory.registration_history", e, c), 1);
}

#[test]
fn consumer_ack_lost_after_real_commit_stays_pending_until_recovered() {
    let _g = db();
    let (e, c) = scope(0x6200);
    let (mut alloc, mut store, traj) = ports();
    let mut traj = FaultAfterCommitTrajectory {
        inner: traj,
        armed: true,
    };
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let input = reg_input(e, c);
    // History already durable, but the dropped ack yields a pending receipt.
    let r = registration::register(&mut alloc, &mut store, &mut traj, op, &input).unwrap();
    assert_eq!(r.history, HistoryStatus::Pending);
    assert_eq!(count("trajectory.registration_history", e, c), 1);
    // Fresh connections see the committed history; reconcile is Committed.
    let (mut _a2, mut s2, mut t2) = ports();
    assert_eq!(
        registration::reconcile(&mut s2, &mut t2, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    let ev = store.outbox_event(e, c, op).unwrap().unwrap();
    assert_eq!(ev.event_id, r.event_id);
    assert_eq!(t2.deliver(&ev).unwrap(), Delivered::Duplicate);
    assert_eq!(count("mission.missions", e, c), 1);
    assert_eq!(count("mission.registration_outbox", e, c), 1);
    assert_eq!(count("trajectory.registration_history", e, c), 1);
}

#[test]
fn m1_producer_ack_loss_recovers_same_durable_policy_and_identity() {
    let _g = db();
    let (e, c) = scope(0x6300);
    let (mut a, s, mut t) = ports();
    let mut s = FaultAfterCommitStore {
        inner: s,
        armed: true,
    };
    let op = registration::prepare_operation(&mut a).unwrap();
    let input = policy::input(e, c);
    assert_eq!(
        registration::register(&mut a, &mut s, &mut t, op, &input),
        Err(Fail::Store("commit_unknown"))
    );
    assert_eq!(count("mission.missions", e, c), 1);
    assert_eq!(count("mission.registration_outbox", e, c), 1);
    assert_eq!(count("trajectory.registration_history", e, c), 0);
    let original = s.outbox_event(e, c, op).unwrap().unwrap();
    assert_eq!(original.version, 2);
    assert_eq!(
        serde_json::to_value(&original).unwrap()["fields"]["m1_permission"],
        policy::permission()
    );
    drop(s);
    drop(t);
    let (mut a, mut s, mut t) = ports();
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    let retry = registration::register(&mut a, &mut s, &mut t, op, &input).unwrap();
    assert_eq!(retry.event_id, original.event_id);
    assert_eq!(s.outbox_event(e, c, op).unwrap().unwrap(), original);
    let history: serde_json::Value = runtime_client().query_one(
        "SELECT contract FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0],
    ).unwrap().get(0);
    assert_eq!(history, serde_json::to_value(original).unwrap());
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(count(table, e, c), 1);
    }
}

#[test]
fn m1_consumer_ack_loss_keeps_committed_history_before_fresh_recovery() {
    let _g = db();
    let (e, c) = scope(0x6400);
    let (mut a, mut s, t) = ports();
    let mut t = FaultAfterCommitTrajectory {
        inner: t,
        armed: true,
    };
    let op = registration::prepare_operation(&mut a).unwrap();
    let input = policy::input(e, c);
    let receipt = registration::register(&mut a, &mut s, &mut t, op, &input).unwrap();
    assert_eq!(receipt.history, HistoryStatus::Pending);
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(count(table, e, c), 1);
    }
    let original = s.outbox_event(e, c, op).unwrap().unwrap();
    let durable: serde_json::Value = runtime_client().query_one(
        "SELECT contract FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0],
    ).unwrap().get(0);
    assert_eq!(durable, serde_json::to_value(&original).unwrap());
    assert_eq!(durable["fields"]["m1_permission"], policy::permission());
    drop(s);
    drop(t);
    let (_, mut s, mut t) = ports();
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    assert_eq!(s.outbox_event(e, c, op).unwrap().unwrap(), original);
    assert_eq!(t.deliver(&original).unwrap(), Delivered::Duplicate);
    assert_eq!(receipt.event_id, original.event_id);
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(count(table, e, c), 1);
    }
}

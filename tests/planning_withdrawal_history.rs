//! Version-3 consumer boundary over real PostgreSQL: predecessor qualification,
//! injected ACK loss recovery, and corrupted rows that must never complete.

use duskweave::mission::*;
use duskweave::planning::*;
use duskweave::planning_assessment::PlanningStore;
use duskweave::planning_history::{PlanningHistoryPort, history_view, read_decision};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::trajectory::Delivered;
use duskweave::withdrawal::*;
use duskweave::{Fail, Res};
use postgres::{Client, error::SqlState};
use serde_json::Value;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use {db_support::*, history_support::admin};

struct V3 {
    traj: PgTrajectory,
    e: EngagementId,
    c: CampaignId,
    withdrawn: MissionAuthorityWithdrawn,
    refused: PlanningAssessed,
}

/// Fresh scope with accepted registration history, a committed withdrawal
/// marker and one durable v3 refusal (still unpublished to history).
fn v3(slot: u128) -> V3 {
    let (e, c) = scope(slot);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let w_op = registration::prepare_operation(&mut alloc).unwrap();
    let request = WithdrawalRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(99)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    };
    let withdrawn = store
        .withdraw(&request, w_op, false, &mut alloc)
        .unwrap()
        .unwrap();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let request = PlanningRequest {
        engagement_id: e,
        campaign_id: c,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: false,
    };
    let refused = store
        .assess(&request, op, false, &mut alloc)
        .unwrap()
        .unwrap();
    assert_eq!(
        (refused.version, refused.owner_revision, refused.decision),
        (3, 2, NonpositiveDecision::RefusedAuthorityWithdrawn)
    );
    assert_eq!(
        refused.withdrawal,
        Some(WithdrawalBasis {
            operation_id: w_op,
            event_id: withdrawn.event_id,
        })
    );
    refused.validate().unwrap();
    V3 {
        traj,
        e,
        c,
        withdrawn,
        refused,
    }
}

/// Admin-owned fixture row in trajectory.withdrawal_history. `Some` payload
/// writes an accepted+completed row; `None` writes a valid anomaly marker.
fn insert_wh(a: &mut Client, v: &V3, event: Uuid, op: Uuid, contract: Option<&Value>) {
    let done = a.execute(
        "INSERT INTO trajectory.withdrawal_history \
         (engagement_id,campaign_id,operation_id,event_id,producer,kind,version,obligation,status,contract,anomaly_category,completed_at) \
         VALUES ($1,$2,$3,$4,'mission','mission_authority_withdrawn',1,'trajectory.withdrawal_history.v1',$5,$6,$7,$8)",
        &[
            &v.e.0,
            &v.c.0,
            &op,
            &event,
            &if contract.is_some() { "accepted" } else { "anomaly" },
            &contract,
            &contract.is_none().then_some("conflicting_identity"),
            &contract.map(|_| std::time::SystemTime::now()),
        ],
    );
    assert_eq!(done.unwrap(), 1);
}

fn wh_rows(v: &V3, status: &str) -> i64 {
    count_where(
        "trajectory.withdrawal_history",
        &format!("AND status='{status}'"),
        v.e,
        v.c,
    )
}

fn publish(t: &mut PgTrajectory, ev: &PlanningAssessed) -> Delivered {
    PlanningHistoryPort::publish(t, ev).unwrap()
}

/// Persist an accepted predecessor row carrying the referenced identities.
fn accept_wh(v: &V3, payload: &Value) {
    insert_wh(
        &mut admin(),
        v,
        v.withdrawn.event_id.0,
        v.withdrawn.operation_id.0,
        Some(payload),
    );
    assert_eq!(wh_rows(v, "accepted"), 1);
}

/// Every rejecting case must leave zero planning-history effects.
fn unsupported(mut v: V3) {
    assert_eq!(
        publish(&mut v.traj, &v.refused),
        Delivered::Unresolved("unsupported_predecessor")
    );
    assert_eq!(count("trajectory.planning_history", v.e, v.c), 0);
}

/// Same withdrawal relocated to a different scope, internally coherent.
fn rescoped(w: &MissionAuthorityWithdrawn) -> Value {
    let (e, c) = scope(0xd14);
    let mut w = w.clone();
    w.request.engagement_id = e;
    w.request.campaign_id = c;
    w.affected_entity = c;
    serde_json::to_value(&w).unwrap()
}

/// Same withdrawal rebound to a foreign registration operation.
fn foreign_registration(w: &MissionAuthorityWithdrawn) -> Value {
    let mut w = w.clone();
    w.registration_operation_id = OperationId(Uuid::from_u128(0xdeed));
    serde_json::to_value(&w).unwrap()
}

/// Same withdrawal rebound to distinct event/operation identities.
fn shifted(w: &MissionAuthorityWithdrawn, event: Uuid, op: Uuid) -> Value {
    let mut w = w.clone();
    w.event_id = EventId(event);
    w.operation_id = OperationId(op);
    w.causation_id = OperationId(op);
    w.correlation_id = OperationId(op);
    serde_json::to_value(&w).unwrap()
}

/// Axis override: `None` retains the referenced identity on that axis.
fn axis(v: &V3, foreign_event: Option<u128>, foreign_op: Option<u128>) -> (Uuid, Uuid) {
    (
        foreign_event.map_or(v.withdrawn.event_id.0, Uuid::from_u128),
        foreign_op.map_or(v.withdrawn.operation_id.0, Uuid::from_u128),
    )
}

struct LostAck(PgTrajectory);

impl PlanningHistoryPort for LostAck {
    fn publish(&mut self, event: &PlanningAssessed) -> Res<Delivered> {
        assert_eq!(
            PlanningHistoryPort::publish(&mut self.0, event)?,
            Delivered::Completed
        );
        // Independent restricted observer: the accepted consumer row is fully
        // durable before the caller sees the injected commit failure.
        let mut observer = runtime_client();
        let (e, c) = (event.engagement_id, event.campaign_id);
        let row = observer
            .query_one(
                "SELECT operation_id,event_id,contract,obligation,completed_at IS NOT NULL \
                 FROM trajectory.planning_history \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
                &[&e.0, &c.0],
            )
            .unwrap();
        assert_eq!(row.get::<_, Uuid>(0), event.operation_id.0);
        assert_eq!(row.get::<_, Uuid>(1), event.event_id.0);
        assert_eq!(row.get::<_, Value>(2), serde_json::to_value(event).unwrap());
        assert_eq!(row.get::<_, String>(3), "trajectory.planning_history.v1");
        assert!(row.get::<_, bool>(4));
        // Exactly one accepted effect and no conflicting anomaly.
        let counts = observer
            .query_one(
                "SELECT count(*) FILTER (WHERE status='accepted'), \
                 count(*) FILTER (WHERE status='anomaly') \
                 FROM trajectory.planning_history \
                 WHERE engagement_id=$1 AND campaign_id=$2",
                &[&e.0, &c.0],
            )
            .unwrap();
        assert_eq!((counts.get::<_, i64>(0), counts.get::<_, i64>(1)), (1, 0));
        // Producer row keeps its own durable ids/payload/obligation.
        let produced = observer
            .query_one(
                "SELECT event_id,contract,publication_obligation FROM mission.planning_assessments \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
                &[&e.0, &c.0, &event.operation_id.0],
            )
            .unwrap();
        assert_eq!(produced.get::<_, Uuid>(0), event.event_id.0);
        assert_eq!(
            produced.get::<_, Value>(1),
            serde_json::to_value(event).unwrap()
        );
        assert_eq!(
            produced.get::<_, String>(2),
            "trajectory.planning_history.v1"
        );
        Err(Fail::Store("commit_unknown"))
    }

    fn inspect(&mut self, _: &PlanningAssessed) -> Res<Delivered> {
        panic!("interrupted consumer must not inspect or retry")
    }
}

#[test]
fn v3_commit_then_ack_loss_recovers_durable_refusal_and_completion() {
    let _guard = db();
    let mut v = v3(0xd10);
    assert_eq!(
        WithdrawalHistoryPort::publish(&mut v.traj, &v.withdrawn).unwrap(),
        Delivered::Completed
    );
    let mut interrupted = LostAck(PgTrajectory::new(runtime_client()));
    let failed = history_view(&mut interrupted, &v.refused, false);
    assert_eq!(
        (failed.state, failed.reason, failed.complete),
        ("unknown", "consumer_commit_unknown", false)
    );
    drop(interrupted);
    // Fresh restricted producer: identity-scoped read-only recovery returns the
    // exact durable v3 refusal without allocation or current Mission state.
    let recovered = read_decision(
        &mut PgMissionStore::new(runtime_client()),
        &v.refused.request,
        v.refused.operation_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered, v.refused);
    assert_eq!(count("mission.planning_assessments", v.e, v.c), 1);
    // Fresh restricted consumer sees the original completion, not a new row.
    let mut fresh = PgTrajectory::new(runtime_client());
    let view = history_view(&mut fresh, &recovered, true);
    assert_eq!((view.state, view.complete), ("completed", true));
    let completed = "SELECT completed_at FROM trajectory.planning_history \
        WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'";
    let at = |o: &mut Client| -> std::time::SystemTime {
        o.query_one(completed, &[&v.e.0, &v.c.0]).unwrap().get(0)
    };
    let mut observer = runtime_client();
    let before = at(&mut observer);
    assert_eq!(
        PlanningHistoryPort::publish(&mut fresh, &recovered).unwrap(),
        Delivered::Duplicate
    );
    assert_eq!(before, at(&mut observer));
    assert_eq!(
        count_where(
            "trajectory.planning_history",
            "AND status='accepted'",
            v.e,
            v.c
        ),
        1
    );
    // Completion never upgrades the refusal into permission or a completed
    // assessment: the durable decision stays a version-3 refusal.
    assert_eq!(
        (recovered.decision, recovered.version, recovered.withdrawal),
        (
            NonpositiveDecision::RefusedAuthorityWithdrawn,
            3,
            v.refused.withdrawal
        )
    );
}

#[test]
fn v3_pends_until_withdrawal_predecessor_is_accepted() {
    let _guard = db();
    let mut v = v3(0xd11);
    // No withdrawal-history predecessor: bounded pending, zero effects.
    assert_eq!(
        publish(&mut v.traj, &v.refused),
        Delivered::Unresolved("missing_predecessor")
    );
    assert_eq!(count("trajectory.planning_history", v.e, v.c), 0);
    assert_eq!(
        WithdrawalHistoryPort::publish(&mut v.traj, &v.withdrawn).unwrap(),
        Delivered::Completed
    );
    assert_eq!(publish(&mut v.traj, &v.refused), Delivered::Completed);
    assert_eq!(
        PlanningHistoryPort::inspect(&mut v.traj, &v.refused).unwrap(),
        Delivered::Completed
    );
}

#[test]
fn v3_coherent_but_foreign_withdrawal_is_unsupported() {
    let _guard = db();
    for (slot, kind) in [(0xd13, "scope"), (0xd15, "registration")] {
        let v = v3(slot);
        let payload = match kind {
            // Valid under its own coherent engagement/campaign, not this scope.
            "scope" => rescoped(&v.withdrawn),
            // Valid under its own registration operation, not this mission's.
            _ => foreign_registration(&v.withdrawn),
        };
        // The stored contract stays standalone-valid under its own scope.
        let decoded: MissionAuthorityWithdrawn = serde_json::from_value(payload.clone()).unwrap();
        decoded.validate().unwrap();
        accept_wh(&v, &payload);
        unsupported(v);
    }
}

#[test]
fn v3_catalog_identity_mismatch_is_unsupported() {
    let _guard = db();
    // Valid decoded contract, mismatched persisted catalog on one axis; the
    // other queried axis retains the referenced identity.
    for (slot, foreign_event, foreign_op) in
        [(0xd16, None, Some(0xd161u128)), (0xd17, Some(0xd171), None)]
    {
        let v = v3(slot);
        let (event, op) = axis(&v, foreign_event, foreign_op);
        insert_wh(
            &mut admin(),
            &v,
            event,
            op,
            Some(&serde_json::to_value(&v.withdrawn).unwrap()),
        );
        assert_eq!(wh_rows(&v, "accepted"), 1);
        unsupported(v);
    }
}

#[test]
fn v3_malformed_or_invalid_header_is_bounded_decode_failure() {
    let _guard = db();
    for (slot, bad_producer) in [(0xd18, false), (0xd19, true)] {
        let mut v = v3(slot);
        let payload = if bad_producer {
            // Well-formed payload whose decoded fixed header fails validation.
            let mut p = serde_json::to_value(&v.withdrawn).unwrap();
            p["producer"] = serde_json::json!("adversary");
            p
        } else {
            serde_json::json!({})
        };
        accept_wh(&v, &payload);
        // Bounded decode failure: no raw driver or payload text escapes.
        assert_eq!(
            PlanningHistoryPort::publish(&mut v.traj, &v.refused),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(count("trajectory.planning_history", v.e, v.c), 0);
    }
}

#[test]
fn v3_split_identity_axes_are_unsupported() {
    let _guard = db();
    let v = v3(0xd1a);
    let mut a = admin();
    // One row matches only the referenced event; the other only the operation.
    for (event, op) in [
        (v.withdrawn.event_id.0, Uuid::from_u128(0xd1a1)),
        (Uuid::from_u128(0xd1a2), v.withdrawn.operation_id.0),
    ] {
        insert_wh(
            &mut a,
            &v,
            event,
            op,
            Some(&shifted(&v.withdrawn, event, op)),
        );
    }
    // Both rows persist under the intact unique indexes (distinct per axis).
    assert_eq!(wh_rows(&v, "accepted"), 2);
    unsupported(v);
}

#[test]
fn v3_anomaly_on_one_referenced_axis_is_unsupported() {
    let _guard = db();
    // A valid anomaly row matching only one referenced axis is an unsupported
    // predecessor; its other identity differs from the referenced withdrawal.
    for (slot, foreign_event, foreign_op) in
        [(0xd1b, None, Some(0xd1b1u128)), (0xd1c, Some(0xd1c1), None)]
    {
        let v = v3(slot);
        let (event, op) = axis(&v, foreign_event, foreign_op);
        insert_wh(&mut admin(), &v, event, op, None);
        assert_eq!(wh_rows(&v, "anomaly"), 1);
        assert_ne!(
            (event, op),
            (v.withdrawn.event_id.0, v.withdrawn.operation_id.0)
        );
        unsupported(v);
    }
}

#[test]
fn v3_accepted_plus_anomaly_is_unsupported() {
    let _guard = db();
    // The real accepted predecessor plus an anomaly sharing either referenced
    // axis is ambiguous and never a first planning acceptance.
    for (slot, foreign_event, foreign_op) in
        [(0xd1d, None, Some(0xd1d1u128)), (0xd1e, Some(0xd1e1), None)]
    {
        let mut v = v3(slot);
        assert_eq!(
            WithdrawalHistoryPort::publish(&mut v.traj, &v.withdrawn).unwrap(),
            Delivered::Completed
        );
        let (event, op) = axis(&v, foreign_event, foreign_op);
        insert_wh(&mut admin(), &v, event, op, None);
        assert_eq!(wh_rows(&v, "accepted") + wh_rows(&v, "anomaly"), 2);
        unsupported(v);
    }
}

#[test]
fn withdrawal_history_check_constraints_reject_forbidden_mutations() {
    let _guard = db();
    let mut v = v3(0xd1f);
    assert_eq!(
        WithdrawalHistoryPort::publish(&mut v.traj, &v.withdrawn).unwrap(),
        Delivered::Completed
    );
    // 0005 CHECK constraints already prohibit each forbidden persisted state;
    // every mutation must fail with SQLSTATE 23514 and leave the row intact.
    let mut a = admin();
    for mutation in [
        "producer='adversary'",
        "kind='planning_assessed'",
        "version=2",
        "obligation='trajectory.other.v1'",
        "status='pending'",
        "contract=NULL",
        "completed_at=NULL",
    ] {
        let err = a
            .execute(
                &format!(
                    "UPDATE trajectory.withdrawal_history SET {mutation} \
                     WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'"
                ),
                &[&v.e.0, &v.c.0],
            )
            .unwrap_err();
        assert_eq!(
            err.as_db_error().map(|d| d.code()),
            Some(&SqlState::CHECK_VIOLATION),
            "mutation {mutation} must hit a CHECK constraint"
        );
    }
    // The predecessor row remains exactly as committed.
    let row = a
        .query_one(
            "SELECT contract, completed_at IS NOT NULL, status FROM trajectory.withdrawal_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&v.e.0, &v.c.0],
        )
        .unwrap();
    assert_eq!(
        row.get::<_, Value>(0),
        serde_json::to_value(&v.withdrawn).unwrap()
    );
    assert_eq!(
        (row.get::<_, bool>(1), row.get::<_, String>(2).as_str()),
        (true, "accepted")
    );
}

//! Transaction-ordering and stale-snapshot coverage for the session fence:
//! competing prepares serialize to exactly one claim, prepare/withdrawal
//! orderings refuse the loser in both commit orders, and writers holding
//! stale Repeatable Read or Serializable snapshots abort inside the guard
//! instead of silently passing.

use duskweave::m1_session::{SessionAction, SessionRequest, session};
use duskweave::mission::*;
use duskweave::postgres_m1_session::PgSessionStore;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::registration;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest, WithdrawalStore};
use duskweave::{Fail, Res};
use serde_json::Value;
use std::thread;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;
use session_support::*;

fn req(e: EngagementId, c: CampaignId) -> SessionRequest {
    SessionRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(OPERATOR)),
        expected_mission_revision: 1,
    }
}

fn withdraw_req(e: EngagementId, c: CampaignId) -> WithdrawalRequest {
    WithdrawalRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(OPERATOR)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    }
}

fn register_m1(e: EngagementId, c: CampaignId) {
    let raw = serde_json::to_vec(&registration(e, c, now(), 0)).unwrap();
    let input = duskweave::input::parse_register(&raw).unwrap();
    let (mut a, mut s, mut t) = ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut s, &mut t, op, &input).unwrap();
}

fn prepare(
    e: EngagementId,
    c: CampaignId,
    op: OperationId,
) -> Res<Option<duskweave::m1_session::SessionRecord>> {
    let mut store = PgSessionStore::new(broker_client());
    let mut reader = PgMissionStore::new(runtime_client());
    session(
        &mut store,
        &mut reader,
        SessionAction::Prepare,
        &req(e, c),
        op,
    )
}

/// Fresh deliberate operation allocation — deterministic ids would collide
/// with durable history across reruns.
fn fresh_op() -> OperationId {
    registration::prepare_operation(&mut ports().0).unwrap()
}

fn withdraw(
    e: EngagementId,
    c: CampaignId,
    op: OperationId,
) -> Res<Option<duskweave::withdrawal::MissionAuthorityWithdrawn>> {
    let mut store = PgMissionStore::new(runtime_client());
    let mut alloc = PgAllocator::new(runtime_client());
    store.withdraw(&withdraw_req(e, c), op, false, &mut alloc)
}

#[test]
fn concurrent_prepares_serialize_to_exactly_one_claim() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7600);
    register_m1(e, c);
    let winner = fresh_op();
    let loser = fresh_op();
    let t1 = thread::spawn(move || prepare(e, c, winner));
    let t2 = thread::spawn(move || prepare(e, c, loser));
    let r1 = t1.join().unwrap();
    let r2 = t2.join().unwrap();
    let (oks, refused): (Vec<_>, Vec<_>) =
        [r1, r2].into_iter().partition(|r| matches!(r, Ok(Some(_))));
    assert_eq!(oks.len(), 1);
    assert_eq!(refused, vec![Err(Fail::State("session_refused"))]);
    let (_, generation, claimed, _) = fence(e, c).unwrap();
    assert_eq!(generation, 1);
    let claimed = claimed.unwrap();
    assert!(claimed == winner.0 || claimed == loser.0);
    assert_eq!(history_count(e, c), 1);
}

#[test]
fn withdrawal_committed_first_blocks_later_claim() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7601);
    register_m1(e, c);
    assert!(withdraw(e, c, fresh_op()).unwrap().is_some());
    assert_eq!(
        prepare(e, c, fresh_op()),
        Err(Fail::State("session_refused"))
    );
    assert_eq!(history_count(e, c), 0);
    assert_eq!(fence(e, c).unwrap().0, "idle");
}

#[test]
fn prepare_committed_first_blocks_ordinary_withdrawal() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7602);
    register_m1(e, c);
    assert!(prepare(e, c, fresh_op()).unwrap().is_some());
    assert!(withdraw(e, c, fresh_op()).is_err());
    assert_eq!(count("mission.withdrawals", e, c), 0);
    let (phase, _, _, _) = fence(e, c).unwrap();
    assert_eq!(phase, "prepared_no_effects");
}

#[test]
fn stale_repeatable_read_withdrawal_aborts_after_competing_prepare() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7603);
    register_m1(e, c);
    // An ordinary writer fixes its snapshot before the fence changes.
    let mut stale = runtime_client();
    let mut tx = stale
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::RepeatableRead)
        .start()
        .unwrap();
    tx.query_one("SELECT count(*) FROM mission.missions", &[])
        .unwrap();
    // The competing prepare commits a fence change the snapshot cannot see.
    assert!(prepare(e, c, fresh_op()).unwrap().is_some());
    // The guard's fence UPDATE must abort the stale transaction rather than
    // silently pass on the old snapshot.
    let conflict = tx
        .execute(
            "INSERT INTO mission.withdrawals (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract) \
             VALUES ($1,$2,$3,$4,$5,'{}')",
            &[
                &e.0,
                &c.0,
                &Uuid::from_u128(0xD031),
                &Uuid::from_u128(0xD032),
                &Uuid::from_u128(0xD033),
            ],
        )
        .unwrap_err();
    assert_eq!(
        conflict.code(),
        Some(&postgres::error::SqlState::T_R_SERIALIZATION_FAILURE),
        "{conflict:?}"
    );
    drop(tx);
    let (phase, _, _, _) = fence(e, c).unwrap();
    assert_eq!(phase, "prepared_no_effects");
    assert_eq!(count("mission.withdrawals", e, c), 0);
}

#[test]
fn stale_serializable_prepare_aborts_after_competing_withdrawal() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7604);
    register_m1(e, c);
    let mut stale = broker_client();
    let mut tx = stale
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    tx.query_one("SELECT count(*) FROM execution.session_fences", &[])
        .unwrap();
    // The competing withdrawal commits its fence epoch before prepare runs.
    assert!(withdraw(e, c, fresh_op()).unwrap().is_some());
    let stale_op = fresh_op();
    let source: Value = runtime_client()
        .query_one(
            "SELECT contract FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    let conflict = tx
        .query_opt(
            "SELECT execution.prepare_session($1,$2,$3,$4,$5,$6)",
            &[
                &e.0,
                &c.0,
                &stale_op.0,
                &Uuid::from_u128(OPERATOR),
                &1i64,
                &source,
            ],
        )
        .unwrap_err();
    assert_eq!(
        conflict.code(),
        Some(&postgres::error::SqlState::T_R_SERIALIZATION_FAILURE),
        "{conflict:?}"
    );
    drop(tx);
    assert_eq!(history_count(e, c), 0);
    assert_eq!(fence(e, c).unwrap().0, "idle");
}

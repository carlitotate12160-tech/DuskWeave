//! Real role/fence/lifecycle coverage for the prepared session slice on the
//! owned disposable database: guarded functions, Broker membership, ordinary
//! writer denial, backfill/reapply, idempotent generations and read-only
//! recovery. Admin access is fixture setup only; runtime assertions use the
//! restricted logins.

use duskweave::m1_session::{SessionAction, SessionRequest, session};
use duskweave::mission::*;
use duskweave::postgres_m1_session::PgSessionStore;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::registration::{self, MissionStore};
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest, WithdrawalStore};
use duskweave::{Fail, Res};
use postgres::NoTls;
use serde_json::Value;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;
use session_support::*;
#[path = "support/upgrade_db.rs"]
mod upgrade_db;

fn req(e: EngagementId, c: CampaignId) -> SessionRequest {
    SessionRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(OPERATOR)),
        expected_mission_revision: 1,
    }
}

/// Session operations use the same deliberate fresh allocation path as
/// registration operations; deterministic ids would collide across reruns.
fn session_op() -> OperationId {
    registration::prepare_operation(&mut ports().0).unwrap()
}

/// Register a current-window M1 scope through the ordinary runtime path.
fn register_m1(e: EngagementId, c: CampaignId, shift: i64) -> registration::Receipt {
    let raw = serde_json::to_vec(&registration(e, c, now(), shift)).unwrap();
    let input = duskweave::input::parse_register(&raw).unwrap();
    let (mut a, mut s, mut t) = ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut s, &mut t, op, &input).unwrap()
}

fn prepare(e: EngagementId, c: CampaignId, op: OperationId) -> Res<Option<Value>> {
    let mut store = PgSessionStore::new(broker_client());
    let mut reader = PgMissionStore::new(runtime_client());
    session(
        &mut store,
        Some(&mut reader),
        SessionAction::Prepare,
        &req(e, c),
        op,
    )
    .map(|r| r.map(|r| serde_json::to_value(r).unwrap()))
}

fn broker_oid() -> u32 {
    runtime_client()
        .query_one("SELECT oid FROM pg_roles WHERE rolname=$1", &[&BROKER])
        .unwrap()
        .get(0)
}

fn admin_db() -> postgres::Client {
    let mut cfg = dsn("DW_TEST_ADMIN_DATABASE_URL");
    cfg.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    cfg.connect(NoTls).unwrap()
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

#[test]
fn migration_backfills_existing_v1_v2_scopes_idle_and_reapplies() {
    let _g = db();
    let db = upgrade_db::OwnedUpgradeDatabase::create().unwrap();
    let mut admin = upgrade_db::admin_in(&db.name);
    let mut runtime = upgrade_db::runtime_in(&db.name);
    for sql in [
        MIGRATION,
        PLANNING_MIGRATION,
        HISTORY_MIGRATION,
        V2_MIGRATION,
        WITHDRAWAL_MIGRATION,
        WITHDRAWAL_REFUSAL_MIGRATION,
        ELIGIBILITY_MIGRATION,
    ] {
        admin.batch_execute(sql).unwrap();
    }
    // A v1 and a v2 Mission row exist before the session migration.
    for (n, permission) in [(1u128, false), (2, true)] {
        let (e, c) = scope(0x7100 + n);
        let mut reg = registration(e, c, now(), 0);
        if !permission {
            reg.as_object_mut().unwrap().remove("m1_permission");
        }
        let raw = serde_json::to_vec(&reg).unwrap();
        let input = duskweave::input::parse_register(&raw).unwrap();
        let op = OperationId(Uuid::from_u128(0xC100 + n));
        let mut store = PgMissionStore::new(upgrade_db::runtime_in(&db.name));
        let (mission, event) =
            Mission::register(&input, op, EventId(Uuid::from_u128(0xC200 + n)), 100).unwrap();
        assert!(matches!(
            store.commit_registration(&mission, &event).unwrap(),
            registration::CommitEffect::Fresh
        ));
    }
    admin.batch_execute(SESSION_MIGRATION).unwrap();
    admin.batch_execute(SESSION_MIGRATION).unwrap();
    for n in [1u128, 2] {
        let (e, c) = scope(0x7100 + n);
        let row = runtime
            .query_one(
                "SELECT phase,generation,operation_id FROM execution.session_fences \
                 WHERE engagement_id=$1 AND campaign_id=$2",
                &[&e.0, &c.0],
            )
            .unwrap();
        assert_eq!(
            (
                row.get::<_, String>(0),
                row.get::<_, i64>(1),
                row.get::<_, Option<Uuid>>(2)
            ),
            ("idle".to_string(), 0, None)
        );
    }
    db.finish().unwrap();
}

#[test]
fn broker_prepare_claims_once_and_replays_original_identity() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7200);
    let receipt = register_m1(e, c, 0);
    let op = session_op();
    let record = prepare(e, c, op).unwrap().unwrap();
    assert_eq!(record["generation"], 1);
    assert_eq!(record["kind"], "prepared_no_effects");
    assert_eq!(record["writer_oid"], broker_oid());
    assert_eq!(
        record["registration_operation_id"],
        receipt.operation_id.to_string()
    );
    assert_eq!(
        record["registration_event_id"],
        receipt.event_id.to_string()
    );
    let (phase, generation, claimed, writer) = fence(e, c).unwrap();
    assert_eq!((phase.as_str(), generation), ("prepared_no_effects", 1));
    assert_eq!(claimed, Some(op.0));
    assert_eq!(writer, Some(broker_oid()));
    // Idempotent replay: same identity, same record, no new generation.
    assert_eq!(prepare(e, c, op).unwrap().unwrap(), record);
    assert_eq!(history_count(e, c), 1);
    assert_eq!(fence(e, c).unwrap().1, 1);
    // Operation identity is scope-qualified: the same op under another
    // campaign claims its own generation and never collides cross-scope.
    let (e2, c2) = scope(0x7201);
    register_m1(e2, c2, 0);
    let other = prepare(e2, c2, op).unwrap().unwrap();
    assert_eq!(other["generation"], 1);
    assert_eq!(fence(e, c).unwrap().1, 1);
}

#[test]
fn prepare_refusals_leave_no_durable_claim() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7300);
    let receipt = register_m1(e, c, 0);
    let refused = Fail::State("session_refused");
    // Wrong operator.
    let mut wrong = req(e, c);
    wrong.operator_ref = OperatorRef(Uuid::from_u128(0x99));
    assert_eq!(
        session(
            &mut PgSessionStore::new(broker_client()),
            Some(&mut PgMissionStore::new(runtime_client())),
            SessionAction::Prepare,
            &wrong,
            session_op(),
        ),
        Err(Fail::State("session_prepare_refused"))
    );
    // Wrong revision and a session op colliding with the registration op.
    for (revision, op) in [(2i64, session_op()), (1, receipt.operation_id)] {
        let mut r = req(e, c);
        r.expected_mission_revision = revision;
        match session(
            &mut PgSessionStore::new(broker_client()),
            Some(&mut PgMissionStore::new(runtime_client())),
            SessionAction::Prepare,
            &r,
            op,
        ) {
            Err(Fail::Input(_)) | Err(Fail::State("session_refused")) => {}
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    // Missing scope refuses with no fence row.
    let (me, mc) = scope(0x7399);
    assert_eq!(
        prepare(me, mc, session_op()),
        Err(Fail::State("session_mission_missing"))
    );
    assert!(fence(me, mc).is_none());
    // Elapsed windows refuse inside the guarded function.
    let (ee, ec) = scope(0x7301);
    register_m1(ee, ec, -200000);
    assert_eq!(prepare(ee, ec, session_op()), Err(refused));
    assert_eq!(history_count(ee, ec), 0);
    assert_eq!(fence(ee, ec).unwrap().0, "idle");
    // Missing attachment refuses at the source check before SQL.
    let (ne, nc) = scope(0x7302);
    let raw = serde_json::to_vec(&reg_json(ne, nc)).unwrap();
    let input = duskweave::input::parse_register(&raw).unwrap();
    let (mut a, mut s, mut t) = ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut s, &mut t, op, &input).unwrap();
    assert_eq!(
        prepare(ne, nc, session_op()),
        Err(Fail::State("session_prepare_refused"))
    );
    assert_eq!(history_count(ne, nc), 0);
    // Corrupt catalog: the missions row diverging from the original contract
    // fails closed at the port decode, before SQL, with no claim durable.
    let mut admin = admin_db();
    admin
        .execute(
            "UPDATE mission.missions SET operator_ref=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &Uuid::from_u128(0x77)],
        )
        .unwrap();
    assert_eq!(
        prepare(e, c, session_op()),
        Err(Fail::Store("contract_decode"))
    );
    admin
        .execute(
            "UPDATE mission.missions SET operator_ref=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &Uuid::from_u128(OPERATOR)],
        )
        .unwrap();
    admin
        .execute(
            "UPDATE mission.registration_outbox SET contract='{}' WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(
        prepare(e, c, session_op()),
        Err(Fail::Store("contract_decode"))
    );
    assert_eq!(history_count(e, c), 0);
    assert_eq!(fence(e, c).unwrap().0, "idle");
}

#[test]
fn ordinary_writers_and_impersonators_are_fenced() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7400);
    register_m1(e, c, 0);
    let op = session_op();
    assert!(prepare(e, c, op).unwrap().is_some());
    let mut rt = runtime_client();
    // Direct ordinary INSERTs into every guarded authority table fail.
    for sql in [
        "INSERT INTO mission.missions (engagement_id,campaign_id,operator_ref,authority_ref,\
         authority_revision,goal_ref,included_assets,excluded_assets,exercise_mode,starts_at,ends_at,revision,operation_id) \
         VALUES ($1,$2,$3,$4,1,$5,'[]','[]','blind',1,2,1,$6)",
        "INSERT INTO mission.registration_outbox (engagement_id,campaign_id,operation_id,event_id,contract) \
         VALUES ($1,$2,$3,$4,'{}')",
        "INSERT INTO mission.withdrawals (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract) \
         VALUES ($1,$2,$3,$4,$5,'{}')",
    ] {
        assert!(
            rt.execute(
                sql,
                &[
                    &e.0,
                    &c.0,
                    &Uuid::from_u128(0x66),
                    &Uuid::from_u128(0x67),
                    &Uuid::from_u128(0x68),
                    &Uuid::from_u128(0x69),
                ],
            )
            .is_err(),
            "guarded insert must fail: {sql}"
        );
    }
    // The ordinary withdrawal path and the Broker login's own INSERT fail.
    let mut store = PgMissionStore::new(runtime_client());
    let mut alloc = PgAllocator::new(runtime_client());
    assert!(
        store
            .withdraw(&withdraw_req(e, c), session_op(), false, &mut alloc)
            .is_err()
    );
    let mut broker = broker_client();
    assert!(
        broker
            .execute(
                "INSERT INTO mission.withdrawals (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract) \
                 VALUES ($1,$2,$3,$4,$5,'{}')",
                &[&e.0, &c.0, &Uuid::from_u128(0x6A), &Uuid::from_u128(0x6B), &Uuid::from_u128(0x6C)],
            )
            .is_err()
    );
    // Baseline first: clear any leftover mutation grant so this test is
    // self-healing even if a prior run stopped between GRANT and REVOKE.
    admin_db_client()
        .batch_execute(
            "REVOKE UPDATE, DELETE ON execution.session_fences FROM dw_runtime; \
             REVOKE UPDATE, DELETE, TRUNCATE ON execution.session_history FROM dw_runtime",
        )
        .unwrap();
    // Runtime cannot mutate, truncate or role-shift into the fence.
    for sql in [
        "UPDATE execution.session_fences SET operation_id=NULL",
        "DELETE FROM execution.session_history",
        "TRUNCATE execution.session_fences",
        "SET ROLE dw_m1_broker",
    ] {
        assert!(
            rt.batch_execute(sql).is_err(),
            "runtime must be denied: {sql}"
        );
    }
    // Even an over-provisioned grant cannot mutate the fence: RLS filters
    // all non-owner rows so DML silently affects zero, and the trigger
    // denies TRUNCATE outright. Assert the effect, not just the error.
    admin_db_client()
        .batch_execute(
            "GRANT UPDATE, DELETE ON execution.session_fences TO dw_runtime; \
             GRANT UPDATE, DELETE, TRUNCATE ON execution.session_history TO dw_runtime",
        )
        .unwrap();
    for sql in [
        "UPDATE execution.session_fences SET operation_id=NULL",
        "DELETE FROM execution.session_history",
    ] {
        match rt.execute(sql, &[]) {
            Err(_) => {}
            Ok(0) => {}
            Ok(n) => panic!("over-granted mutation affected {n} rows: {sql}"),
        }
    }
    assert!(
        rt.batch_execute("TRUNCATE execution.session_history")
            .is_err()
    );
    assert_eq!(fence(e, c).unwrap().0, "prepared_no_effects");
    assert_eq!(history_count(e, c), 1);
    admin_db_client()
        .batch_execute(
            "REVOKE UPDATE, DELETE ON execution.session_fences FROM dw_runtime; \
             REVOKE UPDATE, DELETE, TRUNCATE ON execution.session_history FROM dw_runtime",
        )
        .unwrap();
    assert_eq!(
        session(
            &mut PgSessionStore::new(rt),
            Some(&mut PgMissionStore::new(runtime_client())),
            SessionAction::Prepare,
            &req(e, c),
            session_op(),
        ),
        Err(Fail::Config("unqualified_session_writer"))
    );
    // A second Broker login cannot replay, claim or release the first claim;
    // a custom GUC is never an actor identity.
    assert_eq!(
        session(
            &mut PgSessionStore::new(other_broker_client()),
            None::<&mut PgMissionStore>,
            SessionAction::Release,
            &req(e, c),
            op,
        ),
        Err(Fail::State("session_refused"))
    );
    let mut other = other_broker_client();
    other.batch_execute("SET dw.fake_writer='0'").unwrap();
    assert!(
        other
            .query_opt(
                "SELECT execution.release_prepared_session($1,$2,$3,$4,$5)",
                &[&e.0, &c.0, &op.0, &Uuid::from_u128(OPERATOR), &1i64],
            )
            .is_err()
    );
    // An unrelated campaign stays writable throughout.
    let (oe, oc) = scope(0x7401);
    register_m1(oe, oc, 0);
    assert_eq!(history_count(oe, oc), 0);
    assert_eq!(fence(oe, oc).unwrap().0, "idle");
    assert_eq!(count("mission.missions", e, c), 1);
    assert_eq!(count("mission.withdrawals", e, c), 0);
    assert_eq!(history_count(e, c), 1);
}

#[test]
fn release_preserves_history_and_generations_are_exact() {
    let _g = db();
    ensure_broker_logins();
    let (e, c) = scope(0x7500);
    register_m1(e, c, 0);
    let first = session_op();
    assert!(prepare(e, c, first).unwrap().is_some());
    let mut store = PgSessionStore::new(broker_client());
    let released = session(
        &mut store,
        None::<&mut PgMissionStore>,
        SessionAction::Release,
        &req(e, c),
        first,
    )
    .unwrap()
    .unwrap();
    assert_eq!(released.kind, "released_no_effects");
    assert_eq!(released.generation, 1);
    assert_eq!(fence(e, c).unwrap().0, "idle");
    assert_eq!(history_count(e, c), 2);
    // Ordinary authority writes work again after explicit release.
    let mut mission = PgMissionStore::new(runtime_client());
    let mut alloc = PgAllocator::new(runtime_client());
    let second_scope = scope(0x7501);
    register_m1(second_scope.0, second_scope.1, 0);
    assert!(
        mission
            .withdraw(&withdraw_req(e, c), session_op(), false, &mut alloc)
            .is_ok()
    );
    // Released history survives withdrawal; a fresh claim is refused.
    assert_eq!(
        session(
            &mut PgSessionStore::new(broker_client()),
            None::<&mut PgMissionStore>,
            SessionAction::Recover,
            &req(e, c),
            first,
        )
        .unwrap()
        .unwrap()
        .kind,
        "released_no_effects"
    );
    assert_eq!(
        prepare(e, c, session_op()),
        Err(Fail::State("session_refused"))
    );
    // A generation-2 scope: old release/recovery cannot clear it.
    let (ge, gc) = scope(0x7502);
    register_m1(ge, gc, 0);
    let g1 = session_op();
    let g2 = session_op();
    assert!(prepare(ge, gc, g1).unwrap().is_some());
    assert!(
        session(
            &mut PgSessionStore::new(broker_client()),
            None::<&mut PgMissionStore>,
            SessionAction::Release,
            &req(ge, gc),
            g1,
        )
        .unwrap()
        .is_some()
    );
    assert!(prepare(ge, gc, g2).unwrap().is_some());
    assert_eq!(fence(ge, gc).unwrap().1, 2);
    // Old operation release replay returns its record without clearing gen 2.
    let replay = session(
        &mut PgSessionStore::new(broker_client()),
        None::<&mut PgMissionStore>,
        SessionAction::Release,
        &req(ge, gc),
        g1,
    )
    .unwrap()
    .unwrap();
    assert_eq!(replay.kind, "released_no_effects");
    assert_eq!(replay.generation, 1);
    assert_eq!(fence(ge, gc).unwrap().2, Some(g2.0));
    assert_eq!(fence(ge, gc).unwrap().1, 2);
    // Old prepare replay likewise returns only its own released record.
    let old = prepare(ge, gc, g1).unwrap().unwrap();
    assert_eq!(old["kind"], "released_no_effects");
    assert_eq!(fence(ge, gc).unwrap().1, 2);
    assert_eq!(history_count(ge, gc), 3);
    // Recovery of an absent identity mutates nothing.
    assert_eq!(
        session(
            &mut PgSessionStore::new(broker_client()),
            None::<&mut PgMissionStore>,
            SessionAction::Recover,
            &req(ge, gc),
            session_op(),
        )
        .unwrap(),
        None
    );
    assert_eq!(history_count(ge, gc), 3);
}

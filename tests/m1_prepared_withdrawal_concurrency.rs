//! Deterministic advisory-lock/MVCC barriers and old-row migration qualification.
use duskweave::Fail;
use duskweave::m1_session::{SessionAction, session};
use duskweave::mission::*;
use duskweave::postgres_m1_session::PgSessionStore;
use duskweave::postgres_mission::PgMissionStore;
use postgres::IsolationLevel;
use serde_json::Value;
use std::sync::mpsc;
use std::time::{Duration, Instant};
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_prepared_withdrawal.rs"]
mod prepared;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/upgrade_db.rs"]
mod upgrade;
use prepared::*;

fn wait_advisory(pid: i32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut reader = db_support::runtime_client();
    while !reader.query_one(
        "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND NOT granted)",
        &[&pid]).unwrap().get::<_,bool>(0) {
        assert!(Instant::now()<deadline,"competing caller did not reach the advisory barrier");
        std::thread::yield_now();
    }
}
fn release_result(
    client: postgres::Client,
    case: &Case,
) -> duskweave::Res<Option<duskweave::m1_session::SessionRecord>> {
    session(
        &mut PgSessionStore::new(client),
        None::<&mut PgMissionStore>,
        SessionAction::Release,
        &session_support::req(case.e, case.c),
        case.session,
    )
}

#[test]
fn release_and_withdrawal_obey_both_deterministic_commit_orders() {
    let _g = db_support::db();
    for withdrawal_first in [true, false] {
        let case = Case::new(0x7b00 + u128::from(withdrawal_first));
        let mut winner = session_support::broker_client();
        let mut tx = winner
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .unwrap();
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!("{}:{}", case.e, case.c)],
        )
        .unwrap();
        if withdrawal_first {
            raw(
                &mut tx,
                &case,
                case.operation,
                &case.request(),
                &serde_json::to_value(&case.source).unwrap(),
            )
            .unwrap();
        } else {
            tx.query_one(
                "SELECT execution.release_prepared_session($1,$2,$3,$4,1)",
                &[
                    &case.e.0,
                    &case.c.0,
                    &case.session.0,
                    &case.request().withdrawal.operator_ref.0,
                ],
            )
            .unwrap();
        }
        let (ready_tx, ready_rx) = mpsc::channel();
        let other = case.clone();
        let worker = std::thread::spawn(move || {
            let mut client = session_support::broker_client();
            let pid: i32 = client
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);
            ready_tx.send(pid).unwrap();
            if withdrawal_first {
                release_result(client, &other).map(|r| r.is_some())
            } else {
                submit(client, &other.request(), other.operation).map(|r| r.is_some())
            }
        });
        let pid = ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        wait_advisory(pid);
        tx.commit().unwrap();
        let result = worker.join().unwrap();
        if withdrawal_first {
            assert_eq!(result, Ok(true));
            assert!(case.recover().unwrap().is_some());
        } else {
            assert!(
                matches!(
                    result,
                    Err(Fail::Store("serialization_retry"))
                        | Err(Fail::State("prepared_withdrawal_refused"))
                ),
                "{result:?}"
            );
            assert_eq!(case.recover().unwrap(), None);
        }
        assert_eq!(session_support::fence(case.e, case.c).unwrap().0, "idle");
    }
}

#[test]
fn old_repeatable_read_and_serializable_snapshots_cannot_change_new_generation() {
    let _g = db_support::db();
    for (slot, isolation) in [
        (0x7b10, IsolationLevel::RepeatableRead),
        (0x7b11, IsolationLevel::Serializable),
    ] {
        let case = Case::new(slot);
        let mut stale = session_support::broker_client();
        let mut tx = stale
            .build_transaction()
            .isolation_level(isolation)
            .start()
            .unwrap();
        tx.query_one("SELECT to_jsonb(f) FROM execution.session_fences f WHERE engagement_id=$1 AND campaign_id=$2",
            &[&case.e.0,&case.c.0]).unwrap();
        release(&case);
        let newer = session_support::session_op();
        session_support::prepare(case.e, case.c, newer).unwrap();
        let before = case.snapshot();
        let conflict = raw(
            &mut tx,
            &case,
            case.operation,
            &case.request(),
            &serde_json::to_value(&case.source).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            conflict.code().map(postgres::error::SqlState::code),
            Some("40001")
        );
        tx.rollback().unwrap();
        assert_eq!(case.snapshot(), before);
        assert_eq!(
            session_support::fence(case.e, case.c).unwrap().2,
            Some(newer.0)
        );
        assert!(
            case.submit().is_err(),
            "stale generation must NOT pass because the login is original"
        );
        assert_eq!(case.snapshot(), before);
    }
}

#[test]
fn ordinary_stale_writers_abort_on_epoch_even_when_prepared_identity_is_unchanged() {
    let _g = db_support::db();
    for (slot, isolation) in [
        (0x7b20, IsolationLevel::RepeatableRead),
        (0x7b21, IsolationLevel::Serializable),
    ] {
        let case = Case::new(slot);
        let mut stale = db_support::runtime_client();
        let mut tx = stale
            .build_transaction()
            .isolation_level(isolation)
            .start()
            .unwrap();
        tx.query_one("SELECT to_jsonb(f) FROM execution.session_fences f WHERE engagement_id=$1 AND campaign_id=$2",
            &[&case.e.0,&case.c.0]).unwrap();
        case.submit().unwrap();
        let before = case.snapshot();
        let conflict = tx
            .execute(
                "INSERT INTO mission.withdrawals
            (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract)
            VALUES($1,$2,$3,$4,$5,'{}')",
                &[
                    &case.e.0,
                    &case.c.0,
                    &session_support::session_op().0,
                    &uuid::Uuid::from_u128(95),
                    &case.source.operation_id.0,
                ],
            )
            .unwrap_err();
        assert_eq!(
            conflict.code().map(postgres::error::SqlState::code),
            Some("40001")
        );
        tx.rollback().unwrap();
        assert_eq!(case.snapshot(), before);
    }
}

fn upgrade_snapshot(name: &str, e: EngagementId, c: CampaignId, with_link: bool) -> Vec<Value> {
    let mut client = upgrade::runtime_in(name);
    let mut tables = vec![
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
        "mission.withdrawals",
        "execution.session_fences",
        "execution.session_history",
    ];
    if with_link {
        tables.push("execution.prepared_withdrawals");
    }
    tables
        .into_iter()
        .map(|table| {
            client
                .query_one(
                    &format!(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb)
         FROM {table} t WHERE engagement_id=$1 AND campaign_id=$2"),
                    &[&e.0, &c.0],
                )
                .unwrap()
                .get(0)
        })
        .collect()
}

#[test]
fn upgrade_and_reapply_preserve_existing_prepared_rows_and_owner_history() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let owned = upgrade::OwnedUpgradeDatabase::create().unwrap();
    {
        let mut admin = upgrade::admin_in(&owned.name);
        for sql in [
            db_support::MIGRATION,
            db_support::PLANNING_MIGRATION,
            db_support::HISTORY_MIGRATION,
            db_support::WITHDRAWAL_MIGRATION,
            db_support::ELIGIBILITY_MIGRATION,
            db_support::SESSION_MIGRATION,
        ] {
            admin.batch_execute(sql).unwrap();
        }
        let (e, c) = db_support::scope(0x7b30);
        let input = duskweave::input::parse_register(
            session_support::registration(e, c, session_support::now(), 0)
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let mut a = duskweave::postgres_mission::PgAllocator::new(upgrade::runtime_in(&owned.name));
        let mut reader = PgMissionStore::new(upgrade::runtime_in(&owned.name));
        let mut history =
            duskweave::postgres_trajectory::PgTrajectory::new(upgrade::runtime_in(&owned.name));
        let reg = duskweave::registration::prepare_operation(&mut a).unwrap();
        duskweave::registration::register(&mut a, &mut reader, &mut history, reg, &input).unwrap();
        let mut cfg: postgres::Config = session_support::broker_dsn(session_support::db_port())
            .parse()
            .unwrap();
        cfg.dbname(&owned.name);
        let op = duskweave::registration::prepare_operation(&mut a).unwrap();
        session(
            &mut PgSessionStore::new(cfg.connect(postgres::NoTls).unwrap()),
            Some(&mut reader),
            SessionAction::Prepare,
            &session_support::req(e, c),
            op,
        )
        .unwrap();
        let before = upgrade_snapshot(&owned.name, e, c, false);
        for _ in 0..2 {
            admin
                .batch_execute(db_support::PREPARED_WITHDRAWAL_MIGRATION)
                .unwrap();
            assert_eq!(upgrade_snapshot(&owned.name, e, c, false), before);
        }
        let request = duskweave::m1_prepared_withdrawal::PreparedWithdrawalRequest {
            withdrawal: session_support::withdraw_req(e, c),
            session_operation_id: op,
            expected_session_generation: 1,
        };
        let withdraw = duskweave::registration::prepare_operation(&mut a).unwrap();
        duskweave::m1_prepared_withdrawal::withdraw_prepared(
            &mut duskweave::postgres_prepared_withdrawal::PgPreparedWithdrawalStore::new(
                cfg.connect(postgres::NoTls).unwrap(),
            ),
            Some(&mut reader),
            &request,
            withdraw,
            false,
        )
        .unwrap()
        .unwrap();
        let after = upgrade_snapshot(&owned.name, e, c, true);
        admin
            .batch_execute(db_support::PREPARED_WITHDRAWAL_MIGRATION)
            .unwrap();
        assert_eq!(upgrade_snapshot(&owned.name, e, c, true), after);
    }
    owned.finish().unwrap();
}

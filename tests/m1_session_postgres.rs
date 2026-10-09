//! Real role/fence/lifecycle/catalog and migration qualification tests for M1 sessions.

#[path = "support/m1_session.rs"]
pub mod m1_session_support;
#[path = "support/registration_db.rs"]
pub mod registration_db;

use duskweave::m1_session::{self, FenceOutcome, SessionFence};
use duskweave::mission::OperationId;
use duskweave::postgres_m1_session::PostgresSessionFence;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use uuid::Uuid;

#[test]
fn prepare_missing_mission_refused() {
    let _g = registration_db::db();
    let mut client = m1_session_support::broker_client();
    let (e, c) = registration_db::scope(400);
    let op = OperationId(Uuid::from_u128(401));

    let request = m1_session_support::valid_request(e, c);

    let mut store = PgMissionStore::new(m1_session_support::broker_client());
    let mut tx = client.transaction().unwrap();
    let mut fence = PostgresSessionFence { tx: &mut tx };

    let out = m1_session::prepare(&mut store, &mut fence, &request, op).unwrap();

    assert_eq!(out, FenceOutcome::Refused("mission_missing"));
}

#[test]
fn guard_writer_triggers() {
    let _g = registration_db::db();
    let (e, c) = registration_db::scope(401);
    let mut a = PgAllocator::new(registration_db::runtime_client());
    let mut store = PgMissionStore::new(registration_db::runtime_client());
    let mut traj = PgTrajectory::new(registration_db::runtime_client());

    let op = registration::prepare_operation(&mut a).unwrap();

    let _receipt = registration_db::reg(&mut a, &mut store, &mut traj, op, e, c).unwrap();

    let idle_count =
        registration_db::count_where("execution.session_fences", "AND phase = 'idle'", e, c);
    assert_eq!(idle_count, 1);
}

#[test]
fn prepare_and_release_lifecycle() {
    let _g = registration_db::db();
    let mut client = m1_session_support::broker_client();
    let (e, c) = registration_db::scope(402);
    let mut a = PgAllocator::new(registration_db::runtime_client());
    let mut store = PgMissionStore::new(registration_db::runtime_client());
    let mut traj = PgTrajectory::new(registration_db::runtime_client());

    let op = registration::prepare_operation(&mut a).unwrap();
    registration_db::reg(&mut a, &mut store, &mut traj, op, e, c).unwrap();

    let session_op = OperationId(Uuid::from_u128(403));
    let request = m1_session_support::valid_request(e, c);

    let mut store = PgMissionStore::new(m1_session_support::broker_client());
    let mut tx = client.transaction().unwrap();
    let mut fence = PostgresSessionFence { tx: &mut tx };
    let out = m1_session::prepare(&mut store, &mut fence, &request, session_op).unwrap();
    tx.commit().unwrap();

    let record = match out {
        FenceOutcome::Durable(r) => r,
        _ => panic!("Expected durable record, got {:?}", out),
    };
    assert_eq!(record.operation_id, session_op);
    assert_eq!(record.kind, "prepared_no_effects");

    let op2 = registration::prepare_operation(&mut a).unwrap();
    let err = registration_db::reg(&mut a, &mut store, &mut traj, op2, e, c).unwrap_err();
    assert!(matches!(err, duskweave::Fail::Store(_)));

    let mut tx = client.transaction().unwrap();
    let mut fence = PostgresSessionFence { tx: &mut tx };
    let out = m1_session::release(&mut fence, &request, session_op).unwrap();
    tx.commit().unwrap();

    let record2 = match out {
        FenceOutcome::Durable(r) => r,
        _ => panic!("Expected durable record, got {:?}", out),
    };
    assert_eq!(record2.kind, "released_no_effects");
}

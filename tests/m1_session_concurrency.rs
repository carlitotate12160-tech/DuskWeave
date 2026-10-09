//! Real transaction ordering and fixed-snapshot counterexamples for M1 sessions.

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
use std::thread;
use uuid::Uuid;

#[test]
fn prepare_concurrency() {
    let _g = registration_db::db();
    let (e, c) = registration_db::scope(410);
    let mut a = PgAllocator::new(registration_db::runtime_client());
    let mut store = PgMissionStore::new(registration_db::runtime_client());
    let mut traj = PgTrajectory::new(registration_db::runtime_client());
    
    let op = registration::prepare_operation(&mut a).unwrap();
    registration_db::reg(&mut a, &mut store, &mut traj, op, e, c).unwrap();
    
    let session_op = OperationId(Uuid::from_u128(411));
    let request = m1_session_support::valid_request(e, c);
    
    let req1 = request.clone();
    let t1 = thread::spawn(move || {
        let mut client = m1_session_support::broker_client();
        let mut store = PgMissionStore::new(m1_session_support::broker_client());
        let mut tx = client.transaction().unwrap();
        let mut fence = PostgresSessionFence { tx: &mut tx };
        let out = m1_session::prepare(&mut store, &mut fence, &req1, session_op);
        if out.is_ok() {
            tx.commit().unwrap();
        }
        out
    });
    
    let req2 = request.clone();
    let session_op2 = OperationId(Uuid::from_u128(412));
    let t2 = thread::spawn(move || {
        let mut client = m1_session_support::broker_client();
        let mut store = PgMissionStore::new(m1_session_support::broker_client());
        let mut tx = client.transaction().unwrap();
        let mut fence = PostgresSessionFence { tx: &mut tx };
        let out = m1_session::prepare(&mut store, &mut fence, &req2, session_op2);
        if out.is_ok() {
            tx.commit().unwrap();
        }
        out
    });
    
    let out1 = t1.join().unwrap();
    let out2 = t2.join().unwrap();
    
    let (durable, other) = if matches!(out1, Ok(FenceOutcome::Durable(_))) {
        (out1, out2)
    } else {
        (out2, out1)
    };
    
    assert!(matches!(durable, Ok(FenceOutcome::Durable(_))));
    
    match other {
        Ok(FenceOutcome::Refused("already_claimed")) => {}
        Err(duskweave::Fail::Store("serialization_retry")) => {}
        Err(duskweave::Fail::Store("commit_unknown")) => {}
        Err(duskweave::Fail::Store("storage_error")) => {}
        _ => panic!("Expected conflict, got {:?}", other),
    }
}

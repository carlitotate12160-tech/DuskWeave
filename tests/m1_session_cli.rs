//! Actual entrypoint/receipts, lost-ACK and fresh-process recovery.

#[path = "support/m1_session.rs"]
pub mod m1_session_support;
#[path = "support/registration_db.rs"]
pub mod registration_db;

use duskweave::mission::OperationId;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use std::process::Command;
use uuid::Uuid;
use std::fs;

#[test]
fn cli_prepare_recover_release() {
    let _g = registration_db::db();
    let (e, c) = registration_db::scope(420);
    
    let mut a = PgAllocator::new(registration_db::runtime_client());
    let mut store = PgMissionStore::new(registration_db::runtime_client());
    let mut traj = PgTrajectory::new(registration_db::runtime_client());
    let op = registration::prepare_operation(&mut a).unwrap();
    registration_db::reg(&mut a, &mut store, &mut traj, op, e, c).unwrap();
    
    let session_op = OperationId(Uuid::from_u128(421));
    let req = m1_session_support::valid_request(e, c);
    
    let dir = std::env::temp_dir().join(format!("dw_test_{}", Uuid::from_u128(9999).to_string()));
    fs::create_dir_all(&dir).unwrap();
    let req_path = dir.join("req.json");
    fs::write(&req_path, serde_json::to_string(&req).unwrap()).unwrap();
    
    let db_url = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    
    let mut prepare = Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .arg("m1-session")
        .arg("--action").arg("prepare")
        .arg("--operation").arg(session_op.0.to_string())
        .arg("--input").arg(req_path.to_str().unwrap())
        .env("DW_DATABASE_URL", &db_url)
        .spawn()
        .unwrap();
    let status1 = prepare.wait().unwrap();
    assert!(status1.success());
    
    let mut recover = Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .arg("m1-session")
        .arg("--action").arg("recover")
        .arg("--operation").arg(session_op.0.to_string())
        .arg("--input").arg(req_path.to_str().unwrap())
        .env("DW_DATABASE_URL", &db_url)
        .spawn()
        .unwrap();
    let status2 = recover.wait().unwrap();
    assert!(status2.success());
    
    let mut release = Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .arg("m1-session")
        .arg("--action").arg("release")
        .arg("--operation").arg(session_op.0.to_string())
        .arg("--input").arg(req_path.to_str().unwrap())
        .env("DW_DATABASE_URL", &db_url)
        .spawn()
        .unwrap();
    let status3 = release.wait().unwrap();
    assert!(status3.success());
}

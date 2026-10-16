//! Physical post-COMMIT acknowledgment loss and database-only recovery tests for M1 Terrain DNS.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/m1_terrain.rs"]
mod terrain_support;

use duskweave::Fail;
use duskweave::mission::OperationId;
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_trajectory::PgTrajectory;
use std::time::Duration;
use terrain_support::*;
use uuid::Uuid;

fn count_rows(table: &str, op: OperationId) -> i64 {
    admin_client()
        .query_one(
            &format!("SELECT count(*) FROM {table} WHERE operation_id = $1"),
            &[&op.0],
        )
        .unwrap()
        .get(0)
}

#[test]
fn physical_terrain_commit_ack_loss_preserves_durable_identity_and_recovers_via_cli() {
    let _g = db();
    let (e, c, _reg, now) = register_m1_campaign(0x8201);
    let raw = dns_request_json(e, c, now - 10);
    let req = parse_req(&raw);
    let op = allocate_op();

    let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
    let mut relayed_store = PgTerrainStore::new(through("DW_TERRAIN_DATABASE_URL", relay.port()));
    let mut mission = duskweave::postgres_mission::PgMissionStore::new(runtime_client());
    let req_clone = req.clone();

    let outcome = std::thread::scope(|s| {
        let worker = s.spawn(move || relayed_store.record_dns(&mut mission, &req_clone, op));
        assert!(
            relay.observed(Duration::from_secs(10)),
            "COMMIT frame must leave client"
        );
        assert!(
            relay.consumed(Duration::from_secs(10)),
            "PostgreSQL backend must confirm COMMIT before ACK loss"
        );
        for tbl in [
            "terrain.observations",
            "terrain.materials",
            "terrain.evaluations",
            "terrain.claim_revisions",
            "terrain.current_claims",
            "terrain.outbox",
        ] {
            assert_eq!(
                count_rows(tbl, op),
                1,
                "{tbl} must be durably committed before caller receives ACK"
            );
        }
        assert_eq!(count_rows("trajectory.terrain_history", op), 0);
        drop(relay);
        worker.join().unwrap()
    });

    assert_eq!(outcome, Err(Fail::Unresolved("terrain_commit_unknown")));

    let path = std::env::temp_dir().join(format!("dw-terrain-ack-{}.json", op.0));
    std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
    let op_str = op.0.to_string();

    // Fresh CLI inspect sees committed owner publication and pending Trajectory history.
    let (ok_insp, _, r_insp) = cli_run("m1-terrain-inspect", &path, &op_str, &[]);
    assert!(ok_insp);
    assert_eq!(r_insp["publication"], "committed");
    assert_eq!(r_insp["history"], "pending");
    assert_eq!(r_insp["result"], "pending");
    assert_eq!(r_insp["operation_id"], op_str);

    // Fresh CLI recover completes Trajectory history once without creating duplicate revisions.
    let (ok_rec1, _, r_rec1) = cli_run("m1-terrain-recover", &path, &op_str, &[]);
    assert!(ok_rec1);
    assert_eq!(r_rec1["publication"], "committed");
    assert_eq!(r_rec1["history"], "completed");
    assert_eq!(r_rec1["result"], "accepted");

    let (ok_rec2, _, r_rec2) = cli_run("m1-terrain-recover", &path, &op_str, &[]);
    assert!(ok_rec2);
    for field in [
        "operation_id",
        "event_id",
        "observation_id",
        "material_id",
        "evaluation_id",
        "claim_id",
        "publication",
        "history",
    ] {
        assert_eq!(
            r_rec2[field], r_rec1[field],
            "stable {field} across recovery"
        );
    }
    assert_eq!(count_rows("terrain.claim_revisions", op), 1);
    assert_eq!(count_rows("trajectory.terrain_history", op), 1);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn physical_trajectory_commit_ack_loss_and_rolled_back_terrain_tx() {
    let _g = db();
    let (e, c, _reg, now) = register_m1_campaign(0x8202);
    let raw = dns_request_json(e, c, now - 10);
    let req = parse_req(&raw);
    let op = allocate_op();

    let (mut mission, mut store, _) = stores();
    let (ev, _, _, _) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();

    let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
    let mut relayed_traj = PgTrajectory::new(through("DW_TRAJECTORY_DATABASE_URL", relay.port()));
    let ev_clone = ev.clone();

    let traj_outcome = std::thread::scope(|s| {
        let worker = s.spawn(move || relayed_traj.record_terrain(&ev_clone));
        assert!(relay.observed(Duration::from_secs(10)));
        assert!(relay.consumed(Duration::from_secs(10)));
        assert_eq!(count_rows("trajectory.terrain_history", op), 1);
        drop(relay);
        worker.join().unwrap()
    });
    assert_eq!(
        traj_outcome,
        Err(Fail::Unresolved("trajectory_commit_unknown"))
    );

    let path = std::env::temp_dir().join(format!(
        "dw-terrain-traj-ack-{}.json",
        Uuid::from_u128(0x8202)
    ));
    std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
    let (ok_rec, _, r_rec) = cli_run("m1-terrain-recover", &path, &op.0.to_string(), &[]);
    std::fs::remove_file(&path).unwrap();
    assert!(ok_rec);
    assert_eq!(r_rec["history"], "completed");
    assert_eq!(count_rows("trajectory.terrain_history", op), 1);
}

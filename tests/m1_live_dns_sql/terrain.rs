//! Schema-0030 terrain SQL regressions: integrity holds under the active fence.
use super::fixture::*;
use serde_json::json;
use uuid::Uuid;

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_terrain_integrity_hold_persists_under_active_fence() {
    assert!(worker());
    let _g = terrain_support::native::qualified_existing_db();
    duskweave::postgres_m1_terrain::qualify_terrain(&mut terrain_support::terrain_client())
        .expect("terrain role qualification");
    duskweave::postgres_terrain_history::qualify_terrain_trajectory(
        &mut terrain_support::trajectory_client(),
    )
    .expect("trajectory role qualification");
    let (e, c, _ev, now) = register_live_campaign(0x5e01);
    // Accept one ordinary observation first (unfenced scope).
    let req = terrain_support::parse_req(&terrain_support::dns_request_json(e, c, now - 20));
    let mut terrain = terrain_support::terrain_client();
    let op = terrain_support::allocate_op();
    let event = duskweave::m1_terrain::evaluate_dns_observation(
        &req,
        None,
        duskweave::m1_terrain_input::TerrainIds::derive(e, c, op).unwrap(),
        (_ev.0, _ev.1, now),
    )
    .unwrap();
    let mut tx = terrain
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    terrain_support::sql_accept(
        &mut tx,
        &serde_json::to_value(&req).unwrap(),
        &serde_json::to_value(&event).unwrap(),
    )
    .unwrap();
    tx.commit().unwrap();
    // Charge the live fence over the same scope with a committed result lineage.
    let (run, generation, result_ref) = fabricate_live_run(e.0, c.0, true, true);
    admin()
        .execute(
            "UPDATE execution.autonomous_runs SET terrain_operation=$1 WHERE run_id=$2",
            &[&op.0, &run],
        )
        .unwrap();
    admin()
        .execute(
            "INSERT INTO execution.live_dns_results SELECT $1,$2,x.attempt,$3,$4,2,'terrain.issued_dns.v2' FROM execution.autonomous_runs x WHERE x.run_id=$2",
            &[&result_ref, &run, &generation, &json!({"status":"completed"})],
        )
        .unwrap();
    // The qualified Terrain owner's validated conflict command is admitted and
    // the hold persists before any refusal; the current view observes it.
    let mut variant = req.clone();
    variant.perceived_at += 5;
    let mut raw = serde_json::to_value(&variant).unwrap();
    raw["answers"] = json!([{"ipv4":"198.51.100.99","ttl_seconds":60}]);
    let mut terrain = terrain_support::terrain_client();
    let mut tx = terrain
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    let blocks: bool = tx
        .query_one(
            "SELECT terrain.record_dns_integrity_conflict($1,$2)",
            &[&raw, &op.0],
        )
        .unwrap()
        .get(0);
    tx.commit().unwrap();
    assert!(blocks);
    let holds = admin()
        .query_one(
            "SELECT count(*) FROM terrain.integrity_conflicts WHERE operation_id=$1 AND blocks_use",
            &[&op.0],
        )
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(
        holds, 1,
        "restrictive hold persisted under the active fence"
    );
    // A committed Mission withdrawal in between does not release the persisted
    // hold, and the withdrawn mission can no longer accept terrain commands.
    live_withdrawal(
        &withdrawal_q(e.0, c.0, Uuid::from_u128(0x11)),
        Uuid::new_v4(),
        None,
        false,
    )
    .unwrap();
    assert_eq!(
        admin()
            .query_one(
                "SELECT count(*) FROM terrain.integrity_conflicts WHERE operation_id=$1 AND blocks_use",
                &[&op.0]
            )
            .unwrap()
            .get::<_, i64>(0),
        1,
        "withdrawal does not erase a committed integrity hold"
    );
    let mut replay_client = terrain_support::terrain_client();
    let mut tx = replay_client
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    let replay = tx.query_one(
        "SELECT terrain.record_dns_integrity_conflict($1,$2)",
        &[&raw, &op.0],
    );
    assert!(
        replay.is_err(),
        "a withdrawn mission refuses further terrain commands: {replay:?}"
    );
    // Arbitrary writes remain denied: no ordinary SQL grant, no UPDATE/DELETE.
    assert!(
        terrain_support::terrain_client()
            .execute(
                "INSERT INTO terrain.integrity_conflicts VALUES($1,$2,$3,$4,1,true,'input_mismatch')",
                &[&e.0, &c.0, &Uuid::new_v4(), &Uuid::new_v4()]
            )
            .is_err(),
        "ordinary restricted INSERT stays denied"
    );
    assert!(
        admin()
            .execute(
                "DELETE FROM terrain.integrity_conflicts WHERE operation_id=$1",
                &[&op.0]
            )
            .is_err(),
        "the charged fence denies truth mutation even to setup roles"
    );
    // Runtime roles never reach the restrictive command.
    assert!(
        db_support::runtime_client()
            .query_one(
                "SELECT terrain.record_dns_integrity_conflict($1,$2)",
                &[&raw, &op.0]
            )
            .is_err()
    );
}

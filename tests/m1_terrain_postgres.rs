//! Real restricted-PostgreSQL integration tests for M1 Terrain DNS and Trajectory history.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_terrain.rs"]
mod terrain_support;

use duskweave::Fail;
use duskweave::m1_terrain::verify_mission_permission;
use duskweave::m1_terrain::{EvaluationOutcome, TerrainClaimStatus};
use duskweave::mission::{AssetRef, ExerciseMode, GoalRef, OperationId, OperatorRef};
use duskweave::postgres_m1_terrain::qualify_terrain;
use duskweave::postgres_terrain_history::qualify_terrain_trajectory;
use duskweave::trajectory::HistoryStatus;
use serde_json::json;
use terrain_support::*;
use uuid::Uuid;

fn table_count(table: &str, op: OperationId) -> i64 {
    admin_client()
        .query_one(
            &format!("SELECT count(*) FROM {table} WHERE operation_id = $1"),
            &[&op.0],
        )
        .unwrap()
        .get(0)
}

#[test]
fn regression_restricted_sql_anomalies_never_retain_rejected_bytes() {
    let _g = db();
    let sentinel = "Bearer SYNTHETIC_UNSAFE_TERRAIN_VALUE";
    for (slot, accepted_first) in [(0x8110, false), (0x8111, true)] {
        let (e, c, _, now) = register_m1_campaign(slot);
        let (mut mission, mut store, mut traj) = stores();
        let req = parse_req(&dns_request_json(e, c, now - 10));
        let op = allocate_op();
        let (ev, _, _, _) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();
        let mut unsafe_event = ev.clone();
        unsafe_event.material.answers[0].ipv4 = sentinel.into();
        let rejected = traj.record_terrain(&unsafe_event);
        assert_eq!(rejected, Err(Fail::Store("contract_decode")));
        assert!(!format!("{rejected:?}").contains(sentinel));
        if accepted_first {
            assert_eq!(traj.record_terrain(&ev).unwrap(), HistoryStatus::Completed);
        }
        let mut bad = serde_json::to_value(&ev).unwrap();
        bad["material"]["answers"][0]["ipv4"] = json!(sentinel);
        bad["raw_packet"] = json!(sentinel);
        let result =
            trajectory_client().query_one("SELECT trajectory.record_terrain_history($1)", &[&bad]);
        assert!(result.is_ok(), "SQL must return a static disposition");
        assert_eq!(result.unwrap().get::<_, String>(0), "anomaly");
        let retained: bool = trajectory_client().query_one(
            "SELECT EXISTS(SELECT 1 FROM trajectory.terrain_history WHERE operation_id=$1 AND position($2 in contract::text)>0)",
            &[&op.0, &sentinel],
        ).unwrap().get(0);
        assert!(
            !retained,
            "rejected bytes must be absent from durable contracts"
        );
        assert_eq!(traj.record_terrain(&ev).unwrap(), HistoryStatus::Anomaly);
        let accepted: i64 = trajectory_client().query_one(
            "SELECT count(*) FROM trajectory.terrain_history WHERE operation_id=$1 AND status='accepted' AND contract=$2",
            &[&op.0, &serde_json::to_value(&ev).unwrap()],
        ).unwrap().get(0);
        assert_eq!(accepted, i64::from(accepted_first));
        let path = std::env::temp_dir().join(format!("dw-terrain-sentinel-{}.json", op.0));
        std::fs::write(&path, serde_json::to_vec(&req).unwrap()).unwrap();
        let (ok, stdout, receipt) = cli_run("m1-terrain-inspect", &path, &op.0.to_string(), &[]);
        assert!(ok);
        assert!(!stdout.contains(sentinel));
        assert!(!serde_json::to_string(&receipt).unwrap().contains(sentinel));
        assert_eq!(receipt["history"], "anomaly");
        assert_eq!(receipt["claim"]["routing_usable"], false);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn regression_admitted_identity_conflict_invalidates_fresh_reads_but_not_later_use() {
    let _g = db();
    let (e, c, _, now) = register_m1_campaign(0x8112);
    let (mut mission, mut store, mut traj) = stores();
    let req = parse_req(&dns_request_json(e, c, now - 20));
    let op = allocate_op();
    let (ev, _, _, _) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();
    assert_eq!(traj.record_terrain(&ev).unwrap(), HistoryStatus::Completed);
    let mut bad = req.clone();
    bad.answers[0].ipv4 = "198.51.100.250".into();
    assert_eq!(
        store.record_dns(&mut mission, &bad, op),
        Err(Fail::Conflict("input_mismatch"))
    );
    let (_, current, at, auth) = store.inspect_dns(&mut mission, &req, op).unwrap().unwrap();
    assert!(
        !current.view_with_authority(at, auth).routing_usable,
        "admitted identity conflict must durably block current use"
    );
    assert_eq!(traj.inspect_terrain(&ev).unwrap(), "anomaly");
    let mut later = req.clone();
    later.expected_claim_revision = 1;
    later.perceived_at += 10;
    let later_op = allocate_op();
    let (clean, cur, at, auth) = store
        .record_dns(&mut mission, &later, later_op)
        .unwrap()
        .unwrap();
    assert!(cur.view_with_authority(at, auth).routing_usable);
    assert_eq!(
        traj.record_terrain(&clean).unwrap(),
        HistoryStatus::Completed
    );
    assert_eq!(traj.inspect_terrain(&ev).unwrap(), "anomaly");
}

#[test]
fn regression_ineligible_or_unattributed_variants_do_not_hold_other_applicability() {
    let _g = db();
    let (e, c, _, now) = register_m1_campaign(0x8113);
    let (mut mission, mut store, mut traj) = stores();
    let req = parse_req(&dns_request_json(e, c, now - 20));
    let op = allocate_op();
    let (ev, _, _, _) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();
    traj.record_terrain(&ev).unwrap();
    for case in 0..4 {
        let mut bad = req.clone();
        match case {
            0 => bad.operator_ref = OperatorRef(Uuid::from_u128(0x999)),
            1 => bad
                .limitations
                .push(duskweave::m1_terrain_input::DnsLimitation::PartialAliasChain),
            2 => bad.answers[0].ttl_seconds = 0,
            _ => {
                bad.asset_ref = None;
                bad.subject_name = "api.example.invalid".into();
            }
        }
        assert_eq!(
            store.record_dns(&mut mission, &bad, op),
            Err(Fail::Conflict("input_mismatch"))
        );
        let (original, cur, at, auth) = store.inspect_dns(&mut mission, &req, op).unwrap().unwrap();
        assert_eq!(original, ev);
        assert!(cur.view_with_authority(at, auth).routing_usable);
        assert_eq!(traj.inspect_terrain(&ev).unwrap(), "completed");
    }
    let mut later = req.clone();
    later.perceived_at += 10;
    later.expected_claim_revision = 1;
    let op2 = allocate_op();
    let (clean, _, _, _) = store
        .record_dns(&mut mission, &later, op2)
        .unwrap()
        .unwrap();
    traj.record_terrain(&clean).unwrap();
    let mut disputed = req.clone();
    disputed.answers[0].ipv4 = "198.51.100.250".into();
    assert_eq!(
        store.record_dns(&mut mission, &disputed, op),
        Err(Fail::Conflict("input_mismatch"))
    );
    let (_, current, at, auth) = store
        .inspect_dns(&mut mission, &later, op2)
        .unwrap()
        .unwrap();
    assert!(
        current.view_with_authority(at, auth).routing_usable,
        "historical identity conflict must not poison independent later applicability"
    );
    assert_eq!(traj.inspect_terrain(&clean).unwrap(), "completed");
}

#[test]
fn regression_postgres_late_conflict_repeat_and_later_qualified_recovery() {
    let _g = db();
    let (e, c, _, now) = register_m1_campaign(0x8114);
    let (mut mission, mut store, mut traj) = stores();
    let mut req = parse_req(&dns_request_json(e, c, now - 15));
    let mut events = Vec::new();
    for (revision, offset, usable) in [
        (0, -15, true),
        (1, -25, false),
        (2, -20, false),
        (3, -10, true),
    ] {
        req.expected_claim_revision = revision;
        req.perceived_at = now + offset;
        let op = allocate_op();
        let (event, current, at, auth) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();
        assert_eq!(current.view_with_authority(at, auth).routing_usable, usable);
        assert_eq!(event.claim.previous_claim_revision, revision);
        assert_eq!(
            traj.record_terrain(&event).unwrap(),
            HistoryStatus::Completed
        );
        events.push((req.clone(), event));
    }
    for (request, accepted) in &events {
        let (historical, current, at, auth) = store
            .inspect_dns(&mut mission, request, accepted.operation_id)
            .unwrap()
            .unwrap();
        assert_eq!(&historical, accepted);
        assert!(current.view_with_authority(at, auth).routing_usable);
    }
    assert!(events[1].1.claim.conflicted);
    assert!(events[2].1.claim.conflicted);
}

#[test]
fn regression_identity_conflict_frontier_blocks_an_intermediate_observation() {
    let _g = db();
    let (e, c, _, now) = register_m1_campaign(0x8115);
    let (mut mission, mut store, mut traj) = stores();
    let mut req = parse_req(&dns_request_json(e, c, now - 30));
    let op = allocate_op();
    let (original, _, _, _) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();
    traj.record_terrain(&original).unwrap();
    let mut variant = req.clone();
    variant.perceived_at = now - 10;
    variant.answers[0].ipv4 = "198.51.100.250".into();
    assert_eq!(
        store.record_dns(&mut mission, &variant, op),
        Err(Fail::Conflict("input_mismatch"))
    );
    let (_, held, _, _) = store.inspect_dns(&mut mission, &req, op).unwrap().unwrap();
    assert_eq!(
        held.temporal_frontier, variant.perceived_at,
        "inspection must expose the same hold frontier used for subsequent acceptance"
    );
    let (_, replayed, _, _) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();
    assert_eq!(replayed.temporal_frontier, variant.perceived_at);
    req.perceived_at = now - 20;
    req.expected_claim_revision = 1;
    let (between, _, _, _) = store
        .record_dns(&mut mission, &req, allocate_op())
        .unwrap()
        .unwrap();
    assert!(between.claim.conflicted);
    assert_eq!(between.claim.temporal_frontier, now - 10);
    req.perceived_at = now - 5;
    req.expected_claim_revision = 2;
    let (later, current, at, auth) = store
        .record_dns(&mut mission, &req, allocate_op())
        .unwrap()
        .unwrap();
    assert_eq!(later.claim.previous_claim_revision, 2);
    assert!(current.view_with_authority(at, auth).routing_usable);
    assert_eq!(
        traj.record_terrain(&later).unwrap(),
        HistoryStatus::Completed
    );
    assert_eq!(traj.inspect_terrain(&original).unwrap(), "anomaly");
}

#[test]
fn postgres_record_inspect_idempotent_replay_and_rotation_vs_conflict() {
    let _g = db();
    let (e, c, _reg, now) = register_m1_campaign(0x8101);
    let (mut mission, mut store, mut traj) = stores();

    let raw1 = dns_request_json(e, c, now - 10);
    let req1 = parse_req(&raw1);
    let op1 = allocate_op();

    let (ev1, cur1, ts1, auth1) = store.record_dns(&mut mission, &req1, op1).unwrap().unwrap();
    assert!(auth1);
    assert_eq!(ev1.claim.claim_revision, 1);
    assert_eq!(ev1.claim.claim_status, TerrainClaimStatus::Observed);
    assert!(ev1.claim.view_with_authority(ts1, auth1).routing_usable);

    for tbl in [
        "terrain.observations",
        "terrain.materials",
        "terrain.evaluations",
        "terrain.claim_revisions",
        "terrain.current_claims",
        "terrain.outbox",
    ] {
        assert_eq!(table_count(tbl, op1), 1, "expected 1 row in {tbl}");
    }

    assert_eq!(traj.inspect_terrain(&ev1).unwrap(), "pending");
    assert_eq!(traj.record_terrain(&ev1).unwrap(), HistoryStatus::Completed);
    assert_eq!(traj.inspect_terrain(&ev1).unwrap(), "completed");
    assert_eq!(table_count("trajectory.terrain_history", op1), 1);

    // Idempotent replay of both Terrain and Trajectory adds no rows.
    let (ev1_replay, cur1_replay, _, auth1_replay) =
        store.record_dns(&mut mission, &req1, op1).unwrap().unwrap();
    assert_eq!(ev1_replay, ev1);
    assert_eq!(cur1_replay, cur1);
    assert!(auth1_replay);
    assert_eq!(traj.record_terrain(&ev1).unwrap(), HistoryStatus::Completed);
    assert_eq!(table_count("terrain.claim_revisions", op1), 1);
    assert_eq!(table_count("trajectory.terrain_history", op1), 1);

    // Conflicting payload under same operation_id is rejected without overwrite.
    let mut mutated_raw = raw1.clone();
    mutated_raw["answers"] = json!([{"ipv4": "198.51.100.250", "ttl_seconds": 60}]);
    let mutated_req = parse_req(&mutated_raw);
    assert_eq!(
        store.record_dns(&mut mission, &mutated_req, op1),
        Err(Fail::Conflict("input_mismatch"))
    );
    assert_eq!(
        store.inspect_dns(&mut mission, &mutated_req, op1),
        Err(Fail::Conflict("input_mismatch"))
    );

    // Stale expected_claim_revision (0 when current is 1) is rejected.
    let op_stale = allocate_op();
    let mut stale_raw = dns_request_json(e, c, now - 5);
    stale_raw["expected_claim_revision"] = json!(0);
    assert_eq!(
        store.record_dns(&mut mission, &parse_req(&stale_raw), op_stale),
        Err(Fail::Conflict("claim_revision_mismatch"))
    );

    // Normal RRset rotation at later perceived_at with expected_claim_revision = 1 revises to revision 2.
    let op2 = allocate_op();
    let mut raw2 = dns_request_json(e, c, now - 5);
    raw2["expected_claim_revision"] = json!(1);
    raw2["answers"] = json!([{"ipv4": "198.51.100.20", "ttl_seconds": 60}]);
    let req2 = parse_req(&raw2);
    let (ev2, cur2, _, _) = store.record_dns(&mut mission, &req2, op2).unwrap().unwrap();
    assert_eq!(ev2.claim.claim_revision, 2);
    assert_eq!(cur2.claim_revision, 2);
    assert_eq!(ev2.claim.claim_status, TerrainClaimStatus::Observed);
    assert_eq!(traj.record_terrain(&ev2).unwrap(), HistoryStatus::Completed);

    // Inspecting op1 still returns op1's revision 1 claim alongside current_claim revision 2.
    let (insp_ev1, insp_cur1, _, _) = store
        .inspect_dns(&mut mission, &req1, op1)
        .unwrap()
        .unwrap();
    assert_eq!(insp_ev1.claim.claim_revision, 1);
    assert_eq!(insp_cur1.claim_revision, 2);

    // Same-applicability conflict (perceived_at == prior perceived_at) with expected_claim_revision = 2 suspends use.
    let op3 = allocate_op();
    let mut raw3 = raw2.clone();
    raw3["expected_claim_revision"] = json!(2);
    raw3["answers"] = json!([{"ipv4": "198.51.100.30", "ttl_seconds": 60}]);
    let req3 = parse_req(&raw3);
    let (ev3, cur3, ts3, auth3) = store.record_dns(&mut mission, &req3, op3).unwrap().unwrap();
    assert_eq!(
        ev3.evaluation.outcome,
        EvaluationOutcome::ApplicabilityConflict
    );
    assert_eq!(ev3.claim.claim_status, TerrainClaimStatus::Conflicted);
    assert!(cur3.conflicted);
    assert!(!cur3.view_with_authority(ts3, auth3).routing_usable);
}

#[test]
fn postgres_enforces_mission_policy_and_withdrawal_blocks_acceptance_and_current_view() {
    let _g = db();
    let (e, c, reg, now) = register_m1_campaign(0x8102);
    let (mut mission, mut store, _traj) = stores();

    let base = parse_req(&dns_request_json(e, c, now - 10));
    let mut bad = base.clone();
    bad.expected_mission_revision = 2;
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("stale_revision")
    );

    let mut bad = base.clone();
    bad.exercise_mode = ExerciseMode::DefenderInformed;
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("mode_mismatch")
    );

    let mut bad = base.clone();
    bad.goal_ref = GoalRef(Uuid::from_u128(0x999));
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("purpose_mismatch")
    );

    let mut bad = base.clone();
    bad.operator_ref = OperatorRef(Uuid::from_u128(0x999));
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("purpose_mismatch")
    );

    let mut bad = base.clone();
    bad.vantage_ref = Uuid::from_u128(0x999);
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("vantage_mismatch")
    );

    let mut bad = base.clone();
    bad.resolver_ipv4 = "192.0.2.99".to_string();
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("resolver_mismatch")
    );

    let mut bad = base.clone();
    bad.asset_ref = Some(AssetRef(Uuid::from_u128(0x999)));
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("asset_not_permitted")
    );

    let mut bad = base.clone();
    bad.subject_name = "api.excluded.example.invalid".to_string();
    bad.asset_ref = None;
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("name_not_permitted")
    );

    let mut bad = base.clone();
    bad.perceived_at = now + 9999;
    assert_eq!(
        verify_mission_permission(&mut mission, &bad).unwrap(),
        Err("outside_operating_window")
    );

    // Record one valid claim before withdrawal.
    let op1 = allocate_op();
    let (ev1, _, _, auth_before) = store.record_dns(&mut mission, &base, op1).unwrap().unwrap();
    assert!(auth_before);

    // Cross-campaign reuse of op1 is rejected.
    let (e2, c2, _, now2) = register_m1_campaign(0x8103);
    let req_other = parse_req(&dns_request_json(e2, c2, now2 - 10));
    assert_eq!(
        store.record_dns(&mut mission, &req_other, op1),
        Err(Fail::Conflict("operation_conflict"))
    );

    // Withdraw Mission authority for (e, c).
    let marker = policy_support::marker(&reg);
    policy_support::insert_marker(&mut runtime_client(), &marker);

    // New record after withdrawal is refused.
    let op2 = allocate_op();
    let mut after_raw = dns_request_json(e, c, now - 5);
    after_raw["expected_claim_revision"] = json!(1);
    assert_eq!(
        store
            .record_dns(&mut mission, &parse_req(&after_raw), op2)
            .unwrap(),
        Err("authority_withdrawn")
    );

    // Inspecting or replaying the pre-withdrawal operation preserves history but marks authority_active=false and routing_usable=false.
    let (insp_ev, insp_cur, insp_now, auth_after) = store
        .inspect_dns(&mut mission, &base, op1)
        .unwrap()
        .unwrap();
    assert_eq!(insp_ev, ev1);
    assert!(!auth_after);
    assert!(
        !insp_cur
            .view_with_authority(insp_now, auth_after)
            .routing_usable
    );
}

#[test]
fn postgres_least_privilege_and_trajectory_anomaly_dominance() {
    let _g = db();
    // Non-terrain roles fail qualify_terrain; non-trajectory roles fail qualify_terrain_trajectory.
    for mut bad_tr in [
        runtime_client(),
        broker_client(),
        cognition_client(),
        trajectory_client(),
    ] {
        assert_eq!(
            qualify_terrain(&mut bad_tr),
            Err(Fail::Config("unqualified_terrain_role"))
        );
        assert!(
            bad_tr
                .execute("DELETE FROM terrain.observations", &[])
                .is_err()
        );
    }
    for mut bad_tj in [
        runtime_client(),
        broker_client(),
        cognition_client(),
        terrain_client(),
    ] {
        assert_eq!(
            qualify_terrain_trajectory(&mut bad_tj),
            Err(Fail::Config("unqualified_trajectory_role"))
        );
        assert!(
            bad_tj
                .execute("SELECT trajectory.record_terrain_history('{}'::jsonb)", &[])
                .is_err()
        );
    }

    // Even dw_m1_terrain cannot DELETE or UPDATE append-only terrain tables or mutate trajectory.terrain_history.
    let mut tr = terrain_client();
    assert!(tr.execute("DELETE FROM terrain.observations", &[]).is_err());
    assert!(
        tr.execute("UPDATE terrain.observations SET perceived_at = 1", &[])
            .is_err()
    );
    assert!(
        tr.execute("DELETE FROM terrain.current_claims", &[])
            .is_err()
    );
    assert!(
        tr.execute("DELETE FROM trajectory.terrain_history", &[])
            .is_err()
    );

    // Trajectory anomaly dominance when source contract does not match tampered event.
    let (e, c, _, now) = register_m1_campaign(0x8104);
    let (mut mission, mut store, mut traj) = stores();
    let req = parse_req(&dns_request_json(e, c, now - 10));
    let op = allocate_op();
    let (ev, _, _, _) = store.record_dns(&mut mission, &req, op).unwrap().unwrap();
    assert_eq!(traj.record_terrain(&ev).unwrap(), HistoryStatus::Completed);

    let mut tampered = ev.clone();
    tampered.request_hash = "f".repeat(64);
    assert_eq!(
        traj.record_terrain(&tampered).unwrap(),
        HistoryStatus::Anomaly
    );
    assert_eq!(traj.inspect_terrain(&ev).unwrap(), "anomaly");
    assert_eq!(traj.record_terrain(&ev).unwrap(), HistoryStatus::Anomaly);
}

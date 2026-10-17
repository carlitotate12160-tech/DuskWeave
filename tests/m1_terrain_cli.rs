//! End-to-end CLI tests for m1-terrain-record, m1-terrain-inspect, and m1-terrain-recover.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_terrain.rs"]
mod terrain_support;

use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use terrain_support::*;
use uuid::Uuid;

fn write_temp_json(tag: &str, val: &Value) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("dw-terrain-cli-{tag}-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(val).unwrap()).unwrap();
    path
}

#[test]
fn regression_cli_partial_alias_is_withheld() {
    let _g = db();
    let (e, c, _, now) = register_m1_campaign(0x8310);
    let mut raw = dns_request_json(e, c, now - 10);
    raw["limitations"] = json!(["partial_alias_chain"]);
    let path = write_temp_json("partial", &raw);
    let (ok, _, receipt) = cli_run("m1-terrain-record", &path, "new", &[]);
    assert!(ok);
    assert_eq!(receipt["claim"]["routing_usable"], false);
    assert_eq!(receipt["claim"]["claim_status"], "withheld");
    let op = receipt["operation_id"].as_str().unwrap();
    let stored: Value = trajectory_client()
        .query_one(
            "SELECT contract FROM trajectory.terrain_history WHERE operation_id=$1",
            &[&Uuid::parse_str(op).unwrap()],
        )
        .unwrap()
        .get(0);
    assert_eq!(stored["material"]["limitations"], raw["limitations"]);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn regression_cli_identity_conflict_blocks_replay_inspect_recover_then_later_use_recovers() {
    let _g = db();
    let (e, c, _, now) = register_m1_campaign(0x8311);
    let raw = dns_request_json(e, c, now - 25);
    let path = write_temp_json("identity-original", &raw);
    let (ok, _, first) = cli_run("m1-terrain-record", &path, "new", &[]);
    assert!(ok);
    let op = first["operation_id"].as_str().unwrap();
    let mut bad = raw.clone();
    bad["answers"][0]["ipv4"] = json!("198.51.100.250");
    let bad_path = write_temp_json("identity-variant", &bad);
    let (ok, _, refusal) = cli_run("m1-terrain-record", &bad_path, op, &[]);
    assert!(!ok);
    assert_eq!(refusal["reason"], "input_mismatch");
    for cmd in [
        "m1-terrain-record",
        "m1-terrain-inspect",
        "m1-terrain-recover",
    ] {
        let (ok, _, receipt) = cli_run(cmd, &path, op, &[]);
        assert!(ok);
        assert_eq!(receipt["history"], "anomaly");
        assert_eq!(receipt["claim"]["routing_usable"], false);
        assert_eq!(receipt["current_claim"]["routing_usable"], false);
        for field in [
            "operation_id",
            "event_id",
            "observation_id",
            "material_id",
            "evaluation_id",
            "claim_id",
        ] {
            assert_eq!(receipt[field], first[field]);
        }
    }
    let mut later = raw.clone();
    later["perceived_at"] = json!(now - 10);
    later["expected_claim_revision"] = json!(1);
    let later_path = write_temp_json("identity-later", &later);
    let (ok, _, clean) = cli_run("m1-terrain-record", &later_path, "new", &[]);
    assert!(ok);
    assert_eq!(clean["claim"]["routing_usable"], true);
    let (_, _, historical) = cli_run("m1-terrain-inspect", &path, op, &[]);
    assert_eq!(historical["history"], "anomaly");
    assert_eq!(historical["claim"]["routing_usable"], false);
    assert_eq!(historical["current_claim"]["routing_usable"], true);
    for path in [path, bad_path, later_path] {
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn regression_cli_verified_precommit_refusal_and_failed_inspection_differ() {
    let _g = db();
    let (e, c, _, now) = register_m1_campaign(0x8312);
    let mut raw = dns_request_json(e, c, now - 10);
    raw["expected_claim_revision"] = json!(1);
    let path = write_temp_json("precommit", &raw);
    let op = allocate_op();
    let (ok, _, receipt) = cli_run("m1-terrain-record", &path, &op.0.to_string(), &[]);
    assert!(!ok);
    assert_eq!(receipt["publication"], "not_committed");
    assert_eq!(receipt["reason"], "claim_revision_mismatch");
    let count: i64 = terrain_client()
        .query_one(
            "SELECT count(*) FROM terrain.outbox WHERE operation_id=$1",
            &[&op.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    let (ok, _, receipt) = cli_run(
        "m1-terrain-inspect",
        &path,
        &op.0.to_string(),
        &[("DW_TRAJECTORY_DATABASE_URL", "invalid")],
    );
    assert!(!ok);
    assert_eq!(receipt["publication"], "unknown");
    assert_eq!(receipt["history"], "unknown");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cli_record_inspect_recover_and_idempotent_redelivery() {
    let _g = db();
    let (e, c, _reg, now) = register_m1_campaign(0x8301);
    let raw = dns_request_json(e, c, now - 10);
    let path = write_temp_json("valid", &raw);

    let (ok, _stdout, rec) = cli_run("m1-terrain-record", &path, "new", &[]);
    assert!(ok, "m1-terrain-record failed: {rec:?}");
    assert_eq!(rec["kind"], "m1_terrain_receipt_v1");
    assert_eq!(rec["result"], "accepted");
    assert_eq!(rec["reason"], "qualified_dns_claim");
    assert_eq!(rec["publication"], "committed");
    assert_eq!(rec["history"], "completed");
    assert_eq!(rec["source_kind"], "operator_reviewed_dns");
    assert_eq!(rec["claim"]["trust_tier"], "tier_1");
    assert_eq!(rec["claim"]["claim_status"], "observed");
    assert_eq!(rec["claim"]["claim_revision"], 1);
    assert_eq!(rec["claim"]["freshness"], "fresh");
    assert_eq!(rec["claim"]["authority_active"], true);
    assert_eq!(rec["claim"]["routing_usable"], true);

    // Verify all 6 identities are non-nil and mutually distinct.
    let id_fields = [
        "operation_id",
        "event_id",
        "observation_id",
        "material_id",
        "evaluation_id",
        "claim_id",
    ];
    let mut seen = HashSet::new();
    for f in id_fields {
        let s = rec[f].as_str().expect(f);
        let u = Uuid::parse_str(s).expect(f);
        assert!(!u.is_nil());
        assert!(seen.insert(u), "duplicate identity on {f}");
    }

    let op_str = rec["operation_id"].as_str().unwrap().to_string();

    // Fresh-process inspect, recover, and duplicate record preserve exact identities and source limits.
    for cmd in [
        "m1-terrain-inspect",
        "m1-terrain-recover",
        "m1-terrain-record",
    ] {
        let (ok2, _, r2) = cli_run(cmd, &path, &op_str, &[]);
        assert!(ok2, "{cmd} failed: {r2:?}");
        for f in id_fields {
            assert_eq!(r2[f], rec[f], "{cmd} changed {f}");
        }
        assert_eq!(r2["publication"], "committed");
        assert_eq!(r2["history"], "completed");
        assert_eq!(r2["source_kind"], "operator_reviewed_dns");
        assert_eq!(r2["claim"]["claim_revision"], 1);
    }

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn cli_rejects_malformed_extra_field_forged_and_unsafe_inputs_without_echo() {
    let _g = db();
    let (e, c, _reg, now) = register_m1_campaign(0x8302);
    let secret_marker = "SECRET_CANARY_DO_NOT_ECHO_99182";

    let mut extra = dns_request_json(e, c, now - 10);
    extra["raw_packet_ leak"] = json!(secret_marker);
    let p_extra = write_temp_json("extra", &extra);
    let (ok, stdout, rec) = cli_run("m1-terrain-record", &p_extra, "new", &[]);
    std::fs::remove_file(&p_extra).unwrap();
    assert!(!ok);
    assert!(
        !stdout.contains(secret_marker),
        "must never echo rejected input"
    );
    assert_eq!(rec["result"], "unavailable");
    assert_eq!(rec["reason"], "invalid_input");
    assert_eq!(rec["publication"], "not_committed");

    let mut forged = dns_request_json(e, c, now - 10);
    forged["source_kind"] = json!(secret_marker);
    let p_forged = write_temp_json("forged", &forged);
    let (ok, stdout, rec) = cli_run("m1-terrain-record", &p_forged, "new", &[]);
    std::fs::remove_file(&p_forged).unwrap();
    assert!(!ok);
    assert!(!stdout.contains(secret_marker));
    assert_eq!(rec["reason"], "invalid_input");

    let valid_path = write_temp_json("args", &dns_request_json(e, c, now - 10));
    let (ok_new_insp, _, rec_new_insp) = cli_run("m1-terrain-inspect", &valid_path, "new", &[]);
    assert!(!ok_new_insp);
    assert_eq!(rec_new_insp["reason"], "invalid_args");

    let (ok_nil, _, rec_nil) = cli_run(
        "m1-terrain-record",
        &valid_path,
        &Uuid::nil().to_string(),
        &[],
    );
    assert!(!ok_nil);
    assert_eq!(rec_nil["reason"], "invalid_args");

    let (ok_missing, _, rec_missing) = cli_run(
        "m1-terrain-inspect",
        &valid_path,
        &Uuid::from_u128(0x999999).to_string(),
        &[],
    );
    assert!(!ok_missing);
    assert_eq!(rec_missing["reason"], "operation_not_found");

    // Unqualified role configuration is rejected before domain execution.
    let rt_dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let (ok_bad_role, _, rec_bad_role) = cli_run(
        "m1-terrain-record",
        &valid_path,
        "new",
        &[("DW_TERRAIN_DATABASE_URL", &rt_dsn)],
    );
    assert!(!ok_bad_role);
    assert_eq!(rec_bad_role["result"], "unavailable");
    assert_eq!(rec_bad_role["publication"], "not_committed");

    std::fs::remove_file(&valid_path).unwrap();
}

#[test]
fn cli_handles_ttl_zero_withdrawal_and_pending_history_recovery() {
    let _g = db();
    let (e, c, reg, now) = register_m1_campaign(0x8303);

    // TTL=0 material is recorded as historical withheld claim with routing_usable = false.
    let mut ttl0 = dns_request_json(e, c, now - 10);
    ttl0["answers"] = json!([{"ipv4": "198.51.100.10", "ttl_seconds": 0}]);
    let p_ttl0 = write_temp_json("ttl0", &ttl0);
    let (ok_ttl0, _, r_ttl0) = cli_run("m1-terrain-record", &p_ttl0, "new", &[]);
    std::fs::remove_file(&p_ttl0).unwrap();
    assert!(ok_ttl0);
    assert_eq!(r_ttl0["publication"], "committed");
    assert_eq!(r_ttl0["reason"], "ttl_zero_not_usable");
    assert_eq!(r_ttl0["claim"]["claim_status"], "withheld");
    assert_eq!(r_ttl0["claim"]["freshness"], "expired");
    assert_eq!(r_ttl0["claim"]["routing_usable"], false);

    // Valid revision 2 on same claim key.
    let mut rev2 = dns_request_json(e, c, now - 5);
    rev2["expected_claim_revision"] = json!(1);
    let p_rev2 = write_temp_json("rev2", &rev2);
    let (ok_rev2, _, r_rev2) = cli_run("m1-terrain-record", &p_rev2, "new", &[]);
    assert!(ok_rev2);
    assert_eq!(r_rev2["claim"]["claim_revision"], 2);
    assert_eq!(r_rev2["claim"]["routing_usable"], true);
    let op_rev2 = r_rev2["operation_id"].as_str().unwrap().to_string();

    // Withdraw Mission authority -> new record is refused, and inspect of rev2 reports routing_usable = false.
    let marker = policy_support::marker(&reg);
    policy_support::insert_marker(&mut runtime_client(), &marker);

    let mut rev3 = dns_request_json(e, c, now - 2);
    rev3["expected_claim_revision"] = json!(2);
    let p_rev3 = write_temp_json("rev3", &rev3);
    let (ok_withdrawn, _, r_withdrawn) = cli_run("m1-terrain-record", &p_rev3, "new", &[]);
    std::fs::remove_file(&p_rev3).unwrap();
    assert!(!ok_withdrawn);
    assert_eq!(r_withdrawn["result"], "refused");
    assert_eq!(r_withdrawn["reason"], "authority_withdrawn");
    assert_eq!(r_withdrawn["publication"], "not_committed");

    let (ok_insp_after, _, r_insp_after) = cli_run("m1-terrain-inspect", &p_rev2, &op_rev2, &[]);
    std::fs::remove_file(&p_rev2).unwrap();
    assert!(ok_insp_after);
    assert_eq!(r_insp_after["publication"], "committed");
    assert_eq!(r_insp_after["claim"]["authority_active"], false);
    assert_eq!(r_insp_after["claim"]["routing_usable"], false);
    assert_eq!(r_insp_after["current_claim"]["authority_active"], false);
    assert_eq!(r_insp_after["current_claim"]["routing_usable"], false);
}

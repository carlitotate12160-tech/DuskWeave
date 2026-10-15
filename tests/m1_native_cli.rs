use std::process::Command;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_native_inference.rs"]
mod native;
#[path = "support/m1_inference_permission.rs"]
mod permission;
use serde_json::{Value, json};

fn cli_output(command: &str, input: &std::path::Path, operation: &str) -> (bool, Vec<Value>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    child
        .env_clear()
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").expect("fixture DSN"),
        );
    for variable in [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = std::env::var_os(variable) {
            child.env(variable, value);
        }
    }
    let output = child
        .arg(command)
        .arg("--input")
        .arg(input)
        .arg("--operation")
        .arg(operation)
        .output()
        .unwrap();
    assert!(output.stderr.is_empty());
    let ok = output.status.success();
    if !ok {
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("inference_authority_unavailable"),
            "expected bounded continuation refusal"
        );
    }
    let receipts: Vec<Value> = output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .filter(|line| ok || line.first() == Some(&b'{'))
        .map(|line| serde_json::from_slice(line).expect("safe JSON receipt"))
        .collect();
    (ok, receipts)
}
fn cli(command: &str, input: &std::path::Path, operation: &str) -> Vec<Value> {
    let (ok, receipts) = cli_output(command, input, operation);
    assert!(ok, "safe receipts: {receipts:?}");
    receipts
}

#[test]
fn native_missing_key_publishes_and_fresh_recovery_never_generates() {
    let _guard = native::db();
    native::setup();
    let (e, c) = db_support::scope(0x9701);
    let mut raw = permission::registration(e, c);
    let now: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    raw["starts_at"] = json!(now - 60);
    raw["ends_at"] = json!(now + 3600);
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v2");
    p["profile"] = json!("deepseek-responses-envelope32-v1");
    p["starts_at"] = json!(now - 30);
    p["ends_at"] = json!(now + 1800);
    p["operation_limits"]["json_depth"] = json!(32);
    p["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_repair_fields_codes"));
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut allocator, mut mission, mut history) = db_support::ports();
    let op = duskweave::registration::prepare_operation(&mut allocator).unwrap();
    duskweave::registration::register(&mut allocator, &mut mission, &mut history, op, &input)
        .unwrap();
    let request = json!({"version":"m1_context_prepare_request_v1","mission":{
        "version":"m1_inference_reservation_request_v1","engagement_id":e,"campaign_id":c,
        "operator_ref":raw["operator_ref"],"expected_mission_revision":1,
        "permission_id":raw["m1_inference_permission"]["permission_id"],"goal_ref":raw["goal_ref"],"exercise_mode":"blind"},"candidates":[]});
    let path = std::env::temp_dir().join(format!(
        "dw-native-{}.json",
        uuid::Uuid::from_u128(now as u128 ^ e.0.as_u128())
    ));
    std::fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
    let first = cli("m1-infer", &path, "new");
    let receipt = first.last().unwrap();
    assert_eq!(receipt["proposal_disposition"], "live_key_missing");
    assert_eq!(receipt["publication"], "committed");
    assert_eq!(receipt["history"], "completed");
    assert_eq!(receipt["dispatch_granted"], false);
    let decision = receipt["decision"].as_str().unwrap();
    for command in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let out = cli(command, &path, decision);
        let r = out.last().unwrap();
        for field in [
            "decision",
            "event",
            "proposal_disposition",
            "publication",
            "history",
        ] {
            assert_eq!(r[field], receipt[field]);
        }
        assert_eq!(r["dispatch_granted"], false);
    }
    let mut a = native::admin();
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM execution.inference_calls WHERE e=$1 AND c=$2",
            &[&e.0, &c.0]
        )
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2",
            &[&e.0, &c.0]
        )
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    assert_eq!(a.query_one("SELECT count(*) FROM trajectory.reasoning_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",&[&e.0,&c.0]).unwrap().get::<_,i64>(0),1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_commands_are_registered_and_validate_input_before_database_access() {
    for command in ["m1-infer", "m1-infer-inspect", "m1-infer-recover"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_duskweave"));
        child.env_clear().arg(command);
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            child.env("SystemRoot", root);
        }
        let output = child.output().expect("native CLI process");
        assert!(!output.status.success());
        let error = String::from_utf8(output.stdout).unwrap();
        assert!(error.contains("invalid_args"), "{command}: {error}");
        assert!(!error.contains("unknown_command"));
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn native_core_connection_rejects_direct_foreign_insert_grants() {
    let _guard = native::db();
    let (_, q) = native::registered(0x9703);
    let path = std::env::temp_dir().join(format!("dw-native-core-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(&q).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    child
        .env_clear()
        .env(
            "SystemRoot",
            std::env::var_os("SystemRoot").unwrap_or_default(),
        )
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_NATIVE_COREBAD_DATABASE_URL").unwrap(),
        )
        .args([
            "m1-infer",
            "--input",
            path.to_str().unwrap(),
            "--operation",
            "new",
        ]);
    for variable in [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = std::env::var_os(variable) {
            child.env(variable, value);
        }
    }
    let result = child.output().unwrap();
    assert!(
        !result.status.success(),
        "Core with native foreign INSERT must be rejected before operation allocation"
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("inference_database_unqualified"));
    assert!(result.stderr.is_empty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_continuation_and_anomaly_dominance() {
    let _guard = native::db();
    native::setup();
    let (e, c) = db_support::scope(0x9702);
    let mut raw = permission::registration(e, c);
    let now: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    raw["starts_at"] = json!(now - 60);
    raw["ends_at"] = json!(now + 3600);
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v2");
    p["profile"] = json!("deepseek-responses-envelope32-v1");
    p["starts_at"] = json!(now - 30);
    p["ends_at"] = json!(now + 1800);
    p["operation_limits"]["json_depth"] = json!(32);
    p["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_repair_fields_codes"));
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut allocator, mut mission, mut history) = db_support::ports();
    let op = duskweave::registration::prepare_operation(&mut allocator).unwrap();
    duskweave::registration::register(&mut allocator, &mut mission, &mut history, op, &input)
        .unwrap();
    let request = json!({"version":"m1_context_prepare_request_v1","mission":{
        "version":"m1_inference_reservation_request_v1","engagement_id":e,"campaign_id":c,
        "operator_ref":raw["operator_ref"],"expected_mission_revision":1,
        "permission_id":raw["m1_inference_permission"]["permission_id"],"goal_ref":raw["goal_ref"],"exercise_mode":"blind"},"candidates":[]});
    let path = std::env::temp_dir().join(format!(
        "dw-native-cont-{}.json",
        uuid::Uuid::from_u128(now as u128 ^ e.0.as_u128())
    ));
    std::fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
    let first = cli("m1-infer", &path, "new");
    let receipt1 = first.last().unwrap();
    assert_eq!(receipt1["publication"], "committed");
    let decision1: uuid::Uuid = receipt1["decision"].as_str().unwrap().parse().unwrap();

    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let snapshot1: Value = broker
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    // Terminal completion automatically closed decision1 hold; capacity charged 2 calls
    assert_eq!(snapshot1["active_holds"], 0);
    assert_eq!(snapshot1["charged"]["calls"], 2);

    // Second decision on the same campaign now succeeds and charges another 2 calls
    let second = cli("m1-infer", &path, "new");
    let receipt2 = second.last().unwrap();
    assert_eq!(receipt2["publication"], "committed");
    let decision2: uuid::Uuid = receipt2["decision"].as_str().unwrap().parse().unwrap();
    assert_ne!(decision1, decision2);

    let snapshot2: Value = broker
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(snapshot2["active_holds"], 0);
    assert_eq!(snapshot2["charged"]["calls"], 4);

    // Fault-inject conflicting accepted content; the immutable Cognition source
    // remains unchanged. Only its genuine redelivery may record the conflict.
    let mut admin = native::admin();
    admin
        .execute(
            "UPDATE trajectory.reasoning_history SET contract=jsonb_set(contract,'{occurred_at_ms}','1') WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 AND status='accepted'",
            &[&e.0, &c.0, &decision2],
        )
        .unwrap();
    let conflict = cli("m1-infer-recover", &path, &decision2.to_string());
    assert_eq!(conflict.last().unwrap()["history"], "anomaly");
    admin.execute("UPDATE trajectory.reasoning_history h SET contract=q.payload FROM cognition.reasoning_decisions q WHERE h.operation_id=q.d AND q.d=$1 AND h.status='accepted'",&[&decision2]).unwrap();
    let before: i64 = admin.query_one("SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",&[&e.0,&c.0]).unwrap().get(0);
    let (ok, _) = cli_output("m1-infer", &path, "new");
    assert!(
        !ok,
        "late verified conflict invalidates dependent continuation despite the stored closure"
    );
    assert_eq!(admin.query_one("SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",&[&e.0,&c.0]).unwrap().get::<_,i64>(0),before);
    assert_eq!(
        admin
            .query_one(
                "SELECT count(*) FROM execution.inference_calls WHERE e=$1 AND c=$2",
                &[&e.0, &c.0]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );

    // Closing decision2 must be refused due to anomaly dominance
    let close_err = broker.execute(
        "SELECT execution.close_inference($1,$2,$3)",
        &[&e.0, &c.0, &decision2],
    );
    assert!(close_err.is_err(), "anomaly must prevent closure");

    std::fs::remove_file(path).unwrap();
}

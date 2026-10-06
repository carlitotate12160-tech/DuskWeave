use serde_json::json;
use std::process::Command;
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_policy.rs"]
mod support;

#[test]
fn production_register_check_withdraw_and_fresh_subprocess_preserve_identity_without_query_writes()
{
    let _guard = db_support::db();
    let (e, c) = db_support::scope(1);
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let registration = support::registration(e, c, support::now());
    let op = Uuid::from_u128(0x41).to_string();
    let registered = support::file_command(
        "register",
        registration.to_string().as_bytes(),
        &["--operation", &op],
        Some(&dsn),
    );
    assert!(registered.status.success());
    assert!(registered.stderr.is_empty());
    let registered_text = String::from_utf8(registered.stdout).unwrap();
    let before = support::persisted(e, c);
    for purpose in [
        support::discovery("www.example.invalid"),
        support::contact("www.example.invalid", 0x21),
    ] {
        let q = support::query_json(e, c, purpose);
        for _ in 0..2 {
            let output =
                support::file_command("m1-policy-check", q.to_string().as_bytes(), &[], Some(&dsn));
            let receipt = support::receipt(&output, "matches_name_policy", "policy_match");
            let snapshot = &receipt["snapshot"];
            assert_eq!(receipt["engagement_id"], json!(e));
            assert_eq!(receipt["campaign_id"], json!(c));
            assert_eq!(snapshot["registration_operation_id"], op);
            assert!(registered_text.contains(snapshot["registration_event_id"].as_str().unwrap()));
            assert_eq!(snapshot["effective_mission_revision"], 1);
            assert_eq!(snapshot["authority_revision"], 17);
            assert_eq!(snapshot["policy_version"], 1);
            assert!(receipt["checked_at"].is_i64());
            assert_eq!(support::persisted(e, c), before);
        }
    }
    for (purpose, reason) in [
        (
            support::contact("www.example.invalid", 0x22),
            "name_not_permitted",
        ),
        (
            support::discovery("excluded.example.invalid"),
            "name_not_permitted",
        ),
    ] {
        let q = support::query_json(e, c, purpose);
        support::receipt(
            &support::file_command("m1-policy-check", q.to_string().as_bytes(), &[], Some(&dsn)),
            reason,
            "policy_no_match",
        );
    }
    let withdrawal = json!({"engagement_id": e, "campaign_id": c, "operator_ref": Uuid::from_u128(0x11),
        "expected_mission_revision": 1, "reason": "operator_requested"});
    let withdrawal_op = Uuid::from_u128(0x51).to_string();
    let withdrawn = support::file_command(
        "withdraw",
        withdrawal.to_string().as_bytes(),
        &["--operation", &withdrawal_op, "--recover", "false"],
        Some(&dsn),
    );
    assert!(withdrawn.status.success());
    let after = support::persisted(e, c);
    for revision in [1, 2] {
        let mut q = support::query_json(e, c, support::discovery("www.example.invalid"));
        q["expected_mission_revision"] = json!(revision);
        let receipt = support::receipt(
            &support::file_command("m1-policy-check", q.to_string().as_bytes(), &[], Some(&dsn)),
            "authority_withdrawn",
            "policy_no_match",
        );
        assert_eq!(receipt["snapshot"]["effective_mission_revision"], 2);
        assert_eq!(receipt["snapshot"]["registration_operation_id"], op);
        assert_eq!(support::persisted(e, c), after);
    }
}

#[test]
fn valid_input_config_missing_scope_and_corrupt_store_fail_with_safe_receipts() {
    let (e, c) = db_support::scope(2);
    let raw = support::query_json(e, c, support::discovery("www.example.invalid")).to_string();
    for dsn in [
        None,
        Some("SENTINEL"),
        Some("postgresql://host.invalid/db"),
        Some("postgresql://127.0.0.1/db"),
        Some("postgresql://synthetic:SENTINEL@127.0.0.1:1/db"),
    ] {
        support::receipt(
            &support::file_command("m1-policy-check", raw.as_bytes(), &[], dsn),
            "invalid_configuration",
            "unavailable",
        );
    }
    let _guard = db_support::db();
    let dsn = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    support::receipt(
        &support::file_command("m1-policy-check", raw.as_bytes(), &[], Some(&dsn)),
        "mission_missing",
        "unavailable",
    );
    let event = support::register(&support::registration(e, c, support::now()));
    let mut admin = support::admin();
    let mut bad = serde_json::to_value(&event).unwrap();
    bad["version"] = json!(99);
    admin.execute("UPDATE mission.registration_outbox SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2", &[&e.0, &c.0, &bad]).unwrap();
    let before = support::persisted(e, c);
    support::receipt(
        &support::file_command("m1-policy-check", raw.as_bytes(), &[], Some(&dsn)),
        "contract_decode",
        "unavailable",
    );
    assert_eq!(support::persisted(e, c), before);
    let admin_dsn = std::env::var("DW_TEST_ADMIN_DATABASE_URL").unwrap();
    let admin_dsn = format!(
        "{}/{}",
        admin_dsn.rsplit_once('/').unwrap().0,
        db_support::dsn("DW_TEST_DATABASE_URL")
            .get_dbname()
            .unwrap()
    );
    support::receipt(
        &support::file_command("m1-policy-check", raw.as_bytes(), &[], Some(&admin_dsn)),
        "invalid_configuration",
        "unavailable",
    );
}

#[test]
fn malformed_bounded_files_and_closed_flags_never_echo_input_or_paths() {
    let (e, c) = db_support::scope(3);
    let mut bad = support::query_json(e, c, support::discovery("www.example.invalid"));
    bad["purpose"]["discovery_disclosure"]["secret"] = json!("SENTINEL");
    for raw in [
        bad.to_string().into_bytes(),
        b"SENTINEL".to_vec(),
        vec![b' '; 16385],
        Vec::new(),
    ] {
        support::receipt(
            &support::file_command("m1-policy-check", &raw, &[], None),
            "invalid_input",
            "unavailable",
        );
    }
    let raw = support::query_json(e, c, support::discovery("www.example.invalid")).to_string();
    for extra in [&["--input", "SENTINEL"][..], &["--extra", "SENTINEL"][..]] {
        support::receipt(
            &support::file_command("m1-policy-check", raw.as_bytes(), extra, None),
            "invalid_input",
            "unavailable",
        );
    }
    for args in [
        vec!["m1-policy-check"],
        vec!["m1-policy-check", "--input"],
        vec!["m1-policy-check", "--input", "--other"],
        vec!["m1-policy-check", "--input", "SENTINEL-MISSING.json"],
        vec!["m1-policy-check", "--SENTINEL", "SENTINEL"],
    ] {
        support::receipt(&support::cli(&args, None), "invalid_input", "unavailable");
    }
    let mut bytes = raw.into_bytes();
    bytes.resize(16384, b' ');
    support::receipt(
        &support::file_command("m1-policy-check", &bytes, &[], None),
        "invalid_configuration",
        "unavailable",
    );
    bytes.push(b' ');
    support::receipt(
        &support::file_command("m1-policy-check", &bytes, &[], None),
        "invalid_input",
        "unavailable",
    );
}

#[test]
fn missing_policy_input_returns_a_non_authoritative_json_receipt() {
    let output = Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .arg("m1-policy-check")
        .env_remove("DW_DATABASE_URL")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let receipt: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(receipt["check_kind"], "m1_name_policy_snapshot_v1");
    assert_eq!(receipt["result"], "unavailable");
    for flag in [
        "current_permission",
        "dispatch_granted",
        "acquisition_qualified",
    ] {
        assert_eq!(receipt[flag], false);
    }
    assert!(output.stderr.is_empty());
}

#[test]
fn existing_command_discriminator_errors_are_preserved() {
    for (args, expected) in [
        (vec![], "error=missing_command\n"),
        (vec!["not-a-command"], "error=unknown_command\n"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_duskweave"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        assert!(output.stderr.is_empty());
    }
}

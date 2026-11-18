use std::process::Command;

#[test]
fn successor_bootstrap_hint_is_closed_before_file_or_database_access() {
    for (hint, expected) in [
        ("reasoning-bounded-v1", "error=unreadable_input"),
        ("unknown", "error=invalid_args"),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_duskweave"));
        command.env_clear();
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        let output = command
            .args([
                "m1-run",
                "--input",
                "missing-lifecycle-request.invalid",
                "--timing-profile",
                hint,
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn run_entrypoints_are_registered_and_reject_missing_input_before_database() {
    for command in ["m1-run", "m1-run-inspect", "m1-run-recover", "m1-run-stop"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_duskweave"));
        child.env_clear();
        for key in ["SystemRoot", "LLVM_PROFILE_FILE"] {
            if let Some(v) = std::env::var_os(key) {
                child.env(key, v);
            }
        }
        let out = child.arg(command).output().unwrap();
        assert!(!out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "error=invalid_args",
            "{command}"
        );
    }
}

#[test]
fn inspect_recover_and_stop_only_parse_their_broker_configuration() {
    for operation in ["m1-run-inspect", "m1-run-recover", "m1-run-stop"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_duskweave"));
        child
            .env_clear()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env("DW_DATABASE_URL", "synthetic-sensitive-core")
            .env(
                "DW_INFERENCE_BROKER_DATABASE_URL",
                "synthetic-sensitive-broker",
            );
        if let Some(root) = std::env::var_os("SystemRoot") {
            child.env("SystemRoot", root);
        }
        let output = child
            .args([operation, "--run", "00000000-0000-4000-8000-000000000001"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let expected = if operation == "m1-run-stop" {
            "{\"run_id\":\"00000000-0000-4000-8000-000000000001\",\"local_stop\":{\"stop_latched\":false,\"disposal_confirmed\":false},\"result\":\"unknown\",\"reason\":\"invalid_dsn\",\"continuation_blocked\":true}\nerror=invalid_dsn"
        } else {
            "error=invalid_dsn"
        };
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

fn synthetic_run_request(
    version: &str,
    run_profile: &str,
    inference_profile: Option<&str>,
) -> Vec<u8> {
    let id = |n: u128| uuid::Uuid::from_u128(n).to_string();
    let mut doc = serde_json::json!({
        "version": version,
        "run_id": id(0x10),
        "run_profile": run_profile,
        "execution_mode": duskweave::m1_run::FIXTURE_MODE,
        "deadline_at_ms": 1_800_000_000_000i64,
        "context": {
            "version": "m1_context_prepare_request_v1",
            "mission": {
                "version": "m1_inference_reservation_request_v1",
                "engagement_id": id(0x20),
                "campaign_id": id(0x30),
                "operator_ref": id(0x40),
                "expected_mission_revision": 1,
                "permission_id": id(0x50),
                "goal_ref": id(0x60),
                "exercise_mode": "blind"
            },
            "candidates": [{"asset_ref": id(0x70), "priority": "high"}]
        },
        "seed_names": [{"asset_ref": id(0x70), "name": "target.example.invalid"}]
    });
    if let Some(profile) = inference_profile {
        doc["inference_profile"] = serde_json::json!(profile);
    }
    serde_json::to_vec(&doc).unwrap()
}

#[test]
fn version_profile_matrix_keeps_legacy_and_https_loop_ingress_closed() {
    use duskweave::Fail;
    use duskweave::m1_https::VERSION_HTTPS_LOOP;
    use duskweave::m1_inference_profile::FIXTURE_PRO_V2_PROFILE;
    use duskweave::m1_run::{FIXTURE_PROFILE, PROVIDER_FIXTURE_PROFILE, RunRequest};

    for (version, run_profile, inference_profile) in [
        ("m1_autonomous_run_request_v1", FIXTURE_PROFILE, None),
        (
            "m1_autonomous_run_request_v1",
            PROVIDER_FIXTURE_PROFILE,
            None,
        ),
        (
            VERSION_HTTPS_LOOP,
            VERSION_HTTPS_LOOP,
            Some(FIXTURE_PRO_V2_PROFILE),
        ),
    ] {
        let raw = synthetic_run_request(version, run_profile, inference_profile);
        assert!(
            RunRequest::parse(&raw).is_ok(),
            "{version}/{run_profile}/{inference_profile:?}"
        );
    }

    for (version, run_profile, inference_profile) in [
        ("m1_autonomous_run_request_v1", VERSION_HTTPS_LOOP, None),
        (
            "m1_autonomous_run_request_v1",
            FIXTURE_PROFILE,
            Some(FIXTURE_PRO_V2_PROFILE),
        ),
        (
            "m1_autonomous_run_request_v1",
            FIXTURE_PROFILE,
            Some("unknown-inference-profile"),
        ),
        ("m1_autonomous_run_request_v1", "unknown-run-profile", None),
        (VERSION_HTTPS_LOOP, VERSION_HTTPS_LOOP, None),
        (
            VERSION_HTTPS_LOOP,
            VERSION_HTTPS_LOOP,
            Some("deepseek-responses-low-v1"),
        ),
        (
            VERSION_HTTPS_LOOP,
            FIXTURE_PROFILE,
            Some(FIXTURE_PRO_V2_PROFILE),
        ),
    ] {
        let raw = synthetic_run_request(version, run_profile, inference_profile);
        let err = RunRequest::parse(&raw).err();
        assert_eq!(
            err,
            Some(Fail::Input("invalid_run_request")),
            "{version}/{run_profile}/{inference_profile:?}"
        );
    }
}

#[test]
fn legacy_v1_with_https_loop_profile_rejects_through_real_cli_before_database() {
    struct TempFile(std::path::PathBuf);
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    let path = std::env::temp_dir().join(format!(
        "dw-m1-run-cli-v1-https-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        synthetic_run_request(
            "m1_autonomous_run_request_v1",
            duskweave::m1_https::VERSION_HTTPS_LOOP,
            None,
        ),
    )
    .unwrap();
    let _cleanup = TempFile(path.clone());

    let mut command = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    command.env_clear();
    for key in ["SystemRoot", "LLVM_PROFILE_FILE"] {
        if let Some(v) = std::env::var_os(key) {
            command.env(key, v);
        }
    }
    let output = command
        .args(["m1-run", "--input"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "error=invalid_run_request"
    );
}

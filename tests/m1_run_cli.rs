use std::process::Command;

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
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "error=invalid_dsn"
        );
    }
}

//! The launcher entrypoint refuses an ordinary caller before admission.
#[cfg(target_os = "linux")]
#[test]
fn ordinary_launcher_invocation_exits_with_refusal_and_no_output() {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_dw-boundary-launcher"));
    command.env_clear().stdin(std::process::Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

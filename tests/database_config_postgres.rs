use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

#[path = "support/registration_db.rs"]
mod db_support;

fn command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    let (engagement, campaign) = db_support::scope(91);
    cmd.args([
        "inspect",
        "--engagement",
        &engagement.to_string(),
        "--campaign",
        &campaign.to_string(),
    ]);
    cmd
}

fn run(cmd: &mut Command) -> Output {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("database configuration child exceeded supervision bound");
        }
        std::thread::yield_now();
    }
}

fn assert_result(output: Output, success: bool, expected: &str) {
    assert_eq!(output.status.success(), success);
    assert!(
        output.stdout == expected.as_bytes(),
        "bounded receipt mismatch"
    );
    assert!(output.stderr.is_empty());
}

fn runtime_dsn() -> String {
    std::env::var("DW_TEST_DATABASE_URL").unwrap_or_else(|_| panic!("runtime test source required"))
}

#[test]
fn explicit_environment_reaches_the_restricted_database_consumer() {
    let _guard = db_support::db();
    let mut runtime = db_support::runtime_client();
    assert!(duskweave::postgres_mission::qualify_runtime(&mut runtime).is_ok());
    assert_result(
        run(command()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env("DW_DATABASE_URL", runtime_dsn())),
        true,
        "inspect result=empty\n",
    );
}

#[test]
fn environment_bootstrap_preserves_role_durability_and_tls_refusal() {
    let _guard = db_support::db();
    let admin = std::env::var("DW_TEST_ADMIN_DATABASE_URL").unwrap();
    assert_result(
        run(command()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env("DW_DATABASE_URL", admin)),
        false,
        "error=unqualified_runtime\n",
    );
    for (option, expected) in [
        (
            "options=-c%20synchronous_commit%3Doff",
            "error=unqualified_runtime\n",
        ),
        ("sslmode=require", "error=connect_failed\n"),
    ] {
        let dsn = runtime_dsn();
        let separator = if dsn.contains('?') { '&' } else { '?' };
        assert_result(
            run(command()
                .env("DW_DATABASE_CONFIG_MODE", "env-local")
                .env("DW_DATABASE_URL", format!("{dsn}{separator}{option}"))),
            false,
            expected,
        );
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CredentialFile(PathBuf);

    impl CredentialFile {
        fn new(dsn: &str, mode: u32) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "dw-db-config-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
            let file = Self(dir.join("source"));
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(mode)
                .open(&file.0)
                .unwrap()
                .write_all(dsn.as_bytes())
                .unwrap();
            file
        }

        fn command(&self) -> Command {
            let mut cmd = command();
            cmd.env("DW_DATABASE_URL_FILE", &self.0);
            cmd
        }
    }

    impl Drop for CredentialFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_dir(self.0.parent().unwrap());
        }
    }

    #[test]
    fn protected_file_reaches_real_database_without_environment_dsn() {
        let _guard = db_support::db();
        for mode in [0o400, 0o600] {
            let source = CredentialFile::new(&format!(" \n{}\n\t", runtime_dsn()), mode);
            let mut cmd = source.command();
            assert!(cmd.get_envs().all(|(key, _)| key != "DW_DATABASE_URL"));
            assert_result(run(&mut cmd), true, "inspect result=empty\n");
            assert_result(
                run(cmd.env("DW_DATABASE_CONFIG_MODE", "file")),
                true,
                "inspect result=empty\n",
            );
        }
    }

    #[test]
    fn protected_file_preserves_runtime_qualification_and_tls_requirement() {
        let _guard = db_support::db();
        let source =
            CredentialFile::new(&std::env::var("DW_TEST_ADMIN_DATABASE_URL").unwrap(), 0o600);
        assert_result(
            run(&mut source.command()),
            false,
            "error=unqualified_runtime\n",
        );
        let dsn = runtime_dsn();
        let separator = if dsn.contains('?') { '&' } else { '?' };
        for (option, expected) in [
            (
                "options=-c%20synchronous_commit%3Doff",
                "error=unqualified_runtime\n",
            ),
            ("sslmode=require", "error=connect_failed\n"),
        ] {
            let source = CredentialFile::new(&format!("{dsn}{separator}{option}"), 0o600);
            assert_result(run(&mut source.command()), false, expected);
        }
    }
}

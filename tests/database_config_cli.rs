use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const SECRET: &str = "dw_synthetic_config_secret";
const DSN: &str =
    "host=127.0.0.1 user=synthetic password=dw_synthetic_config_secret dbname=synthetic";

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
    cmd.args([
        "inspect",
        "--engagement",
        "00000000-0000-0000-0000-000000000001",
        "--campaign",
        "00000000-0000-0000-0000-000000000002",
    ]);
    cmd
}

fn run(cmd: &mut Command) -> Output {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("configuration child exceeded supervision bound");
        }
        std::thread::yield_now();
    }
}

fn refused(cmd: &mut Command, category: &str) {
    let output = run(cmd);
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains(SECRET));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(SECRET));
    assert_eq!(output.stdout, format!("error={category}\n").as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn implicit_environment_credentials_are_refused() {
    refused(
        command().env("DW_DATABASE_URL", DSN),
        "database_config_conflict",
    );
}

#[test]
fn hostaddr_loopback_override_is_refused() {
    let listener = std::net::TcpListener::bind("127.0.0.2:0").unwrap();
    let dsn = format!(
        "{DSN} hostaddr=127.0.0.2 port={}",
        listener.local_addr().unwrap().port()
    );
    drop(listener);
    refused(
        command()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env("DW_DATABASE_URL", dsn),
        "non_loopback_host",
    );
}

fn rejected_dsns() -> Vec<(&'static str, &'static str)> {
    vec![
        ("dw_synthetic_config_secret", "invalid_dsn"),
        (
            "host=localhost user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "host=10.0.0.9 user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "host=/tmp user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "host=127.0.0.1,127.0.0.1 user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "host=127.0.0.1 hostaddr=127.0.0.1 user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "host=::1 hostaddr=::1 user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "hostaddr=127.0.0.1 user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "user=u password=dw_synthetic_config_secret",
            "non_loopback_host",
        ),
        (
            "host=127.0.0.1 password=dw_synthetic_config_secret",
            "missing_credentials",
        ),
        (
            "host=127.0.0.1 user='' password=dw_synthetic_config_secret",
            "missing_credentials",
        ),
        ("host=127.0.0.1 user=u", "missing_credentials"),
        ("host=127.0.0.1 user=u password=''", "missing_credentials"),
    ]
}

#[test]
fn environment_profile_refuses_bad_dsns_without_disclosure() {
    for (dsn, category) in rejected_dsns() {
        refused(
            command()
                .env("DW_DATABASE_CONFIG_MODE", "env-local")
                .env("DW_DATABASE_URL", dsn),
            category,
        );
    }
    refused(
        command()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env("DW_DATABASE_URL", "x".repeat(16 * 1024 + 1)),
        "database_config_oversize",
    );
}

#[test]
fn profiles_and_sources_are_explicit_and_exclusive() {
    for mode in ["", "FILE", "env", " env-local", SECRET] {
        refused(
            command().env("DW_DATABASE_CONFIG_MODE", mode),
            "invalid_database_config_mode",
        );
    }
    for mode in [None, Some("file")] {
        for dsn in ["", DSN] {
            let mut cmd = command();
            if let Some(mode) = mode {
                cmd.env("DW_DATABASE_CONFIG_MODE", mode);
            }
            refused(cmd.env("DW_DATABASE_URL", dsn), "database_config_conflict");
        }
    }
    for path in ["", "synthetic-file"] {
        refused(
            command()
                .env("DW_DATABASE_CONFIG_MODE", "env-local")
                .env("DW_DATABASE_URL", DSN)
                .env("DW_DATABASE_URL_FILE", path),
            "database_config_conflict",
        );
    }
    refused(
        command().env("DW_DATABASE_CONFIG_MODE", "env-local"),
        "missing_env",
    );
    for dsn in ["", " \t\n"] {
        refused(
            command()
                .env("DW_DATABASE_CONFIG_MODE", "env-local")
                .env("DW_DATABASE_URL", dsn),
            "missing_env",
        );
    }
}

#[test]
fn non_unicode_configuration_is_refused() {
    #[cfg(unix)]
    let bad = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![0xff])
    };
    #[cfg(windows)]
    let bad = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0xd800])
    };
    refused(
        command().env("DW_DATABASE_CONFIG_MODE", &bad),
        "invalid_database_config_mode",
    );
    refused(
        command()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env("DW_DATABASE_URL", &bad),
        "missing_env",
    );
    refused(
        command().env("DW_DATABASE_URL", &bad),
        "database_config_conflict",
    );
    refused(
        command()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env("DW_DATABASE_URL_FILE", &bad),
        "database_config_conflict",
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
fn file_mode_is_unqualified_without_read_or_fallback() {
    let path = std::env::temp_dir().join("dw-config-file-not-read");
    for mode in [None, Some("file")] {
        let mut cmd = command();
        if let Some(mode) = mode {
            cmd.env("DW_DATABASE_CONFIG_MODE", mode);
        }
        refused(
            cmd.env("DW_DATABASE_URL_FILE", &path),
            "database_file_platform_unqualified",
        );
    }
    refused(&mut command(), "database_file_platform_unqualified");
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Source(PathBuf);

    impl Source {
        fn new(bytes: &[u8], mode: u32) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "dw-config-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
            let source = Self(dir);
            std::fs::write(source.path(), bytes).unwrap();
            std::fs::set_permissions(source.path(), std::fs::Permissions::from_mode(mode)).unwrap();
            source
        }

        fn path(&self) -> PathBuf {
            self.0.join("source")
        }

        fn command(&self) -> Command {
            let mut cmd = command();
            cmd.env("DW_DATABASE_URL_FILE", self.path());
            cmd
        }
    }

    impl Drop for Source {
        fn drop(&mut self) {
            for name in ["source", "link", "alias"] {
                let _ = std::fs::remove_file(self.0.join(name));
            }
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    #[test]
    fn file_paths_and_unreadable_sources_are_refused() {
        refused(&mut command(), "missing_database_url_file");
        for path in ["", "relative/source"] {
            refused(
                command().env("DW_DATABASE_URL_FILE", path),
                "database_file_absolute_required",
            );
        }
        let source = Source::new(DSN.as_bytes(), 0o600);
        refused(
            command().env("DW_DATABASE_URL_FILE", source.0.join("absent")),
            "database_file_open_failed",
        );
        refused(
            command().env("DW_DATABASE_URL_FILE", &source.0),
            "database_file_unsafe",
        );
        std::fs::set_permissions(source.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
        refused(&mut source.command(), "database_file_open_failed");
    }

    #[test]
    fn file_size_encoding_and_dsn_policy_are_bounded() {
        let source = Source::new(&vec![b'x'; 16 * 1024 + 1], 0o600);
        refused(&mut source.command(), "database_config_oversize");
        let source = Source::new(&[0xff], 0o600);
        refused(&mut source.command(), "database_file_invalid_utf8");
        let source = Source::new(b" \t\n", 0o600);
        refused(&mut source.command(), "invalid_dsn");
        for (dsn, category) in rejected_dsns() {
            let source = Source::new(dsn.as_bytes(), 0o600);
            refused(&mut source.command(), category);
        }
        for mode in [0o400, 0o600] {
            let source = Source::new(SECRET.as_bytes(), mode);
            refused(&mut source.command(), "invalid_dsn");
        }
    }

    #[test]
    fn file_permission_special_bits_links_and_fifo_are_refused() {
        for mode in [
            0o000, 0o200, 0o440, 0o640, 0o644, 0o700, 0o4600, 0o2600, 0o1600,
        ] {
            let source = Source::new(DSN.as_bytes(), mode);
            if mode & 0o400 == 0 {
                refused(&mut source.command(), "database_file_open_failed");
            } else {
                refused(&mut source.command(), "database_file_unsafe");
            }
        }
        let source = Source::new(DSN.as_bytes(), 0o600);
        std::fs::hard_link(source.path(), source.0.join("link")).unwrap();
        refused(&mut source.command(), "database_file_unsafe");
        let source = Source::new(DSN.as_bytes(), 0o600);
        symlink(source.path(), source.0.join("link")).unwrap();
        refused(
            command().env("DW_DATABASE_URL_FILE", source.0.join("link")),
            "database_file_open_failed",
        );
        symlink(&source.0, source.0.join("alias")).unwrap();
        refused(
            command().env("DW_DATABASE_URL_FILE", source.0.join("alias/source")),
            "database_file_open_failed",
        );
        let source = Source::new(DSN.as_bytes(), 0o600);
        std::fs::remove_file(source.path()).unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            source.path(),
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .unwrap();
        refused(&mut source.command(), "database_file_unsafe");
    }

    #[test]
    fn foreign_owner_is_refused() {
        let source = Source::new(SECRET.as_bytes(), 0o600);
        let mut chown = Command::new("sudo");
        chown
            .args(["-n", "chown", "65534", "--"])
            .arg(source.path());
        assert!(
            run(&mut chown).status.success(),
            "foreign-owner fixture setup required"
        );
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(source.path()).unwrap().uid(), 65534);
        refused(&mut source.command(), "database_file_open_failed");
    }
}

    use super::*;
    use std::process::{Command, Stdio};
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            use std::os::unix::fs::DirBuilderExt;
            let id = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
            let path = std::env::temp_dir().join(format!("dw-stop-{}", id.trim()));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.path().join("stop.sock"));
            let _ = std::fs::remove_file(self.path().join("entered"));
            let _ = std::fs::remove_dir(self.path());
        }
    }
    #[test]
    fn full_stop_ingress_backlog_cannot_block_the_host_fallback() {
        let directory = Directory::new();
        let path = directory.path().join("stop.sock");
        let listener = UnixListener::bind(&path).unwrap();
        rustix::net::listen(&listener, 0).unwrap();
        let _held = UnixStream::connect(&path).unwrap();
        let marker = directory.path().join("entered");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "m1_effect_control::session::tests::backlog_probe",
                "--ignored",
                "--test-threads=1",
            ])
            .env("DW_TEST_STOP_INGRESS_PATH", &path)
            .env("DW_TEST_STOP_INGRESS_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let entered = Instant::now() + Duration::from_secs(2);
        while !marker.exists() && Instant::now() < entered {
            std::thread::sleep(Duration::from_millis(2));
        }
        let end = Instant::now() + Duration::from_millis(100);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= end {
                break None;
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        if status.is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(marker.exists(), "probe entered actual ingress connector");
        assert!(
            status.is_some_and(|s| s.success()),
            "full ingress backlog must refuse promptly for the fixed host fallback"
        );
    }
    #[test]
    #[ignore = "bounded subprocess probe for the ingress backlog regression"]
    fn backlog_probe() {
        let Some(path) = std::env::var_os("DW_TEST_STOP_INGRESS_PATH") else {
            return;
        };
        let marker = std::env::var_os("DW_TEST_STOP_INGRESS_MARKER").unwrap();
        std::fs::write(marker, b"entered").unwrap();
        assert!(connect(Path::new(&path)).is_err());
    }

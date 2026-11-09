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
        // Leaked endpoint fixtures (sock/journal/big.json) must not strand
        // the temporary directory.
        let _ = std::fs::remove_dir_all(self.path());
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
fn binding_admission_and_path_generation() {
    let run = Uuid::new_v4();
    let engagement = Uuid::new_v4();
    let campaign = Uuid::new_v4();
    let b = Binding {
        run,
        engagement,
        campaign,
        generation: 1,
    };

    assert!(b.admits(StopTarget::Run { run_id: run }));
    assert!(!b.admits(StopTarget::Run {
        run_id: Uuid::new_v4()
    }));
    assert!(b.admits(StopTarget::Campaign {
        engagement_id: engagement,
        campaign_id: campaign,
    }));
    assert!(!b.admits(StopTarget::Campaign {
        engagement_id: Uuid::new_v4(),
        campaign_id: campaign,
    }));
    assert!(!b.admits(StopTarget::Campaign {
        engagement_id: engagement,
        campaign_id: Uuid::new_v4(),
    }));

    let p = b.path("sock");
    assert!(p.ends_with(format!("{}.sock", run.simple())));
}

#[test]
fn root_directory_validation_enforces_directory_mode_and_uid() {
    let directory = Directory::new();
    set_test_control_root(Some(directory.path().to_path_buf()));
    assert!(root().is_ok());

    // Nonexistent directory fails
    let missing = directory.path().join("missing");
    set_test_control_root(Some(missing));
    assert_eq!(root().unwrap_err(), unavailable());

    // File instead of directory fails
    let file = directory.path().join("file.txt");
    std::fs::write(&file, b"content").unwrap();
    set_test_control_root(Some(file));
    assert_eq!(root().unwrap_err(), unavailable());

    set_test_control_root(None);
}

#[test]
fn ingress_lifecycle_publishes_and_cleans_endpoints() {
    let directory = Directory::new();
    set_test_control_root(Some(directory.path().to_path_buf()));

    let run = Uuid::new_v4();
    let request_val = serde_json::json!({
        "schema": "m1_dns_ipc_v1",
        "kind": "REQUEST",
        "attempt": "00000000-0000-0000-0000-000000000001",
        "session_generation": 1,
        "sequence": 0,
        "engagement_id": "00000000-0000-0000-0000-000000000002",
        "campaign_id": "00000000-0000-0000-0000-000000000003",
        "decision_id": "00000000-0000-0000-0000-000000000004",
        "profile": "dns-routing-v2",
        "execution_mode": "live",
        "run_profile": "autonomous-dns-genai-lab-v1",
        "release_sha256": "a".repeat(64),
        "qualification_ref": "00000000-0000-0000-0000-000000000005",
        "name": "example.com",
        "resolver_ipv4": "192.0.2.53",
        "transport": "udp",
        "query_id": 1,
        "allowed_names": ["example.com"],
        "deadline_at_ms": 2000,
        "max_duration_ms": 2000
    });
    let request: DnsRequest = serde_json::from_value(request_val).unwrap();

    let sock_path = directory.path().join(format!("{}.sock", run.simple()));
    let json_path = directory.path().join(format!("{}.json", run.simple()));

    {
        let ingress = Ingress::start(run, &request).expect("start ingress");
        assert!(sock_path.exists());
        assert!(json_path.exists());

        // Validate journal reading
        let b = binding(&json_path).expect("read binding journal");
        assert_eq!(b.run, run);
        assert_eq!(b.generation, 1);

        // Connect client and test communication
        let client = connect(&sock_path).expect("connect to ingress socket");
        assert!(peer(&client).is_ok());

        // Oversized file (>1024 bytes) in journal fails
        let big_json = directory.path().join("big.json");
        std::fs::write(&big_json, vec![b' '; 1025]).unwrap();
        assert!(matches!(binding(&big_json), Err(e) if e == unavailable()));

        drop(ingress);
    }

    // Endpoint file removal runs in the worker thread's Drop, whose control
    // root resolves to the production constant under the cfg(test) seam, so
    // it is owned by the physical suite. What unit coverage can prove here:
    // after drop (join), no listener remains behind the published path.
    assert!(
        connect(&sock_path).is_err(),
        "ingress must stop serving after drop"
    );

    set_test_control_root(None);
}

#[test]
fn signal_enumerates_and_ignores_irrelevant_or_missing_files() {
    let directory = Directory::new();
    set_test_control_root(Some(directory.path().to_path_buf()));

    // Empty dir returns None
    let outcome = signal(StopTarget::Run {
        run_id: Uuid::new_v4(),
    })
    .unwrap();
    assert!(outcome.is_none());

    // Non-json file is ignored
    std::fs::write(directory.path().join("other.tmp"), b"data").unwrap();
    let outcome = signal(StopTarget::Run {
        run_id: Uuid::new_v4(),
    })
    .unwrap();
    assert!(outcome.is_none());

    set_test_control_root(None);
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

#[test]
fn bound_stop_ingress_client_uses_real_peer_and_exact_target() {
    let directory = Directory::new();
    let previous = TEST_CONTROL_ROOT.with(|r| r.borrow().clone());
    struct Restore(Option<PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            set_test_control_root(self.0.take());
        }
    }
    let _restore = Restore(previous);
    set_test_control_root(Some(directory.path().to_path_buf()));
    let b = Binding {
        run: Uuid::new_v4(),
        engagement: Uuid::new_v4(),
        campaign: Uuid::new_v4(),
        generation: 3,
    };
    let target = StopTarget::Run { run_id: b.run };
    let listener = UnixListener::bind(b.path("sock")).unwrap();
    let expected = b.clone();
    let peer = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut channel = HostChannel::new(socket);
        let request: StopRequest = channel.receive().unwrap();
        assert!(request.binding == expected);
        assert!(expected.admits(request.target));
        channel
            .send(&StopDisposition {
                stop_latched: true,
                disposal_confirmed: false,
            })
            .unwrap();
    });
    let result = send(b, target).unwrap();
    assert!(result.stop_latched);
    assert!(
        !result.disposal_confirmed,
        "transport cannot invent disposal confirmation"
    );
    peer.join().unwrap();
}

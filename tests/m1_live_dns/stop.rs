//! Private launcher consumer tests; not linked into the ordinary core.
use super::*;
#[test]
#[ignore = "requires the packet-owned Linux host and namespace privileges; preparation only"]
fn owned_lab_pair_prepares_without_start_authority() {
    prepare_closed_pair(Fault::None);
}
#[test]
#[ignore = "requires the packet-owned Linux host; journal-loss fault, no START"]
fn owned_lab_pair_missing_journal_cannot_ack_with_workers_remaining() {
    prepare_closed_pair(Fault::JournalLoss);
}
#[test]
#[ignore = "requires the packet-owned Linux host; confirmation-loss fault, no START"]
fn owned_lab_pair_unconfirmed_reap_remains_unresolved_across_stop() {
    prepare_closed_pair(Fault::ReapUnknown);
}
#[derive(Clone, Copy, PartialEq)]
enum Fault {
    None,
    JournalLoss,
    ReapUnknown,
}
fn prepare_closed_pair(fault: Fault) {
    // No v6 consent or release is supplied. This test cannot call START.
    let raw = private_raw().unwrap();
    let attempt = uuid::Uuid::parse_str(
        std::fs::read_to_string("/proc/sys/kernel/random/uuid")
            .unwrap()
            .trim(),
    )
    .unwrap();
    let now = super::super::qualification::now_ms().unwrap();
    let request = DnsRequest {
        schema: "m1_dns_ipc_v1".into(),
        kind: "REQUEST".into(),
        attempt,
        session_generation: 1,
        sequence: 0,
        engagement_id: attempt,
        campaign_id: attempt,
        decision_id: attempt,
        profile: "dns-routing-v2".into(),
        execution_mode: "live".into(),
        run_profile: "autonomous-dns-genai-lab-v1".into(),
        release_sha256: "a".repeat(64),
        qualification_ref: attempt,
        name: "seed.duskweave.test".into(),
        resolver_ipv4: "192.0.2.53".into(),
        transport: "udp".into(),
        query_id: 1,
        allowed_names: vec!["seed.duskweave.test".into()],
        deadline_at_ms: now + 10000,
        max_duration_ms: 2000,
    };
    let image = super::super::docker::metadata(&[
        "image",
        "inspect",
        "--format",
        "{{.Id}}",
        "dw-live-dns-20261030:candidate",
    ])
    .unwrap();
    let session = super::super::attempt::open(attempt, &request).unwrap();
    let started = Instant::now();
    let mut prepared = Pair::prepare(image.trim(), &session, raw);
    let elapsed = started.elapsed();
    let error = prepared.as_ref().err().copied();
    if fault == Fault::JournalLoss && prepared.is_ok() {
        std::fs::remove_file(
            Path::new("/run/duskweave/launcher")
                .join(attempt.simple().to_string())
                .join("session.json"),
        )
        .unwrap();
    }
    if let (Fault::ReapUnknown, Ok(pair)) = (fault, prepared.as_mut()) {
        // The physical child is still killed/reaped. Only the confirmation is withheld.
        pair.reap_confirmation_unknown = true;
    }
    let disposed = match prepared.as_mut() {
        Ok(pair) => pair.dispose(),
        Err(_) => super::super::attempt::dispose(attempt),
    };
    let remaining = super::super::docker::metadata(&[
        "ps",
        "--all",
        "--filter",
        &format!("label=org.duskweave.attempt={attempt}"),
        "--format",
        "{{.ID}}",
    ])
    .unwrap();
    if fault == Fault::ReapUnknown {
        let dir = Path::new("/run/duskweave/launcher").join(attempt.simple().to_string());
        assert!(!disposed);
        assert!(dir.join("session.json").exists());
        assert!(dir.join("transport.pending").exists());
        assert!(
            !super::super::attempt::stop(attempt, attempt, 1),
            "a later STOP must not reinterpret unconfirmed transport as zero resources"
        );
        assert!(
            prepared
                .as_mut()
                .unwrap()
                .process
                .try_wait()
                .unwrap()
                .is_some()
        );
        // Only this test observer possesses the withheld proof for its own child.
        assert!(super::super::attempt::transport_reaped(attempt));
    }
    // Test ownership permits exact cleanup even when the assertion exposes false success.
    let cleaned = super::super::attempt::dispose(attempt);
    drop(prepared);
    eprintln!(
        "pair_preparation_ms={} start_authority=false disposal_confirmed={} result={error:?}",
        elapsed.as_millis(),
        disposed
    );
    assert!(error.is_none(), "pair preparation failed: {error:?}");
    assert!(cleaned, "test-owned resources must be disposed");
    assert!(
        !disposed || remaining.trim().is_empty(),
        "a successful ACK cannot leave the known capture/exporter pair running"
    );
    if fault == Fault::None {
        assert!(disposed);
    }
}

#[test]
fn partial_ready_cannot_refresh_the_absolute_read_deadline() {
    let (source, mut peer) = UnixStream::pair().unwrap();
    let mut source = TimedSocket {
        io: source,
        end: Instant::now() + Duration::from_millis(5000),
    };
    peer.write_all(&[0, 0]).unwrap();
    let mut prefix = [0; 2];
    source.read_exact(&mut prefix).unwrap();
    source.end = Instant::now() - Duration::from_millis(1);
    peer.write_all(&[0, 1, b'{']).unwrap();
    assert!(
        source.read_exact(&mut prefix).is_err(),
        "partial READY must not buy another timeout"
    );
}

#[test]
fn failed_descriptor_restore_kills_and_reaps_the_spawned_child() {
    let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    let pid = child.id();
    assert!(
        restored_child(
            Ok(child),
            Err(Fail::Config("live_host_unqualified")),
            Instant::now() + Duration::from_millis(1000)
        )
        .is_err()
    );
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "child must be reaped before the failure returns"
    );
}

#[test]
fn capture_configuration_uses_its_non_root_identity() {
    let unique = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
    let dir = std::env::temp_dir().join(format!("dw-oci-{}", unique.trim()));
    std::fs::create_dir(&dir).unwrap();
    config(&dir, "/dw-dns-capture").unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("process.json")).unwrap()).unwrap();
    std::fs::remove_file(dir.join("process.json")).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    assert_eq!(
        value["user"]["uid"], 65532,
        "capture must use the UID required by its actual Go entrypoint"
    );
}
#[test]
fn raw_relay_rejects_truncation_and_extra_frames_without_interpreting_payload() {
    let mut framed = 3u32.to_be_bytes().to_vec();
    framed.extend_from_slice(b"raw");
    let mut sink = Vec::new();
    forward_raw(&mut framed.as_slice(), &mut sink).unwrap();
    assert_eq!(sink, framed);
    for mut malformed in [&framed[..5], &[framed.as_slice(), b"x"].concat()] {
        assert!(forward_raw(&mut malformed, &mut Vec::new()).is_err());
    }
}

#[test]
#[ignore = "requires the packet-owned root journal; the held run guard is a deterministic barrier"]
fn held_run_guard_prevents_stop_from_latching_a_release_in_flight() {
    use std::os::unix::fs::OpenOptionsExt;
    let run = uuid::Uuid::parse_str(
        std::fs::read_to_string("/proc/sys/kernel/random/uuid")
            .unwrap()
            .trim(),
    )
    .unwrap();
    let root = Path::new("/run/duskweave/launcher");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(format!("run-guard-{}", run.simple())))
        .unwrap();
    rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive).unwrap();
    // Keep the barrier held until the bounded STOP has returned. No scheduler sleep
    // or Docker latency can release it early.
    let stopped = std::thread::spawn(move || super::super::attempt::stop_run(run))
        .join()
        .unwrap();
    assert_eq!(
        stopped,
        (false, false),
        "STOP cannot latch through a held run-release guard"
    );
    assert!(!root.join(format!("run-stop-{}", run.simple())).exists());
    rustix::fs::flock(&file, rustix::fs::FlockOperation::Unlock).unwrap();
    assert!(super::super::attempt::stop_run(run).0);
    assert!(root.join(format!("run-stop-{}", run.simple())).exists());
}

#[test]
fn relay_raw_framing_and_copy() {
    use std::io::Cursor;

    // 1. raw_prefix valid
    let mut framed = (5u32).to_be_bytes().to_vec();
    framed.extend_from_slice(b"hello");
    let (prefix, count) = raw_prefix(&mut Cursor::new(&framed)).unwrap();
    assert_eq!(count, 5);
    assert_eq!(prefix, (5u32).to_be_bytes());

    // 2. raw_prefix invalid (count 0)
    assert!(raw_prefix(&mut Cursor::new([0, 0, 0, 0])).is_err());

    // 3. raw_prefix invalid (count > MAX_FRAME)
    assert!(raw_prefix(&mut Cursor::new((65537u32).to_be_bytes())).is_err());

    // 4. raw_prefix truncated
    assert!(raw_prefix(&mut Cursor::new([0, 0, 1])).is_err());

    // 5. copy_raw valid
    let mut sink = Vec::new();
    copy_raw(&mut Cursor::new(b"abcdefgh"), &mut sink, 8).unwrap();
    assert_eq!(&sink, b"abcdefgh");

    // 6. raw_end checks EOF
    assert!(raw_end(&mut Cursor::new([])).is_ok());
    assert!(raw_end(&mut Cursor::new([1])).is_err());

    // 7. forward_raw end-to-end
    let mut src = (4u32).to_be_bytes().to_vec();
    src.extend_from_slice(b"data");
    let mut dest = Vec::new();
    forward_raw(&mut Cursor::new(&src), &mut dest).unwrap();
    assert_eq!(dest, src);
}

#[test]
fn relay_config_and_remaining_helpers() {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("dw-relay-test-{unique}"));
    std::fs::create_dir_all(&dir).unwrap();

    // /dw-dns-capture creates process.json with uid 65532
    config(&dir, "/dw-dns-capture").unwrap();
    let text = std::fs::read_to_string(dir.join("process.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["user"]["uid"], 65532);

    // /dw-dns-exporter creates process.json with uid 65533
    config(&dir, "/dw-dns-exporter").unwrap();
    let text = std::fs::read_to_string(dir.join("process.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["user"]["uid"], 65533);

    // Invalid binary is refused
    assert_eq!(
        config(&dir, "/invalid").unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
    let _ = std::fs::remove_dir_all(&dir);

    // remaining helper
    assert!(remaining(Instant::now() + Duration::from_secs(5)).is_ok());
    assert_eq!(
        remaining(Instant::now() - Duration::from_secs(1)).unwrap_err(),
        Fail::State("live_effect_deadline")
    );

    // socket_pair
    let (s1, s2) = socket_pair().unwrap();
    drop((s1, s2));

    // clear_failed_transport
    let id = uuid::Uuid::new_v4();
    clear_failed_transport(&Fail::State("some_error"), id);
    clear_failed_transport(&Fail::Unresolved("live_disposal_unconfirmed"), id);
}

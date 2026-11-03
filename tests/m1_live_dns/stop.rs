//! Private launcher consumer tests; not linked into the ordinary core.
use super::*;
use std::path::Path;
use std::time::Instant;

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
#[ignore = "requires the packet-owned Linux host; legacy pending marker, no START"]
fn owned_lab_pair_legacy_pending_marker_refuses_confirmed_disposal() {
    prepare_closed_pair(Fault::LegacyPending);
}
#[derive(Clone, Copy, PartialEq)]
enum Fault {
    None,
    JournalLoss,
    LegacyPending,
}
fn prepare_closed_pair(fault: Fault) {
    // No v6 consent or release is supplied. This test cannot call START.
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
    let mut pair = Pair::prepare(image.trim(), &session).expect("owned pair preparation");
    let elapsed = started.elapsed();
    let dir = Path::new("/run/duskweave/launcher").join(attempt.simple().to_string());
    if fault == Fault::JournalLoss {
        std::fs::remove_file(dir.join("session.json")).unwrap();
    }
    if fault == Fault::LegacyPending {
        std::fs::write(dir.join("transport.pending"), []).unwrap();
    }
    let disposed = pair.dispose();
    if fault == Fault::LegacyPending {
        assert!(!disposed, "historical child marker must block disposal");
        assert!(dir.join("transport.pending").exists());
        assert!(!super::super::attempt::stop(attempt, attempt, 1));
        std::fs::remove_file(dir.join("transport.pending")).unwrap();
    }
    let cleaned = super::super::attempt::dispose(attempt);
    drop(pair);
    let remaining = super::super::docker::metadata(&[
        "ps",
        "--all",
        "--filter",
        &format!("label=org.duskweave.attempt={attempt}"),
        "--format",
        "{{.ID}}",
    ])
    .unwrap();
    eprintln!(
        "pair_preparation_ms={} start_authority=false disposal_confirmed={}",
        elapsed.as_millis(),
        disposed
    );
    assert!(cleaned, "test-owned resources must be disposed");
    assert!(remaining.trim().is_empty(), "known pair must be gone");
    if fault == Fault::None {
        assert!(disposed);
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
fn relay_stages_complete_frame_and_eof_before_exporter_write() {
    use std::io::{Cursor, Read, Write};

    struct NoEof(Cursor<Vec<u8>>);
    impl Read for NoEof {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let n = self.0.read(out)?;
            if n == 0 {
                return Err(std::io::ErrorKind::WouldBlock.into());
            }
            Ok(n)
        }
    }
    struct ShortSink(Vec<u8>);
    impl Write for ShortSink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.push(bytes[0]);
            Ok(1)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut at_cap = (MAX_FRAME as u32).to_be_bytes().to_vec();
    at_cap.extend(vec![b'x'; MAX_FRAME]);
    let mut sink = ShortSink(Vec::new());
    forward_raw(&mut Cursor::new(&at_cap), &mut sink).unwrap();
    assert_eq!(sink.0, at_cap, "partial writes retain complete framing");

    for malformed in [
        vec![0, 0, 0, 0],
        ((MAX_FRAME + 1) as u32).to_be_bytes().to_vec(),
        at_cap[..at_cap.len() - 1].to_vec(),
        [at_cap.as_slice(), b"x"].concat(),
    ] {
        let mut sink = Vec::new();
        assert!(forward_raw(&mut Cursor::new(malformed), &mut sink).is_err());
        assert!(sink.is_empty(), "no candidate bytes before complete EOF");
    }
    let mut sink = Vec::new();
    assert!(forward_raw(&mut NoEof(Cursor::new(at_cap)), &mut sink).is_err());
    assert!(
        sink.is_empty(),
        "withheld EOF cannot leak a candidate prefix"
    );
}

use super::*;
use uuid::Uuid;

fn request(attempt: Uuid, generation: u64) -> DnsRequest {
    DnsRequest {
        schema: "m1_dns_ipc_v1".into(),
        kind: "dns_lookup".into(),
        attempt,
        session_generation: generation,
        sequence: 0,
        engagement_id: Uuid::new_v4(),
        campaign_id: Uuid::new_v4(),
        decision_id: Uuid::new_v4(),
        profile: "dns-routing-v2".into(),
        execution_mode: "live".into(),
        run_profile: "autonomous-dns-genai-lab-v1".into(),
        release_sha256: "b".repeat(64),
        qualification_ref: Uuid::new_v4(),
        name: "seed.duskweave.test".into(),
        resolver_ipv4: "192.0.2.53".into(),
        transport: "udp".into(),
        query_id: 0,
        allowed_names: vec!["seed.duskweave.test".into()],
        deadline_at_ms: i64::MAX,
        max_duration_ms: 60000,
    }
}

#[test]
fn receive_start_accepts_only_the_matching_attempt_generation() {
    let attempt = Uuid::new_v4();
    let (a, b) = UnixStream::pair().unwrap();
    let mut host = HostChannel::new(a);
    let mut client = HostChannel::new(b);
    let q = request(attempt, 7);
    // A Start for a different session is refused, not misread.
    client
        .send_command(&HostCommand::Start {
            attempt,
            generation: 8,
            deadline_at_ms: 42,
        })
        .unwrap();
    assert_eq!(
        receive_start(&mut host, &q).unwrap_err(),
        Fail::State("live_effect_start_refused")
    );
    let (a, b) = UnixStream::pair().unwrap();
    let mut host = HostChannel::new(a);
    let mut client = HostChannel::new(b);
    client
        .send_command(&HostCommand::Start {
            attempt,
            generation: 7,
            deadline_at_ms: 4242,
        })
        .unwrap();
    assert_eq!(receive_start(&mut host, &q).unwrap(), 4242);
    // Non-Start commands never open the attempt.
    let (a, b) = UnixStream::pair().unwrap();
    let mut host = HostChannel::new(a);
    let mut client = HostChannel::new(b);
    client
        .send_command(&HostCommand::Ack {
            attempt: Uuid::new_v4(),
            generation: 1,
        })
        .unwrap();
    assert_eq!(
        receive_start(&mut host, &q).unwrap_err(),
        Fail::State("live_effect_start_refused")
    );
}

#[test]
fn acknowledged_accepts_matching_ack_and_renew_rejects_others() {
    let attempt = Uuid::new_v4();
    let q = request(attempt, 2);

    // 1. Matching Renew returns Ok(false)
    let (a, b) = UnixStream::pair().unwrap();
    let mut host = HostChannel::new(a);
    let mut client = HostChannel::new(b);
    client
        .send_command(&HostCommand::Renew {
            attempt,
            generation: 2,
            deadline_at_ms: 100,
        })
        .unwrap();
    assert!(!acknowledged(&mut host, &q).unwrap());

    // 2. Matching Ack returns Ok(true)
    let (a, b) = UnixStream::pair().unwrap();
    let mut host = HostChannel::new(a);
    let mut client = HostChannel::new(b);
    client
        .send_command(&HostCommand::Ack {
            attempt,
            generation: 2,
        })
        .unwrap();
    assert!(acknowledged(&mut host, &q).unwrap());

    // 3. Mismatched Ack returns protocol refused
    let (a, b) = UnixStream::pair().unwrap();
    let mut host = HostChannel::new(a);
    let mut client = HostChannel::new(b);
    client
        .send_command(&HostCommand::Ack {
            attempt: Uuid::new_v4(),
            generation: 2,
        })
        .unwrap();
    assert_eq!(
        acknowledged(&mut host, &q).unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );

    // 4. Other command returns protocol refused
    let (a, b) = UnixStream::pair().unwrap();
    let mut host = HostChannel::new(a);
    let mut client = HostChannel::new(b);
    client
        .send_command(&HostCommand::StopRun {
            run_id: Uuid::new_v4(),
        })
        .unwrap();
    assert_eq!(
        acknowledged(&mut host, &q).unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );
}

#[test]
fn completed_channels_and_control_readable() {
    use std::io::{Read, Write};
    let (send, recv) = std::sync::mpsc::channel();

    // Empty channel
    assert!(completed(&recv).unwrap().is_none());

    // Successful result
    let sample_res: duskweave::m1_dns::DnsResult = serde_json::from_value(serde_json::json!({
        "schema": "m1_dns_ipc_v1",
        "kind": "RESULT",
        "attempt": "00000000-0000-0000-0000-000000000001",
        "session_generation": 1,
        "sequence": 3,
        "status": "completed",
        "observed_at_ms": 1000,
        "rcode": 0,
        "truncated": false,
        "complete": true,
        "withheld_aliases": 0,
        "answers": [{"ipv4": "198.51.100.7", "ttl_seconds": 30}],
        "aliases": []
    }))
    .unwrap();
    send.send(Ok(sample_res.clone())).unwrap();
    let res = completed(&recv).unwrap().unwrap();
    assert_eq!(res.sequence, 3);

    // Disconnected channel
    drop(send);
    assert_eq!(
        completed(&recv).unwrap_err(),
        Fail::State("dns_ipc_failure")
    );

    // control_readable tests
    let (mut a, mut b) = UnixStream::pair().unwrap();
    a.set_nonblocking(true).unwrap();

    // No data available -> Ok(false)
    assert!(!control_readable(&a).unwrap());

    // Data available -> Ok(true)
    b.write_all(b"x").unwrap();
    assert!(control_readable(&a).unwrap());

    // Closed peer -> Err
    drop(b);
    let mut buf = [0u8; 1];
    assert_eq!(a.read(&mut buf).unwrap(), 1);
    assert_eq!(buf[0], b'x');
    assert_eq!(
        control_readable(&a).unwrap_err(),
        Fail::State("live_host_unavailable")
    );
}

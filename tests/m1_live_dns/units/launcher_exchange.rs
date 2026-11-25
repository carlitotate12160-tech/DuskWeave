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

fn https_channel_pair() -> (HostChannel<UnixStream>, HostChannel<UnixStream>) {
    let (host, peer) = UnixStream::pair().unwrap();
    host.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    (HostChannel::new(host), HostChannel::new(peer))
}

#[test]
fn receive_https_start_and_https_acknowledged_enforce_phase_and_identity() {
    let q = crate::attempt::https::tests::sample_request(Uuid::new_v4(), 1000);
    let (mut host, mut peer) = https_channel_pair();

    peer.send_command(&HostCommand::StartPhase {
        attempt: q.attempt,
        generation: q.session_generation,
        phase: HttpsPhase::Connect,
        deadline_at_ms: 2000,
    })
    .unwrap();
    assert_eq!(
        receive_https_start(&mut host, &q, HttpsPhase::Connect).unwrap(),
        2000
    );

    for bad in [
        HostCommand::StartPhase {
            attempt: q.attempt,
            generation: q.session_generation,
            phase: HttpsPhase::Tls,
            deadline_at_ms: 2000,
        },
        HostCommand::StartPhase {
            attempt: q.attempt,
            generation: q.session_generation + 1,
            phase: HttpsPhase::Connect,
            deadline_at_ms: 2000,
        },
        HostCommand::AckHttps {
            attempt: q.attempt,
            generation: q.session_generation,
        },
    ] {
        peer.send_command(&bad).unwrap();
        assert_eq!(
            receive_https_start(&mut host, &q, HttpsPhase::Connect).unwrap_err(),
            Fail::State("live_effect_start_refused")
        );
    }

    peer.send_command(&HostCommand::RenewHttps {
        attempt: q.attempt,
        generation: q.session_generation,
        deadline_at_ms: 2000,
    })
    .unwrap();
    assert!(!https_acknowledged(&mut host, &q).unwrap());

    peer.send_command(&HostCommand::AckHttps {
        attempt: q.attempt,
        generation: q.session_generation,
    })
    .unwrap();
    assert!(https_acknowledged(&mut host, &q).unwrap());

    peer.send_command(&HostCommand::AckHttps {
        attempt: q.attempt,
        generation: q.session_generation + 1,
    })
    .unwrap();
    assert_eq!(
        https_acknowledged(&mut host, &q).unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );
}

#[test]
fn https_wait_rejects_protocol_drift_and_disconnects() {
    let q = crate::attempt::https::tests::sample_request(
        Uuid::new_v4(),
        crate::qualification::now_ms().unwrap(),
    );
    let (mut host, mut peer) = https_channel_pair();
    let dl = q.deadline_at_ms;
    let mut wait = HttpsWait {
        channel: &mut host,
        q: &q,
        deadline: dl,
        waiting_phase: None,
        io_revoked: false,
    };

    wait.on_ready(HttpsPhase::Tls).unwrap();
    assert!(matches!(
        peer.reply().unwrap(),
        HostReply::ReadyPhase {
            phase: HttpsPhase::Tls,
            ..
        }
    ));
    assert_eq!(
        wait.on_ready(HttpsPhase::Head).unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );

    assert_eq!(
        wait.on_renew(q.attempt, q.session_generation, dl + 1)
            .unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );
    let (cmd_tx, _cmd_rx) = std::sync::mpsc::sync_channel(1);
    assert_eq!(
        wait.on_start(
            &cmd_tx,
            q.attempt,
            q.session_generation,
            HttpsPhase::Head,
            dl
        )
        .unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );

    peer.send_command(&HostCommand::StopRun { run_id: q.run_id })
        .unwrap();
    assert_eq!(
        wait.command(&cmd_tx).unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );

    let (evt_tx, evt_rx) = std::sync::mpsc::sync_channel(1);
    drop(evt_tx);
    assert_eq!(
        wait.drain(&evt_rx).unwrap_err(),
        Fail::State("https_ipc_failure")
    );
}

use super::*;
use std::os::unix::net::UnixStream;

#[test]
fn an_unready_boundary_never_sends_host_commands() {
    let mut boundary = LiveBoundary::new(Uuid::new_v4());
    let error = boundary
        .command(&HostCommand::Ack {
            attempt: Uuid::new_v4(),
            generation: 1,
        })
        .unwrap_err();
    assert_eq!(error, Fail::State("live_host_unavailable"));
}

#[test]
fn exited_reports_clean_exit_and_reaps_killed_children() {
    let mut child = std::process::Command::new("/bin/true").spawn().unwrap();
    assert!(
        exited(&mut child, 5000).unwrap(),
        "clean exit reports success"
    );
    let mut child = std::process::Command::new("/bin/sleep")
        .arg("5")
        .spawn()
        .unwrap();
    assert!(!exited(&mut child, 50).unwrap());
    assert!(
        child.try_wait().unwrap().is_some(),
        "the killed child is reaped, not leaked"
    );
}

#[test]
fn killed_launcher_child_is_reaped_within_its_original_owner_budget() {
    let mut process = std::process::Command::new("/bin/sleep")
        .arg("10")
        .env_clear()
        .spawn()
        .unwrap();
    let path = std::path::PathBuf::from(format!("/proc/{}", process.id()));
    let clean = exited(&mut process, 100).unwrap();
    let end = Instant::now() + Duration::from_millis(50);
    while path.exists() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(1));
    }
    let reaped = !path.exists();
    let _ = process.wait();
    assert!(!clean);
    assert!(reaped);
}

#[test]
fn stop_command_mapping_preserves_target_discriminants() {
    use super::authority::StopTarget;
    let run_id = Uuid::new_v4();
    let cmd = stop_command(StopTarget::Run { run_id });
    match cmd {
        HostCommand::StopRun { run_id: r } => assert_eq!(r, run_id),
        _ => panic!("expected StopRun"),
    }

    let engagement_id = Uuid::new_v4();
    let campaign_id = Uuid::new_v4();
    let cmd = stop_command(StopTarget::Campaign {
        engagement_id,
        campaign_id,
    });
    match cmd {
        HostCommand::StopCampaign {
            engagement_id: e,
            campaign_id: c,
        } => {
            assert_eq!(e, engagement_id);
            assert_eq!(c, campaign_id);
        }
        _ => panic!("expected StopCampaign"),
    }
}

fn sample_request() -> DnsRequest {
    serde_json::from_value(serde_json::json!({
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
        "name": "seed.duskweave.test",
        "resolver_ipv4": "192.0.2.53",
        "transport": "udp",
        "query_id": 1,
        "allowed_names": ["seed.duskweave.test"],
        "deadline_at_ms": 2000,
        "max_duration_ms": 2000
    }))
    .unwrap()
}

#[test]
fn live_boundary_peer_framing_and_transitions() {
    let req = sample_request();
    let run = Uuid::new_v4();
    let (client, server) = UnixStream::pair().unwrap();
    // Owned child that exits naturally once the server releases its stdin gate.
    let mut child = std::process::Command::new("/bin/cat")
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let gate = child.stdin.take().unwrap();
    // Socket IO and thread completion stay bounded on every path.
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    // Narrow cleanup guard: reap the fake peer and disarm only the fixture
    // request so boundary Drop never invokes the installed launcher.
    struct Guard(LiveBoundary);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(mut peer) = self.0.peer.take() {
                drop(peer.channel);
                let _ = exited(&mut peer.process, 1000);
            }
            self.0.request = None;
        }
    }

    let (res_tx, res_rx) = std::sync::mpsc::channel();
    let mut guard = Guard(LiveBoundary::with_peer(
        run,
        Some(req.clone()),
        HostChannel::new(client),
        child,
        res_rx,
    ));
    let boundary = &mut guard.0;

    // ready on already charged boundary fails
    assert_eq!(
        boundary.ready(&req).unwrap_err(),
        Fail::State("live_effect_already_charged")
    );

    // Start command exchange
    let server_h = std::thread::spawn(move || {
        let mut server_chan = HostChannel::new(server);
        let cmd = server_chan.command().unwrap();
        match cmd {
            HostCommand::Start {
                attempt,
                generation,
                deadline_at_ms,
            } => {
                assert_eq!(attempt, req.attempt);
                assert_eq!(generation, 1);
                assert_eq!(deadline_at_ms, 5000);
            }
            _ => panic!("expected Start"),
        }
        let cmd = server_chan.command().unwrap();
        match cmd {
            HostCommand::Renew {
                attempt,
                generation,
                deadline_at_ms,
            } => {
                assert_eq!(attempt, req.attempt);
                assert_eq!(generation, 1);
                assert_eq!(deadline_at_ms, 6000);
            }
            _ => panic!("expected Renew"),
        }
        let cmd = server_chan.command().unwrap();
        match cmd {
            HostCommand::Ack {
                attempt,
                generation,
            } => {
                assert_eq!(attempt, req.attempt);
                assert_eq!(generation, 1);
            }
            _ => panic!("expected Ack"),
        }
        server_chan
            .send_reply(&HostReply::Stopped {
                disposal_confirmed: true,
            })
            .unwrap();
        // Releasing the stdin gate lets the owned child exit on its own.
        drop(gate);
    });

    boundary.start(&req, 5000).unwrap();
    boundary.renew(&req, 6000).unwrap();

    // Result receiver: empty returns None
    assert!(boundary.result().unwrap().is_none());

    // Result receiver: send result
    let sample_res: DnsResult = serde_json::from_value(serde_json::json!({
        "schema": "m1_dns_ipc_v1",
        "kind": "RESULT",
        "attempt": req.attempt,
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
    res_tx
        .send(Ok(HostReply::Result {
            result: sample_res.clone(),
        }))
        .unwrap();
    let got = boundary.result().unwrap().unwrap();
    assert_eq!(got.sequence, 3);

    // Acknowledge reaps child and marks closed
    let confirmed = boundary.acknowledge(&req).unwrap();
    assert!(confirmed);

    server_h.join().unwrap();
}

#[test]
fn uncharged_boundary_stop_returns_true() {
    let mut boundary = LiveBoundary::new(Uuid::new_v4());
    assert!(boundary.stop().unwrap());
}

#[path = "effect_socket_peer.rs"]
pub(super) mod peer;

#[test]
fn qualification_and_stop_validate_replies_from_an_owned_process() {
    use super::authority::StopTarget;
    let request = sample_request();
    let binding = LiveBinding {
        qualification_ref: request.qualification_ref,
        release_sha256: request.release_sha256.clone(),
        resolver_ipv4: request.resolver_ipv4.clone(),
    };
    {
        let _peer = peer::Scope::new("good");
        qualify(&binding, &request.run_profile).unwrap();
        let target = StopTarget::Run {
            run_id: Uuid::new_v4(),
        };
        let result = stop(target).unwrap();
        assert!(result.stop_latched && result.disposal_confirmed);
        let result = stop(StopTarget::Campaign {
            engagement_id: request.engagement_id,
            campaign_id: request.campaign_id,
        })
        .unwrap();
        assert!(result.stop_latched && !result.disposal_confirmed);
    }
    for mode in ["refuse", "wrong_identity"] {
        let _peer = peer::Scope::new(mode);
        assert!(qualify(&binding, &request.run_profile).is_err());
        assert!(
            stop(StopTarget::Run {
                run_id: Uuid::new_v4()
            })
            .is_err()
        );
    }
}

#[test]
fn ordinary_boundary_ready_result_ack_and_stop_use_the_real_process_protocol() {
    use std::os::unix::fs::DirBuilderExt;
    let root = std::env::temp_dir().join(format!("dw-peer-{}", Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    super::session::set_test_control_root(Some(root.clone()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            super::super::session::set_test_control_root(None);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root);
    let request = sample_request();
    {
        let _peer = peer::Scope::new("good");
        let mut boundary = LiveBoundary::new(Uuid::new_v4());
        boundary.ready(&request).unwrap();
        boundary.start(&request, 2000).unwrap();
        let end = Instant::now() + Duration::from_secs(2);
        let result = loop {
            if let Some(result) = boundary.result().unwrap() {
                break result;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        };
        result.validate(&request, 2000).unwrap();
        assert!(boundary.acknowledge(&request).unwrap());
        assert!(boundary.closed);
    }
    {
        let _peer = peer::Scope::new("good");
        let mut boundary = LiveBoundary::new(Uuid::new_v4());
        boundary.ready(&request).unwrap();
        assert!(boundary.stop().unwrap());
        assert!(boundary.closed && boundary.peer.is_none());
    }
    {
        let _peer = peer::Scope::new("wrong_identity");
        let mut boundary = LiveBoundary::new(Uuid::new_v4());
        assert!(boundary.ready(&request).is_err());
        assert!(
            boundary.peer.is_none(),
            "failed preparation must reap the owned child"
        );
    }
}

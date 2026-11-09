use super::*;
use uuid::Uuid;

#[test]
fn binding_projection_keeps_only_the_bound_fields() {
    let q = DnsRequest {
        schema: "m1_dns_ipc_v1".into(),
        kind: "dns_lookup".into(),
        attempt: Uuid::new_v4(),
        session_generation: 2,
        sequence: 0,
        engagement_id: Uuid::new_v4(),
        campaign_id: Uuid::new_v4(),
        decision_id: Uuid::new_v4(),
        profile: "dns-routing-v2".into(),
        execution_mode: "live".into(),
        run_profile: "autonomous-dns-genai-lab-v1".into(),
        release_sha256: "b".repeat(64),
        qualification_ref: Uuid::from_u128(0x71),
        name: "seed.duskweave.test".into(),
        resolver_ipv4: "192.0.2.53".into(),
        transport: "udp".into(),
        query_id: 0,
        allowed_names: vec!["seed.duskweave.test".into()],
        deadline_at_ms: i64::MAX,
        max_duration_ms: 60000,
    };
    let binding = binding_of(&q);
    assert_eq!(binding.qualification_ref, Uuid::from_u128(0x71));
    assert_eq!(binding.release_sha256, "b".repeat(64));
    assert_eq!(binding.resolver_ipv4, "192.0.2.53");
}

fn dummy_qualified() -> Qualified {
    Qualified {
        manifest: Manifest {
            qualification_ref: Uuid::new_v4(),
            purpose: "LAB".into(),
            run_profile: "autonomous-dns-genai-lab-v1".into(),
            release_sha256: "a".repeat(64),
            launcher_sha256: "a".repeat(64),
            capture_sha256: "a".repeat(64),
            exporter_sha256: "a".repeat(64),
            image_digest: format!("sha256:{}", "a".repeat(64)),
            boot_id: Uuid::new_v4(),
            resolver_ipv4: "192.0.2.53".into(),
            valid_from_ms: 0,
            valid_until_ms: i64::MAX,
            core_uid: 1001,
        },
        manifest_sha256: "b".repeat(64),
    }
}

#[test]
fn dispatch_stop_commands_and_refused() {
    let qualified = dummy_qualified();

    // 1. HostCommand::Stop
    let (client, server) = UnixStream::pair().unwrap();
    let mut client_chan = HostChannel::new(client);
    client_chan
        .send_command(&HostCommand::Stop {
            engagement_id: Uuid::new_v4(),
            campaign_id: Uuid::new_v4(),
            generation: 1,
        })
        .unwrap();
    dispatch(server, &qualified).unwrap();
    match client_chan.reply().unwrap() {
        HostReply::Stopped { .. } => {}
        _ => panic!("expected Stopped reply"),
    }

    // 2. HostCommand::StopRun
    let (client, server) = UnixStream::pair().unwrap();
    let mut client_chan = HostChannel::new(client);
    let run_id = Uuid::new_v4();
    client_chan
        .send_command(&HostCommand::StopRun { run_id })
        .unwrap();
    dispatch(server, &qualified).unwrap();
    match client_chan.reply().unwrap() {
        HostReply::RunStopped { run_id: r, .. } => assert_eq!(r, run_id),
        _ => panic!("expected RunStopped reply"),
    }

    // 3. HostCommand::StopCampaign
    let (client, server) = UnixStream::pair().unwrap();
    let mut client_chan = HostChannel::new(client);
    let engagement_id = Uuid::new_v4();
    let campaign_id = Uuid::new_v4();
    client_chan
        .send_command(&HostCommand::StopCampaign {
            engagement_id,
            campaign_id,
        })
        .unwrap();
    dispatch(server, &qualified).unwrap();
    match client_chan.reply().unwrap() {
        HostReply::CampaignStopped {
            engagement_id: e,
            campaign_id: c,
            ..
        } => {
            assert_eq!(e, engagement_id);
            assert_eq!(c, campaign_id);
        }
        _ => panic!("expected CampaignStopped reply"),
    }

    // 4. Invalid command (e.g. Ack) sent to dispatch -> Refused
    let (client, server) = UnixStream::pair().unwrap();
    let mut client_chan = HostChannel::new(client);
    client_chan
        .send_command(&HostCommand::Ack {
            attempt: Uuid::new_v4(),
            generation: 1,
        })
        .unwrap();
    dispatch(server, &qualified).unwrap();
    match client_chan.reply().unwrap() {
        HostReply::Refused => {}
        _ => panic!("expected Refused reply"),
    }
}

#[test]
fn real_control_lifecycle_runs_with_actual_root_journals_and_isolated_io() {
    let mut command = std::process::Command::new("/usr/bin/sudo");
    command.args(["-n", "--", "/usr/bin/env"]);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        let mut assignment = std::ffi::OsString::from("LLVM_PROFILE_FILE=");
        assignment.push(profile);
        command.arg(assignment);
    }
    command
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "control::tests::root_protocol_probe",
            "--ignored",
            "--test-threads=1",
        ])
        .env_clear()
        .stdin(std::process::Stdio::null());
    assert!(command.status().unwrap().success());
}

#[test]
#[ignore = "root-owned temporary journal witness; reached through the isolated child above"]
fn root_protocol_probe() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    for fault in [
        crate::relay::unit_tests::Fault::None,
        crate::relay::unit_tests::Fault::Ready,
        crate::relay::unit_tests::Fault::Result,
        crate::relay::unit_tests::Fault::Ack,
        crate::relay::unit_tests::Fault::WrongStart,
    ] {
        protocol_episode(fault);
    }
    watchdog_episode();
}

fn protocol_episode(fault: crate::relay::unit_tests::Fault) {
    use crate::relay::unit_tests::{Fault, resource_scope};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let root = crate::attempt::tests::TestRootGuard::new();
    let network = crate::network::tests::fixture::Network::new();
    let qualified = dummy_qualified();
    let mut q = crate::attempt::tests::sample_request(
        Uuid::new_v4(),
        crate::qualification::now_ms().unwrap(),
    );
    q.qualification_ref = qualified.manifest.qualification_ref;
    crate::docker::tests::simulate_absent_attempt(q.attempt);
    let _watchdog = watchdog::tests::child_scope(false);
    let renewed = Arc::new(AtomicBool::new(false));
    network.signals.borrow_mut().push((4, renewed.clone()));
    let _resources = resource_scope(
        &qualified.manifest.image_digest,
        &q,
        fault,
        (fault == Fault::None).then_some(renewed.clone()),
    );
    crate::network::create(
        q.attempt,
        42,
        &q.resolver_ipv4,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
    let (host, peer) = UnixStream::pair().unwrap();
    host.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut host = HostChannel::new(host);
    let request = q.clone();
    let client = std::thread::spawn(move || {
        let mut channel = HostChannel::new(peer);
        let ready = channel.reply();
        if fault == Fault::Ready {
            assert!(ready.is_err());
            return;
        }
        assert!(
            matches!(ready.unwrap(), HostReply::Ready { attempt, generation }
            if (attempt, generation) == (request.attempt, request.session_generation))
        );
        let deadline = crate::qualification::now_ms().unwrap() + 1500;
        channel
            .send_command(&HostCommand::Start {
                attempt: request.attempt,
                generation: request.session_generation + u64::from(fault == Fault::WrongStart),
                deadline_at_ms: deadline,
            })
            .unwrap();
        if fault == Fault::None {
            channel
                .send_command(&HostCommand::Renew {
                    attempt: request.attempt,
                    generation: request.session_generation,
                    deadline_at_ms: deadline,
                })
                .unwrap();
        }
        let result = channel.reply();
        if matches!(fault, Fault::Result | Fault::WrongStart) {
            assert!(result.is_err());
            return;
        }
        let HostReply::Result { result } = result.unwrap() else {
            panic!("safe result required");
        };
        result.validate(&request, deadline).unwrap();
        if fault == Fault::Ack {
            return;
        }
        channel
            .send_command(&HostCommand::Ack {
                attempt: request.attempt,
                generation: request.session_generation,
            })
            .unwrap();
        assert!(matches!(
            channel.reply().unwrap(),
            HostReply::Stopped {
                disposal_confirmed: true
            }
        ));
    });
    let peer = host.peer().unwrap();
    let result = open(&mut host, &qualified.manifest, Uuid::new_v4(), &q, &peer);
    assert_eq!(
        result.is_ok(),
        fault == Fault::None,
        "{fault:?}: {result:?}"
    );
    drop(host);
    drop(peer);
    client.join().unwrap();
    assert!(
        !root.path().join(q.attempt.simple().to_string()).exists(),
        "all outcomes reconcile the owned journal"
    );
    assert!(
        crate::custody::tests::fixture_map_is_empty(),
        "actual OS ownership, no metadata fixture"
    );
    if matches!(fault, Fault::Ready | Fault::WrongStart) {
        assert!(
            !network
                .events
                .borrow()
                .iter()
                .any(|event| std::fs::read_to_string(&event.input)
                    .is_ok_and(|input| input.contains("add element"))),
            "preparation/protocol refusal must not grant egress"
        );
    }
    if fault == Fault::None {
        assert!(
            renewed.load(Ordering::SeqCst),
            "real attempt renewal reached network I/O before result"
        );
        let rules: Vec<String> = network
            .events
            .borrow()
            .iter()
            .filter_map(|event| std::fs::read_to_string(&event.input).ok())
            .filter(|input| input.contains("add element"))
            .collect();
        assert_eq!(rules.len(), 2, "one START and one renewal, no replay");
    }
}

fn watchdog_episode() {
    let _root = crate::attempt::tests::TestRootGuard::new();
    let _network = crate::network::tests::fixture::Network::new();
    let q = crate::attempt::tests::sample_request(
        Uuid::new_v4(),
        crate::qualification::now_ms().unwrap(),
    );
    crate::docker::tests::simulate_absent_attempt(q.attempt);
    crate::attempt::open(Uuid::new_v4(), &q).unwrap();
    let (mut socket, peer) = UnixStream::pair().unwrap();
    HostChannel::new(peer.try_clone().unwrap())
        .send_command(&HostCommand::Ack {
            attempt: q.attempt,
            generation: q.session_generation,
        })
        .unwrap();
    peer.shutdown(std::net::Shutdown::Write).unwrap();
    watchdog::serve(socket.try_clone().unwrap()).unwrap();
    assert!(matches!(
        HostChannel::new(peer).reply().unwrap(),
        HostReply::Ready { .. }
    ));
    assert!(crate::attempt::stopped(&q));
    let _ = &mut socket;
}

#[test]
fn dispatch_rechecks_qualification_before_returning_any_grant() {
    let mut qualified = dummy_qualified();
    qualified.manifest.valid_until_ms = 0;
    let (host, peer) = UnixStream::pair().unwrap();
    let mut peer = HostChannel::new(peer);
    peer.send_command(&HostCommand::Qualify {
        binding: LiveBinding {
            qualification_ref: qualified.manifest.qualification_ref,
            release_sha256: qualified.manifest.release_sha256.clone(),
            resolver_ipv4: qualified.manifest.resolver_ipv4.clone(),
        },
        run_profile: qualified.manifest.run_profile.clone(),
    })
    .unwrap();
    assert_eq!(
        dispatch(host, &qualified),
        Err(duskweave::Fail::Config("live_host_unqualified"))
    );
    assert!(
        peer.reply().is_err(),
        "expired qualification produces no Qualified reply"
    );
}

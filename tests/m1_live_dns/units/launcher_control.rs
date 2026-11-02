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
    let (r, w) = UnixStream::pair().unwrap();
    let raw = RawPipe { read: r, write: w };
    let mut client_chan = HostChannel::new(client);
    client_chan
        .send_command(&HostCommand::Stop {
            engagement_id: Uuid::new_v4(),
            campaign_id: Uuid::new_v4(),
            generation: 1,
        })
        .unwrap();
    dispatch(server, &qualified, raw).unwrap();
    match client_chan.reply().unwrap() {
        HostReply::Stopped { .. } => {}
        _ => panic!("expected Stopped reply"),
    }

    // 2. HostCommand::StopRun
    let (client, server) = UnixStream::pair().unwrap();
    let (r, w) = UnixStream::pair().unwrap();
    let raw = RawPipe { read: r, write: w };
    let mut client_chan = HostChannel::new(client);
    let run_id = Uuid::new_v4();
    client_chan
        .send_command(&HostCommand::StopRun { run_id })
        .unwrap();
    dispatch(server, &qualified, raw).unwrap();
    match client_chan.reply().unwrap() {
        HostReply::RunStopped { run_id: r, .. } => assert_eq!(r, run_id),
        _ => panic!("expected RunStopped reply"),
    }

    // 3. HostCommand::StopCampaign
    let (client, server) = UnixStream::pair().unwrap();
    let (r, w) = UnixStream::pair().unwrap();
    let raw = RawPipe { read: r, write: w };
    let mut client_chan = HostChannel::new(client);
    let engagement_id = Uuid::new_v4();
    let campaign_id = Uuid::new_v4();
    client_chan
        .send_command(&HostCommand::StopCampaign {
            engagement_id,
            campaign_id,
        })
        .unwrap();
    dispatch(server, &qualified, raw).unwrap();
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
    let (r, w) = UnixStream::pair().unwrap();
    let raw = RawPipe { read: r, write: w };
    let mut client_chan = HostChannel::new(client);
    client_chan
        .send_command(&HostCommand::Ack {
            attempt: Uuid::new_v4(),
            generation: 1,
        })
        .unwrap();
    dispatch(server, &qualified, raw).unwrap();
    match client_chan.reply().unwrap() {
        HostReply::Refused => {}
        _ => panic!("expected Refused reply"),
    }
}

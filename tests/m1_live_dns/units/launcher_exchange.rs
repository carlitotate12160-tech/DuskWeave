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

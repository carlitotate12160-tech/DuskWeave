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

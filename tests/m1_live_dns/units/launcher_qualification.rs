use super::*;

#[test]
fn hex_digest_acceptance_is_exact() {
    assert!(hex(&"a".repeat(64)));
    assert!(hex(&"0123456789abcdef".repeat(4)));
    for bad in [
        "",
        &"a".repeat(63),
        &"a".repeat(65),
        &"g".repeat(64),
        &"A".repeat(64),
        &format!("{}.json", "a".repeat(59)),
    ] {
        assert!(!hex(bad), "{bad}");
    }
}

#[test]
fn manifest_binding_match_requires_every_bound_field() {
    let manifest = Manifest {
        qualification_ref: uuid::Uuid::from_u128(0x51),
        purpose: "LAB".into(),
        run_profile: "autonomous-dns-genai-lab-v1".into(),
        release_sha256: "b".repeat(64),
        launcher_sha256: "c".repeat(64),
        capture_sha256: "d".repeat(64),
        exporter_sha256: "e".repeat(64),
        image_digest: format!("sha256:{}", "f".repeat(64)),
        boot_id: uuid::Uuid::from_u128(0x61),
        resolver_ipv4: "192.0.2.53".into(),
        valid_from_ms: 0,
        valid_until_ms: i64::MAX,
        core_uid: 65534,
    };
    let binding = LiveBinding {
        qualification_ref: uuid::Uuid::from_u128(0x51),
        release_sha256: "b".repeat(64),
        resolver_ipv4: "192.0.2.53".into(),
    };
    assert!(manifest.matches(&binding, "autonomous-dns-genai-lab-v1"));
    let mut other = binding.clone();
    other.release_sha256 = "0".repeat(64);
    assert!(!manifest.matches(&other, "autonomous-dns-genai-lab-v1"));
    let mut other = binding.clone();
    other.qualification_ref = uuid::Uuid::from_u128(0x52);
    assert!(!manifest.matches(&other, "autonomous-dns-genai-lab-v1"));
    assert!(!manifest.matches(&binding, "autonomous-dns-fixture-v2"));
}

use super::*;
use crate::custody::tests::{install_fixture_map, simulate_root_file};
use crate::qualification::{Manifest, Qualified};
use serde_json::{Value, json};

fn valid_manifest_value() -> Value {
    json!({
        "qualification_ref": Uuid::new_v4(),
        "purpose": "HTTPS_LOOP_LAB",
        "run_profile": "autonomous-https-genai-lab-v1",
        "release_sha256": "a".repeat(64),
        "launcher_sha256": "b".repeat(64),
        "capture_sha256": "c".repeat(64),
        "exporter_sha256": "d".repeat(64),
        "image_digest": format!("sha256:{}", "e".repeat(64)),
        "boot_id": Uuid::new_v4(),
        "resolver_ipv4": "192.0.2.53",
        "https_lab_root_version": HTTPS_LAB_ROOT_VERSION,
        "https_lab_root_sha256": HTTPS_LAB_ROOT_SHA256,
        "valid_from_ms": 1_000,
        "valid_until_ms": 100_000,
        "core_uid": 1000
    })
}

fn manifest_bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).expect("manifest json")
}

#[test]
fn parse_manifest_accepts_closed_https_lab_tuple() {
    let raw = manifest_bytes(&valid_manifest_value());
    let parsed: Manifest = parse_manifest(&raw).expect("valid https lab manifest");
    assert_eq!(parsed.purpose, "HTTPS_LOOP_LAB");
    assert_eq!(parsed.run_profile, "autonomous-https-genai-lab-v1");
    let qualified: Qualified = super::super::parse_manifest(&raw).expect("dispatch https lab");
    assert_eq!(qualified.manifest.purpose, "HTTPS_LOOP_LAB");
    assert_eq!(qualified.manifest_sha256.len(), 64);

    let mut old_lab = valid_manifest_value();
    old_lab["purpose"] = json!("LAB");
    old_lab["run_profile"] = json!("autonomous-live-dns-v1");
    assert!(parse_manifest(&manifest_bytes(&old_lab)).is_err());
}

#[test]
fn parse_manifest_rejects_drifted_or_unknown_fields() {
    for (key, bad) in [
        ("purpose", json!("LAB")),
        ("run_profile", json!("autonomous-live-dns-v1")),
        ("resolver_ipv4", json!("198.51.100.53")),
        ("https_lab_root_version", json!("wrong-root-v1")),
        ("https_lab_root_sha256", json!("0000")),
        ("extra_field", json!(true)),
    ] {
        let mut value = valid_manifest_value();
        value.as_object_mut().expect("obj").insert(key.into(), bad);
        assert!(
            matches!(
                parse_manifest(&manifest_bytes(&value)),
                Err(Fail::Config("live_host_unqualified"))
            ),
            "drift on {key} must fail"
        );
    }
}

#[test]
fn verify_root_custody_accepts_pinned_pem_and_rejects_bad_files() {
    let _g = install_fixture_map();
    let dir = std::env::temp_dir().join(format!("dw-https-root-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let absent = dir.join("missing.pem");
    with_test_root_pem(absent, || {
        assert_eq!(
            verify_root_custody().unwrap_err(),
            Fail::Config("live_host_unqualified")
        );
    });

    let good = dir.join("good.pem");
    let pinned = include_bytes!("../../../boundary/fixtures/https-lab-root-v1.pem");
    std::fs::write(&good, pinned).expect("write good");
    simulate_root_file(&good);
    with_test_root_pem(good.clone(), || {
        assert!(verify_root_custody().is_ok());
    });

    let bad = dir.join("bad.pem");
    std::fs::write(&bad, b"not-the-pinned-root-pem").expect("write bad");
    simulate_root_file(&bad);
    with_test_root_pem(bad, || {
        assert_eq!(
            verify_root_custody().unwrap_err(),
            Fail::Config("live_host_unqualified")
        );
    });

    #[cfg(unix)]
    {
        let link = dir.join("link.pem");
        std::os::unix::fs::symlink(&good, &link).expect("symlink");
        with_test_root_pem(link, || {
            assert_eq!(
                verify_root_custody().unwrap_err(),
                Fail::Config("live_host_unqualified")
            );
        });
    }

    let _ = std::fs::remove_dir_all(&dir);
}

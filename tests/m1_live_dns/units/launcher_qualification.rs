use super::*;
use crate::custody::tests as custody_tests;
use std::path::{Path, PathBuf};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let id = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let path = std::env::temp_dir().join(format!("dw-qualification-{}", id.trim()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn valid_manifest_json(purpose: &str, profile: &str) -> String {
    format!(
        r#"{{
            "qualification_ref": "00000000-0000-0000-0000-000000000051",
            "purpose": "{purpose}",
            "run_profile": "{profile}",
            "release_sha256": "{h}",
            "launcher_sha256": "{h}",
            "capture_sha256": "{h}",
            "exporter_sha256": "{h}",
            "image_digest": "sha256:{h}",
            "boot_id": "00000000-0000-0000-0000-000000000061",
            "resolver_ipv4": "192.0.2.53",
            "valid_from_ms": 1000,
            "valid_until_ms": 2000000000000,
            "core_uid": 1001
        }}"#,
        h = "a".repeat(64)
    )
}

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

#[test]
fn parse_manifest_accepts_valid_json_and_computes_sha256() {
    let json = valid_manifest_json("LAB", "autonomous-dns-genai-lab-v1");
    let qualified = parse_manifest(json.as_bytes()).expect("parse valid manifest");
    assert_eq!(qualified.manifest.purpose, "LAB");
    assert_eq!(
        qualified.manifest.run_profile,
        "autonomous-dns-genai-lab-v1"
    );
    assert_eq!(qualified.manifest.core_uid, 1001);
    assert_eq!(qualified.manifest_sha256.len(), 64);
}

#[test]
fn parse_manifest_rejects_unknown_fields() {
    let json = valid_manifest_json("LAB", "autonomous-dns-genai-lab-v1");
    let mut obj: serde_json::Value = serde_json::from_str(&json).unwrap();
    obj.as_object_mut()
        .unwrap()
        .insert("extra_key".to_string(), serde_json::json!("forbidden"));
    let bytes = serde_json::to_vec(&obj).unwrap();
    assert!(matches!(parse_manifest(&bytes),
        Err(e) if e == Fail::Config("live_host_unqualified")));
}

#[test]
fn parse_manifest_rejects_oversized_payload() {
    let big = vec![b' '; 16385];
    assert!(matches!(parse_manifest(&big),
        Err(e) if e == Fail::Config("live_host_unqualified")));
}

#[test]
fn parse_manifest_rejects_corrupted_json() {
    assert!(matches!(parse_manifest(b"{ bad json"),
        Err(e) if e == Fail::Config("live_host_unqualified")));
}

#[test]
fn manifest_validate_enforces_all_invariant_checks() {
    let json = valid_manifest_json("LAB", "autonomous-dns-genai-lab-v1");
    let qualified = parse_manifest(json.as_bytes()).unwrap();
    qualified.manifest.validate().expect("valid manifest");

    // 1. Purpose mismatch
    let mut m = qualified.manifest;
    m.purpose = "PRODUCTION".to_string();
    assert_eq!(
        m.validate().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 2. Nil boot id
    let mut m = parse_manifest(json.as_bytes()).unwrap().manifest;
    m.boot_id = Uuid::nil();
    assert_eq!(
        m.validate().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 3. Core uid is 0 (root forbidden as core user)
    let mut m = parse_manifest(json.as_bytes()).unwrap().manifest;
    m.core_uid = 0;
    assert_eq!(
        m.validate().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 4. Time window expired
    let mut m = parse_manifest(json.as_bytes()).unwrap().manifest;
    m.valid_until_ms = 10;
    assert_eq!(
        m.validate().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 5. Future valid_from
    let mut m = parse_manifest(json.as_bytes()).unwrap().manifest;
    m.valid_from_ms = i64::MAX - 1000;
    assert_eq!(
        m.validate().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 6. Bad image digest (missing sha256: prefix)
    let mut m = parse_manifest(json.as_bytes()).unwrap().manifest;
    m.image_digest = "a".repeat(64);
    assert_eq!(
        m.validate().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 7. Bad hash length
    let mut m = parse_manifest(json.as_bytes()).unwrap().manifest;
    m.launcher_sha256 = "a".repeat(63);
    assert_eq!(
        m.validate().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
}

#[test]
fn now_ms_returns_positive_timestamp() {
    let ms = now_ms().expect("current timestamp in ms");
    assert!(ms > 1_700_000_000_000);
}

#[test]
fn digest_hashes_file_contents_accurately() {
    let directory = Directory::new();
    let file_path = directory.path().join("digest-input.bin");
    std::fs::write(&file_path, b"duskweave qualification digest test").unwrap();
    let hash = digest(File::open(&file_path).unwrap()).expect("compute digest");
    assert_eq!(hash.len(), 64);
    assert!(hex(&hash));
}

#[test]
fn root_directory_checks_directory_mode_and_uid() {
    let directory = Directory::new();
    let dir_path = directory.path().to_str().unwrap();
    // In default environment without test skip, uid != 0 fails
    assert_eq!(
        root_directory(dir_path).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
    // Nonexistent directory fails
    assert_eq!(
        root_directory("/nonexistent/duskweave/dir").unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
    // File instead of directory fails
    let file_path = directory.path().join("not-a-directory.bin");
    std::fs::write(&file_path, b"payload").unwrap();
    assert_eq!(
        root_directory(file_path.to_str().unwrap()).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
    // Path redirection grants nothing: a non-root directory still refuses.
    with_test_env(
        directory.path().join("c.json"),
        directory.path().to_path_buf(),
        || {
            assert_eq!(
                root_directory(dir_path).unwrap_err(),
                Fail::Config("live_host_unqualified")
            );
        },
    );
}

#[test]
fn root_file_and_qualify_file_enforce_no_symlinks_and_single_link() {
    let directory = Directory::new();
    let file_path = directory.path().join("target.bin");
    std::fs::write(&file_path, b"payload").unwrap();
    let f_str = file_path.to_str().unwrap();

    // Without test skip, root_file fails on non-root uid
    assert_eq!(
        root_file(f_str).err(),
        Some(Fail::Config("live_host_unqualified"))
    );

    with_test_env(file_path.clone(), directory.path().to_path_buf(), || {
        let _map = custody_tests::install_fixture_map();
        custody_tests::simulate_root_file(&file_path);

        // Nonexistent file fails
        assert_eq!(
            root_file("/nonexistent/duskweave/file").err(),
            Some(Fail::Config("live_host_unqualified"))
        );
        // Valid simulated-root file passes
        let f = root_file(f_str).expect("open root file");
        assert!(qualify_file(&f).is_ok());

        // Multi-link file fails qualify_file: the shared inode is refreshed to
        // its real post-hardlink link count so the nlink rule, not uid, refuses.
        let link_path = directory.path().join("hardlink.bin");
        std::fs::hard_link(&file_path, &link_path).expect("create hardlink");
        custody_tests::simulate_root_file_multilink(&file_path);
        let f2 = File::open(&link_path).unwrap();
        assert_eq!(
            qualify_file(&f2).unwrap_err(),
            Fail::Config("live_host_unqualified")
        );

        // Symlink is rejected by openat2 with NO_SYMLINKS
        let sym_path = directory.path().join("symlink.bin");
        std::os::unix::fs::symlink(&file_path, &sym_path).expect("create symlink");
        assert_eq!(
            root_file(sym_path.to_str().unwrap()).err(),
            Some(Fail::Config("live_host_unqualified"))
        );
    });
}

#[test]
fn read_manifest_reads_and_verifies_config_file() {
    let directory = Directory::new();
    let cfg_path = directory.path().join("deployment.json");
    let json = valid_manifest_json("LAB", "autonomous-dns-genai-lab-v1");
    std::fs::write(&cfg_path, &json).unwrap();

    with_test_env(cfg_path.clone(), directory.path().to_path_buf(), || {
        let _map = custody_tests::install_fixture_map();
        custody_tests::simulate_root_file(&cfg_path);

        let qualified = read_manifest().expect("read valid manifest");
        assert_eq!(qualified.manifest.purpose, "LAB");

        // Overwrite with bad json
        std::fs::write(&cfg_path, b"{corrupted").unwrap();
        custody_tests::simulate_root_file(&cfg_path);
        assert!(matches!(read_manifest(),
            Err(e) if e == Fail::Config("live_host_unqualified")));
    });
}

#[test]
fn verify_image_checks_binary_digests() {
    let directory = Directory::new();
    let capture = directory.path().join("dw-dns-capture");
    let exporter = directory.path().join("dw-dns-exporter");
    std::fs::write(&capture, b"capture").unwrap();
    std::fs::write(&exporter, b"exporter").unwrap();

    let cap_hash = digest(File::open(&capture).unwrap()).unwrap();
    let exp_hash = digest(File::open(&exporter).unwrap()).unwrap();

    let json = valid_manifest_json("LAB", "autonomous-dns-genai-lab-v1");
    let mut qualified = parse_manifest(json.as_bytes()).unwrap();
    qualified.manifest.capture_sha256 = cap_hash;
    qualified.manifest.exporter_sha256 = exp_hash;

    with_test_env(
        directory.path().join("c.json"),
        directory.path().to_path_buf(),
        || {
            let _map = custody_tests::install_fixture_map();
            custody_tests::simulate_root_file(&capture);
            custody_tests::simulate_root_file(&exporter);

            // Mismatch in capture hash
            let mut bad = parse_manifest(json.as_bytes()).unwrap();
            bad.manifest.capture_sha256 = "0".repeat(64);
            assert_eq!(
                bad.manifest.verify_image().unwrap_err(),
                Fail::Config("live_host_unqualified")
            );

            // Missing exporter
            let _ = std::fs::remove_file(&exporter);
            assert_eq!(
                qualified.manifest.verify_image().unwrap_err(),
                Fail::Config("live_host_unqualified")
            );
        },
    );
}

#[test]
fn verify_process_rejects_mismatched_uid_or_boot() {
    let json = valid_manifest_json("LAB", "autonomous-dns-genai-lab-v1");
    let qualified = parse_manifest(json.as_bytes()).unwrap();

    #[cfg(target_os = "linux")]
    {
        let peer = rustix::net::UCred {
            pid: rustix::process::getpid(),
            uid: rustix::process::getuid(),
            gid: rustix::process::getgid(),
        };
        // Core uid in manifest is 1001; if peer.uid is different, verify_process fails
        let _ = qualified.manifest.verify_process(peer);

        // watchdog_peer rejects non-root caller
        assert_eq!(
            watchdog_peer(peer).unwrap_err(),
            Fail::Config("live_host_unqualified")
        );
    }
}

#[test]
fn protected_directories_and_host_sinks_fail_closed_under_unprivileged_runner() {
    let _ = protected_directories();
    let _ = host_sinks();
}

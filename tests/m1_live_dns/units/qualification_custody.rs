use super::*;
use crate::custody::tests as custody_tests;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let id = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let path = std::env::temp_dir().join(format!("dw-qcustody-{}", id.trim()));
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
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn valid_manifest_json() -> String {
    let h = "a".repeat(64);
    format!(
        r#"{{
            "qualification_ref": "00000000-0000-0000-0000-000000000051",
            "purpose": "LAB",
            "run_profile": "autonomous-dns-genai-lab-v1",
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
        }}"#
    )
}

#[test]
fn mapped_caller_root_directory_requires_uid_zero() {
    let dir = TempDir::new();
    // Nonzero uid refuses with an empty fixture map.
    {
        let _map = custody_tests::install_fixture_map();
        assert!(root_directory(dir.path().to_str().unwrap()).is_err());
    }
    // UID 0 with all other facts valid succeeds.
    {
        let _map = custody_tests::install_fixture_map();
        custody_tests::simulate_root_directory(dir.path());
        assert!(root_directory(dir.path().to_str().unwrap()).is_ok());
    }
}

#[test]
fn mapped_caller_read_manifest_requires_uid_zero() {
    let dir = TempDir::new();
    let cfg = dir.path().join("deployment.json");
    std::fs::write(&cfg, valid_manifest_json()).unwrap();

    with_test_env(cfg.clone(), dir.path().to_path_buf(), || {
        // Nonzero uid refuses.
        {
            let _map = custody_tests::install_fixture_map();
            assert!(read_manifest().is_err());
        }
        // UID 0 with valid facts parses.
        {
            let _map = custody_tests::install_fixture_map();
            custody_tests::simulate_root_file(&cfg);
            let qualified = read_manifest().expect("read manifest");
            assert_eq!(qualified.manifest.purpose, "LAB");
        }
    });
}

#[test]
fn real_custody_uses_actual_filesystem_owner() {
    assert!(custody_tests::fixture_map_is_empty());
    let euid = rustix::process::geteuid().as_raw();
    eprintln!("qualification real_custody euid={euid}");

    let dir = TempDir::new();
    let root_dir = dir.path().join("deploy");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root_dir)
        .unwrap();
    std::fs::set_permissions(&root_dir, std::fs::Permissions::from_mode(0o700)).unwrap();

    let file = dir.path().join("release.bin");
    std::fs::write(&file, b"payload").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    if euid == 0 {
        // Root-owned objects pass.
        assert!(root_directory(root_dir.to_str().unwrap()).is_ok());
        let f = root_file(file.to_str().unwrap()).expect("root file");
        assert!(qualify_file(&f).is_ok());

        // Non-root ownership refuses.
        let nr_dir = dir.path().join("nr-dir");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&nr_dir)
            .unwrap();
        rustix::fs::chown(
            &nr_dir,
            Some(rustix::process::Uid::from_raw(65_534)),
            Some(rustix::process::Gid::from_raw(65_534)),
        )
        .unwrap();
        assert!(root_directory(nr_dir.to_str().unwrap()).is_err());

        let nr_file = dir.path().join("nr-file");
        std::fs::write(&nr_file, b"x").unwrap();
        std::fs::set_permissions(&nr_file, std::fs::Permissions::from_mode(0o600)).unwrap();
        rustix::fs::chown(
            &nr_file,
            Some(rustix::process::Uid::from_raw(65_534)),
            Some(rustix::process::Gid::from_raw(65_534)),
        )
        .unwrap();
        assert!(root_file(nr_file.to_str().unwrap()).is_err());

        // Forbidden mode refuses.
        let fm_dir = dir.path().join("fm-dir");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&fm_dir)
            .unwrap();
        std::fs::set_permissions(&fm_dir, std::fs::Permissions::from_mode(0o722)).unwrap();
        assert!(root_directory(fm_dir.to_str().unwrap()).is_err());

        let fm_file = dir.path().join("fm-file");
        std::fs::write(&fm_file, b"x").unwrap();
        std::fs::set_permissions(&fm_file, std::fs::Permissions::from_mode(0o622)).unwrap();
        assert!(root_file(fm_file.to_str().unwrap()).is_err());

        // Multi-link file refuses.
        let link = dir.path().join("hardlink.bin");
        std::fs::hard_link(&file, &link).unwrap();
        let f2 = File::open(&link).unwrap();
        assert!(qualify_file(&f2).is_err());

        // Symlink is refused by openat2(NO_SYMLINKS).
        let sym = dir.path().join("symlink.bin");
        std::os::unix::fs::symlink(&file, &sym).unwrap();
        assert!(root_file(sym.to_str().unwrap()).is_err());

        // Wrong object type refuses at the applicable consumer.
        assert!(root_file(root_dir.to_str().unwrap()).is_err());
        assert!(root_directory(file.to_str().unwrap()).is_err());
    } else {
        // Actual non-root-owned temporary objects refuse.
        assert!(root_directory(root_dir.to_str().unwrap()).is_err());
        assert!(root_file(file.to_str().unwrap()).is_err());
    }
}

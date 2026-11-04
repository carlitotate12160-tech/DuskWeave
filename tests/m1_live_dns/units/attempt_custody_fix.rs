use super::*;
use crate::custody::{MetadataFacts, ObjectKind};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[test]
fn test_root_guard_restores_outer_scope_and_on_unwind() {
    let real = root();
    {
        let outer = TestRootGuard::new();
        let outer_path = outer.path().to_path_buf();
        assert_eq!(root(), outer_path);
        {
            let inner = TestRootGuard::new();
            let inner_path = inner.path().to_path_buf();
            assert_ne!(inner_path, outer_path);
            assert_eq!(root(), inner_path);
        }
        // The inner guard restores the outer root.
        assert_eq!(root(), outer_path);
    }
    // The outer drop restores the previous (real) root.
    assert_eq!(root(), real);

    // A panicking inner guard still restores the outer root on unwind.
    let outer = TestRootGuard::new();
    let outer_path = outer.path().to_path_buf();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _inner = TestRootGuard::new();
        panic!("root unwind");
    }));
    assert!(result.is_err());
    assert_eq!(root(), outer_path);
    drop(outer);
    assert_eq!(root(), real);
}

#[test]
fn mapped_caller_owned_root_requires_uid_zero() {
    let _g = TestRootGuard::new();
    {
        let _map = custody_tests::install_fixture_map();
        assert!(owned_root().is_err());
    }
    {
        let _map = custody_tests::install_fixture_map();
        custody_tests::simulate_root_directory(_g.path());
        assert!(owned_root().is_ok());
        // Same map, same object: change only the observed UID and the real
        // caller must refuse.
        let meta = std::fs::symlink_metadata(_g.path()).unwrap();
        custody_tests::register_fixture(
            meta.dev(),
            meta.ino(),
            MetadataFacts {
                kind: ObjectKind::Directory,
                uid: 65_534,
                mode: 0o700,
                nlink: meta.nlink(),
                len: meta.len(),
            },
        );
        assert!(owned_root().is_err());
    }
}

#[test]
fn mapped_caller_session_meta_requires_uid_zero() {
    let _g = TestRootGuard::new();
    let path = _g.path().join("session.json");
    std::fs::write(&path, b"{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    {
        let _map = custody_tests::install_fixture_map();
        assert!(!qualify_session_meta(
            &std::fs::symlink_metadata(&path).unwrap()
        ));
    }
    {
        let _map = custody_tests::install_fixture_map();
        custody_tests::simulate_root_file(&path);
        assert!(qualify_session_meta(
            &std::fs::symlink_metadata(&path).unwrap()
        ));
        // Same map, same object: change only the observed UID and the real
        // caller must refuse.
        let meta = std::fs::symlink_metadata(&path).unwrap();
        custody_tests::register_fixture(
            meta.dev(),
            meta.ino(),
            MetadataFacts {
                kind: ObjectKind::Regular,
                uid: 65_534,
                mode: 0o600,
                nlink: 1,
                len: meta.len(),
            },
        );
        assert!(!qualify_session_meta(&meta));
    }
}

#[test]
fn custody_portable_attempt_owned_root() {
    use std::os::unix::fs::MetadataExt;
    let _g = TestRootGuard::new();
    let meta = std::fs::symlink_metadata(root()).unwrap();
    assert_ne!(meta.uid(), 0);
    assert!(meta.is_dir() && meta.mode() & 0o077 == 0);
    assert_eq!(
        owned_root().unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
}

#[test]
fn custody_portable_attempt_session_meta() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let _g = TestRootGuard::new();
    let path = root().join("session.json");
    std::fs::write(&path, b"{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let meta = std::fs::symlink_metadata(&path).unwrap();
    assert_ne!(meta.uid(), 0);
    assert!(meta.is_file() && meta.nlink() == 1 && meta.mode() & 0o077 == 0 && meta.len() <= 16384);
    assert!(!qualify_session_meta(&meta));
}

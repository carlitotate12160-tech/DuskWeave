use super::*;
use std::collections::HashMap;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

// --- Test-only observation seam (scoped fixture map) -------------------------

thread_local! {
    static FIXTURE_MAP: std::cell::RefCell<Vec<HashMap<(u64, u64), MetadataFacts>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn fixture_lookup(dev: u64, ino: u64) -> Option<MetadataFacts> {
    FIXTURE_MAP.with(|m| {
        m.borrow()
            .last()
            .and_then(|map| map.get(&(dev, ino)).copied())
    })
}

pub(crate) fn fixture_map_is_empty() -> bool {
    FIXTURE_MAP.with(|m| m.borrow().last().map(HashMap::is_empty).unwrap_or(true))
}

/// Pops the scoped fixture map on normal exit and on unwind, restoring any
/// enclosing map for nested use.
pub(crate) struct FixtureMapGuard;

impl Drop for FixtureMapGuard {
    fn drop(&mut self) {
        FIXTURE_MAP.with(|m| {
            m.borrow_mut().pop();
        });
    }
}

pub(crate) fn install_fixture_map() -> FixtureMapGuard {
    FIXTURE_MAP.with(|m| m.borrow_mut().push(HashMap::new()));
    FixtureMapGuard
}

pub(crate) fn register_fixture(dev: u64, ino: u64, facts: MetadataFacts) {
    FIXTURE_MAP.with(|m| {
        let mut maps = m.borrow_mut();
        if let Some(map) = maps.last_mut() {
            map.insert((dev, ino), facts);
        }
    });
}

pub(crate) fn dev_ino(path: &Path) -> (u64, u64) {
    let meta = std::fs::symlink_metadata(path).expect("fixture path must exist");
    (meta.dev(), meta.ino())
}

/// Register a real directory as a simulated root-owned deployment directory.
pub(crate) fn simulate_root_directory(path: &Path) {
    let meta = std::fs::symlink_metadata(path).expect("fixture path must exist");
    register_fixture(
        meta.dev(),
        meta.ino(),
        MetadataFacts {
            kind: ObjectKind::Directory,
            uid: 0,
            mode: 0o700,
            nlink: meta.nlink(),
            len: meta.len(),
        },
    );
}

/// Register a real regular file as a simulated root-owned, single-link private
/// file. Both deployment-file and session-journal predicates accept `0o600`.
pub(crate) fn simulate_root_file(path: &Path) {
    let meta = std::fs::symlink_metadata(path).expect("fixture path must exist");
    register_fixture(
        meta.dev(),
        meta.ino(),
        MetadataFacts {
            kind: ObjectKind::Regular,
            uid: 0,
            mode: 0o600,
            nlink: 1,
            len: meta.len(),
        },
    );
}

/// Refresh a real regular file's simulated facts to a multi-link configuration
/// so a hardlink arrangement is refused specifically for `nlink != 1`.
pub(crate) fn simulate_root_file_multilink(path: &Path) {
    let meta = std::fs::symlink_metadata(path).expect("fixture path must exist");
    register_fixture(
        meta.dev(),
        meta.ino(),
        MetadataFacts {
            kind: ObjectKind::Regular,
            uid: 0,
            mode: 0o600,
            nlink: 2,
            len: meta.len(),
        },
    );
}

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let id = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let path = std::env::temp_dir().join(format!("dw-custody-{tag}-{}", id.trim()));
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

// --- Canonical predicate vectors --------------------------------------------

#[test]
fn deploy_directory_predicate_vector() {
    let base = MetadataFacts {
        kind: ObjectKind::Directory,
        uid: 0,
        mode: 0o700,
        nlink: 2,
        len: 0,
    };
    assert!(deploy_directory(&base));
    assert!(!deploy_directory(&MetadataFacts { uid: 1, ..base }));
    assert!(!deploy_directory(&MetadataFacts {
        kind: ObjectKind::Regular,
        ..base
    }));
    assert!(!deploy_directory(&MetadataFacts {
        kind: ObjectKind::Other,
        ..base
    }));
    assert!(!deploy_directory(&MetadataFacts {
        mode: 0o722,
        ..base
    }));
}

#[test]
fn deploy_file_predicate_vector() {
    let base = MetadataFacts {
        kind: ObjectKind::Regular,
        uid: 0,
        mode: 0o600,
        nlink: 1,
        len: 1024,
    };
    assert!(deploy_file(&base));
    assert!(!deploy_file(&MetadataFacts { uid: 1, ..base }));
    assert!(!deploy_file(&MetadataFacts {
        kind: ObjectKind::Directory,
        ..base
    }));
    assert!(!deploy_file(&MetadataFacts {
        kind: ObjectKind::Other,
        ..base
    }));
    assert!(!deploy_file(&MetadataFacts {
        mode: 0o622,
        ..base
    }));
    assert!(!deploy_file(&MetadataFacts { nlink: 2, ..base }));
    assert!(deploy_file(&MetadataFacts {
        len: 134_217_728,
        ..base
    }));
    assert!(!deploy_file(&MetadataFacts {
        len: 134_217_729,
        ..base
    }));
}

#[test]
fn control_root_predicate_vector() {
    let base = MetadataFacts {
        kind: ObjectKind::Directory,
        uid: 0,
        mode: 0o700,
        nlink: 2,
        len: 0,
    };
    assert!(control_root(&base));
    assert!(!control_root(&MetadataFacts { uid: 1, ..base }));
    assert!(!control_root(&MetadataFacts {
        kind: ObjectKind::Regular,
        ..base
    }));
    assert!(!control_root(&MetadataFacts {
        mode: 0o710,
        ..base
    }));
}

#[test]
fn session_journal_predicate_vector() {
    let base = MetadataFacts {
        kind: ObjectKind::Regular,
        uid: 0,
        mode: 0o600,
        nlink: 1,
        len: 100,
    };
    assert!(session_journal(&base));
    assert!(!session_journal(&MetadataFacts { uid: 1, ..base }));
    assert!(!session_journal(&MetadataFacts {
        kind: ObjectKind::Directory,
        ..base
    }));
    assert!(!session_journal(&MetadataFacts {
        mode: 0o640,
        ..base
    }));
    assert!(!session_journal(&MetadataFacts { nlink: 2, ..base }));
    assert!(session_journal(&MetadataFacts {
        len: 16_384,
        ..base
    }));
    assert!(!session_journal(&MetadataFacts {
        len: 16_385,
        ..base
    }));
}

// --- Fixture-map isolation and restoration -----------------------------------

#[test]
fn registering_one_object_does_not_bless_another() {
    let dir = TempDir::new("isolation");
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();

    let _map = install_fixture_map();
    simulate_root_file(&a);

    let meta_a = std::fs::symlink_metadata(&a).unwrap();
    assert_eq!(observed(&meta_a).uid, 0);
    assert!(deploy_file(&observed(&meta_a)));

    let meta_b = std::fs::symlink_metadata(&b).unwrap();
    assert_ne!(observed(&meta_b).uid, 0);
    assert!(!deploy_file(&observed(&meta_b)));
}

#[test]
fn unregistered_replacement_inode_uses_real_facts() {
    let dir = TempDir::new("replacement");
    let path = dir.path().join("journal");
    std::fs::write(&path, b"first").unwrap();

    let _map = install_fixture_map();
    let old_ino = dev_ino(&path).1;
    simulate_root_file(&path);
    assert_eq!(observed(&std::fs::symlink_metadata(&path).unwrap()).uid, 0);

    // Replace the file with a fresh inode via rename; the removed inode's
    // registration must not bless the replacement.
    let replacement = dir.path().join("replacement");
    std::fs::write(&replacement, b"second").unwrap();
    let new_ino = dev_ino(&replacement).1;
    assert_ne!(old_ino, new_ino, "replacement must have a distinct inode");
    std::fs::rename(&replacement, &path).unwrap();
    let meta = std::fs::symlink_metadata(&path).unwrap();
    assert_eq!(meta.ino(), new_ino);
    assert_ne!(observed(&meta).uid, 0);
    assert!(!session_journal(&observed(&meta)));
}

#[test]
fn nested_fixture_maps_restore_the_outer_scope() {
    let dir = TempDir::new("nested");
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    let (dev_a, ino_a) = dev_ino(&a);
    let (dev_b, ino_b) = dev_ino(&b);

    let _outer = install_fixture_map();
    register_fixture(
        dev_a,
        ino_a,
        MetadataFacts {
            kind: ObjectKind::Regular,
            uid: 0,
            mode: 0o600,
            nlink: 1,
            len: 1,
        },
    );
    {
        let _inner = install_fixture_map();
        assert!(fixture_map_is_empty());
        register_fixture(
            dev_b,
            ino_b,
            MetadataFacts {
                kind: ObjectKind::Regular,
                uid: 0,
                mode: 0o600,
                nlink: 1,
                len: 1,
            },
        );
        assert!(fixture_lookup(dev_b, ino_b).is_some());
    }
    // Inner scope popped: outer registration A remains, B is gone.
    assert!(fixture_lookup(dev_a, ino_a).is_some());
    assert!(fixture_lookup(dev_b, ino_b).is_none());
}

#[test]
fn fixture_map_cleans_up_on_panic() {
    let dir = TempDir::new("panic");
    let a = dir.path().join("a");
    std::fs::write(&a, b"a").unwrap();
    let (dev, ino) = dev_ino(&a);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _map = install_fixture_map();
        register_fixture(
            dev,
            ino,
            MetadataFacts {
                kind: ObjectKind::Regular,
                uid: 0,
                mode: 0o600,
                nlink: 1,
                len: 1,
            },
        );
        panic!("fixture unwind");
    }));
    assert!(result.is_err());
    assert!(fixture_map_is_empty());
}

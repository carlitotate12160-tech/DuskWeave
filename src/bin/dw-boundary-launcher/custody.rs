//! Root-only custody facts and the canonical predicates shared by the release
//! identity and attempt-journal consumers. Facts are always extracted from real
//! filesystem metadata; ownership never accepts a caller-supplied UID.

use std::os::unix::fs::MetadataExt;

/// What a custody object is. Only directory and regular are ever accepted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ObjectKind {
    Directory,
    Regular,
    Other,
}

/// The complete typed facts a custody predicate decides on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct MetadataFacts {
    pub kind: ObjectKind,
    pub uid: u32,
    pub mode: u32,
    pub nlink: u64,
    pub len: u64,
}

/// Extract the custody-relevant facts from real filesystem metadata.
pub(super) fn facts(m: &std::fs::Metadata) -> MetadataFacts {
    let kind = if m.is_dir() {
        ObjectKind::Directory
    } else if m.is_file() {
        ObjectKind::Regular
    } else {
        ObjectKind::Other
    };
    MetadataFacts {
        kind,
        uid: m.uid(),
        mode: m.mode(),
        nlink: m.nlink(),
        len: m.len(),
    }
}

/// Facts used for a custody decision. In a test build, a thread-local fixture
/// map may replace the real facts for an already-observed `(device, inode)`.
/// A missing entry always uses the real facts; release builds have no map.
pub(super) fn observed(m: &std::fs::Metadata) -> MetadataFacts {
    let real = facts(m);
    #[cfg(test)]
    {
        if let Some(replaced) = tests::fixture_lookup(m.dev(), m.ino()) {
            return replaced;
        }
    }
    real
}

/// Deployment directories: root-owned and group/world-write-free.
pub(super) fn deploy_directory(f: &MetadataFacts) -> bool {
    f.kind == ObjectKind::Directory && f.uid == 0 && f.mode & 0o022 == 0
}

/// Deployment/executable files: root-owned, single-link, bounded size and
/// group/world-write-free.
pub(super) fn deploy_file(f: &MetadataFacts) -> bool {
    f.kind == ObjectKind::Regular
        && f.uid == 0
        && f.nlink == 1
        && f.mode & 0o022 == 0
        && f.len <= 134_217_728
}

/// Launcher control root: root-owned and fully non-group/non-world-writable.
pub(super) fn control_root(f: &MetadataFacts) -> bool {
    f.kind == ObjectKind::Directory && f.uid == 0 && f.mode & 0o077 == 0
}

/// Session journals: root-owned, single-link, bounded size, fully private.
pub(super) fn session_journal(f: &MetadataFacts) -> bool {
    f.kind == ObjectKind::Regular
        && f.uid == 0
        && f.nlink == 1
        && f.mode & 0o077 == 0
        && f.len <= 16_384
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/launcher_custody.rs"]
pub(crate) mod tests;

//! Klasifikasi debu (kemampuan #2).

use crate::scan::DustEntry;
use std::path::Path;

/// Jenis "debu" yang dikenali DustTrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DustKind {
    /// Artefak sementara aplikasi (`*.tmp`, `*~`, `~$*`).
    TempFile,
    /// Cadangan / salinan lama (`*.bak`, `*.orig`).
    Backup,
    /// Log dan jejak diagnostik (`*.log`, `*.part`).
    Log,
    /// Sampah sistem file (`Thumbs.db`, `.DS_Store`, `.fuse_hidden*`).
    SystemJunk,
    /// Editor swap (`*.swp`, `*.swo`).
    EditorSwap,
    /// Bukan debu — lolos dari heuristik.
    Clean,
}

impl DustKind {
    /// Seberapa "layak dijebak"? Semakin tinggi semakin aman dikarantina.
    pub fn trap_priority(self) -> u8 {
        match self {
            DustKind::SystemJunk => 5,
            DustKind::TempFile => 4,
            DustKind::EditorSwap => 3,
            DustKind::Backup => 2,
            DustKind::Log => 1, // log bisa bernilai historis: prioritas terendah
            DustKind::Clean => 0,
        }
    }

    /// Nama pendek untuk laporan.
    pub fn label(self) -> &'static str {
        match self {
            DustKind::TempFile => "temp",
            DustKind::Backup => "backup",
            DustKind::Log => "log",
            DustKind::SystemJunk => "system-junk",
            DustKind::EditorSwap => "editor-swap",
            DustKind::Clean => "clean",
        }
    }
}

/// Klasifikasikan satu entri berdasarkan nama & metadata.
pub fn classify(entry: &DustEntry) -> DustKind {
    let name = entry
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let stem = Path::new(&name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    if name.starts_with(".ds_store") || name == "thumbs.db" || name.starts_with(".fuse_hidden") {
        return DustKind::SystemJunk;
    }
    if name.starts_with("~$") || stem == "tmp" || stem == "temp" || name.ends_with('~') {
        return DustKind::TempFile;
    }
    if stem == "swp" || stem == "swo" {
        return DustKind::EditorSwap;
    }
    if stem == "bak" || stem == "orig" {
        return DustKind::Backup;
    }
    if stem == "log" || stem == "part" {
        return DustKind::Log;
    }
    DustKind::Clean
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(p: &str) -> DustEntry {
        DustEntry {
            path: PathBuf::from(p),
            size_bytes: 1,
            age_secs: 9999,
            is_dir: false,
        }
    }

    #[test]
    fn classifies_each_family() {
        assert_eq!(classify(&entry("a.tmp")), DustKind::TempFile);
        assert_eq!(classify(&entry("notes.txt~")), DustKind::TempFile);
        assert_eq!(classify(&entry("old.bak")), DustKind::Backup);
        assert_eq!(classify(&entry("app.log")), DustKind::Log);
        assert_eq!(classify(&entry(".DS_Store")), DustKind::SystemJunk);
        assert_eq!(classify(&entry(".main.rs.swp")), DustKind::EditorSwap);
        assert_eq!(classify(&entry("lib.rs")), DustKind::Clean);
    }

    #[test]
    fn logs_have_lowest_trap_priority() {
        assert!(DustKind::Log.trap_priority() < DustKind::TempFile.trap_priority());
        assert_eq!(DustKind::Clean.trap_priority(), 0);
    }
}

//! Pemindaian filesystem (kemampuan #1).
//!
//! Hanya membaca metadata (`read_dir` + `metadata`) — tidak pernah membuka
//! isi file untuk ditulis/dihapus.

use crate::error::{DustError, DustResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Konfigurasi pemindaian.
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Direktori akar tempat debu dicari.
    pub root: PathBuf,
    /// Kedalaman maksimum rekursi (0 = hanya akar).
    pub max_depth: usize,
    /// Nama direktori yang selalu dilewati (mis. `.git`).
    pub skip_dirs: Vec<String>,
}

impl ScanConfig {
    /// Konfigurasi baru dengan akar tertentu dan pengaturan wajar.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            max_depth: 8,
            skip_dirs: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(), // jangan terjebak menguras hasil build sendiri
            ],
        }
    }

    /// Builder: atur kedalaman maksimum.
    pub fn with_max_depth(mut self, d: usize) -> Self {
        self.max_depth = d;
        self
    }
}

/// Satu butir "debu" yang ditemukan scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DustEntry {
    /// Lokasi absolut relatif terhadap akar scan.
    pub path: PathBuf,
    /// Ukuran dalam byte (0 untuk direktori).
    pub size_bytes: u64,
    /// Umur sejak modifikasi terakhir, dalam detik.
    pub age_secs: u64,
    /// Apakah entri ini direktori?
    pub is_dir: bool,
}

impl DustEntry {
    fn age_from(mtime: SystemTime) -> u64 {
        SystemTime::now()
            .duration_since(mtime)
            .unwrap_or(Duration::ZERO)
            .as_secs()
    }
}

/// Mesin pemindai (kemampuan #1: *scan*).
#[derive(Debug, Clone)]
pub struct Scanner {
    cfg: ScanConfig,
}

impl Scanner {
    /// Buat scanner dari konfigurasi.
    pub fn new(cfg: ScanConfig) -> Self {
        Self { cfg }
    }

    /// Telusuri akar secara rekursif (iteratif, aman stack), kumpulkan
    /// kandidat debu: file dengan pola nama artefak sementara.
    pub fn walk(&self) -> DustResult<Vec<DustEntry>> {
        if !self.cfg.root.exists() {
            return Err(DustError::Io(format!(
                "akar scan tidak ada: {}",
                self.cfg.root.display()
            )));
        }
        let mut out = Vec::new();
        let mut stack: Vec<(PathBuf, usize)> = vec![(self.cfg.root.clone(), 0)];

        while let Some((dir, depth)) = stack.pop() {
            let rd = match fs::read_dir(&dir) {
                Ok(rd) => rd,
                // Direktori tak terbaca: lewati, catat sebagai peringatan via eprintln.
                Err(e) => {
                    eprintln!("[dusttrap] WARN: lewati {}: {e}", dir.display());
                    continue;
                }
            };
            for item in rd.flatten() {
                let name = item.file_name().to_string_lossy().to_string();
                let path = item.path();
                let meta = match item.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if meta.is_dir() {
                    if self.cfg.skip_dirs.iter().any(|s| s == &name) {
                        continue;
                    }
                    if depth + 1 < self.cfg.max_depth {
                        stack.push((path, depth + 1));
                    }
                    continue;
                }

                if looks_like_dust(Path::new(&name)) {
                    out.push(DustEntry {
                        path,
                        size_bytes: meta.len(),
                        age_secs: DustEntry::age_from(
                            meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                        ),
                        is_dir: false,
                    });
                }
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }
}

/// Heuristik ringan: apakah nama file terlihat seperti debu?
pub(crate) fn looks_like_dust(name: &Path) -> bool {
    let n = match name.file_name().and_then(|s| s.to_str()) {
        Some(s) => s.to_ascii_lowercase(),
        None => return false,
    };
    const DUST_EXT: &[&str] = &["tmp", "temp", "bak", "swp", "swo", "orig", "part", "log"];
    const DUST_PREFIX: &[&str] = &[".ds_store", "thumbs.db", "~$", ".fuse_hidden"];
    has_any_ext(&n, DUST_EXT) || DUST_PREFIX.iter().any(|p| n.starts_with(p))
}

fn has_any_ext(name_lower: &str, exts: &[&str]) -> bool {
    match name_lower.rsplit_once('.') {
        Some((_, ext)) => exts.contains(&ext),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristics_match_common_dust() {
        assert!(looks_like_dust(Path::new("report.tmp")));
        assert!(looks_like_dust(Path::new(".DS_Store")));
        assert!(looks_like_dust(Path::new("~$draft.docx")));
        assert!(!looks_like_dust(Path::new("src")));
        assert!(!looks_like_dust(Path::new("main.rs")));
    }

    #[test]
    fn missing_root_is_io_error() {
        let cfg = ScanConfig::new(PathBuf::from("/nonexistent-dusttrap-root"));
        let res = Scanner::new(cfg).walk();
        assert!(matches!(res, Err(DustError::Io(_))));
    }
}

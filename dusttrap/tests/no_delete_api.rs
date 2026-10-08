//! Uji statis: pastikan TIDAK ADA jalur hapus file yang terkompilasi di
//! kode produksi crate ini (di luar folder scratch milik unit test sendiri).

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                    // Abaikan file uji internal (cfg(test)) — hanya periksa jalur produksi.
                    let in_tests = p.components().any(|c| c.as_os_str() == "tests");
                    let is_unit_test_file = fs::read_to_string(&p)
                        .map(|s| s.contains("#[cfg(test)]"))
                        .unwrap_or(false);
                    if !in_tests && !is_unit_test_file {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

#[test]
fn production_code_never_calls_fs_delete_apis() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for f in rust_files(&src) {
        let text = fs::read_to_string(&f).expect("baca sumber");
        for api in ["remove_file", "remove_dir", "remove_dir_all", "unlink", "std::fs::File::delete"] {
            // Baris komentar diizinkan; panggilan kode tidak.
            for line in text.lines() {
                let t = line.trim_start();
                if t.starts_with("//") || t.starts_with("//!") {
                    continue;
                }
                assert!(
                    !line.contains(api),
                    "JALUR HAPUS DITEMUKAN di {}: {line}",
                    f.display()
                );
            }
        }
    }
}

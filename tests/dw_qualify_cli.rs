#![cfg(feature = "operator-tooling")]
//! Real Cargo-built operator binary integration tests exercising `dw-qualify` CLI branches.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dw-qualify-cli-{}-{id}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_dw_qualify(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dw-qualify"))
        .args(args)
        .output()
        .expect("spawn dw-qualify binary")
}

#[test]
fn cli_valid_fixture_tree_reports_counts_and_exits_zero() {
    let root = TempRoot::new();
    root.write(
        "README.md",
        "# Title\n[self](#title)\n[ext](https://example.invalid)\n",
    );
    let out = run_dw_qualify(&["check-links", "--root", root.0.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("OK relative links: 1"), "{stdout}");
    assert!(stdout.contains("Broken relative links: 0"), "{stdout}");
    assert!(stdout.contains("Broken anchors: 0"), "{stdout}");
    assert!(
        stdout.contains("External links (not fetched): 1"),
        "{stdout}"
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn cli_broken_link_and_anchor_report_issues_and_exit_one() {
    let root = TempRoot::new();
    root.write(
        "README.md",
        "# Heading\n[missing](absent.md)\n[bad](#absent)\n",
    );
    let out = run_dw_qualify(&["check-links", "--root", root.0.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Broken relative links: 1"), "{stdout}");
    assert!(stdout.contains("Broken anchors: 1"), "{stdout}");
    assert!(
        stdout.contains("BROKEN README.md:2 -> absent.md"),
        "{stdout}"
    );
    assert!(stdout.contains("ANCHOR README.md:3 -> #absent"), "{stdout}");
}

#[test]
fn cli_malformed_arguments_print_usage_and_exit_two() {
    for args in [&[][..], &["check-links"], &["other", "--root", "."]] {
        let out = run_dw_qualify(args);
        assert_eq!(out.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("usage: dw-qualify check-links --root DIRECTORY"),
            "{stderr}"
        );
    }
}

#[test]
fn cli_nonexistent_root_reports_io_failure_and_exit_two() {
    let root = TempRoot::new();
    let missing = root.0.join("does-not-exist");
    let out = run_dw_qualify(&["check-links", "--root", missing.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("link_check_io_failed"), "{stderr}");
}

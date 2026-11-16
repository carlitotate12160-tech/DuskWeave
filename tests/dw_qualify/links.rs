//! Characterization/negative controls for the retired Python link-check consumer.
use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("dw-links-{}-{id}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, content: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn relative_paths_encoded_spaces_parent_paths_and_duplicate_anchors() {
    let f = Fixture::new();
    f.write(
        "A B.md",
        "# `Heading` [label](https://example.invalid)\n# Heading label\n",
    );
    f.write("docs/readme.md", "[one](../A%20B.md#heading-label)\n[two](../A%20B.md#heading-label-1 \"title\")\n[self](#self)\n# Self\n");
    let report = links::check(&f.0).unwrap();
    assert_eq!(report.ok, 3);
    assert_eq!(report.external, 1);
    assert!(report.broken.is_empty() && report.bad_anchor.is_empty());
}

#[test]
fn missing_target_and_heading_are_distinct_failures_with_source_lines() {
    let f = Fixture::new();
    f.write(
        "README.md",
        "# Root\n[missing](absent.md)\n[anchor](#absent)\n",
    );
    let report = links::check(&f.0).unwrap();
    assert_eq!(report.broken, ["BROKEN README.md:2 -> absent.md"]);
    assert_eq!(report.bad_anchor, ["ANCHOR README.md:3 -> #absent"]);
}

#[test]
fn fences_comments_and_skipped_trees_are_not_link_consumers() {
    let f = Fixture::new();
    f.write(
        "README.md",
        "```md\n[x](missing.md)\n```\n<!-- [x](missing.md) -->\n",
    );
    f.write("target/ignored.md", "[x](missing.md)");
    f.write(".github/ignored.md", "[x](missing.md)");
    let report = links::check(&f.0).unwrap();
    assert_eq!(report.ok, 0);
    assert!(report.broken.is_empty());
}

#[test]
fn external_schemes_are_counted_without_network_requests() {
    let f = Fixture::new();
    f.write("README.md", "[a](https://127.0.0.1:1/) [b](http://invalid/) [c](mailto:x) [d](ftp://invalid/) [e](file:///absent)");
    let report = links::check(&f.0).unwrap();
    assert_eq!(report.external, 5);
    assert!(report.broken.is_empty());
}

#[test]
fn unicode_headings_duplicate_slugs_and_invalid_percent_escapes() {
    assert_eq!(
        markdown::slug("`Title` [label](target) & More"),
        "title-label--more"
    );
    assert_eq!(markdown::unquote("%E7%95%8C%20%25%Q0"), "界 %%Q0");
    let anchors = markdown::anchors("# 界\n# 界\n```\n# Hidden\n```\n");
    assert!(anchors.contains("界") && anchors.contains("界-1"));
    assert!(!anchors.contains("hidden"));
}

#[test]
fn directory_targets_and_non_markdown_fragments_do_not_require_heading_parsing() {
    let f = Fixture::new();
    f.write("file.txt", "data");
    f.write("docs/README.md", "# Directory\n");
    f.write(
        "README.md",
        "[dir](docs#anything) [file](file.txt#anything)",
    );
    let report = links::check(&f.0).unwrap();
    assert_eq!(report.ok, 2);
    assert!(report.bad_anchor.is_empty());
}

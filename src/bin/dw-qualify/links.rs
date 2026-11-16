//! Repository traversal and relative path/heading validation, without egress.
use super::markdown;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct Report {
    pub ok: usize,
    pub external: usize,
    pub broken: Vec<String>,
    pub bad_anchor: Vec<String>,
}

fn collect(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        collect_entry(&entry?, files)?;
    }
    Ok(())
}

fn collect_entry(entry: &fs::DirEntry, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let path = entry.path();
    if entry.file_type()?.is_dir() {
        if !skipped(&entry.file_name().to_string_lossy()) {
            collect(&path, files)?;
        }
    } else if path.extension().is_some_and(|e| e == "md") {
        files.push(path);
    }
    Ok(())
}

fn skipped(name: &str) -> bool {
    matches!(name, ".git" | "node_modules" | "target" | ".github")
}

fn external(target: &str) -> bool {
    ["http://", "https://", "mailto:", "ftp:", "file:"]
        .iter()
        .any(|prefix| target.starts_with(prefix))
}

fn anchor_matches(path: &Path, fragment: &str) -> io::Result<bool> {
    Ok(markdown::anchors(&fs::read_to_string(path)?)
        .contains(&markdown::unquote(fragment).to_lowercase()))
}

fn requires_anchor(path: &Path, link: &str, fragment: &str) -> bool {
    !fragment.is_empty()
        && !path.is_dir()
        && (path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            || link.is_empty())
}

fn classify(source: &Path, target: &str) -> io::Result<&'static str> {
    let (link, fragment) = target.split_once('#').unwrap_or((target, ""));
    let path = if link.is_empty() {
        source.to_path_buf()
    } else {
        source
            .parent()
            .unwrap_or(Path::new("."))
            .join(markdown::unquote(link))
    };
    if !path.exists() {
        return Ok("BROKEN");
    }
    if requires_anchor(&path, link, fragment) && !anchor_matches(&path, fragment)? {
        return Ok("ANCHOR");
    }
    Ok("OK")
}

pub(super) fn check(root: &Path) -> io::Result<Report> {
    let root = root.canonicalize()?;
    let mut files = Vec::new();
    collect(&root, &mut files)?;
    files.sort();
    let mut report = Report::default();
    for file in files {
        check_file(&file, &root, &mut report)?;
    }
    Ok(report)
}

fn check_file(file: &Path, root: &Path, report: &mut Report) -> io::Result<()> {
    for (line, target) in markdown::links(&fs::read_to_string(file)?) {
        if external(&target) {
            report.external += 1;
        } else {
            let kind = classify(file, &target)?;
            report.record(file, root, line, &target, kind);
        }
    }
    Ok(())
}

impl Report {
    fn record(&mut self, file: &Path, root: &Path, line: usize, target: &str, kind: &str) {
        let source = file
            .strip_prefix(root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let issue = format!("{kind} {source}:{line} -> {target}");
        match kind {
            "BROKEN" => self.broken.push(issue),
            "ANCHOR" => self.bad_anchor.push(issue),
            _ => self.ok += 1,
        }
    }
}

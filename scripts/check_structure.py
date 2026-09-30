#!/usr/bin/env python3
"""DuskWeave repository structure and hygiene check.

Enforces the file-level parts of QUALITY_BAR.md and AGENTS.md:
- every source/document file stays under the 400 LOC hard cap;
- no generic dumping-ground paths (utils/, helpers/, common/, misc/,
  managers/ directories or same-named files);
- no unresolved merge conflict markers in checked text files;
- Rust hygiene: no unwrap()/expect()/unsafe on production code paths
  (active automatically once .rs files exist; test modules and files
  under tests/ or benches/ are exempt).

Exit code 0 when clean, 1 when any violation is found.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKIP_DIRS = {".git", "node_modules", "target", ".venv", "venv", "dist", "build"}
CHECKED_EXTS = {
    ".md", ".rs", ".go", ".zig", ".c", ".h", ".cc", ".cpp", ".hpp",
    ".py", ".sh", ".toml", ".yml", ".yaml",
}
SKIP_FILES = {
    "cargo.lock", "go.sum", "go.mod", "package-lock.json", "yarn.lock",
    "poetry.lock", "uv.lock", "pipfile.lock", "composer.lock",
}
FORBIDDEN_NAMES = {"utils", "helpers", "common", "misc", "managers"}
RUST_TEST_PATH_SEGMENTS = {"tests", "benches"}
HARD_CAP = 400

MARKER_LEFT = re.compile(r"^<{7}")
MARKER_RIGHT = re.compile(r"^>{7}")
MARKER_MID = re.compile(r"^={7}\s*$")
UNWRAP_RE = re.compile(r"\.(?:unwrap|expect)\s*\(")
UNSAFE_RE = re.compile(r"\bunsafe\b")
RUST_TEST_BOUNDARY = re.compile(r"^\s*#\[\s*cfg\s*\(\s*test\s*\)\s*\]", re.M)
RUST_COMMENT = re.compile(r"^\s*//")


def iter_files(root):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for name in filenames:
            yield os.path.join(dirpath, name)


def read_text(path):
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except (OSError, UnicodeDecodeError):
        return None


def check_forbidden_path(rel):
    parts = rel.split("/")
    dirs = parts[:-1]
    stem = os.path.splitext(parts[-1])[0].lower()
    bad = [seg for seg in dirs if seg.lower() in FORBIDDEN_NAMES]
    if stem in FORBIDDEN_NAMES:
        bad.append(parts[-1])
    return bad


def check_conflict_markers(rel, lines):
    hits = []
    is_md = rel.endswith(".md")
    for i, line in enumerate(lines, 1):
        if MARKER_LEFT.match(line) or MARKER_RIGHT.match(line):
            hits.append(f"{rel}:{i}")
        elif not is_md and MARKER_MID.match(line):
            hits.append(f"{rel}:{i}")
    return hits


def check_rust_hygiene(rel, text):
    segments = rel.split("/")[:-1]
    if any(s in RUST_TEST_PATH_SEGMENTS for s in segments):
        return []
    boundary = RUST_TEST_BOUNDARY.search(text)
    production = text[: boundary.start()] if boundary else text
    hits = []
    for i, line in enumerate(production.split("\n"), 1):
        if RUST_COMMENT.match(line):
            continue
        if UNWRAP_RE.search(line):
            hits.append(f"{rel}:{i} unwrap/expect on production path")
        if UNSAFE_RE.search(line):
            hits.append(f"{rel}:{i} unsafe block")
    return hits


def main():
    oversize, forbidden, markers, rust = [], [], [], []
    for path in sorted(iter_files(ROOT)):
        rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
        name = os.path.basename(path).lower()
        ext = os.path.splitext(name)[1]

        bad = check_forbidden_path(rel)
        if bad:
            forbidden.append(f"{rel} (forbidden: {', '.join(bad)})")

        if ext not in CHECKED_EXTS or name in SKIP_FILES:
            continue
        text = read_text(path)
        if text is None:
            continue
        lines = text.split("\n")
        if len(lines) > HARD_CAP:
            oversize.append(f"{rel} ({len(lines)} lines > {HARD_CAP})")
        markers.extend(check_conflict_markers(rel, lines))
        if ext == ".rs":
            rust.extend(check_rust_hygiene(rel, text))

    failed = False
    for label, items in (
        ("Files over 400 LOC hard cap", oversize),
        ("Forbidden dumping-ground paths", forbidden),
        ("Conflict markers", markers),
        ("Rust production hygiene", rust),
    ):
        print(f"{label}: {len(items)}")
        for item in items:
            print(f"  FAIL  {item}")
        failed = failed or bool(items)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

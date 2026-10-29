#!/usr/bin/env python3
"""DuskWeave repository structure and hygiene check.

Enforces the file-level parts of QUALITY_BAR.md and AGENTS.md:
- category LOC reference points (advisory): production/tooling <=400
  (migrations and other .sql files count physical lines like any file and
  get the 400 review reference), test/benchmark files <=500,
  Markdown <=600;
- no generic dumping-ground paths (utils/, helpers/, common/, misc/,
  managers/ directories or same-named files) [FAIL];
- no unresolved merge conflict markers in checked text files [FAIL];
- Rust hygiene: no unwrap()/expect()/unsafe on production code paths [FAIL]
  (active automatically once .rs files exist; test modules and files
  under tests/ or benches/ are exempt).

Exit code 0 when clean of hard violations (file size excess is advisory),
1 when any forbidden path, conflict marker, or Rust hygiene violation is found.
"""
import os
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKIP_DIRS = {".git", "node_modules", "target", ".venv", "venv", "dist", "build"}
CHECKED_EXTS = {
    ".md", ".rs", ".go", ".zig", ".c", ".h", ".cc", ".cpp", ".hpp",
    ".py", ".ps1", ".sh", ".sql", ".toml", ".yml", ".yaml",
}
SKIP_FILES = {
    "cargo.lock", "go.sum", "go.mod", "package-lock.json", "yarn.lock",
    "poetry.lock", "uv.lock", "pipfile.lock", "composer.lock",
}
FORBIDDEN_NAMES = {"utils", "helpers", "common", "misc", "managers"}
RUST_TEST_PATH_SEGMENTS = {"tests", "benches"}
HARD_CAP = 400
TEST_CAP = 500
DOCUMENT_CAP = 600

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
            if name.lower() == "nul":
                continue
            yield os.path.join(dirpath, name)


PHYSICAL_EOL = re.compile(r"\r\n|\r|\n")


def split_physical(text):
    """Physical text lines: only LF, CRLF (one delimiter) and lone CR
    terminate a line; U+2028/U+2029/FF/VT/NEL are content. A final
    terminator closes the last line without adding a phantom line."""
    parts = PHYSICAL_EOL.split(text)
    if parts and parts[-1] == "":
        parts.pop()
    return parts


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


def file_limit(rel):
    if rel.endswith(".md"):
        return DOCUMENT_CAP
    parts = rel.split("/")
    if any(p in RUST_TEST_PATH_SEGMENTS for p in parts[:-1]) or parts[-1] == "tests.rs":
        return TEST_CAP
    return HARD_CAP


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


SDK_REVISION = "a9504f1fe3641b4d21a8c44bea4379bed4394f4b"
SDK_BASELINE_TREE = "a4673c51b9a276b7fa012f716a1de130db9aab60"
SDK_PATCH_FILES = {"src/client/web_config.rs", "src/webc/web_client.rs", "src/webc/error.rs",
                   "src/client/client_impl.rs", "src/client/client_types.rs"}
# Pin the authorized whitespace correction exactly; this is not a general patch allowance.
SDK_COSMETIC_BLOBS = {"src/adapter/adapters/zai/mod.rs": "8c5c856234d7829fdf87108a229adbb310b0f387"}


def git_object(kind, data):
    return hashlib.sha1(kind.encode() + b" " + str(len(data)).encode() + b"\0" + data).hexdigest()


def baseline_tree(entries):
    tree = {}
    for path, blob in entries:
        parts, node = path.split("/"), tree
        for name in parts[:-1]:
            node = node.setdefault(name, {})
        node[parts[-1]] = blob

    def digest(node):
        raw = bytearray()
        for name in sorted(node, key=lambda n: (n + "/" if isinstance(node[n], dict) else n).encode()):
            value = node[name]
            mode, oid = ("40000", digest(value)) if isinstance(value, dict) else ("100644", value)
            raw.extend(mode.encode() + b" " + name.encode() + b"\0" + bytes.fromhex(oid))
        return git_object("tree", raw)
    return digest(tree)


def sdk_scope(root):
    directory = Path(root) / "third_party" / "genai"
    if not directory.exists():
        return set(), []
    try:
        manifest = json.loads((directory / "source-manifest.json").read_text(encoding="utf-8"))
        if manifest.get("hash_representation") != "git-lf":
            raise ValueError("unexpected source hash representation")
        files = manifest["files"]
        if manifest["upstream_revision"] != SDK_REVISION or len(files) != 160:
            raise ValueError("unexpected snapshot revision or count")
        paths = [entry["path"] for entry in files]
        if len(set(paths)) != len(paths):
            raise ValueError("duplicate snapshot member")
        if any(PurePosixPath(p).is_absolute() or ".." in PurePosixPath(p).parts or "\\" in p for p in paths):
            raise ValueError("invalid snapshot member")
        if baseline_tree([(entry["path"], entry["upstream_blob"]) for entry in files]) != SDK_BASELINE_TREE:
            raise ValueError("baseline membership or identity differs")
        actual = {p.relative_to(directory).as_posix() for p in directory.rglob("*") if p.is_file()}
        if actual != set(paths) | {"source-manifest.json"}:
            raise ValueError("snapshot membership differs")
        for entry in files:
            path = directory / entry["path"]
            # Git text custody is LF; Windows autocrlf may materialize CRLF.
            # Normalize only that representation, never whitespace or content.
            data = path.read_bytes().replace(b"\r\n", b"\n")
            if path.is_symlink() or hashlib.sha256(data).hexdigest() != entry["sha256"]:
                raise ValueError("patched member identity differs")
            if entry["path"] not in SDK_PATCH_FILES:
                expected = SDK_COSMETIC_BLOBS.get(entry["path"], entry["upstream_blob"])
                if git_object("blob", data) != expected:
                    raise ValueError("out-of-map SDK modification")
        return {"third_party/genai/" + p for p in paths}, []
    except (OSError, ValueError, KeyError, TypeError) as error:
        return set(), [f"third_party/genai: {error}"]


def main():
    oversize, forbidden, markers, rust = [], [], [], []
    sdk, provenance = sdk_scope(ROOT)
    for path in sorted(iter_files(ROOT)):
        rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
        name = os.path.basename(path).lower()
        ext = os.path.splitext(name)[1]

        bad = check_forbidden_path(rel)
        if bad and rel not in sdk:
            forbidden.append(f"{rel} (forbidden: {', '.join(bad)})")

        if ext not in CHECKED_EXTS or name in SKIP_FILES:
            continue
        text = read_text(path)
        if text is None:
            continue
        lines = split_physical(text)
        cap = file_limit(rel)
        if len(lines) > cap and rel not in sdk:
            oversize.append(f"{rel} ({len(lines)} lines > {cap})")
        markers.extend(check_conflict_markers(rel, lines))
        if ext == ".rs" and rel not in sdk:
            rust.extend(check_rust_hygiene(rel, text))

    print(f"Files over category LOC reference point (advisory): {len(oversize)}")
    for item in oversize:
        print(f"  WARN  {item}")

    failed = False
    for label, items in (
        ("SDK provenance", provenance),
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

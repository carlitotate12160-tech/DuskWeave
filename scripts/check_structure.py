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
RUST_COMMENT = re.compile(r"^\s*//")
RUST_TOKEN = re.compile(
    r"//[^\r\n]*|/\*|(?:b|c)?r(#*)\"|(?:b|c)?\"(?:\\.|[^\"\\])*\"|"
    r"b?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\r\n])'|"
    r"'(?:r#)?[^\W\d]\w*|(?:b|c)?\"|b?'|"
    r"(?:r#)?[^\W\d]\w*|->|=>|::|[#\[\](){};!<>,=:]",
    re.S,
)
DELIM_PAIRS = {"(": ")", "[": "]", "{": "}"}
BLOCK_ITEMS = {"mod", "struct", "enum", "union", "trait", "impl", "fn", "loop", "extern", "const", "{"}
SEMI_ITEMS = {"use", "type", "static", "const", "let"}
NON_MACRO_KW = frozenset("if while match return break in loop else for let async await const continue crate dyn enum extern false fn impl mod move mut pub ref self Self static struct super trait true type unsafe use where".split())


class _HygieneError(Exception):
    pass


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
        if MARKER_LEFT.match(line) or MARKER_RIGHT.match(line) or (not is_md and MARKER_MID.match(line)):
            hits.append(f"{rel}:{i}")
    return hits


def _lex_rust(text):
    tokens, pos, n = [], 0, len(text)
    while match := RUST_TOKEN.search(text, pos):
        tok, start, pos = match[0], match.start(), match.end()
        if tok.startswith("//"):
            continue
        if tok == "/*":
            depth = 1
            while depth and (nxt := re.search(r"/\*|\*/", text[pos:])):
                depth, pos = depth + (1 if nxt[0] == "/*" else -1), pos + nxt.end()
            if depth:
                raise _HygieneError(start, "unterminated block comment")
        elif match[1] is not None:
            if (end := text.find('"' + match[1], pos)) < 0:
                raise _HygieneError(start, "unterminated raw string literal")
            pos = end + 1 + len(match[1])
            tokens.append(("lit", text[start:pos], start, pos))
        elif tok in ('"', 'b"', 'c"', "'", "b'") or (tok.startswith("'") and not tok.endswith("'") and pos < n and text[pos] == "'"):
            raise _HygieneError(start, f"unterminated {'string' if '\"' in tok else 'character'} literal")
        else:
            kind = "lit" if tok.endswith(('"', "'")) else "lifetime" if tok.startswith("'") else "ident" if tok[0].isalpha() or tok[0] == "_" else "punct"
            tokens.append((kind, tok, start, pos))
    return tokens


def _skip_balanced(tokens, idx, message):
    stack, start, i, n = [DELIM_PAIRS[tokens[idx][1]]], tokens[idx][2], idx + 1, len(tokens)
    while i < n and stack:
        t, s = tokens[i][1], tokens[i][2]
        if t in DELIM_PAIRS:
            stack.append(DELIM_PAIRS[t])
        elif t in (")", "]", "}") and t != stack.pop():
            raise _HygieneError(s, message)
        i += 1
    if stack:
        raise _HygieneError(start, message)
    return i


def _scan_to(tokens, k, stops, allow_angles, attr_start, label):
    angle, n = 0, len(tokens)
    while k < n:
        t, s = tokens[k][1], tokens[k][2]
        if angle == 0 and t in stops:
            return k
        if t in DELIM_PAIRS:
            k = _skip_balanced(tokens, k, f"unmatched '{t}' in #[cfg(test)] {label}")
        elif allow_angles and t in ("<", ">") and (t == "<" or angle):
            angle, k = angle + (1 if t == "<" else -1), k + 1
        elif t in (")", "]", "}") or (t == ";" and ";" not in stops):
            raise _HygieneError(s, f"malformed #[cfg(test)] {label} item")
        else:
            k += 1
    raise _HygieneError(attr_start, f"unterminated #[cfg(test)] {label} item")


def _skip_item_prefix(tokens, k):
    n = len(tokens)
    while k < n and tokens[k][1] == "#":
        if k + 1 >= n or tokens[k + 1][1] != "[":
            raise _HygieneError(tokens[k][2], "malformed attribute after #[cfg(test)]")
        k = _skip_balanced(tokens, k + 1, "unterminated attribute after #[cfg(test)]")
    if k < n and tokens[k][1] == "pub":
        k = _skip_balanced(tokens, k + 1, "unmatched '(' in visibility") if k + 1 < n and tokens[k + 1][1] == "(" else k + 1
    while k < n and (tokens[k][1] in ("async", "unsafe", "default") or (tokens[k][1] == "const" and k + 1 < n and tokens[k + 1][1] in ("fn", "unsafe", "extern"))):
        k += 1
    if k < n and tokens[k][1] == "extern":
        j = k + 2 if k + 1 < n and tokens[k + 1][0] == "lit" else k + 1
        return j if j < n and tokens[j][1] == "fn" else k
    return k


def _consume_macro_item(tokens, k, attr_start):
    n, m = len(tokens), k + 1
    while m + 1 < n and tokens[m][1] == "::" and tokens[m + 1][0] == "ident":
        m += 2
    if m >= n or tokens[m][1] != "!":
        return None
    m += 2 if tokens[k][1] == "macro_rules" and m + 1 < n and tokens[m + 1][0] == "ident" else 1
    if m >= n or tokens[m][1] not in DELIM_PAIRS:
        raise _HygieneError(attr_start, "malformed #[cfg(test)] macro item")
    open_ch = tokens[m][1]
    m = _skip_balanced(tokens, m, f"unmatched '{open_ch}' in #[cfg(test)] macro")
    if open_ch == "{" or (m < n and tokens[m][1] in ("}", ";")):
        return m + (1 if m < n and tokens[m][1] == ";" and open_ch != "{" else 0)
    raise _HygieneError(attr_start, "missing ';' after #[cfg(test)] macro invocation")


def _consume_cfg_test_item(tokens, idx, text=None):
    attr_start, n = tokens[idx][2], len(tokens)
    k = _skip_item_prefix(tokens, idx + 7)
    if k >= n or tokens[k][1] in (")", "]", "}"):
        raise _HygieneError(attr_start, "orphaned #[cfg(test)] attribute")
    t = tokens[k][1]
    if (t in SEMI_ITEMS and not (t == "const" and k + 1 < n and tokens[k + 1][1] == "{")) or (
            t == "extern" and k + 1 < n and tokens[k + 1][1] == "crate"):
        return _scan_to(tokens, k + (2 if t == "extern" else 1), {";"}, False, attr_start, t) + 1
    if t in BLOCK_ITEMS:
        if t in ("fn", "mod") and (k + 1 >= n or tokens[k + 1][0] != "ident"):
            raise _HygieneError(attr_start, f"malformed #[cfg(test)] {t} item")
        if t == "mod" and (k + 2 >= n or tokens[k + 2][1] not in (";", "{")):
            raise _HygieneError(attr_start, "malformed #[cfg(test)] mod boundary")
        end = _scan_to(tokens, k if t == "{" else k + 1, {"{", ";"} if t in ("mod", "struct", "fn") else {"{"}, t in ("struct", "enum", "union", "trait", "impl", "fn"), attr_start, t)
        return end + 1 if tokens[end][1] == ";" else _skip_balanced(tokens, end, f"unmatched '{{' in #[cfg(test)] {t}")
    if t == "if" and k + 1 < n and tokens[k + 1][1] == "let":
        j = k + 2
        while j < n and tokens[j][1] not in ("=", "{", "}", ")", "]", ";"):
            j = _skip_balanced(tokens, j, "unmatched delimiter in #[cfg(test)] if") if tokens[j][1] in ("(", "[") else j + 1
        j, last_call = (j + 1, False) if k + 2 < j < n and tokens[j][1] == "=" and not any(x[1] == "{" for x in tokens[k + 2:j]) else (n, False)
        while j < n and not (text is not None and any(c in text[tokens[j - 1][3]:tokens[j][2]] for c in "&|+-*%^~@")):
            kind, tok = tokens[j][0], tokens[j][1]
            if tok == "(" and tokens[j - 1][1] not in ("=", "::"):
                j, last_call = _skip_balanced(tokens, j, "unmatched '(' in #[cfg(test)] if"), True
            elif (tok == "::" and j + 1 < n and tokens[j + 1][0] == "ident") or (kind == "ident" and (tok not in NON_MACRO_KW or tok in ("self", "Self", "super", "crate"))):
                j, last_call = j + 1, False
            elif tok == "{" and last_call and ((end := _skip_balanced(tokens, j, "unmatched '{' in #[cfg(test)] if")) >= n or tokens[end][1] != "else"):
                return end
            else:
                break
    if tokens[k][0] == "ident" and t not in NON_MACRO_KW and (macro_end := _consume_macro_item(tokens, k, attr_start)) is not None:
        return macro_end
    raise _HygieneError(attr_start, f"unsupported #[cfg(test)] {t} boundary")


def _excluded_cfg_test_spans(tokens, text=None):
    spans, i, n = [], 0, len(tokens)
    while i < n:
        texts = [tok[1] for tok in tokens[i:i + 8]]
        if texts == ["#", "!", "[", "cfg", "(", "test", ")", "]"]:
            raise _HygieneError(tokens[i][2], "unsupported #![cfg(test)] inner attribute")
        if texts[:7] == ["#", "[", "cfg", "(", "test", ")", "]"]:
            end_idx = _consume_cfg_test_item(tokens, i, text)
            spans.append((tokens[i][2], tokens[end_idx - 1][3]))
            i = end_idx
        elif tokens[i][1] == "#":
            j = i + 2 if i + 1 < n and tokens[i + 1][1] == "!" else i + 1
            i = _skip_balanced(tokens, j, "unterminated attribute") if j < n and tokens[j][1] == "[" else i + 1
        elif tokens[i][0] == "ident" and tokens[i][1] not in NON_MACRO_KW and i + 1 < n and tokens[i + 1][1] == "!":
            m = i + 3 if tokens[i][1] == "macro_rules" and i + 2 < n and tokens[i + 2][0] == "ident" else i + 2
            i = _skip_balanced(tokens, m, "unterminated macro token tree") if m < n and tokens[m][1] in DELIM_PAIRS else i + 1
        else:
            i += 1
    return spans


def check_rust_hygiene(rel, text):
    if any(s in RUST_TEST_PATH_SEGMENTS for s in rel.split("/")[:-1]):
        return []
    try:
        spans = _excluded_cfg_test_spans(_lex_rust(text), text)
    except _HygieneError as err:
        return [f"{rel}:{len(PHYSICAL_EOL.findall(text[:err.args[0]])) + 1} {err.args[1]}"]
    out, prev = [], 0
    for start, end in spans:
        out.extend([text[prev:start], "".join(c if c in ("\r", "\n") else " " for c in text[start:end])])
        prev = end
    hits = []
    for i, line in enumerate(split_physical("".join(out) + text[prev:]), 1):
        if not RUST_COMMENT.match(line) and UNWRAP_RE.search(line):
            hits.append(f"{rel}:{i} unwrap/expect on production path")
        if not RUST_COMMENT.match(line) and UNSAFE_RE.search(line):
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

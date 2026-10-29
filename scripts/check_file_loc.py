#!/usr/bin/env python3
"""First-party source-file LOC growth ratchet for DuskWeave.

Enforced scope (QUALITY_BAR.md): first-party source/tooling files with
extensions .rs .go .zig .c .h .cc .cpp .hpp .py .ps1 .sh .sql, excluding
versioned SQL under migrations/ (advisory artifact category) and the
pinned third_party/genai/ snapshot (existing provenance/hygiene
controls). Other third_party paths get no vendor exemption. Markdown,
declarative configuration/manifests/workflows and the cumulative
runtime diff stay advisory under their existing owners; the standalone
structure/hygiene inventory in scripts/check_structure.py is unchanged.

Category thresholds come from check_structure.file_limit: 400 physical
lines for production/tooling, 500 for test/benchmark paths. A file at or
below its threshold may grow up to it. A new over-threshold file
(NEW_OVER), an existing compliant file crossing it (CROSSED), or any net
growth of retained over-threshold debt (REGRESSED) fails the gate.
An unchanged oversized file is RETAINED and a smaller one IMPROVED; both
stay visible as debt. Retained debt may cross a pure rename only when
exactly one deleted source and one added destination share identical
normalized text and the same category/threshold; copies, modified moves,
ambiguous identities and category-changing moves are new files.

Usage: check_file_loc.py --base <commit> [--head <commit>] [--repo <path>]

--base must resolve to a commit; there is no implicit or authored
baseline. With --head the committed candidate tree is measured; without
it the working tree is measured via Git inventory and real bytes,
covering staged, unstaged and nonignored untracked files. Deletions are
reported. Exit 0 = no regression, 1 = size regression, 2 = invalid input
or measurement failure (never a zero-count pass).
"""

import argparse
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from check_structure import (  # noqa: E402
    TEST_CAP, file_limit, split_physical)

SOURCE_EXTS = {
    ".rs", ".go", ".zig", ".c", ".h", ".cc", ".cpp", ".hpp",
    ".py", ".ps1", ".sh", ".sql",
}
MIGRATION_RE = re.compile(
    r"migrations/[0-9]{4}_[A-Za-z0-9][A-Za-z0-9_]*\.sql")
PINNED_GENAI_PREFIX = "third_party/genai/"
SCRIPT_REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GIT_TIMEOUT = 120.0


class MeasureError(Exception):
    """Invalid input or measurement failure; never a zero-count pass."""


def in_scope(rel):
    ext = os.path.splitext(rel)[1].lower()
    if ext not in SOURCE_EXTS:
        return False
    if rel.startswith(PINNED_GENAI_PREFIX):
        return False
    if ext == ".sql" and MIGRATION_RE.fullmatch(rel):
        return False
    return True


def category(rel):
    return "test" if file_limit(rel) == TEST_CAP else "production"


def _git(repo, args, stdin=None):
    try:
        proc = subprocess.run(["git", "-C", repo, *args], input=stdin,
                              capture_output=True, timeout=GIT_TIMEOUT)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise MeasureError(f"git {' '.join(args)} failed to run: {exc}")
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip()
        raise MeasureError(
            f"git {' '.join(args)} failed: {detail or proc.returncode}")
    return proc.stdout


def _decode(raw, what):
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise MeasureError(f"{what}: not valid UTF-8") from exc


def repo_root(path):
    out = _git(path, ["rev-parse", "--show-toplevel"])
    return os.path.normpath(_decode(out, "git rev-parse").strip())


def resolve_commit(repo, rev, flag):
    if not rev or not rev.strip():
        raise MeasureError(f"{flag} must name a commit")
    out = _git(repo, ["rev-parse", "--verify", f"{rev}^{{commit}}"])
    return out.decode("ascii").strip()


def count_lines(data, rel, side):
    return len(split_physical(_decode(data, f"{side}:{rel}")))


def normalized(data, rel, side):
    """Physical-line identity for rename matching: LF/CRLF/CR and a
    trailing-terminator difference are normalized away; every other
    character, including non-terminator Unicode, is significant."""
    return tuple(split_physical(_decode(data, f"{side}:{rel}")))


def cat_blobs(repo, oids):
    """Batch-read blob contents: {oid: bytes}; missing objects are errors."""
    blobs = {}
    unique = sorted(set(oids))
    if not unique:
        return blobs
    raw = _git(repo, ["cat-file", "--batch"],
               stdin=("\n".join(unique) + "\n").encode())
    pos = 0
    try:
        for _ in unique:
            header_end = raw.index(b"\n", pos)
            fields = raw[pos:header_end].split(b" ")
            if fields[1] == b"missing":
                raise MeasureError(
                    f"missing git object {fields[0].decode()}")
            size = int(fields[2])
            start = header_end + 1
            blobs[fields[0].decode()] = raw[start:start + size]
            pos = start + size + 1
    except (ValueError, IndexError) as exc:
        raise MeasureError("malformed git cat-file output") from exc
    return blobs


def tree_inventory(repo, rev):
    """{path: (mode, oid)} for every entry of a committed revision."""
    raw = _git(repo, ["ls-tree", "-r", "-z", rev])
    inv = {}
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        meta, _, path = entry.partition(b"\t")
        try:
            mode, _type, oid = meta.split(b" ")
        except ValueError as exc:
            raise MeasureError(
                f"malformed git ls-tree entry at {rev}") from exc
        inv[_decode(path, f"{rev} path")] = (
            mode.decode("ascii"), oid.decode("ascii"))
    return inv


def worktree_changes(repo, base):
    """Changed paths vs base: Git's own diff (checkout-conversion aware)
    plus nonignored untracked files; never the index as a byte proxy."""
    raw = _git(repo, ["diff", "--name-only", "-z", "--no-renames", base])
    tracked = {_decode(e, "worktree path") for e in raw.split(b"\0") if e}
    raw = _git(repo, ["ls-files", "-o", "-z", "--exclude-standard"])
    untracked = {_decode(e, "worktree path") for e in raw.split(b"\0") if e}
    raw = _git(repo, ["ls-files", "-v", "-z"])
    flagged = set()
    for entry in raw.split(b"\0"):
        if entry and entry[:1] in (b"S", b"s", b"h"):
            flagged.add(_decode(entry.partition(b" ")[2], "worktree path"))
    return tracked | untracked | flagged


def load_commit(repo, rev):
    """In-scope contents plus the full path->oid map of a revision."""
    inv = tree_inventory(repo, rev)
    oids = {rel: oid for rel, (mode, oid) in inv.items()}
    scoped = {}
    for rel, (mode, oid) in inv.items():
        if not in_scope(rel):
            continue
        if mode not in ("100644", "100755"):
            raise MeasureError(
                f"{rel}: unsupported entry mode {mode} at {rev}")
        scoped[rel] = oid
    blobs = cat_blobs(repo, scoped.values())
    return {rel: blobs[oid] for rel, oid in scoped.items()}, oids


def load_worktree_files(repo, rels):
    """Real current bytes for changed in-scope paths; absent means gone."""
    files = {}
    for rel in rels:
        path = repo
        for comp in rel.split("/"):
            path = os.path.join(path, comp)
            if os.path.islink(path):
                raise MeasureError(f"{rel}: symlink in enforced scope")
        if not os.path.exists(path):
            continue
        if not os.path.isfile(path):
            raise MeasureError(f"{rel}: not a regular file")
        try:
            with open(path, "rb") as fh:
                files[rel] = fh.read()
        except OSError as exc:
            raise MeasureError(f"{rel}: unreadable: {exc}") from exc
    return files


def rename_destinations(added, deleted, limit_of):
    """One-to-one identical normalized text + same category only."""
    by_text_del = {}
    for rel, text in deleted.items():
        by_text_del.setdefault(text, []).append(rel)
    by_text_add = {}
    for rel, text in added.items():
        by_text_add.setdefault(text, []).append(rel)
    allowed = {}
    for rel, text in added.items():
        sources = [d for d in by_text_del.get(text, [])
                   if limit_of(d) == limit_of(rel)]
        if len(sources) == 1 and len(by_text_add[text]) == 1:
            allowed[rel] = sources[0]
    return allowed


def measure(repo, base, head):
    base_files, base_oids = load_commit(repo, base)
    if head is None:
        changed = worktree_changes(repo, base)
        scoped = sorted(p for p in changed if in_scope(p))
        head_files = load_worktree_files(repo, scoped)
        try:
            candidate = resolve_commit(repo, "HEAD", "--head")
        except MeasureError:
            candidate = "unresolved"
    else:
        head_files, head_oids = load_commit(repo, head)
        changed = {p for p in set(base_oids) | set(head_oids)
                   if base_oids.get(p) != head_oids.get(p)}
        scoped = sorted(p for p in changed if in_scope(p))
        candidate = head
    other = len(changed) - len(scoped)

    deleted = {rel: normalized(base_files[rel], rel, "base")
               for rel in scoped
               if rel in base_files and rel not in head_files}
    added = {rel: normalized(head_files[rel], rel, "candidate")
             for rel in scoped
             if rel not in base_files and rel in head_files}
    renames = rename_destinations(added, deleted, file_limit)

    rows, failures = [], []
    counts = dict.fromkeys(
        ("NEW_OVER", "CROSSED", "REGRESSED", "RETAINED",
         "IMPROVED", "WITHIN", "DELETED"), 0)
    for rel in scoped:
        limit = file_limit(rel)
        note = ""
        if rel not in head_files and rel not in base_files:
            continue  # added and removed inside the candidate window
        if rel not in head_files:
            verdict = "DELETED"
            base_loc, head_loc = count_lines(
                base_files[rel], rel, "base"), None
        else:
            head_loc = count_lines(head_files[rel], rel, "candidate")
            if rel not in base_files:
                base_loc = None
                if head_loc > limit:
                    if rel in renames:
                        verdict = "RETAINED"
                        note = f"renamed-from={renames[rel]}"
                    else:
                        verdict = "NEW_OVER"
                else:
                    verdict = "WITHIN"
            else:
                base_loc = count_lines(base_files[rel], rel, "base")
                if head_loc > limit >= base_loc:
                    verdict = "CROSSED"
                elif base_loc > limit:
                    verdict = ("REGRESSED" if head_loc > base_loc
                               else "RETAINED" if head_loc == base_loc
                               else "IMPROVED")
                else:
                    verdict = ("IMPROVED" if head_loc < base_loc
                               else "WITHIN")
        counts[verdict] += 1
        if verdict in ("NEW_OVER", "CROSSED", "REGRESSED"):
            failures.append(rel)
        rows.append((rel, category(rel), base_loc, head_loc, limit,
                     verdict, note))

    unchanged_debt = sum(
        1 for rel, data in base_files.items()
        if rel not in changed
        and count_lines(data, rel, "base") > file_limit(rel))
    return {
        "base": base, "candidate": candidate, "head_mode": head is not None,
        "rows": rows, "failures": failures, "counts": counts,
        "unchanged_debt": unchanged_debt, "other_changed": other,
    }


def fmt(value, signed=False):
    if value is None:
        return "-"
    return f"{value:+d}" if signed else str(value)


def report(result):
    mode = "commit" if result["head_mode"] else "worktree"
    print(f"base={result['base']} mode={mode} "
          f"candidate={result['candidate']}")
    for rel, cat, base_loc, head_loc, limit, verdict, note in result["rows"]:
        delta = None
        if head_loc is not None:
            delta = head_loc - (base_loc or 0)
        elif base_loc is not None:
            delta = -base_loc
        excess = None if head_loc is None else max(0, head_loc - limit)
        extra = f" {note}" if note else ""
        print(f"{verdict:<9} {rel} cat={cat} base={fmt(base_loc)} "
              f"current={fmt(head_loc)} delta={fmt(delta, signed=True)} "
              f"threshold={limit} excess={fmt(excess)}{extra}")
    c = result["counts"]
    print("unchanged oversized in-scope debt files: "
          f"{result['unchanged_debt']} (retained, not listed)")
    print(f"out-of-scope changed files: {result['other_changed']}")
    print("verdicts: " + " ".join(
        f"{k.lower()}={v}" for k, v in c.items()))
    if result["failures"]:
        print("Gate FAIL: "
              + ", ".join(result["failures"])
              + " (exit 1).")
        return 1
    print("Gate PASS: no prohibited in-scope LOC growth (exit 0).")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="first-party source-file LOC growth ratchet")
    parser.add_argument("--base", required=True)
    parser.add_argument("--head")
    parser.add_argument("--repo", default=SCRIPT_REPO)
    args = parser.parse_args(argv)
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(errors="backslashreplace")
        except (AttributeError, ValueError):
            pass
    try:
        root = repo_root(args.repo)
        base = resolve_commit(root, args.base, "--base")
        head = None
        if args.head is not None:
            head = resolve_commit(root, args.head, "--head")
        return report(measure(root, base, head))
    except MeasureError as exc:
        print(f"loc check error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())

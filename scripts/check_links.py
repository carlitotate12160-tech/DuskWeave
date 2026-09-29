#!/usr/bin/env python3
"""DuskWeave document link check.

Verifies every Markdown link in the repository:
- relative link targets must exist on disk;
- fragment anchors (#section) must resolve to a heading slug (GitHub rules);
- external http(s) links are listed but not fetched (CI must stay deterministic).

Exit code 0 when clean, 1 when any broken relative link or anchor is found.
"""
import os
import re
import sys
from urllib.parse import unquote

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
HEADING_RE = re.compile(r"^#{1,6}\s+(.*)$", re.M)
FENCE_RE = re.compile(r"```.*?```", re.S)
SKIP_DIRS = {".git", "node_modules", "target", ".github"}


def gh_slug(heading: str) -> str:
    h = heading.strip().lower()
    h = re.sub(r"`([^`]*)`", r"\1", h)
    h = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", h)
    h = re.sub(r"[^\w\s\-\u00C0-\uFFFF]", "", h, flags=re.UNICODE)
    return h.replace(" ", "-")


def collect_md(root):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for name in filenames:
            if name.endswith(".md"):
                yield os.path.join(dirpath, name)


def anchors_of(path):
    try:
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
    except OSError:
        return set()
    text = FENCE_RE.sub("", text)
    counts = {}
    for m in HEADING_RE.finditer(text):
        base = gh_slug(m.group(1))
        n = counts.get(base, 0)
        counts[base] = n + 1
        yield base if n == 0 else f"{base}-{n}"


_anchor_cache = {}


def get_anchors(path):
    if path not in _anchor_cache:
        _anchor_cache[path] = set(anchors_of(path))
    return _anchor_cache[path]


def main():
    broken, bad_anchor, external, ok = [], [], [], 0
    for path in sorted(collect_md(ROOT)):
        rel_src = os.path.relpath(path, ROOT).replace(os.sep, "/")
        with open(path, encoding="utf-8") as fh:
            raw = fh.read()
        text = FENCE_RE.sub("", raw)
        text = re.sub(r"<!--.*?-->", "", text, flags=re.S)
        for m in LINK_RE.finditer(text):
            target = m.group(1).strip("<>")
            line = raw[: m.start()].count("\n") + 1
            if target.startswith(("http://", "https://", "mailto:", "ftp:", "file:")):
                external.append(f"{rel_src}:{line} -> {target}")
                continue
            link_path, _, anchor = target.partition("#")
            resolved = (
                os.path.normpath(os.path.join(os.path.dirname(path), unquote(link_path)))
                if link_path
                else path
            )
            if link_path and not os.path.exists(resolved):
                broken.append(f"{rel_src}:{line} -> {target}")
                continue
            if anchor and not os.path.isdir(resolved):
                if resolved.lower().endswith(".md") or not link_path:
                    if unquote(anchor).lower() not in get_anchors(resolved):
                        bad_anchor.append(f"{rel_src}:{line} -> {target}")
                        continue
            ok += 1

    print(f"OK relative links: {ok}")
    print(f"Broken relative links: {len(broken)}")
    for item in broken:
        print(f"  BROKEN  {item}")
    print(f"Broken anchors: {len(bad_anchor)}")
    for item in bad_anchor:
        print(f"  ANCHOR  {item}")
    print(f"External links (listed, not fetched): {len(external)}")
    for item in external:
        print(f"  EXT     {item}")
    return 1 if broken or bad_anchor else 0


if __name__ == "__main__":
    sys.exit(main())

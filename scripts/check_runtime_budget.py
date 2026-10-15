#!/usr/bin/env python3
"""Runtime-diff budget check for DuskWeave.

Counts added+deleted physical lines (git numstat, renames disabled) for
business-runtime paths only: src/*.rs, migrations/*.sql, Cargo.toml,
rust-toolchain.toml. Lockfile, tests, docs and CI-only tooling are outside
the count.

Reference points (QUALITY_BAR.md):
- Preferred ceiling: 300 LOC;
- Review threshold: 400 LOC;
- Reference ceiling: 600 LOC.

A crossing produces an advisory warning and reviewer trigger; measurement
exits 0 on success. Invalid input, bad git references, or binary production
changes fail with non-zero exit codes.

Usage: check_runtime_budget.py --base <commit> --head <commit|INDEX>
INDEX diffs the staged index against base (pre-commit checks).
"""

import argparse
import subprocess
import sys

CAP = 600
REVIEW_THRESHOLD = 400
PREFERRED_CAP = 300


def is_runtime_path(path: str) -> bool:
    if path in ("Cargo.toml", "rust-toolchain.toml"):
        return True
    if path.startswith("src/") and path.endswith(".rs"):
        return True
    if path.startswith("migrations/") and path.endswith(".sql"):
        return True
    return False


def git(repo: str, args: list[str]) -> str:
    out = subprocess.run(
        ["git", "-C", repo, *args], capture_output=True, text=True
    )
    if out.returncode != 0:
        raise SystemExit(f"error: git {' '.join(args)} failed: {out.stderr.strip()}")
    return out.stdout


def numstat(repo: str, base: str, head: str) -> list[tuple[str, str, str]]:
    if head == "INDEX":
        raw = git(repo, ["diff", "--cached", "--numstat", "--no-renames", base])
    else:
        git(repo, ["rev-parse", "--verify", f"{head}^{{commit}}"])
        raw = git(repo, ["diff", "--numstat", "--no-renames", base, head])
    rows = []
    for line in raw.splitlines():
        parts = line.split("\t")
        if len(parts) >= 3:
            rows.append((parts[0], parts[1], parts[2]))
    return rows


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--base", required=True)
    p.add_argument("--head", required=True)
    p.add_argument("--repo", default=".")
    a = p.parse_args()

    for ref in (a.base,) + (() if a.head == "INDEX" else (a.head,)):
        git(a.repo, ["rev-parse", "--verify", f"{ref}^{{commit}}"])

    total = 0
    counted: list[tuple[str, int]] = []
    for added, deleted, path in numstat(a.repo, a.base, a.head):
        if not is_runtime_path(path):
            continue
        if added == "-" or deleted == "-":
            print(f"error: binary production change: {path}")
            return 2
        n = int(added) + int(deleted)
        total += n
        counted.append((path, n))

    resolved_base = git(
        a.repo, ["rev-parse", "--verify", f"{a.base}^{{commit}}"]
    ).strip()
    print(f"base={resolved_base} head={a.head}")
    for path, n in counted:
        print(f"{n:>5}  {path}")
    print(f"total runtime changed lines: {total} (reference ceiling {CAP})")
    if total > CAP:
        print(f"warning: runtime diff exceeds {CAP} LOC reference ceiling: "
              "resolve cohesion/ownership review before merge")
    if total > REVIEW_THRESHOLD:
        print(f"review_required=true threshold={REVIEW_THRESHOLD}: "
              "resolve cohesion/ownership and safe-merge review before acceptance")
    elif total > PREFERRED_CAP:
        print(f"note: above preferred {PREFERRED_CAP}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

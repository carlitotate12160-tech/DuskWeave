#!/usr/bin/env python3
"""Runtime-diff budget gate for DuskWeave.

Counts added+deleted physical lines (git numstat, renames disabled) for
business-runtime paths only: src/*.rs, migrations/*.sql, Cargo.toml,
rust-toolchain.toml. Lockfile, tests, docs and CI-only tooling are outside
the count. Fails above the hard cap or on binary production changes.

Default cap is 600; above 400 emits an explicit review trigger.
A narrow one-shot exception exists for the initial
DW-IMPLEMENT-M0A delivery: it applies only when the resolved base is the
pinned protected-master baseline (M0A_BASE), the candidate tree (INDEX or
commit — never the dirty worktree file) contains the exact M0A exception record in
QUALITY_BAR.md exactly once, and every changed runtime path is inside the
declared M0A runtime map. The initial native-inference delivery has a separate
fixed record, base and 26-path map below, with the same candidate-tree checks.
Anything else uses the default 600; binary runtime changes always fail.

Usage: check_runtime_budget.py --base <commit> --head <commit|INDEX>
INDEX diffs the staged index against base (pre-commit checks).
"""

import argparse
import subprocess
import sys

HARD_CAP = 600
REVIEW_THRESHOLD = 400
PREFERRED_CAP = 300

# Pinned to the protected master at M0A delivery time; originally f52cfcc,
# repointed after docs-only PR #6 advanced master (no runtime drift).
M0A_BASE = "a9da047b2bf0bf4822536187ab3e1734ac56fc12"
M0A_CAP = 1200
M0A_RECORD = (
    "M0A_RUNTIME_DIFF_EXCEPTION: DW-IMPLEMENT-M0A; "
    "base=a9da047b2bf0bf4822536187ab3e1734ac56fc12; cap=1200; initial-only"
)
M0A_RECORD_KEY = "M0A_RUNTIME_DIFF_EXCEPTION"
M0A_RUNTIME_MAP = frozenset(
    {
        "Cargo.toml",
        "rust-toolchain.toml",
        "src/lib.rs",
        "src/mission.rs",
        "src/registration.rs",
        "src/trajectory.rs",
        "src/postgres_mission.rs",
        "src/postgres_trajectory.rs",
        "src/input.rs",
        "src/main.rs",
        "migrations/0001_mission_registration.sql",
    }
)

NATIVE_BASE = "d2b34e5fdcb6cc774b12388d928cd7cd5954fb94"
NATIVE_CAP = 3200
NATIVE_RECORD_KEY = "M1_NATIVE_INFERENCE_RUNTIME_DIFF_EXCEPTION"
NATIVE_RECORD = (
    "M1_NATIVE_INFERENCE_RUNTIME_DIFF_EXCEPTION: DW-IMPLEMENT-M1-NATIVE-INFERENCE; "
    "base=d2b34e5fdcb6cc774b12388d928cd7cd5954fb94; cap=3200; initial-only"
)
NATIVE_RUNTIME_MAP = frozenset({
    "Cargo.toml",
    "migrations/0012_native_inference.sql",
    "migrations/0013_inference_profile_and_continuation.sql",
    "src/database_config.rs",
    "src/database_wait.rs",
    "src/lib.rs",
    "src/m1_cli.rs",
    "src/m1_context_capacity.rs",
    "src/m1_context_cli.rs",
    "src/m1_context_input.rs",
    "src/m1_inference_permission.rs",
    "src/m1_reasoning/mod.rs",
    "src/m1_reasoning/bounded_json.rs",
    "src/m1_reasoning/decision_attempt.rs",
    "src/m1_reasoning/inference.rs",
    "src/m1_reasoning/postgres.rs",
    "src/m1_reasoning/proposal.rs",
    "src/m1_reasoning/provider_diagnostic.rs",
    "src/m1_reasoning/request.rs",
    "src/m1_reasoning/response.rs",
    "src/m1_reasoning/transport.rs",
    "src/m1_reasoning_cli.rs",
    "src/main.rs",
    "src/postgres_reasoning_history.rs",
    "src/postgres_trajectory.rs",
    "src/postgres_trajectory_journal.rs",
})


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


def git_show(repo: str, spec: str) -> str | None:
    out = subprocess.run(
        ["git", "-C", repo, "show", spec], capture_output=True, text=True
    )
    return out.stdout if out.returncode == 0 else None


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


def candidate_exception_cap(repo: str, head: str, native: bool = False) -> int | None:
    """A fixed cap only when the candidate records exactly one matching record."""
    spec = ":QUALITY_BAR.md" if head == "INDEX" else f"{head}:QUALITY_BAR.md"
    content = git_show(repo, spec)
    if content is None:
        return None
    key, record, cap = ((NATIVE_RECORD_KEY, NATIVE_RECORD, NATIVE_CAP) if native
                        else (M0A_RECORD_KEY, M0A_RECORD, M0A_CAP))
    records = [
        line.strip()
        for line in content.splitlines()
        if line.strip().startswith(key)
    ]
    return cap if records == [record] else None


def resolve_cap(repo: str, base: str, head: str, runtime_paths: list[str]) -> tuple[int, str]:
    resolved_base = git(repo, ["rev-parse", "--verify", f"{base}^{{commit}}"]).strip()
    if (
        resolved_base == M0A_BASE
        and candidate_exception_cap(repo, head) == M0A_CAP
        and all(p in M0A_RUNTIME_MAP for p in runtime_paths)
    ):
        return M0A_CAP, "m0a_exception"
    if (
        resolved_base == NATIVE_BASE
        and candidate_exception_cap(repo, head, native=True) == NATIVE_CAP
        and all(p in NATIVE_RUNTIME_MAP for p in runtime_paths)
    ):
        return NATIVE_CAP, "m1_native_inference_exception"
    return HARD_CAP, "default"


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

    cap, policy = resolve_cap(a.repo, a.base, a.head, [p for p, _ in counted])
    resolved_base = git(
        a.repo, ["rev-parse", "--verify", f"{a.base}^{{commit}}"]
    ).strip()
    print(f"policy={policy} cap={cap} base={resolved_base} head={a.head}")
    for path, n in counted:
        print(f"{n:>5}  {path}")
    print(f"total runtime changed lines: {total} (cap {cap})")
    if total > cap:
        print(f"error: runtime diff exceeds {cap} LOC cap")
        return 1
    if total > REVIEW_THRESHOLD:
        print(f"review_required=true threshold={REVIEW_THRESHOLD}: "
              "resolve cohesion/ownership and safe-merge review before acceptance")
    if cap == HARD_CAP and total > PREFERRED_CAP:
        print(f"note: above preferred {PREFERRED_CAP}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

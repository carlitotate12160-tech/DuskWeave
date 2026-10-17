#!/usr/bin/env python3
"""Base-to-candidate McCabe ratchet for the DuskWeave complexity gate.

Owns baseline resolution and per-function comparison. The validated base
commit's tracked src/**/*.rs inventory and bytes are extracted from Git
into an isolated temporary tree — the user worktree is never checked out
or reset and no user-authored baseline report is trusted — then measured
by the same pinned analyzer invocation as the candidate.

Functions match across revisions by (file, lexical owning scope, function
name); line numbers and sibling indices are never cross-revision identity.
A rename or move that cannot be matched is a new function, not laundered
debt. Ambiguous keys (for example several anonymous closures under one
lexical parent) receive no individual allowance: their over-cap own-values
are compared as sorted multisets, so an unchanged or improved group is
explicit retained debt while any worsening or extra over-cap member is a
gate failure. Matching ambiguity is always reported, never a silent PASS.

Statuses: NEW, REGRESSED, RETAINED, IMPROVED, RESOLVED, REMOVED, plus
AMBIGUOUS group reporting. CheckError marks input/base/report failures;
the CLI boundary maps the verdict to exit 0/1 and failures to exit 2.
"""

import os

import complexity_reports as reports

NEW = "new"
REGRESSED = "regressed"
RETAINED = "retained"
IMPROVED = "improved"
RESOLVED = "resolved"
COMPLIANT = "compliant"
AMBIGUOUS = "ambiguous"
REMOVED = "removed"


def resolve_base(git, base):
    """Validated commit SHA for --base; unresolvable refs fail closed."""
    try:
        return (
            git("rev-parse", "--verify", f"{base}^{{commit}}")
            .decode("ascii", "replace")
            .strip()
        )
    except reports.CheckError as exc:
        raise reports.CheckError(
            f"--base {base!r} does not resolve to a commit: {exc}"
        ) from exc


def base_inventory(git, sha):
    """Tracked src/**/*.rs inventory at the base commit; empty fails."""
    out = git("ls-tree", "-r", "-z", "--name-only", sha, "--", "src")
    files = sorted(
        p
        for p in out.decode("utf-8", "replace").split("\0")
        if p.startswith("src/") and p.endswith(".rs")
    )
    if not files:
        raise reports.CheckError(
            f"empty base src/**/*.rs inventory at {sha}"
        )
    return files


def extract_base(git, sha, files, root):
    """Write base blob bytes into an isolated tree rooted at *root*.

    ``git show`` emits raw blob content; any missing or unreadable base
    source fails the measurement instead of silently skipping it.
    """
    for rel in files:
        try:
            data = git("show", f"{sha}:{rel}")
        except reports.CheckError as exc:
            raise reports.CheckError(
                f"base source unavailable at {sha}: {rel}: {exc}"
            ) from exc
        dest = os.path.join(root, *rel.split("/"))
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        with open(dest, "wb") as fh:
            fh.write(data)


def match_key(m):
    """Cross-revision identity: file + lexical owning scope + name."""
    return (m["file"], tuple(m["scope"]), m["name"])


def _group(measurements):
    groups = {}
    for m in measurements:
        groups.setdefault(match_key(m), []).append(m)
    return groups


def _over_caps(group):
    return sorted(m["own"] for m in group if m["own"] > m["cap"])


def _key_identity(k):
    scope = "::".join(k[1])
    return f"{k[0]} {scope + '::' if scope else ''}{k[2]}"


def classify(base, head):
    """Annotate each head measurement with status/base fields.

    Unique keys match 1:1. Duplicated keys match only as a group: the
    sorted over-cap own-values must not grow or worsen, else the group is
    an explicit ambiguous regression. Returns the verdict consumed by the
    CLI and the summary projection.
    """
    base_groups = _group(base)
    head_groups = _group(head)
    counts = {
        NEW: 0, REGRESSED: 0, RETAINED: 0, IMPROVED: 0, RESOLVED: 0,
        COMPLIANT: 0, AMBIGUOUS: 0, REMOVED: 0,
    }
    failures, debt, resolved, removed = [], [], [], []
    for h in head:
        k = match_key(h)
        bg = base_groups.get(k)
        if bg is None:
            status = NEW
        elif len(head_groups[k]) > 1 or len(bg) > 1:
            status = AMBIGUOUS
        else:
            b = bg[0]
            over_h = h["own"] > h["cap"]
            over_b = b["own"] > b["cap"]
            if over_h and over_b:
                if h["own"] > b["own"]:
                    status = REGRESSED
                elif h["own"] < b["own"]:
                    status = IMPROVED
                else:
                    status = RETAINED
            elif over_h:
                status = REGRESSED
            elif over_b:
                status = RESOLVED
            else:
                status = COMPLIANT
            h["base_own"] = b["own"]
            h["base_cap"] = b["cap"]
            h["base_identity"] = b["identity"]
        h["status"] = status
        counts[status] += 1
        if status in (NEW, REGRESSED) and h["own"] > h["cap"]:
            failures.append(h)
        elif status in (RETAINED, IMPROVED):
            debt.append(h)
        elif status == RESOLVED:
            resolved.append(h)

    ambiguous_groups = []
    for k, bg in base_groups.items():
        hg = head_groups.get(k)
        if hg is None:
            removed.extend(bg)
            counts[REMOVED] += len(bg)
            continue
        if len(bg) == 1 and len(hg) == 1:
            continue
        base_over = _over_caps(bg)
        head_over = _over_caps(hg)
        regressed = len(head_over) > len(base_over) or any(
            hv > bv for hv, bv in zip(head_over, base_over)
        )
        ambiguous_groups.append(
            {
                "identity": _key_identity(k),
                "file": k[0],
                "scope": list(k[1]),
                "name": k[2],
                "members": len(hg),
                "head_over": head_over,
                "base_over": base_over,
                "regressed": regressed,
            }
        )

    return {
        "counts": counts,
        "failures": failures,
        "debt": debt,
        "resolved": resolved,
        "removed": removed,
        "ambiguous_groups": ambiguous_groups,
        "gate_fail": bool(failures)
        or any(g["regressed"] for g in ambiguous_groups),
    }

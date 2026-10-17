#!/usr/bin/env python3
"""Base-to-candidate McCabe ratchet for the DuskWeave complexity gate.

Owns baseline resolution and per-function comparison. The validated base
commit's tracked src/**/*.rs inventory and bytes are extracted from Git
into an isolated temporary tree — the user worktree is never checked out
or reset and no user-authored baseline report is trusted — then measured
by the same pinned analyzer invocation as the candidate.

Functions match across revisions by (file, lexical owning scope, function
name); line numbers and sibling indices are never cross-revision identity.
A match is unambiguous only when the function's own key and every
enclosing function/closure key are unique in both revisions; ambiguity
propagates to descendants, so a unique leaf under an ambiguous ancestor
cannot inherit retained debt individually. A rename or move that cannot
be matched is a new function, not laundered debt. Ambiguous members
(direct or inherited) receive no individual allowance: a member at or
below its threshold passes, while an over-threshold member is permitted
only when the nearest enclosing named function/method that is itself
unambiguous — unique key and safe ancestry in both revisions — has
byte-identical source span content (line endings normalized). Named
owners below ambiguous function ancestry are skipped and the search
continues outward. An unchanged safe owner proves the ambiguous group is
retained debt; a changed owner, an absent or non-safe owner, or an
unavailable proof is blocked ambiguity and fails the gate. Multiset
dominance never proves individual members unchanged.

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


def _source_lines(root, rel):
    """Measured source file lines with line endings normalized.

    Missing or undecodable content is an integrity failure (exit 2), not
    a gate verdict.
    """
    path = os.path.join(root, *rel.split("/")) if root else rel
    try:
        with open(path, "rb") as fh:
            data = fh.read()
    except OSError as exc:
        raise reports.CheckError(
            f"source unavailable for owner proof: {rel}: {exc}"
        ) from exc
    try:
        return data.decode("utf-8").splitlines()
    except UnicodeDecodeError as exc:
        raise reports.CheckError(
            f"source not decodable UTF-8 for owner proof: {rel}: {exc}"
        ) from exc


def _span_lines(root, m):
    """Line content of measurement m's span; invalid spans fail closed."""
    lines = _source_lines(root, m["file"])
    start, end = m["start_line"], m["end_line"]
    if not 1 <= start <= end <= len(lines):
        raise reports.CheckError(
            f"invalid owner span {start}-{end} in {m['file']} "
            f"({len(lines)} lines)"
        )
    return lines[start - 1 : end]


def _ambiguous_ancestor(m, base_groups, head_groups):
    """Innermost enclosing function/closure key that is not unique in both
    revisions; None when the complete function ancestry is unambiguous.

    Only scope segments whose node kind is ``function`` produce ancestor
    measurements; impl/trait segments are never function keys. A function
    ancestor key absent from either revision means the lexical context
    changed kind or was not measured there — conservatively ambiguous.
    """
    bad = None
    file, scope = m["file"], m["scope"]
    for i, name in enumerate(scope):
        if m["scope_kinds"][i] != "function":
            continue
        ak = (file, tuple(scope[:i]), name)
        if (
            len(head_groups.get(ak, ())) != 1
            or len(base_groups.get(ak, ())) != 1
        ):
            bad = ak
    return bad


def _member_ambiguous(h, b, base_groups, head_groups):
    """True when the matched member cannot be proven to carry identity.

    Direct ambiguity: the member's own key is duplicated in either
    revision. Inherited ambiguity: an enclosing function/closure key is
    duplicated or one-sided, so the leaf's unique key does not establish
    which ancestor binding it belongs to.
    """
    k = match_key(h)
    if len(head_groups[k]) != 1 or len(base_groups[k]) != 1:
        return True
    return (
        _ambiguous_ancestor(h, base_groups, head_groups) is not None
        or _ambiguous_ancestor(b, base_groups, head_groups) is not None
    )


def _owner_key(k, h0, b0, base_groups, head_groups):
    """Innermost safe named function owner for an ambiguous group.

    A safe owner is a named enclosing function whose own key is unique in
    both revisions and whose own function ancestry is unambiguous there;
    named owners below ambiguous function ancestry are skipped and the
    search continues outward. Non-function scope segments (impls/traits)
    can never be owners, and a segment that is a function on only one
    side of the revision boundary is skipped.
    """
    file, scope, _ = k
    for i in range(len(scope) - 1, -1, -1):
        if (
            scope[i] == "<anonymous>"
            or h0["scope_kinds"][i] != "function"
            or b0["scope_kinds"][i] != "function"
        ):
            continue
        ok = (file, scope[:i], scope[i])
        bs = base_groups.get(ok, ())
        hs = head_groups.get(ok, ())
        if (
            len(bs) == 1
            and len(hs) == 1
            and _ambiguous_ancestor(bs[0], base_groups, head_groups) is None
            and _ambiguous_ancestor(hs[0], base_groups, head_groups) is None
        ):
            return ok
    return None


def classify(base, head, base_root, head_root):
    """Annotate each head measurement with status/base fields.

    A key matches 1:1 only when its own key and every enclosing
    function/closure key are unique in both revisions; ambiguous ancestry
    makes the member ambiguous even when its own key is unique. Ambiguous
    members never match individually: a group with no over-cap head member
    is compliant, and a group with over-cap head members is retained debt
    only when a safe enclosing named owner — itself unambiguous in both
    revisions — has unchanged source span content; otherwise it is blocked
    ambiguity and fails the gate. Returns the verdict consumed by the CLI
    and the summary projection.
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
        elif _member_ambiguous(h, bg[0], base_groups, head_groups):
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
        direct = len(bg) > 1 or len(hg) > 1
        ancestor = _ambiguous_ancestor(hg[0], base_groups, head_groups)
        if ancestor is None:
            ancestor = _ambiguous_ancestor(
                bg[0], base_groups, head_groups
            )
        if not direct and ancestor is None:
            continue
        head_over = _over_caps(hg)
        entry = {
            "identity": _key_identity(k),
            "file": k[0],
            "scope": list(k[1]),
            "name": k[2],
            "members": len(hg),
            "head_over": head_over,
            "base_over": _over_caps(bg),
            "ambiguity": "direct" if direct else "inherited",
            "ancestor": _key_identity(ancestor) if ancestor else None,
            "owner": None,
            "owner_base_span": None,
            "owner_head_span": None,
        }
        if not head_over:
            entry.update(
                disposition="compliant",
                blocked=False,
                reason="no over-cap member; under-cap members pass "
                "without owner proof",
            )
        else:
            owner = _owner_key(
                k, hg[0], bg[0], base_groups, head_groups
            )
            if owner is not None:
                b_owner = base_groups[owner][0]
                h_owner = head_groups[owner][0]
                entry.update(
                    owner=_key_identity(owner),
                    owner_base_span=[
                        b_owner["start_line"], b_owner["end_line"]
                    ],
                    owner_head_span=[
                        h_owner["start_line"], h_owner["end_line"]
                    ],
                )
            if owner is None:
                entry.update(
                    disposition="blocked",
                    blocked=True,
                    reason="no uniquely matched named owner; ambiguous "
                    "over-cap members cannot be proven unchanged",
                )
            elif not base_root:
                entry.update(
                    disposition="blocked",
                    blocked=True,
                    reason="base source tree unavailable; owner proof "
                    "cannot be established",
                )
            elif _span_lines(base_root, b_owner) == _span_lines(
                head_root, h_owner
            ):
                entry.update(
                    disposition="retained",
                    blocked=False,
                    reason="owner source span unchanged; ambiguous "
                    "debt proven retained",
                )
            else:
                entry.update(
                    disposition="blocked",
                    blocked=True,
                    reason="owner source span changed; ambiguous "
                    "over-cap members cannot be proven unchanged",
                )
        ambiguous_groups.append(entry)

    return {
        "counts": counts,
        "failures": failures,
        "debt": debt,
        "resolved": resolved,
        "removed": removed,
        "ambiguous_groups": ambiguous_groups,
        "gate_fail": bool(failures)
        or any(g["blocked"] for g in ambiguous_groups),
    }

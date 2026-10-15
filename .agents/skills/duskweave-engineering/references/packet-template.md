# DuskWeave Bounded Packet Template

Use only when authoring a packet. Fill every field from verified DuskWeave sources.
Do not publish placeholders as ready-to-execute instructions. Reference permanent
authority instead of repeating it. Routine fixes stay in the same branch/PR; compiler
and test failures require correction before merge and do not automatically require
another packet.

---

PROJECT: DuskWeave
MODE: DESIGN | IMPLEMENT | FIX
PACKET: <exact identifier>
WORKSPACE: <verified path>
BRANCH: <actual delivery branch>
BASE: <verified full commit>
OUTCOME: <one coherent reviewable outcome and its offensive/MVP contribution>

Execute the assigned packet directly. Do not return another architecture plan.

## Preconditions & Base / Workspace
- Verified repository identity, worktree (`git worktree list`), branch, and base commit.
- Preserve unrelated worktrees and untracked files.
- Local resources: process-scoped build target, owned PostgreSQL service, fresh disposable test database/login if required.
- Git is candidate custody for ordinary changes: base commit, committed PR head, git diff, and CI. Do not embed candidate HEAD SHA in commit files.

## Necessary Scope (Allowed Files)
- Exact editable file paths. Include required test/consumer changes established by inspection.
- List read-only authority/caller/consumer references separately.
- Do not authorize arbitrary wildcard edits.

## Decisions to Implement
- Concrete architectural and implementation decisions captured by accepted PRDs/ADRs and packet.
- Engineering metrics (runtime diff, file LOC, McCabe, function span) are advisory review reference points under QUALITY_BAR.md, not hard ceilings. A crossing produces a warning and reviewer decision: retain with a concrete cohesion reason or refactor in the same PR.
- No budget exception ledgers, separate authorization packets, or pure-dispatch digest allowances are required for numerical crossings.

## Acceptance Criteria
- Exact observable assertions proving completion.
- Grounded in requirements, owner contracts, failure boundaries, and observable evidence.
- Wiring for runtime slices: traced path from authorized entrypoint through changed production component to consumer and observable result.

## Relevant Checks & Validation
- DESIGN: document conventions, authority references, links, invariants; runtime wiring is N/A.
- IMPLEMENT/FIX: unit, integration, and scenario tests; conflict markers, hygiene, formatting, and Clippy.
- Advisory metrics reported in `metrics` job; successful measurement exits 0 even if thresholds are exceeded.
- Material failures (broken tests, compiler errors, hygiene violations, invalid input) must be corrected before merge in the same PR.

## Genuine STOP Conditions
Pause implementation only for:
- Unresolved or ambiguous authority (missing PRD/ADR)
- Violated architectural invariants (INV-001 through INV-007)
- Material scope expansion beyond assigned outcome
- New unexpected cross-domain dependencies or God Object pressure
- Unrelated work that cannot be safely preserved

Do not STOP for an advisory metric crossing, a lint number, or a routine implementation defect.

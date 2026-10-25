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
- Maintenance per QUALITY_BAR.md: one inspected/no-trigger line for routine changes; when triggered, four brief facts (mechanism/alternative/reason, estimated additions/removals by category, new responsibilities or none, retirement/replacement evidence or retained consumer). Reuse existing decisions; no separate artifact.
- Fix the mechanism, consumer scope and permitted operational steps before handoff. Estimates do not authorize scope expansion; routine local choices remain with the executor.
- For affected contracts, specify producer/artifact/consumer semantics (units, identity role/order and state), actual serialized byte bounds including request/configuration/repair, and lifecycle timing against freshness/deadlines. Preserve required information and original evidence times.
- Runtime diff, file LOC and function span are advisory review reference points under QUALITY_BAR.md, not hard ceilings; a crossing produces a warning and reviewer decision: retain with a concrete cohesion reason or refactor in the same PR. McCabe is an enforced per-function ratchet (QUALITY_BAR.md canonical): a packet that must add or worsen an over-threshold function names the function, reason and smallest remedy.
- No budget exception ledgers, separate authorization packets, or pure-dispatch digest allowances are required for numerical crossings.

## Acceptance Criteria
- Exact observable assertions proving completion.
- Review actual changes/deviations and any newly revealed maintenance trigger under Q1-Q4, including tests/tooling. Estimates and optional legacy cleanup alone are not STOP reasons; concrete correctness/invariant dependencies remain required.
- Grounded in requirements, owner contracts, failure boundaries, and observable evidence.
- Wiring for runtime slices: traced path from authorized entrypoint through changed production component to consumer and observable result.
- For affected capacity/time boundaries, assert the minimum useful case and relevant legal boundary cases; state measured margins and honest omissions/expiry outcomes. Explain N/A where these boundaries are unaffected.

## Relevant Checks & Validation
- DESIGN: document conventions, authority references, links, invariants; runtime wiring is N/A.
- IMPLEMENT/FIX: unit, integration, and scenario tests; conflict markers, hygiene, formatting, and Clippy.
- Exercise an artifact from the real producer through the real consumer where executable, including language/process boundaries; do not substitute mutually consistent mocks. Record evidence in the existing packet/review, not a new gate or report family.
- McCabe ratchet is enforced in the required `metrics` job (exit 1 on regression); LOC/diff metrics remain advisory reports.
- Material failures (broken tests, compiler errors, hygiene violations, invalid input) must be corrected before merge in the same PR.

## Genuine STOP Conditions
Pause implementation only for:
- Unsealed required dependencies (return SPLIT_REQUIRED)
- Unresolved or ambiguous authority (missing PRD/ADR)
- Violated architectural invariants (INV-001 through INV-007)
- Material scope expansion beyond assigned outcome
- New unexpected cross-domain dependencies or God Object pressure
- Unrelated work that cannot be safely preserved

Do not STOP for an advisory metric crossing, a lint number, or a routine implementation defect.

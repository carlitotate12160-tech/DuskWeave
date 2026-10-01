---
name: build-duskweave
description: Execute assigned DuskWeave DESIGN, IMPLEMENT, and FIX packets against verified repository authority. Use only for DuskWeave artifact delivery in its confirmed workspace; do not use for BlackBread, other repositories, or open-ended architecture planning.
---

# Build DuskWeave

## Mission and project isolation

Execute one assigned packet and produce its requested artifacts.
Act as an implementer of the architect's complete packet, not its architecture
planner. Make routine local implementation choices that preserve accepted
contracts; do not invent missing product semantics, ADRs, or system topology.
If an architectural choice is genuinely unresolved, report the precise
dependency instead of presenting a new plan as the deliverable.
DuskWeave is a persistent campaign reasoning and adversary-emulation platform.
Do not import another project's skills, roles, milestones, contracts, or decisions.
Do not treat global memory as project authority.

Verify the workspace from AGENTS.md and docs/ENGINEERING_STATE.md.
Read AGENTS.md, engineering state, docs/BUILD_ORDER.md, invariant packet 00,
the active build-order packet, relevant authority, QUALITY_BAR.md, and the packet.
Use packet 07 for delivery rules when needed. Do not load unrelated stages.
Read supplied snapshots as snapshots; do not claim they prove current repository state.

Use the authority order defined by AGENTS.md and docs/BUILD_ORDER.md.
Draft PRDs cannot override accepted authority in IMPLEMENT, FIX, or review.
A portable installed copy does not override the repository's current skill.
Report material mismatches before executing under stale instructions.

## Always-loaded domain boundaries

Preserve the offensive campaign purpose: authorized active validation, access,
expansion and objective proof under accepted contracts. Non-destructive does not
mean read-only. Do not replace offensive behavior with passive monitoring or
scanner-only output. Low unnecessary footprint and evidence-led adaptation are
engineering requirements, not a universal ban on active work. Current packet
authorization still determines which capabilities may be built or executed.

Preserve Strategic, Access, Expansion, and Objective loops with adaptation
across all four. Do not implement them as a mandatory one-pass kill chain.
Keep CyberTerrain, FootholdGraph, AttackPathView, ObjectiveState, and
CampaignTrajectory distinct. Do not introduce a universal Agent, GlobalContext,
or a manager combining strategy, execution, parsing, evidence, and state.

Apply INV-001..007 from invariant packet 00:
- No God Object.
- Separate the five operational models.
- Reasoning produces proposals; deterministic authority validates execution.
- Observations require reconciliation; inference is not automatically fact.
- Keep proof content attempt-ephemeral and raw client content/operational secrets
  outside core and ordinary surfaces. Separately authorized operational secrets
  may remain only in isolated campaign-scoped custody under PRD-000 INV-005.
- Campaign capabilities cannot alter authoritative audit evidence.
- Follow PRD-000 INV-007: exclude privileged defender-oracle feeds from blind
  reasoning; allow eligible campaign-visible effects and legitimately acquired
  telemetry under position/source/sensitive-data bounds. Keep separately
  authorized defender-informed exercises labeled and evaluated independently.

Retain freshness and provenance when the packet touches environmental knowledge.
Initial access is not automatically a validated foothold; a candidate path is not
a proven transition; successful execution is not automatically objective success.
Keep tool-native schemas and clients outside the domain.

Use ADR-004's component-specific language ownership. Rust core infrastructure
ports are not automatically Go integrations. Do not add a production language
or speculative adapter merely because a skill mentions one.

## Preflight

Before editing:
1. Verify repository root, branch, HEAD, and the packet's exact expected base.
2. Inspect working-tree changes; preserve unrelated user work.
3. Verify active stage and all required accepted/sealed dependencies.
4. Read existing files in the allowed map and their relevant consumers.
5. Check conflicting work/PRs only where repository delivery rules require it.
6. List required verification commands/checks and any unavailable capability.

Reuse verified authority context only while its relevant files are unchanged.
Recheck HEAD, status and authority deltas; read newly affected boundaries.
Do not use stale memory, stage labels, or old CI as current evidence.

Do not reset user changes, invent missing checks, assume remote protection, or
claim CI results from a local run. Missing tools are explicit blockers when a gate
requires them. Do not silently weaken a gate.

## Execution cadence

Batch independent preflight/read operations. Record loaded authority and check
provenance briefly in the session; reread changed/newly affected boundaries only.
Start with focused contract tests. As soon as the complete production path exists,
format and measure the actual cumulative budget against the packet base, before
polishing tests/docs. Preserve the packet's ceilings, correction room and STOP
conditions; crossing a review trigger alone is not a hard-budget failure.

Reuse Cargo build artifacts during normal iterations; do not routinely cargo clean.
Keep the owned PostgreSQL service for the session if configuration is unchanged,
while executing real SQL and fixture isolation on every run. Never reuse a prior
PASS as current evidence. Preserve required coverage-profile cleanup, fresh-port/
process recovery assertions, runtime-role checks and platform-specific checks.
Run DB targets sequentially when their fixtures can interfere.

Perform one final full gate pass per unchanged candidate/platform/configuration.
A full all-targets coverage run also executes that same full test suite; avoid
an identical immediate standalone rerun unless independently required. Rerun
affected checks after fixes and complete required final-head CI. Record changes
that invalidate prior results. A known unavailable tool remains an explicit
blocker; do not repeatedly probe it or substitute cached evidence.

## Mode: DESIGN

Produce only the documents explicitly assigned by the packet.
Use build-order authoring permission when the packet is creating new authority.
Do not demand an existing accepted copy of a PRD being authored.
Write proposed/draft content using the registry's conventions; never self-accept.
For PRDs define semantics, actors, scope, success criteria, and non-goals.
Do not write database schemas, Rust types, tool commands, or runtime protocols.
Do not create ADRs unless the packet explicitly permits them.
Author dependency-ordered drafts within a permitted combined design packet.
This does not authorize implementation against unaccepted dependencies.

Validate terminology, transitions, ownership, invariants, links, file scope,
size, and cross-document consistency. Code tests are N/A for document-only work;
do not claim cargo/test gates passed where no runnable project exists.

## Mode: IMPLEMENT

Require accepted PRDs/ADRs/contracts and sealed earlier dependencies as specified.
Deliver the whole bounded, observable behavior in the assigned packet across
its necessary files; do not stop after one disconnected module or produce a
series of micro-plans in place of implementation.
Run contract-focused TDD: demonstrate relevant failure, implement minimally,
then refactor without changing semantics.
Test happy paths, illegal transitions, failure boundaries, and relevant invariant
negative controls. Avoid tests that only mirror private implementation structure.
Use deterministic fixtures; do not use real client secrets or external targets.
Test controls are not client pilot evidence. Preserve the actual authorized
offensive outcome in later capability packets; do not substitute a harmless
stub and report that an active capability was delivered.
Run repository-required checks. Respect QUALITY_BAR.md budgets and language gates.
Run the smallest relevant failing test before a behavior fix; never fabricate
historical RED evidence for coverage-only tests of already-correct behavior.
For transaction/recovery work, inject failure at the claimed boundary and assert
durable state before recovery, original identity, scope and effect count after
recovery. A fault before commit does not prove lost acknowledgment after commit.
Verify environment-sensitive behavior on each platform claimed as supported;
Linux success alone cannot establish Windows subprocess/environment correctness.
Use real restricted runtime roles for DB assertions; admin is fixture setup only.
Require infrastructure secrets through the packet's environment/config boundary,
without hardcoded defaults or value-bearing diagnostics. This does not authorize
campaign-secret storage outside INV-005 custody.
Do not add unused scaffolding, speculative abstractions, or unrelated cleanup.
For each changed runtime component, show a real path from an authorized
production entrypoint through caller/port, registration or injection,
component, and consumer to an observable output or state transition.
A unit test, exported symbol, or placeholder registration alone is insufficient.
Exercise at least one real entrypoint-to-consumer path when testable at this
stage. Review changed event producers/consumers, adapters, configuration,
migrations, and failure paths for orphan or island code. Remove truly unused
artifacts or finish the wiring within the allowed file map; if the required
consumer or test file is outside the map, STOP with SPLIT_REQUIRED and exact
paths. Document legitimate dynamic wiring with evidence before classifying it
as unused. When no executable runtime exists, report wiring as N/A for DESIGN.

## Mode: FIX

Reproduce or substantiate the reported finding before editing.
Classify it as VALID, FALSE_POSITIVE, or UNVERIFIED.
Fix the root cause within allowed files and verify the affected behavior.
If a valid fix needs different authority, dependencies, or materially larger scope,
report the exact dependency and return SPLIT_REQUIRED.
Do not dismiss a real defect merely because the current packet cannot fix it.

## Review and delivery

Inspect the diff for authority compliance, domain ownership, coupling, evidence,
freshness, failure semantics, INV-005 custody/isolation, audit integrity, and
the mode-specific Defender Knowledge Boundary.
Perform author self-review and the assigned distinct adversarial review step;
switching skills alone is not independent review. Fix substantiated findings.
Recheck the final changed result and any tests affected by those fixes.
Follow repository PR/merge rules where applicable. Never create or report a live
PR, remote check, protected branch, or merge when no such evidence exists.
Commit/publish only within the user's authorization and the packet's instructions.

Report:
- packet/mode and verified base/head;
- artifacts and exact files changed;
- tests/checks actually run and their results;
- candidate SHA, commands, relevant assertions, OS/toolchain/DB/role context,
  local versus CI evidence, and required checks not executed;
- runtime wiring trace and consumer-path test, or DESIGN N/A;
- dead-code/island/orphan disposition for changed scope;
- review findings and dispositions;
- remaining blockers and readiness;
- next action: STOP at the packet boundary.

When available, include approximate preparation/implementation/check time and
the cause of rework or STOP in this report; no new timer, artifact or gate.

Do not promote documents to ACCEPTED, update seals, or edit engineering state
unless the packet allows those files and the acceptance step is authorized.
No automatic next-stage execution.
Treat missing required execution evidence as BLOCKED/UNVERIFIED, never PASS.
Exact-candidate CI logs may establish checks that could not run locally; respect
any explicit local gate and report the remaining limitation. Inspect skipped
jobs/tests and actual scenario assertions. A completed scanner job does not
establish zero findings. A quality label is not evidence or certification.
Measure the cumulative budget against the authorized base. Do not compensate
for over-budget work by unrelated cleanup, comment removal, or out-of-map edits.

## STOP conditions

Return SPLIT_REQUIRED for an unsealed required dependency, new architectural
decision outside assigned design authoring, materially wider file map/scope, or a size limit requiring a new packet.
Return AUTHORITY_CONFLICT for contradictory governing instructions.
Return DESIGN_DRIFT when the pinned base or required baseline differs.
For each, name the exact file/dependency, conflict, and smallest required resolution.
Do not silently create a new PRD/ADR or repair higher authority to unblock yourself.

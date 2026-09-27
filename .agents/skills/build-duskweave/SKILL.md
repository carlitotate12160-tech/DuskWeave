---
name: build-duskweave
description: Execute assigned DuskWeave DESIGN, IMPLEMENT, and FIX packets against verified repository authority. Use only for DuskWeave artifact delivery in its confirmed workspace; do not use for BlackBread, other repositories, or open-ended architecture planning.
---

# Build DuskWeave

## Mission and project isolation

Execute one assigned packet and produce its requested artifacts.
DuskWeave is a persistent campaign reasoning and adversary-emulation platform.
Do not import another project's skills, roles, milestones, contracts, or decisions.
Do not treat global memory as project authority.

Verify the workspace from AGENTS.md and docs/ENGINEERING_STATE.md.
Read AGENTS.md, engineering state, docs/BUILD_ORDER.md, invariant packet 00,
the active build-order packet, relevant authority, QUALITY_BAR.md, and the packet.
Use packet 07 for delivery rules when needed. Do not load unrelated stages.
Read supplied snapshots as snapshots; do not claim they prove current repository state.

Follow:
PRODUCT / authoritative PRD > ACCEPTED ADR > DOMAIN CONTRACT > QUALITY_BAR.md
> AGENTS.md > SKILL.md > IMPLEMENTATION.
Draft PRDs cannot override accepted authority in IMPLEMENT, FIX, or review.
A portable installed copy does not override the repository's current skill.
Report material mismatches before executing under stale instructions.

## Always-loaded domain boundaries

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
- Raw sensitive client material never enters persistent storage or LLM context.
- Campaign capabilities cannot alter authoritative audit evidence.
- Defender-observer verdicts remain isolated from active campaign reasoning.

Retain freshness and provenance when the packet touches environmental knowledge.
Initial access is not automatically a validated foothold; a candidate path is not
a proven transition; successful execution is not automatically objective success.
Keep tool-native schemas and clients outside the domain.

Use Rust for correctness-sensitive core/authority and Go for adapters/collectors.
Use Zig only for a justified native helper, C/C++ for interoperability, and
Python/Nim in research unless accepted authority explicitly allows otherwise.

## Preflight

Before editing:
1. Verify repository root, branch, HEAD, and the packet's exact expected base.
2. Inspect working-tree changes; preserve unrelated user work.
3. Verify active stage and all required accepted/sealed dependencies.
4. Read existing files in the allowed map and their relevant consumers.
5. Check conflicting work/PRs only where repository delivery rules require it.
6. List required verification commands/checks and any unavailable capability.

Do not reset user changes, invent missing checks, assume remote protection, or
claim CI results from a local run. Missing tools are explicit blockers when a gate
requires them. Do not silently weaken a gate.

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
Run contract-focused TDD: demonstrate relevant failure, implement minimally,
then refactor without changing semantics.
Test happy paths, illegal transitions, failure boundaries, and relevant invariant
negative controls. Avoid tests that only mirror private implementation structure.
Use deterministic fixtures; do not use real client secrets or external targets.
Run repository-required checks. Respect QUALITY_BAR.md budgets and language gates.
Do not add unused scaffolding, speculative abstractions, or unrelated cleanup.

## Mode: FIX

Reproduce or substantiate the reported finding before editing.
Classify it as VALID, FALSE_POSITIVE, or UNVERIFIED.
Fix the root cause within allowed files and verify the affected behavior.
If a valid fix needs different authority, dependencies, or materially larger scope,
report the exact dependency and return SPLIT_REQUIRED.
Do not dismiss a real defect merely because the current packet cannot fix it.

## Review and delivery

Inspect the diff for authority compliance, domain ownership, coupling, evidence,
freshness, failure semantics, zero-retention, audit integrity, and defender isolation.
Perform one adversarial review cycle; fix valid in-scope findings.
Recheck the final changed result and any tests affected by those fixes.
Follow repository PR/merge rules where applicable. Never create or report a live
PR, remote check, protected branch, or merge when no such evidence exists.
Commit/publish only within the user's authorization and the packet's instructions.

Report:
- packet/mode and verified base/head;
- artifacts and exact files changed;
- tests/checks actually run and their results;
- review findings and dispositions;
- remaining blockers and readiness;
- next action: STOP at the packet boundary.

Do not promote documents to ACCEPTED, update seals, or edit engineering state
unless the packet allows those files and the acceptance step is authorized.
No automatic next-stage execution.

## STOP conditions

Return SPLIT_REQUIRED for an unsealed required dependency, new architectural
decision outside assigned design authoring, materially wider file map/scope, or a size limit requiring a new packet.
Return AUTHORITY_CONFLICT for contradictory governing instructions.
Return DESIGN_DRIFT when the pinned base or required baseline differs.
For each, name the exact file/dependency, conflict, and smallest required resolution.
Do not silently create a new PRD/ADR or repair higher authority to unblock yourself.

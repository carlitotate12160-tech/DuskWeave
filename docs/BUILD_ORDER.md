# DuskWeave Build Order

## Purpose

This file is the canonical navigation index for DuskWeave build sequencing.

Detailed stage definitions live under `docs/build-order/` so agents do not need to load the entire lifecycle for every task.

Principle:

> Campaign semantics first. Reality/evidence second. Execution third. Tools last.

## Authority hierarchy

1. Product Thesis / accepted PRD
2. Accepted ADR
3. Domain Contract
4. QUALITY_BAR.md
5. AGENTS.md
6. Relevant DuskWeave skill under .agents/skills/
7. Implementation

PRDs define WHAT/WHY. ADRs define architectural HOW. Code may not invent architecture that has no accepted authority.

## Canonical build-order packets

| Packet | Scope | Stages |
| --- | --- | --- |
| [00](build-order/00-authority-and-invariants.md) | Authority hierarchy and hard invariants | Global |
| [01](build-order/01-foundation-design.md) | Repository bootstrap, product thesis, campaign semantics, foundation ADRs | 0–3 |
| [02](build-order/02-reality-and-domain.md) | Observation/evidence, domain contracts, campaign state, terrain, footholds, paths, objectives | 4–10 |
| [03](build-order/03-capability-and-execution.md) | Capability model, execution architecture, first adapters | 11–13 |
| [04](build-order/04-campaign-intelligence.md) | Expansion, chain composition, adaptation | 14–16 |
| [05](build-order/05-stealth-proof-and-observer.md) | Stealth/control-gap, client proof, telemetry, grader | 17–20 |
| [06](build-order/06-memory-and-scenarios.md) | Campaign memory and threat-derived scenarios | 21–22 |
| [07](build-order/07-delivery-and-seals.md) | Slice rules, file rules, STOP conditions, delivery loop, current action, seals | Delivery |

Only read the packet required by the active engineering state, plus packet 00 and packet 07 when needed for invariants or delivery rules.

## Current permitted work

Current seal:

`DW-FOUNDATION-001`

Current engineering state:

`Accepted bounded M0 lane; DW-FOUNDATION-001 remains SEALED at f93087b52c480822544bad0fb5d99d17eedf8ac0; PRD-000..010 / ADR-001..008 and M0 contract ACCEPTED; M0A registration/history MERGED; M0B partial (B1a assessment + B1b history + B2 scope/window + R3 journal extraction MERGED); workflow corrections MERGED; review-policy documentation MERGED through PR #20; M0C-C0 fresh-authority confirmation MERGED through PR #21; baseline planning-CLI complexity FIX MERGED through PR #23; M0C-C1a durable withdrawal/history MERGED through PR #22; C1b versioned durable refusal draft PR #25 (F2/F3 reviewed, F1 blocked); registration-complexity draft PR #26 (scoped review passed, unmerged); partial FIX DW-FIX-M0-TRAJECTORY-COMPLEXITY in delivery; Mission/planning baseline and later PR #24/#25/#26 integration plus positive admission pending; M0 DEMO_PENDING and unsealed`

Current design document:

[ADR-008 Observation & Fact Separation](adr/ADR-008-observation-fact-separation.md) is `ACCEPTED` by the product owner on 2026-09-30. The [minimum M0 contract](contracts/M0-mission-authority-history.md) and its bounded sequencing exception are `ACCEPTED` by the owner on 2026-09-30. M0A durable registration/history is merged through PR #5. PR #13 merged B1a durable nonpositive assessment and PR #14 improved context/check scheduling; PR #15 merged B1b planning-history publication/recovery and PR #16 added advisory CodeRabbit config. PR #17 merged B2 scoped purpose/asset/window refusals as version 2 planning events. PR #18 merged R3's behavior-preserving extraction of planning-history journal mechanics into a narrow Trajectory-owned journal consumed by the existing CLI path; PR #19 merged the document-only workflow-correction packet covering source workspace, tracking, build reuse and local-service reuse; PR #20 merged the document-only review-policy update covering the QUALITY_BAR.md section 7 evidence-based review checklist, the 50-line function review trigger and the qualified pure-dispatch McCabe allowance. PR #21 merged `DW-IMPLEMENT-M0C-C0-FRESH-AUTHORITY`: new CLI assessments asserting `current_authority_confirmed=true` pass a fresh bounded challenge/response exchange on that invocation before the unchanged Mission assessment, while historic duplicates and recovery stay interaction-free. PR #23 merged the baseline-only FIX `DW-FIX-M0-PLANNING-CLI-COMPLEXITY`: the two planning CLI entrypoints moved into binary-private `src/planning_cli.rs` under the measured caps without behavior change. PR #22 merged M0C-C1a durable local withdrawal and history at verified base `73b0b6ff49a2d5ec08f8a71c5ace5d2144e015cc`: local withdrawal -> immediate Mission blocking -> separate Trajectory history -> fresh recovery. Verification and review evidence belongs to [ENGINEERING_STATE.md](ENGINEERING_STATE.md), section 6, and the external head-bound delivery report. The issued partial FIX `DW-FIX-M0-TRAJECTORY-COMPLEXITY` is in delivery as one draft PR to master on that base; it removes only the three scoped Trajectory McCabe violations while preserving real history behavior. Draft PRs #24 (CI McCabe gate), #25 (C1b versioned durable refusal; F2/F3 reviewed, F1 blocked) and #26 (registration-complexity FIX; scoped review passed, unmerged) plus the Mission/planning baseline findings remain pending their own packets; C1a does not change planning event versions or finish positive admission. Exact-master CI and remaining demo/seal obligations are recorded in ENGINEERING_STATE.md. Local M0 implementation is permitted only through issued bounded packets; no full domain seal is implied. Targeted research resolves material ambiguity rather than imposing an operator study for every PRD/ADR. Target runtime/acquisition and tool integration remain unauthorized.

[MVP direction and deferred scope](MVP_AND_DEFERRED_SCOPE.md) records owner-approved direction and the bounded CODE/TEST -> DEMO -> REVIEW/FIX -> SEAL rhythm. The accepted M0 contract identifies which evidence/proof/custody decisions can wait for their actual behavior. Its bounded sequencing exception is accepted; each local M0 runtime packet must contain resolved contracts and an exact file map. Existing dependencies for the full domain/evidence/execution stages are preserved.

DW-DESIGN-001 produced and accepted PRD-000 Product Thesis, PRD-001 Campaign Lifecycle, and PRD-002 Cyber Terrain. Its prior execution prompt is historical.

DW-DESIGN-002 authored four linked PRDs, accepted by the product owner on 2026-09-27 after reconciliation:

- PRD-003 Access & Footholds
- PRD-004 Expansion Loop
- PRD-005 Objective Loop
- PRD-006 Adaptation

PRD-000 INV-004 now defines tier-proportionate corroboration, and INV-007 retains the current campaign position boundary. PRD-002..006 preserve provenance, epistemic limits, authorized action, and reversible proof. PRD-000..010 and ADR-001..008 are ACCEPTED. ADR-001 acceptance on 2026-09-27 includes the core-plus-workers alternative and shared-failure/recovery clarification; PRD-001 distinguishes presumed from confirmed loss consistently with PRD-003. ADR-002 was accepted on 2026-09-27 after the product owner's four requested boundary clarifications. ADR-003 was accepted on 2026-09-28 after its B1-B7 revision. ADR-004 was accepted on 2026-09-28 with the revised reasoning/inference boundaries. ADR-005 was accepted by the product owner on 2026-09-28 for PostgreSQL as the sole initial campaign-core system of record; no SQLite deployment mode was selected. ADR-006 was accepted by the product owner on 2026-09-28 after freshness, mode/causal eligibility, historical-view, Observer-boundary and STALE/PROVISIONAL clarifications. ADR-007 was accepted by the product owner on 2026-09-28 after clarifying the parallel-attempt review case and the ownership of intent-weighted selection. DW-FOUNDATION-COHERENCE-001 reconciled F1–F5 and reran the cross-foundation document checks. The product-owner-approved F6 duration clarification subsequently established operator-authorized rapid, bounded, and persistent engagement envelopes; extended duration is a capability rather than a requirement for every campaign, and deadline expiry reports bounded coverage and residual uncertainty. Product-owner-approved F7 keeps campaign continuity independent of target-side persistence, prohibits arbitrary malware and unmonitored implants, and limits any later temporary managed artifact to explicit authorization, bounded lease/capability, non-sensitive manifest, revocation, expiry, opaque cleanup evidence, and honest residual reporting. Following explicit product-owner authorization, `DW-FOUNDATION-001` was sealed on 2026-09-28 against the reconciled authority baseline `5883fa52cd063083350a41a293e0bd654d500d63`. Stage 4 Reality & Evidence design is permitted. PRD-007 was accepted by the product owner on 2026-09-28 after reconciliation of observation/reconnaissance, owner-qualified use without a global barrier, freshness, protected-edge, and raw-input semantics. PRD-008 was accepted by the product owner on 2026-09-28 after bounded revision of owner-specific burden, reviewable technical material, semantic lineage, late contamination, reviewer authority, time basis, and current-use eligibility. PRD-009 Client Proof was explicitly accepted by the product owner on 2026-09-29 after refinement of remediation, finding traceability, bounded scanner-signal reporting, attack-narrative composition, and completion-scope distinctions. PRD-010 Sensitive Data Handling was explicitly accepted by the product owner on 2026-09-29 after bounded research into real assessment practice and sanitization assurance. On 2026-09-29 the product owner authorized reconciliation of INV-005 so separately authorized operational authentication material may remain reusable only inside isolated campaign-scoped custody, while core state, PostgreSQL, events, evidence, operators, and reasoning retain only opaque references and safe metadata. Proof content remains attempt-ephemeral; secret custody is non-durable by default, cannot cross campaigns/retests, requires current authority for every use, and ends with accountable disposal. The product owner explicitly resealed `DW-FOUNDATION-001` on 2026-09-29 against authority baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0`; the 2026-09-28 baseline remains historical. Runtime and tool integration remain governed by later dependencies and are not authorized by this navigation update.

## Mandatory invariants

The detailed definitions are canonical in [00-authority-and-invariants.md](build-order/00-authority-and-invariants.md).

Summary:

- INV-001: No God Object.
- INV-002: Keep CyberTerrain, FootholdGraph, AttackPathView, ObjectiveState, and CampaignTrajectory separate.
- INV-003: Reasoning is not execution.
- INV-004: Observation is not fact.
- INV-005: Raw client content and operational secrets stay outside ordinary/core surfaces; only separately authorized campaign-scoped secret custody may retain an operational value for bounded reuse, with opaque core references and accountable disposal.
- INV-006: Campaign capabilities cannot alter authoritative audit evidence.
- INV-007: Blind campaigns exclude privileged defender-oracle feeds; authorized campaign-visible effects and legitimately acquired telemetry may inform bounded adaptation, while separately authorized defender-informed exercises are labeled and evaluated independently. See PRD-000 §6.

## Navigation rule for agents

Before substantial work:

1. Read `AGENTS.md`.
2. Read `docs/ENGINEERING_STATE.md`.
3. Read this index.
4. Read `docs/build-order/00-authority-and-invariants.md`.
5. Read only the build-order packet containing the active stage.
6. Read the relevant PRD and accepted ADRs.
7. Read `QUALITY_BAR.md`.
8. Read `.agents/skills/build-duskweave/SKILL.md`.

If the requested work belongs to a later stage than `docs/ENGINEERING_STATE.md` permits, stop with `SPLIT_REQUIRED` or an authority/dependency finding.

## Anti-monolith rule

This index must remain a navigation document, not grow back into a full lifecycle specification.

Target size:

- preferred: under 200 lines
- hard review threshold: 250 lines

Detailed stage semantics belong in `docs/build-order/*.md`.

Do not duplicate a stage definition in both this index and a packet.

## Seal ownership

Current and historical seal status belongs in `docs/ENGINEERING_STATE.md`.

This index describes sequencing; it does not independently declare implementation completion.

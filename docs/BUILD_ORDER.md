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

`DW-FOUNDATION-001; DW-M0-001 (M0 sealed 2026-10-05 at 669f2f3 — scope/limits in ENGINEERING_STATE.md section 5a)`

Current engineering state:

`M1 product/behavior contract and linear cadence ACCEPTED and published through PR #35; enabling R1 architecture/owner/sequencing and six scoped ADRs ACCEPTED on 2026-10-06 and published through PR #36 at 2cedc32. DW-IMPLEMENT-M1-PERMISSION-ATTACHMENT and its selected-suite mutation correction are MERGED through PR #37 at 49ecb848, with exact-master CI SUCCESS. DW-IMPLEMENT-M1-NAME-POLICY-SNAPSHOT is MERGED through PR #38 at 96256a4 on 2026-10-07. The owner approved the M1 cognitive direction on 2026-10-07; the R2 amendment — evidence-led cognitive reasoning, bounded generated-implementation lane, feedback learning and minimum report — is published through PR #39 at d1d22126 and remains pending owner exact-text acceptance. Acquisition/current-effect admission and deployment/loop qualification remain separate. DW-M0-001 remains SEALED at 669f2f3; DW-FOUNDATION-001 remains SEALED at f93087b52c480822544bad0fb5d99d17eedf8ac0. Delivery evidence and blockers live in ENGINEERING_STATE.md section 6.`

Current design document:

The [M1 external orientation and bounded decision-loop contract](contracts/M1-external-orientation-decision-loop.md) is ACCEPTED by the product owner on 2026-10-06 for product/behavior scope and linear delivery. It selects one passive CT source, bounded DNS/HTTPS acquisition, owner-qualified Terrain/Pathing reasoning and evidence-led feedback. It does not accept missing ADRs, grant target permission, or seal M1.

The [enabling architecture/owner/sequencing contract](contracts/M1-enabling-architecture.md) resolves contract section 11 as one documentation-only DESIGN outcome. The owner ACCEPTED its R1 owner contracts, explicit sequencing amendment and six scoped ADRs on 2026-10-06. Accepted publication is complete through PR #36. The initial permission-attachment and test-only correction are merged through #37 at `49ecb848`, with exact-master run 37478945336 SUCCESS. Corrected mutation run 37474068802 tested 35 mutants: 18 caught, 17 unviable, zero missed; the original meaningful survivor remains a historical selected-suite assurance gap, not a production bug. `DW-IMPLEMENT-M1-NAME-POLICY-SNAPSHOT` merged through #38 at `96256a4`; its Mission-owned name/purpose snapshot query always reports false permission/dispatch/acquisition flags, creates no history or effect, and is neither a mandatory acquisition gate nor an effect-start permit. The R2 cognitive amendment — evidence-led reasoning with a purpose-scoped ContextPack, the bounded generated-implementation lane under the fixed effect vocabulary, campaign-local feedback/reflection, the replaceable inference adapter, and the minimum report projection — is published through PR #39 at `d1d22126` with exact-master run 37597647976 SUCCESS, pending owner exact-text acceptance. Current action: `DW-FIX-M1-WITHDRAWAL-COMMIT-CLASSIFICATION` delivers one bounded owner-local correction — server-confirmed COMMIT serialization/deadlock aborts classify as `serialization_retry`, unclassified commit errors stay `commit_unknown` — then STOP for review. The future current-effect/Broker or cognitive-runtime outcome is not yet issued; it requires a separately measured packet. Work measures each subsequent complete outcome separately. The accepted M1 substitutions apply only to their named lane; M0's non-acquisition exception and all full-stage prerequisites outside that scope remain unchanged. This navigation grants no target permission, deployment qualification or M1 seal.

[ADR-008](adr/ADR-008-observation-fact-separation.md) and the [M0 contract](contracts/M0-mission-authority-history.md) remain ACCEPTED. M0 is SEALED at `669f2f3`; [ENGINEERING_STATE.md](ENGINEERING_STATE.md) sections 5a and 6 hold its merged deliveries, owner demo acceptance and evidence limits. Eligible v4 remains historical eligibility, not current dispatch permission.

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

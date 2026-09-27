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

`DW-BOOTSTRAP-001`

Current engineering state:

`Stage 3 — Foundation ADRs; PRD-000..006 ACCEPTED`

Current design document:

`ADR-001 Modular Monolith (PROPOSED draft; acceptance pending)`

DW-DESIGN-001 produced and accepted PRD-000 Product Thesis, PRD-001 Campaign Lifecycle, and PRD-002 Cyber Terrain. Its prior execution prompt is historical.

DW-DESIGN-002 authored four linked PRDs, accepted by the product owner on 2026-09-27 after reconciliation:

- PRD-003 Access & Footholds
- PRD-004 Expansion Loop
- PRD-005 Objective Loop
- PRD-006 Adaptation

PRD-000 INV-004 now defines tier-proportionate corroboration, and INV-007 retains the current campaign position boundary. PRD-002..006 preserve provenance, epistemic limits, authorized action, and reversible proof. All seven PRDs are ACCEPTED; Stage 3 starts with a PROPOSED ADR-001 draft. ADRs 002..007 wait for their predecessors.

## Mandatory invariants

The detailed definitions are canonical in [00-authority-and-invariants.md](build-order/00-authority-and-invariants.md).

Summary:

- INV-001: No God Object.
- INV-002: Keep CyberTerrain, FootholdGraph, AttackPathView, ObjectiveState, and CampaignTrajectory separate.
- INV-003: Reasoning is not execution.
- INV-004: Observation is not fact.
- INV-005: Raw sensitive client data is zero-retention.
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

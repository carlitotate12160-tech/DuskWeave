---
name: duskweave-engineering
description: Design and review DuskWeave product semantics, architecture, contracts, and bounded delivery packets. Use only for DuskWeave architecture, planning, drift checks, or review; do not use for BlackBread or other projects, or as the executor of an assigned build packet.
---

# DuskWeave Engineering

## Scope and authority

Act as DuskWeave's architecture and review partner. Keep the project independent.
Use the user's current DuskWeave instructions and verified project documents.
Do not load another project's engineering skill or import its roles, milestones,
code, terminology, limits, or decisions through memory.

Resolve the workspace from the current task and verify its identity from AGENTS.md
and docs/ENGINEERING_STATE.md. A folder name alone is insufficient.
If no workspace is available, use supplied sources for explicitly provisional
discussion; do not claim a live check, approved packet, acceptance, or seal.

Read in order:
1. AGENTS.md.
2. docs/ENGINEERING_STATE.md.
3. docs/BUILD_ORDER.md.
4. docs/build-order/00-authority-and-invariants.md.
5. Only the active build-order packet; read delivery packet 07 when needed.
6. Relevant PRDs, accepted ADRs, and domain contracts.
7. QUALITY_BAR.md.
8. The repository's relevant skill and the assigned packet.

Authority: PRODUCT / authoritative PRD > ACCEPTED ADR > DOMAIN CONTRACT > QUALITY_BAR.md
> AGENTS.md > SKILL.md > IMPLEMENTATION.
Use each PRD's repository-defined authoritative status; drafts cannot override
accepted authority during implementation, FIX, or review.
Repository skills govern repository procedure; this installed skill is a portable
workflow aid, not a competing authority. Report material mismatches.
Do not treat planned ADR titles, draft documents, chat proposals, or a green test
run as accepted architecture. Missing documents do not authorize invention.

## Domain anchors

Preserve four nested operational loops:
Strategic > Access > Expansion > Objective; adaptation overlays all four.
Nesting does not imply a mandatory one-pass linear sequence.

Keep five operational models distinct:
- CyberTerrain: environmental understanding, relationships, provenance, time.
- FootholdGraph: validated operational access and its condition.
- AttackPathView: derived candidate and validated transition views.
- ObjectiveState: mission requirements and evidence of fulfillment.
- CampaignTrajectory: what happened and why over time.

Inspect access validation separately from initial entry. Preserve access
survivability, recursive expansion, continuous objective discovery, dwell,
retasking, and re-entry as product semantics when relevant.
Do not make terrain own objectives, strategy, execution, or all campaign history.
Do not reduce key terrain to vulnerability severity.

Maintain INV-001..007 as defined in the repository:
no God Object; separate models; reasoning != execution; observation != fact;
sensitive-data isolation and campaign-scoped secret custody; audit integrity;
the mode-specific Defender Knowledge Boundary. Proposals do not confer execution
authority. Tool success does not prove a goal. Do not let campaign capabilities
alter authoritative audit evidence. Keep proof content attempt-ephemeral. Keep
raw client content and operational secret values outside core and ordinary
persistence, logs, traces, LLM context, reports, fixtures, and crash artifacts;
separately authorized operational secrets may remain only in isolated
campaign-scoped custody under PRD-000 INV-005. Use synthetic examples.
Apply PRD-000 INV-007: blind campaigns can adapt to genuinely campaign-visible effects and defender telemetry legitimately obtained from an authorized current campaign position, with epistemic and sensitive-data limits. Privileged defender/Observer/Grader feeds cannot serve as an oracle in blind mode. A separately authorized defender-informed exercise or retest may expose bounded feedback, labeled and evaluated apart from blind results.

Follow the language baseline: Rust for correctness-sensitive core and authority;
Go for adapters, collectors, integrations; Zig only for justified native helpers;
C/C++ for interoperability; Python/Nim in research unless an accepted ADR promotes
a specific component. Do not introduce production languages by skill instruction.

## Architecture workflow

Before recommending implementation:
1. Name the governing requirement and its source/status.
2. Identify the owning bounded context and affected boundaries.
3. State unresolved assumptions; resolve only those needed for this packet.
4. Check current build-order dependencies and architecture drift.
5. Choose the smallest complete behavior that preserves required invariants.
6. Explain observable success, failure semantics, and evidence requirements.

Act as a peer, not a consensus engine. Test user proposals against accepted
requirements, evidence, failure cases, and at least one credible alternative.
Say plainly when a proposal is unsound, explain the impact, recommend a better
choice, and identify any decision reserved for the product owner. Record
assumptions and confidence; do not convert agreement in chat into an ADR.

For product design, define WHAT/WHY, actors, meanings, transitions, success,
non-goals, and unresolved questions. Do not invent runtime types or storage.
For architecture, compare options against accepted requirements before choosing.
Do not preselect a database, broker, service topology, or framework from a title.
For contracts, specify inputs, outputs, failures, state ownership, and provenance
only when the corresponding design stage is permitted.

Treat threat research as evidence, not architecture authority. When needed, use
primary incident reporting. Distinguish documented behavior from design inference.
Keep threat-actor-specific scenarios outside generic core domains.
ATT&CK mappings are optional metadata, not the campaign lifecycle.

## Packet authoring

Use references/packet-template.md when asked for an execution packet.
Finish the necessary design judgment here before handing work to the IDE.
Write one executable prompt for one cohesive, reviewable outcome; include all
files needed for its end-to-end behavior without splitting into trivial
per-file or per-function prompts. A DESIGN packet may deliver multiple
dependency-ordered PRDs when the build order authorizes that combined outcome.
Do not send the IDE an open architectural question, an option comparison, or
an instruction to plan the implementation; if authority is unresolved,
stop and resolve it in the proper product or architecture step first.
Specify mode, verified base, dependencies, exact allowed files, non-goals,
acceptance criteria, tests or document checks, review gates, and STOP conditions.
For any runtime packet, identify the real entrypoint and consumer files in the
allowed map; require a traced runtime path and a test through that path.
A changed production component consumed only by unit tests is an island.
If there is no authorized entrypoint/consumer in scope, split or defer the
runtime packet. Do not impose runtime wiring checks on DESIGN-only documents.
Do not emit unresolved placeholders as an executable packet.
Do not ask the executor to produce a plan when the task is to produce artifacts.
Do not add a governance subsystem to solve an ordinary delivery problem.
Do not continue into the next packet after completing the requested packet.

A DESIGN packet may author missing PRDs only when build order explicitly permits
that authoring. It does not require the very PRD it is tasked to create.
An IMPLEMENT packet requires accepted upstream authority and sealed dependencies.
Do not convert a blocked implementation into an unrequested PRD/ADR packet.

## Review

Review architectural fit before style. Inspect actual changed artifacts and the
exact review base/head. Distinguish local review from live PR/CI verification.
For each finding give source, concrete violation, consequence, and smallest fix.
Classify findings as VALID, FALSE_POSITIVE, or UNVERIFIED with reasoning.
Out-of-scope does not make a valid finding false; report its blocking effect.
Run one adversarial review. In review-only tasks, report findings and proposed
fixes without editing. When edits are authorized, fix valid in-scope findings
and check the final changed result. Expand testing only to resolve a concrete risk or required gate.

Check semantic consistency, ownership, dependency direction, authority vs code,
temporal staleness, evidence strength, sensitive-data boundaries, and scope.
Use current repository budgets; do not borrow limits from other projects.

## Reporting and stop behavior

Return governing authority, assumptions, scope, artifacts/findings, validation,
remaining blockers, and readiness. Distinguish authored, reviewed, accepted,
merged, and sealed. Cite evidence for each claimed completion state.
Never mark a document ACCEPTED or change a seal without the applicable authority
and authorization. If state updates are outside allowed files, report them as
pending instead of editing them.

Use SPLIT_REQUIRED for missing stage dependencies or materially larger scope.
Use AUTHORITY_CONFLICT for contradictory governing documents.
Use DESIGN_DRIFT for a packet's base or baseline mismatch.
Report the exact missing source/dependency and affected task.
For access/tool failures, describe the actual blocker; do not invent a seal.

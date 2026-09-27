# DW-DESIGN-001 — Product foundation

## Invocation and baseline

MODE: DESIGN
PROJECT: DuskWeave
WORKSPACE: D:/DuskWeave
DELIVERY: DESIGN AUTHORITY ONLY

Use the exact EXPECTED_BASE_SHA supplied by the entry prompt.
This packet body is not executable without that verified base.
Execute directly; produce the documents, not another implementation plan.
Do not implement runtime, create ADRs, commit, or continue into DW-DESIGN-002.

Read in order:
1. AGENTS.md.
2. docs/ENGINEERING_STATE.md.
3. docs/BUILD_ORDER.md.
4. docs/build-order/00-authority-and-invariants.md.
5. docs/build-order/01-foundation-design.md.
6. docs/prd/README.md.
7. QUALITY_BAR.md.
8. .agents/skills/build-duskweave/SKILL.md.

Before writing verify:
- HEAD equals EXPECTED_BASE_SHA and the working tree is clean.
- Engineering state and build order still permit DW-DESIGN-001.
- No existing PRD-000..002 conflicts with this authoring task.
- Only DuskWeave sources are used as project authority.

Base mismatch: STOP, DESIGN_DRIFT.
Material authority contradiction: STOP, AUTHORITY_CONFLICT with exact sources.
Missing dependencies or materially larger scope: STOP, SPLIT_REQUIRED.

## Allowed files

Create only:
- docs/prd/PRD-000-product-thesis.md
- docs/prd/PRD-001-campaign-lifecycle.md
- docs/prd/PRD-002-cyber-terrain.md

Do not edit registries, state, skills, build order, AGENTS.md, or QUALITY_BAR.md.
If higher authority must change, report the conflict instead of patching it.
Write drafts in dependency order; creation does not mark a PRD ACCEPTED.

## Shared product foundation

DuskWeave is a persistent campaign reasoning and adversary-emulation platform.
Preserve Strategic > Access > Expansion > Objective nested loops.
Adaptation overlays all four and can redirect earlier decisions.
Do not reduce the product to a scanner pipeline or one-pass kill chain.

Use the supplied campaign inspirations as design inputs:
APT41 for adaptive campaign continuity, staged operations, operator transition,
repeated discovery, long-lived access, and re-entry; Lazarus for modular chain
composition; Volt Typhoon for situational awareness and native-environment use.
These are design framing, not claims of independently verified attribution.
If external factual support is needed, cite primary incident reporting and
separate documented behavior from DuskWeave design inference.
ATT&CK may be metadata; it is not lifecycle or product-taxonomy authority.

Keep these models distinct:
- CyberTerrain: what exists and relationships currently understood.
- FootholdGraph: where access is validated.
- AttackPathView: potential movement and validated transitions.
- ObjectiveState: mission requirements and what remains.
- CampaignTrajectory: what actually happened over time.

## PRD-000 — Product thesis

Define identity, intended users, problem, operating model, campaign fidelity,
difference from scanning and linear orchestration, non-goals, and invariants.
Include low unnecessary footprint, campaign tempo, native-environment awareness,
campaign continuity, and defender control-gap measurement as product concerns.
EDRVisibilityGap, AVCoverageGap, LoggingIntegrityGap, SIEMCorrelationGap, and
TemporalCorrelationGap are later assessment concepts, not an evasion feedback loop.
Active reasoning cannot consume defender verdicts as an adaptive evasion oracle.
Require that raw client credentials/verifiers, customer or financial records,
and authentication stores never become retained platform data.
Do not define proof cryptography, storage, service topology, types, or protocols.

## PRD-001 — Campaign lifecycle

Define states, transitions, completion, failure, retasking, and adaptation triggers.
Distinguish state, event, observation, decision, objective, and position.

Preserve high-level progression:
Mission > Target/Access Research > External Reconnaissance > Access-Path Selection
> Initial Access > Access Validation > Foothold > Access Survivability
> Situational Awareness.

Preserve recursive expansion:
Observe > Update Situational Model > Access Expansion > Internal Path Selection
> Move/Pivot > New Foothold > Observe Again.

Preserve objective processing:
Objective Discovery > Target Validation > Collection > Staging
> Transfer/Exfiltration > Objective Review.
All semantics remain subject to zero-retention and authorized emulation.

Resolve explicitly:
- TransientAccess does not become Foothold without AccessValidation.
- Access Survivability covers primary/alternate position, health/freshness,
  and re-entry possibility without prescribing persistence mechanisms.
- Lateral movement is traversal inside recursive expansion, not a single phase.
- Access Expansion includes identity, privilege, host/application context,
  service authority, network reach, and trust; it is not only credentials.
- Revisit objective discovery after material terrain changes.
- Dwell is valid without continuous action.
- Access loss can lead to re-entry planning, an alternate path, and restored/new
  foothold; it does not automatically imply campaign failure.
- Objective review may lead to continue, dwell, retask, maintain access, or re-enter.
- Adaptation is a control mechanism across loops, not a terminal phase.

Do not specify runtime state machines, Rust types, or execution mechanisms.

## PRD-002 — Cyber Terrain

Define purpose, entity/relationship concepts, provenance, confidence, freshness,
terrain delta/snapshot, key terrain, and relationships with the other four models.
Cover network, compute, identity, application, control, objective-relevance,
and temporal terrain as semantic dimensions.

Explain first/last observed, freshness, confidence, staleness, and refutation.
Terrain at T0 is not automatically valid at T1.
Observation > reconciliation/corroboration > accepted terrain state.
Explain OBSERVED, CORROBORATED, INFERRED, HYPOTHETICAL, STALE, and REFUTED
as product vocabulary, not an implementation enum.

Key Terrain means terrain whose access, loss, observation, or control materially
changes mission progress. Consider connectivity, authority, trust position,
dependency centrality, mission relevance, and access leverage without ranking code.
Do not equate key terrain with highest vulnerability severity.

AttackPathView is derived from terrain, footholds, evidence, capabilities,
and objectives. It cannot become primary environmental truth.
CyberTerrain cannot own foothold validation, objective completion, strategy,
execution, or the entire campaign history.
Do not define schemas, graph databases, algorithms, or Rust structures.

## Non-goals

No implementation of footholds, path algorithms, storage, event buses,
language workspaces, workers, native helpers, execution brokers, gateways,
tool integration, C2, payloads, credential extraction, lateral movement,
persistence, security-control bypass/suppression, proof cryptography,
campaign memory, or autonomous graders. No ADRs and no later PRDs.

## Validation and one adversarial review

Use focused documents following docs/prd/README.md; avoid a mega-PRD.
Respect repository size guidance; link to the owning PRD instead of duplicating.
Check all three documents agree on loops/adaptation, access validation,
survivability, expansion, objective discovery, dwell/re-entry, model separation,
observation vs fact, time/freshness, reasoning vs execution, zero-retention,
audit integrity, defender isolation, and no God Objects.

Review for hidden linear assumptions, terrain owning everything, foothold/objective
leakage, paths becoming primary truth, implementation leaking into product
requirements, actor-specific core design, duplicated authority, inconsistent
terms, undefined transitions, and unbounded state meanings.
Fix valid findings only in the three allowed files.

Confirm exactly three new PRDs, no other changes, no runtime, and no ADRs.
Document-only validation applies; do not invent cargo/test results.

## Final output

DW-DESIGN-001 RESULT
BASE SHA: verified full SHA
STATUS: DESIGN_COMPLETE | SPLIT_REQUIRED | AUTHORITY_CONFLICT | DESIGN_DRIFT
ARTIFACTS: exact paths
CROSS-DOCUMENT REVIEW: PASS | FAIL
INVARIANT REVIEW: INV-001 through INV-007, each with evidence
FILES CHANGED: exact list
VALIDATION: checks actually performed
OPEN QUESTIONS: unresolved items or none
ACCEPTANCE: drafts only; no registry/seal transition performed
NEXT: STOP

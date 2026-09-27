# DW-DESIGN-002 — Coherent campaign semantics

MODE: DESIGN
PROJECT: DuskWeave
WORKSPACE: D:/DuskWeave
DELIVERY: Four linked PRD drafts in one bounded outcome.

Use the exact EXPECTED_BASE_SHA supplied in the entry instruction. This packet
is not executable without that verified SHA. Work directly on the documents;
do not return another plan or delegate architecture choices to the IDE.

## Preconditions and authority

Verify the repository identity, branch, HEAD equals EXPECTED_BASE_SHA, and
clean working tree before writing. Read in order: AGENTS.md,
docs/ENGINEERING_STATE.md, docs/BUILD_ORDER.md, invariant packet 00,
docs/build-order/01-foundation-design.md, PRD-000 through PRD-002,
docs/prd/README.md, QUALITY_BAR.md, and
.agents/skills/build-duskweave/SKILL.md. Use packet 07 for delivery rules.
Confirm PRD-000..002 are ACCEPTED and stage permits this combined DESIGN packet.
Do not treat PRDs to be drafted as already accepted authority.

## One cohesive outcome

Complete campaign semantics from current terrain to validated access, recursive
expansion, objective review, and adaptation. Create the four documents below in
dependency order, with consistent meanings and explicit transitions. Together
they must describe a viable authorized campaign progression including access
loss, re-entry, and retasking, without implying one mandatory linear sequence.

Allowed files: create only
- docs/prd/PRD-003-access-and-footholds.md
- docs/prd/PRD-004-expansion-loop.md
- docs/prd/PRD-005-objective-loop.md
- docs/prd/PRD-006-adaptation.md

Do not change the registry, state, accepted PRDs, skills, build order, ADRs,
contracts, or runtime. If accepted authority is contradictory, stop and report
the exact conflict; do not repair higher authority from this packet.

## Required product semantics

PRD-003: Separate candidate access, transient access, validated foothold,
foothold health, stale or lost position, and re-entry eligibility. Define what
evidence permits each transition and what cannot be inferred from tool success.
Describe access survivability without prescribing implants, persistence methods,
raw credential handling, code, or tool commands. FootholdGraph owns validated
access; CyberTerrain remains environmental knowledge; candidate paths remain
AttackPathView, not proven access.

PRD-004: Define recursive expansion as observe, reconcile, select a justified
candidate, obtain authorized execution, validate a new position, and observe
again. Cover host, identity, privilege, application, service, network reach,
and trust without reducing expansion to credential accumulation. Explain
failed or stale paths and how new access changes terrain understanding and
objective opportunity. A candidate transition does not become a validated
position merely because a move was attempted.

PRD-005: Define objective discovery, target validation, safe simulated
collection, staging, approved proof or transfer, and objective review.
Distinguish attempted action, execution success, evidence of access, and actual
objective fulfillment. Revisit objectives after material terrain or foothold
changes; support continue, dwell, retask, maintain, and re-enter decisions.
Never require retention or transfer of raw client sensitive material.

PRD-006: Define adaptation triggers and decisions across Strategic, Access,
Expansion, and Objective loops. Show how reconciled campaign-visible evidence,
freshness changes, access loss, stalled objectives, and authorized scope cause
re-evaluation, dwell, alternate paths, or stop. Define escalation where evidence
is insufficient and prevent unbounded retries or out-of-scope actions.
Defender-internal alerts, SOC tickets, and Observer/Grader verdicts remain
outside active reasoning under INV-007. Control assessment and separately
authorized retest remain independent of this campaign loop.

Each PRD must state purpose, actors, terms and ownership, valid transitions,
failure or inconclusive cases, observable acceptance criteria, explicit
non-goals, and dependency on its predecessor. Preserve INV-001..007 and
the distinctions among CyberTerrain, FootholdGraph, AttackPathView,
ObjectiveState, and CampaignTrajectory. Keep WHAT/WHY at PRD level: no schema,
runtime types, protocols, service topology, payloads, evasion instructions,
or choice of specific storage or execution framework.

## Validation and review

Check exact four-file diff and no modifications elsewhere. Validate registry
naming conventions, metadata status PROPOSED, dependencies, document length,
links, and cross-PRD consistency; reference earlier authority instead of
duplicating it. Trace one semantic example across all four documents using
synthetic evidence: candidate access, validation, expansion, objective review,
and adaptation after loss. Find a counterexample for false foothold, stale
terrain, unverified objective, sensitive-data leakage, and defender verdict
leakage. Fix valid in-scope findings and recheck final text. Use a distinct
adversarial review pass if available; do not claim a different reviewer merely
because a separate skill was invoked.

DESIGN-only: runtime reachability, code tests, and dead-code checks are N/A.
Document links and cross-document dependencies must be connected.
Do not claim tests, CI, PR, acceptance, or seal without actual evidence.

## Stop and report

For changed base return DESIGN_DRIFT; for contradictory authority
AUTHORITY_CONFLICT; for missing dependency or scope beyond the four files
SPLIT_REQUIRED, with exact source or path. Do not invent a new ADR.
Report verified full base/head, four artifacts, checks run, adversarial
findings/dispositions, unresolved product decisions, and status as
AUTHORED/REVIEWED drafts only. Stop; product-owner acceptance and registry
updates are a separate authorized step. Do not begin Stage 3 or runtime.

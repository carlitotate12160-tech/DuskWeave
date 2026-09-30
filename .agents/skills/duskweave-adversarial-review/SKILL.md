---
name: duskweave-adversarial-review
description: Independently challenge a DuskWeave DESIGN, IMPLEMENT, or FIX packet, PRD/ADR, or changed diff before acceptance or delivery. Use only for review of verified DuskWeave artifacts; do not implement packets or import BlackBread authority.
---

# DuskWeave Adversarial Review

## Mandate

Review the actual artifact against DuskWeave authority. Seek credible failure
cases and unsupported claims; do not merely endorse the author or replay their
self-review. A separate skill is a distinct review workflow, not proof that a
different human or independently isolated agent reviewed the work.

Verify repository identity in `AGENTS.md` and `docs/ENGINEERING_STATE.md`.
Read `docs/BUILD_ORDER.md`, invariant packet 00, the active stage packet, the
assigned packet, relevant accepted PRDs/ADRs/contracts, and `QUALITY_BAR.md`.
Compare exact base and current head or supplied artifact revision. If only a
snapshot is available, label the review provisional. Repository authority
outranks this portable skill; never borrow BlackBread rules.

## Review method

1. Establish what was requested, permitted files, acceptance status, evidence,
   and the actual changed scope. Do not treat an author's conclusion as proof.
2. Test the strongest plausible counterexample against each material claim:
   incorrect semantics or state transition, violated ownership/dependency,
   stale/inferred evidence promoted to fact, and an untested failure path.
3. For runtime changes, trace an authorized entrypoint through registration,
   caller or port, changed component, consumer, and observable outcome. Examine
   event consumers, configuration, migrations, adapters, errors, and negative
   paths for dead code, disconnected islands, and orphan writes. A unit test
   alone does not prove production reachability. Require a relevant consumer
   path test where the stage can execute it.
4. Challenge INV-001..007, especially INV-005 sensitive-data isolation,
   campaign-scoped secret custody, audit integrity, and PRD-000's mode-specific
   Defender Knowledge Boundary.
   In blind mode, privileged defender/Observer/Grader feeds cannot act as an
   oracle. Eligible external/pre-access observations do not require a foothold;
   transient read-only acquisition retains its PROVISIONAL limits. Legitimately
   acquired telemetry requires an authorized current campaign position, scope,
   provenance, and owner reconciliation. Validated-origin requirements apply to
   consequential follow-on use under the governing contract, not every effect.
   Defender-informed exercises require separate authorization, labeling, and
   evaluation. Observer silence without verified coverage is inconclusive.
5. For DESIGN documents, check meaning, cross-document dependencies, scope,
   links, counterexamples, and no premature runtime architecture. State that
   runtime wiring is N/A for a document-only packet.
6. Recheck changed artifacts after valid fixes; do not demand unrelated tests.

For each finding give exact source/path or observed behavior, the conflicting
authority, impact, a reproducible counterexample where feasible, and the
smallest remedy. Classify VALID, FALSE_POSITIVE, or UNVERIFIED; distinguish
blocking from nonblocking. An out-of-scope valid defect remains valid.
If no defect survives scrutiny, state the evidence checked and residual limits.

## Operational evidence and proportionality

Check actual authorized vantage, available capability, source/mode eligibility,
observed result, uncertainty, footprint, and stopping conditions. Do not assume
origin internals, complete sensor coverage, template safety, or current authority.
Prefer the smallest informative authorized behavior; do not invent tradecraft
or architecture to close an evidence gap.

Research only material ambiguity unresolved by accepted authority and available
evidence. Prefer dated primary sources; separate source facts, operational
inference, and DuskWeave decisions. Do not require a comprehensive operator study
for each review or introduce global approval/corroboration gates.

Match evidence to the stage: DESIGN uses source checks and counterexamples;
IMPLEMENT requires applicable deterministic failure tests and real wiring;
DEMO uses explicitly authorized client evidence with bounded claims. Isolated
negative tests remain necessary; never require hazardous fault injection on a
client. A pilot is not universal safety or stealth proof.

Anchor findings in the actual artifact. If an attributed snippet or behavior is
absent, mark attribution UNVERIFIED; retain hypothetical cases as review cases,
not proven defects. Respect owner-specific burden and the actual ADR-004
component responsibility when reviewing Rust-first changes.

## Boundaries and handoff

Review-only requests produce findings; do not edit, self-accept, seal, or merge.
When explicitly assigned FIX, use `build-duskweave` and its allowed file map.
Escalate missing authority as AUTHORITY_CONFLICT, changed baseline as
DESIGN_DRIFT, and materially wider fixes as SPLIT_REQUIRED with exact paths.
Do not invent a PR, CI result, remote check, or independent reviewer identity.
Return verified base/head, scope, findings and dispositions, checks actually
run, blockers, readiness, and the next permitted packet boundary.

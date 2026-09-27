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
4. Challenge INV-001..007, especially sensitive-data zero-retention, audit
   integrity, and separation of defender assessment from active reasoning.
   Observer silence without verified coverage is inconclusive, not evasion.
5. For DESIGN documents, check meaning, cross-document dependencies, scope,
   links, counterexamples, and no premature runtime architecture. State that
   runtime wiring is N/A for a document-only packet.
6. Recheck changed artifacts after valid fixes; do not demand unrelated tests.

For each finding give exact source/path or observed behavior, the conflicting
authority, impact, a reproducible counterexample where feasible, and the
smallest remedy. Classify VALID, FALSE_POSITIVE, or UNVERIFIED; distinguish
blocking from nonblocking. An out-of-scope valid defect remains valid.
If no defect survives scrutiny, state the evidence checked and residual limits.

## Boundaries and handoff

Review-only requests produce findings; do not edit, self-accept, seal, or merge.
When explicitly assigned FIX, use `build-duskweave` and its allowed file map.
Escalate missing authority as AUTHORITY_CONFLICT, changed baseline as
DESIGN_DRIFT, and materially wider fixes as SPLIT_REQUIRED with exact paths.
Do not invent a PR, CI result, remote check, or independent reviewer identity.
Return verified base/head, scope, findings and dispositions, checks actually
run, blockers, readiness, and the next permitted packet boundary.

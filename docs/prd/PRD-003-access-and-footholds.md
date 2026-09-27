# PRD-003: Access & Footholds

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-003 |
| **Title** | Access & Footholds |
| **Status** | PROPOSED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | [PRD-002 Cyber Terrain](PRD-002-cyber-terrain.md) |
| **Target Seal** | DW-PRD-003 |

## 1. Purpose and actors

Define when an authorized campaign can claim an operational position, how it detects degradation, and how it may regain access after loss. This document develops the Access Loop of [PRD-001](PRD-001-campaign-lifecycle.md) from terrain knowledge in PRD-002. Its outcome is credible access continuity, never access inferred from an optimistic tool result.

The campaign operator sets mission scope and permitted objectives; a bounded access reasoner proposes candidates; deterministic authority decides whether a proposed action is authorized; a capability performs an approved action; evidence reconciliation determines what can be claimed. A reviewer can inspect the resulting proof without receiving raw client material. No actor receives authority merely by proposing a route.

## 2. Terms and model ownership

- **Candidate access** is a plausible route to a scoped target. It belongs to AttackPathView as a hypothesis, with its terrain premises and freshness; it is not a position.
- **Transient access** is a short-lived, observed ability to perform a bounded action on a target after an authorized attempt. Its exact context, repeatability, and stability remain unverified; it establishes no foothold.
- **Validated foothold** is an addressable position with evidence of reliable bounded execution, output retrieval, exact identity and privilege context, relevant environment boundary, and operational stability. FootholdGraph owns it and the health of that position.
- **Foothold health** is an assessment derived from separate questions: is the position validated or affirmatively lost; how fresh is its proof; is that proof corroborated, inconclusive, conflicted, or refuted; and is the bounded capability usable, degraded, or unavailable? Health does not enlarge privileges. Stale proof and conflicting proof are different conditions, and both can apply at once.
- **Re-entry eligibility** is a current, conditional assessment that an authorized route could re-establish a lost position; eligibility never asserts recovered access.
- CyberTerrain owns accepted environmental entities and relationships; it does not own campaign access. AttackPathView projects candidate routes. ObjectiveState owns objective progress. CampaignTrajectory records decisions, attempts, validations, loss, and re-entry outcomes in time order; it is not a substitute for current access.

A foothold's validated identity, privilege, target, and available capability bounds matter as much as its location. A route through the same host under a different identity is a distinct claim requiring validation. Provenance and time constrain every claim; a historical foothold does not remain healthy by default. A candidate belongs to AttackPathView, transient access is an unvalidated observation, and a lost position is a historical foothold claim: these are not interchangeable health states. A fresh but conflicted position is uncertain and requires reconciliation; an otherwise corroborated position with expired proof is stale and requires refresh. Neither condition alone establishes loss.

## 3. Access progression and evidence

| From → to | Required product evidence | If absent or contrary |
| :--- | :--- | :--- |
| Terrain knowledge → candidate access | Reconciled, sufficiently fresh terrain premise, justified scoped hypothesis, and an authorized proposed target | Keep as untested or stale hypothesis; do not assert access |
| Candidate → transient access | Outcome of an authorized attempt showing a bounded action's effect on the target, with source and time | An attempted command, return code, or ambiguous response alone does not prove access |
| Transient → validated foothold | Reconciled proof of repeatable bounded execution and retrievable result, exact identity/privilege and environment boundary, stability and authorized scope | Remain transient or inconclusive; no expansion may depend on an unvalidated position |
| Validated → healthy assessment | Fresh, consistent corroboration confirms the same bounded capability, context, and usability | Incomplete or conflicting proof keeps usability uncertain |
| Healthy → stale assessment | The relevant proof exceeds its freshness window without new corroboration | Require refresh before dependent action; elapsed time alone does not prove loss |
| Healthy/stale → uncertain assessment | New results conflict or cannot establish current identity, context, or usability | Require reconciliation; uncertainty may coexist with stale proof |
| Any prior assessment → lost position | Current reconciled affirmative evidence establishes access is unavailable for the validated position | Failed contact, stale proof, or inconclusive check alone cannot establish loss |
| Stale/uncertain → healthy assessment | Fresh reconciled confirmation restores the same bounded capability and context | Changed identity or privilege requires new access validation, not a silent refresh |
| Lost → re-entry eligible | Fresh authorized route remains plausible, scope and safety still permit an attempt, and prerequisites are reevaluated | Mark ineligible or unresolved; eligibility is not a foothold |
| Re-entry attempt → validated | Fresh access validation of the recovered position under current bounds | Record attempt/failure and retain lost status until validated again |

Transient access does not silently pass through to FootholdGraph. A tool's successful exit, an inferred privilege, a discovered identity, a reachable port, or opaque proof derived from an authorized ephemeral client-content read cannot establish a foothold on its own. Resource-specific proof supports only the access claim it actually demonstrates; the full identity, bounded execution, reliability, and stability criteria still apply. Contradictory evidence requires reconciliation before promotion. Freshness is evaluated against the claim being made, not merely against the last observation of the host.

## 4. Survivability, loss, and decisions

Survivability means tracking alternate authorized routes, validating current position health when needed, and preserving enough non-sensitive provenance to choose a safe recovery path after pause or loss. It does not require implants, covert persistence, or retention of raw credentials. A healthy alternate foothold may sustain the campaign while another becomes stale; the failed position cannot be used as an execution origin.

A lost position triggers reassessment of dependent paths and objective opportunities. Re-entry may use an existing alternate position or a fresh candidate after terrain reconciliation. The operator may pause, maintain other positions, retask, or stop if scope or safety no longer permits recovery. Attempts remain bounded; repeated failures do not authorize infinite retries or expanded scope. Loss is not automatically campaign failure.

An access denial visible to the campaign may inform health reconciliation without proving which control caused it. Defender telemetry obtained through an authorized, validated campaign position is campaign-observed evidence subject to reconciliation, scope, provenance, and sensitive-data rules; it does not by itself prove access health. In blind mode, privileged defender-internal feeds and Observer/Grader verdicts cannot steer Access; a separately authorized defender-informed exercise follows PRD-000 INV-007 and remains distinctly labeled. Campaign capabilities cannot alter authoritative audit evidence, and all durable access proof uses opaque derived material; raw secrets remain outside persistence, logs, and reasoning context.

## 5. Illustrative synthetic trace

A current terrain observation suggests a scoped route to synthetic Host A; AttackPathView records a candidate. One authorized bounded action has an observed effect but does not reveal a reliable privilege context, so access remains transient. A separate bounded validation confirms context and stable output; FootholdGraph gains a validated position. After its freshness window expires, the position is stale without a claim of loss. A fresh but conflicting check makes its usability uncertain as well; neither condition permits dependent action. Reconciliation of affirmative evidence then confirms loss. An alternate scoped route becomes re-entry eligible, and only renewed validation can restore a position. [PRD-004](PRD-004-expansion-loop.md) may use only the validated position.

## 6. Observable acceptance criteria

1. Reviewers can distinguish a candidate, transient access, validated foothold, stale position, confirmed loss, and re-entry eligibility for the same synthetic target without conflating their evidence.
2. A successful action without exact identity/privilege or stable bounded execution cannot become a foothold; a candidate path cannot authorize expansion.
3. Freshness expiry, contradictory current evidence, and affirmative access loss produce distinct assessments; dependent actions wait for refresh, reconciliation, or fresh validation as appropriate. Failed contact alone does not prove loss.
4. An alternate position can sustain continuity and a lost position can regain access only through fresh authorization and validation.
5. Durable proof excludes raw sensitive material; audit integrity and the mode-specific Defender Knowledge Boundary in PRD-000 INV-007 remain intact during validation and recovery.

## 7. Explicit non-goals

This PRD does not select credentials, implants, persistence techniques, validation commands, tool adapters, storage schemas, or runtime protocols. It does not classify control gaps or infer access from vulnerability severity. Expansion behavior and objective fulfillment belong to the following PRDs.

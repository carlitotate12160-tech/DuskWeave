# PRD-004: Expansion Loop

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-004 |
| **Title** | Expansion Loop |
| **Status** | PROPOSED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | [PRD-003 Access & Footholds](PRD-003-access-and-footholds.md) |
| **Target Seal** | DW-PRD-004 |

## 1. Purpose and actors

Define recursive, authorized growth of campaign understanding and validated position. Expansion starts from an available position under PRD-003, observes its reachable environment, proposes a justified transition, validates any resulting position, and observes again. These activities can repeat, pause, branch, or return to access and strategic decisions. Expansion is not a fixed lateral-movement stage or a count of acquired credentials.

The campaign operator supplies the scope and objectives. A bounded expansion reasoner proposes candidate transitions from reconciled knowledge; deterministic authority checks each action against authorization, safety, and current position; an approved capability performs it; reconciliation decides whether new terrain or access was established. The operator can stop or retask the campaign; no proposal grants its own execution authority.

## 2. Ownership and expansion dimensions

- CyberTerrain owns accepted host, identity, privilege-related environmental relationships, application, service, network reach, and trust observations with provenance and freshness.
- FootholdGraph owns positions with validated bounded execution access and their current health. It does not contain every discovered identity or path.
- AttackPathView owns possible transitions, premises, and alternative routes. A projected edge or an attempted move is never a proven position.
- ObjectiveState owns goals and their fulfillment; newly discovered opportunity prompts review under [PRD-005](PRD-005-objective-loop.md).
- CampaignTrajectory records the ordered hypotheses, authorizations, attempts, outcomes, reconciliations, and changes in position without becoming a universal campaign model.

Expansion can change reach across hosts, identities, privileges, applications, services, network boundaries, and trust relationships. A discovered role may change environmental understanding without proving usable authority. A validated position may be constrained to one application or identity; it cannot be generalized to an entire host, tenant, or adjacent trust domain. New access can revise the terrain map, and new terrain can change candidate paths, but each model retains its own claim.

## 3. Recursive progression

| Step | Product requirement | Failure or inconclusive outcome |
| :--- | :--- | :--- |
| Observe | Gather campaign-visible, authorized observations from a healthy position or other permitted vantage | Unreconciled signals remain observations; expired premises require refresh |
| Reconcile | Establish provenance, freshness, and defensible terrain changes; identify conflicts | Conflict or missing evidence prevents promotion to accepted terrain |
| Orient and select | Form competing hypotheses from reconciled evidence; compare mission relevance, missing evidence, expected proof, current access bounds, and an option to observe more or dwell | A stale or unjustified candidate is deferred, abandoned, or returned for observation |
| Authorize and attempt | Deterministic authority permits a bounded transition before any capability acts | Denial, safety concern, or scope mismatch ends this attempt without widening scope |
| Validate | Distinguish execution result, transient contact, and new validated position using PRD-003 criteria | Attempt success, reachability, or one output alone never creates a foothold |
| Observe again | Use fresh position and changed vantage to reassess terrain and objective opportunities | If position is lost or uncertain, stop using it as origin and route to adaptation |

This loop may revisit earlier nodes, choose another hypothesis, observe more, dwell, abandon a candidate, or return to Access. A proposal states the hypothesis it tests, why this candidate serves the mission, the expected evidence, a result that would weaken the hypothesis, and a stopping condition; it is not an execution command. It must not repeatedly execute the same failed hypothesis without new evidence, changed authorization, or a justified new condition. A newly validated position changes what may be observed, not what is automatically true about its neighbors.

## 4. Failure, loss, and opportunity

A failed attempt records its limited outcome and can weaken or refute an AttackPathView hypothesis without declaring the whole route impossible. A stale terrain relationship or foothold blocks dependent actions until refreshed. A scope denial or safety condition stops the proposed move. Lost access suspends all transitions that relied on it; surviving positions can continue within their own bounds, or adaptation can consider re-entry under PRD-003.

When expansion discovers a new service, trust relation, or target location, CyberTerrain reconciles those observations first. AttackPathView may then reevaluate routes and ObjectiveState must revisit opportunities relevant to declared mission goals. Discovery does not itself satisfy an objective. CampaignTrajectory preserves the distinction between rejected candidates, actions attempted, and positions actually validated.

Non-destructive does not mean read-only. Within explicit rules of engagement, authorized expansion may use bounded execution, exercise scoped identity or privilege, move between positions, and create temporary reversible state needed to validate access or impact. An action must remain scoped, attributable, safety-bounded, and recoverable where recovery applies; it must not intentionally destroy or corrupt client data, deny service, cause irreversible system change, or alter authoritative audit evidence. Evidence proof is derived without retaining raw client secrets or records. Campaign-visible denial or environmental change can affect a candidate without automatic attribution to a specific control. Defender telemetry obtained through an authorized, validated campaign position is reconciled as campaign-observed evidence. Privileged defender or Observer/Grader feeds cannot steer blind expansion; separately authorized defender-informed exercises follow PRD-000 INV-007 and are evaluated separately.

## 5. Illustrative synthetic trace

From the validated synthetic Host A position in [PRD-003](PRD-003-access-and-footholds.md), a scoped observation suggests a relationship to synthetic Service B. After reconciliation, AttackPathView holds a candidate transition. An authorized attempt returns a response but lacks reliable context: no new foothold is claimed. Fresh validation later confirms bounded execution in Service B; FootholdGraph then gains that distinct position. Observing from B reveals a mission-relevant synthetic asset, prompting objective discovery in PRD-005. If A becomes lost, the ability to act from B depends on B's own health and bounds, not on A's historical status.

## 6. Observable acceptance criteria

1. A synthetic campaign can repeat observe → reconcile → form and compare hypotheses → select or observe more → authorize/attempt → validate → observe from a newly validated position and revise terrain and objective opportunities.
2. A transition candidate, a successful attempt, and an established foothold remain distinguishable; incomplete validation cannot grant a new origin for subsequent actions.
3. Host, identity, privilege, application, service, network, and trust changes can each affect hypotheses without being reduced to possession of credentials.
4. Stale terrain, failed routes, lost origins, scope denials, and conflicting observations each cause a bounded decision and a reviewable outcome.
5. The recursion supports an alternate route and a return to Access or Objective without forcing a one-pass sequence; a scoped reversible state-changing action is allowed when justified and authorized, while destructive effects, audit alteration, sensitive-data retention, and privileged defender-oracle feedback in blind mode remain excluded.

## 7. Explicit non-goals

This PRD does not define graph algorithms, payloads, traversal commands, credential acquisition methods, implementation topology, or storage structures. It does not infer compromise from topology, alter scope on failure, or score defender detection. Objective proof and cross-loop adaptation are developed by the following PRDs.

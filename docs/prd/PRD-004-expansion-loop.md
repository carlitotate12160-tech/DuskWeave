# PRD-004: Expansion Loop

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-004 |
| **Title** | Expansion Loop |
| **Status** | ACCEPTED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | [PRD-003 Access & Footholds](PRD-003-access-and-footholds.md) |
| **Target Seal** | DW-PRD-004 |

## 1. Purpose and actors

Define recursive, authorized growth of campaign understanding and validated position. Expansion starts from an available position under PRD-003, observes its reachable environment, proposes a justified transition, validates any resulting position, and observes again. These activities can repeat, pause, branch, or return to access and strategic decisions. Expansion is not a fixed lateral-movement stage or a count of acquired credentials.

The campaign operator supplies the scope and objectives. A bounded expansion reasoner proposes candidate transitions from reconciled knowledge; deterministic authority checks each action against authorization, safety, and current position; an approved capability performs it; reconciliation decides whether new terrain or access was established. The operator can stop or retask the campaign; no proposal grants its own execution authority.

Expansion inherits the dual-speed operational model from PRD-003 §4: in deliberate mode, each step is fully validated before the next; in exploitation mode, provisional observation and parallel validation accelerate understanding of newly reached positions while maintaining safety invariants.

## 2. Ownership and expansion dimensions

- CyberTerrain owns accepted host, identity, privilege-related environmental relationships, application, service, network reach, and trust observations with provenance, freshness, and epistemic tier (PRD-002 §3.3).
- FootholdGraph owns positions with validated bounded execution access and their current health. It does not contain every discovered identity or path.
- AttackPathView owns possible transitions, premises, and alternative routes. A projected edge or an attempted move is never a proven position.
- ObjectiveState owns goals and their fulfillment; newly discovered opportunity prompts review under [PRD-005](PRD-005-objective-loop.md).
- CampaignTrajectory records the ordered hypotheses, authorizations, attempts, outcomes, reconciliations, tempo transitions, and changes in position without becoming a universal campaign model.

Expansion can change reach across hosts, identities, privileges, applications, services, network boundaries, and trust relationships. A discovered role may change environmental understanding without proving usable authority. A validated position may be constrained to one application or identity; it cannot be generalized to an entire host, tenant, or adjacent trust domain. New access can revise the terrain map, and new terrain can change candidate paths, but each model retains its own claim.

## 3. Recursive progression

| Step | Product requirement | Failure or inconclusive outcome |
| :--- | :--- | :--- |
| Observe | Gather campaign-visible, authorized observations from a healthy position. In exploitation mode, provisional observation from a transient position is permitted for read-only situational awareness (PRD-003 §4.2). Epistemic tier from PRD-002 §3.3 determines the evidentiary burden for each observation. | Unreconciled signals remain observations at their appropriate tier; expired premises require refresh |
| Reconcile | Establish provenance, freshness, tier classification, and defensible terrain changes; identify conflicts. Tier 1 may enter promptly as a sourced OBSERVED claim after validation and reconciliation; Tier 2 and Tier 3 require stronger corroboration before their consequential claims are accepted. | Conflict or missing evidence prevents promotion to accepted terrain at the required tier |
| Orient and select | Form competing hypotheses from reconciled evidence; compare mission relevance, missing evidence, expected proof, current access bounds, and an option to observe more or dwell | A stale or unjustified candidate is deferred, abandoned, or returned for observation |
| Authorize and attempt | Deterministic authority permits a bounded transition before any capability acts. In exploitation mode, a pre-assessed chain of separately authorized actions may execute without per-step reconciliation pauses. | Denial, safety concern, or scope mismatch ends this attempt without widening scope |
| Validate | Distinguish execution result, transient contact, and new validated position using PRD-003 invariants A-1 through A-5. In exploitation mode, validation may proceed in parallel with provisional observation from the new position. | Attempt success, reachability, or one output alone never creates a foothold |
| Observe again | Use fresh position (or provisional position in exploitation mode) and changed vantage to reassess terrain and objective opportunities. Provisional observations carry their origin reference. | If position is lost, uncertain, or presumed lost, stop using it as origin and route to adaptation |

This loop may revisit earlier nodes, choose another hypothesis, observe more, dwell, abandon a candidate, or return to Access. A proposal states the hypothesis it tests, why this candidate serves the mission, the expected evidence, a result that would weaken the hypothesis, and a stopping condition; it is not an execution command. It must not repeatedly execute the same failed hypothesis without new evidence, changed authorization, or a justified transient failure condition (PRD-003 §4.2 bounded retry). A newly validated position changes what may be observed, not what is automatically true about its neighbors.

## 4. Expansion in exploitation mode

When a validated position reveals a time-sensitive expansion opportunity, the campaign may enter exploitation mode for the expansion attempt:

**Immediate situational awareness**: Upon reaching a new position — even transient — the operator may immediately execute read-only native queries to understand the local environment. This mirrors the real-world pattern where the first actions from a new position are identity checks, network enumeration, and service discovery, not formal validation.

**Chain transitions**: Multiple transitions may execute as a pre-assessed chain when each link has a bounded scope, passes deterministic authority at dispatch, and has a defined stopping condition. An unexpected target, identity, scope, safety premise, or origin change stops the chain pending reassessment. For example: authorized use of a campaign-scoped secret reference → service access → identity query → network enumeration may execute as a chain if each action is eligible under current authority. The raw secret remains inside custody/execution boundaries; the chain receives neither its value nor durable permission. The chain's outcomes are reconciled as a batch after completion.

**Provisional terrain from expansion**: New terrain observations from an exploitation-mode expansion carry PROVISIONAL status until the originating position is validated. If validation succeeds, provisional observations promote; if it fails, they downgrade to HYPOTHETICAL.

**Return to deliberate mode**: After the exploitation window closes (opportunity captured, time-sensitive condition resolved, or consolidation begins), the campaign returns to deliberate mode. Post-exploitation consolidation — validating all positions, promoting provisional observations, reconciling chain outcomes, establishing health baselines — occurs in deliberate mode.

## 5. Failure, loss, and opportunity

A failed attempt records its limited outcome and can weaken or refute an AttackPathView hypothesis without declaring the whole route impossible. A stale terrain relationship or foothold blocks dependent actions until refreshed. A scope denial or safety condition stops the proposed move. Lost access — whether presumed or confirmed under PRD-003 §5 — suspends all transitions that relied on it; surviving positions can continue within their own bounds, or adaptation can consider re-entry under PRD-003.

When expansion discovers a new service, trust relation, or target location, CyberTerrain reconciles those observations at the appropriate epistemic tier (PRD-002 §3.3). AttackPathView may then reevaluate routes and ObjectiveState must revisit opportunities relevant to declared mission goals. Discovery does not itself satisfy an objective. CampaignTrajectory preserves the distinction between rejected candidates, actions attempted, and positions actually validated.

Non-destructive does not mean read-only. Within explicit rules of engagement, authorized expansion may use bounded execution, exercise scoped identity or privilege, move between positions, and create temporary reversible state needed to validate access or impact. An action must remain scoped, attributable, safety-bounded, and recoverable where recovery applies; it must not intentionally destroy or corrupt client data, deny service, cause irreversible system change, or alter authoritative audit evidence. Evidence proof is derived without retaining raw client secrets or records in evidence or ordinary surfaces. Separately authorized operational secrets may remain only in campaign-scoped custody under INV-005 and are presented for use solely by an eligible execution path. Campaign-visible denial or environmental change can affect a candidate without automatic attribution to a specific control. Defender telemetry obtained through an authorized campaign position is reconciled as campaign-observed evidence. Privileged defender or Observer/Grader feeds cannot steer blind expansion; separately authorized defender-informed exercises follow PRD-000 INV-007 and are evaluated separately.

## 6. Illustrative synthetic trace

### Deliberate expansion
From the validated synthetic Host A position in [PRD-003](PRD-003-access-and-footholds.md), a scoped observation suggests a relationship to synthetic Service B. After reconciliation at Tier 2, AttackPathView holds a candidate transition. An authorized attempt returns a response but lacks reliable context: no new foothold is claimed. Fresh validation later confirms bounded execution in Service B; FootholdGraph then gains that distinct position. Observing from B reveals a mission-relevant synthetic asset, prompting objective discovery in PRD-005. If A becomes lost, the ability to act from B depends on B's own health and bounds, not on A's historical status.

### Exploitation-mode expansion
The validated Host A position reveals that synthetic Service C has a configuration weakness that may be remediated during an upcoming maintenance window. The reasoner enters exploitation mode, citing the time-sensitive condition. An authorized chain executes: credential use on C → identity check → service enumeration → network interfaces. Results enter terrain as PROVISIONAL from the transient C position. In parallel, validation of the C position proceeds: repeatable execution, stable context, bounded capability. Validation succeeds; provisional observations promote to OBSERVED. The campaign returns to deliberate mode. CampaignTrajectory records the exploitation tempo entry, justification, chain results, validation outcome, and tempo exit.

## 7. Observable acceptance criteria

1. A synthetic campaign can repeat observe → reconcile → form and compare hypotheses → select or observe more → authorize/attempt → validate → observe from a newly validated position and revise terrain and objective opportunities.
2. A transition candidate, a successful attempt, and an established foothold remain distinguishable; incomplete validation cannot grant a new origin for state-changing actions.
3. Host, identity, privilege, application, service, network, and trust changes can each affect hypotheses without being reduced to possession of credentials.
4. Stale terrain, failed routes, lost origins (presumed and confirmed), scope denials, and conflicting observations each cause a bounded decision and a reviewable outcome.
5. Exploitation mode permits provisional observation and chain execution from new positions with parallel validation, while state-changing actions and expansion from those positions still require validated access.
6. Epistemic tier classification from PRD-002 determines observation acceptance speed without compromising provenance tracking.
7. The recursion supports an alternate route and a return to Access or Objective without forcing a one-pass sequence; a scoped reversible state-changing action is allowed when justified and authorized, while destructive effects, audit alteration, sensitive-data retention, and privileged defender-oracle feedback in blind mode remain excluded.

## 8. Explicit non-goals

This PRD does not define graph algorithms, payloads, traversal commands, credential acquisition methods, secret-custody implementation, implementation topology, storage structures, or chain execution frameworks. It does not infer compromise from topology, alter scope on failure, or score defender detection. It does not define specific exploitation-mode entry criteria or timeout parameters; those are engagement configuration. Objective proof and cross-loop adaptation are developed by the following PRDs.

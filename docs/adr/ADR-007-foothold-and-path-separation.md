# ADR-007: FootholdGraph and AttackPathView Separation

| Metadata | Value |
| :--- | :--- |
| **Document ID** | ADR-007 |
| **Title** | FootholdGraph and AttackPathView Separation |
| **Status** | PROPOSED |
| **Stage** | Stage 3 — Foundation ADRs |
| **Direct Dependencies** | [ADR-002](ADR-002-domain-boundaries.md), [ADR-006](ADR-006-cyber-terrain-storage-model.md) — ACCEPTED |
| **Supporting Architecture** | [ADR-001](ADR-001-modular-monolith.md), [ADR-003](ADR-003-domain-events.md), [ADR-005](ADR-005-postgres-system-of-record.md) — ACCEPTED |
| **Product Authority** | [PRD-003](../prd/PRD-003-access-and-footholds.md), [PRD-004](../prd/PRD-004-expansion-loop.md), [PRD-006](../prd/PRD-006-adaptation.md), and PRD-000..002/005 — ACCEPTED |

## 1. Context and problem

DuskWeave must distinguish places where it can currently exercise validated bounded capability from routes it might attempt. A candidate route may be plausible and mission-relevant without any access. A successful tool return may create transient access without a validated position. A route demonstrated yesterday may be stale, blocked or unauthorized today. Combining these meanings in one graph would allow topology, history or planner confidence to masquerade as current access.

Access and expansion are adaptive operator loops: they may observe more, dwell, branch, choose an alternate route, re-enter, consolidate or stop. They are not a mandatory global state machine. The architecture still needs explicit ownership and failure semantics so rapid exploitation tempo cannot convert a hypothesis into authority.

ADR-002 assigns validated positions and health to Access/FootholdGraph and derived route candidates to Pathing/AttackPathView. ADR-005 supplies durable owner records; ADR-006 supplies eligible Terrain views and corrections. ADR-007 resolves the boundary, derivation and invalidation rules without selecting a graph product or traversal algorithm.

## 2. Decision drivers

1. Prevent candidate topology, historical success and transient access from becoming a usable foothold.
2. Represent identity, privilege, capability, freshness and live dependencies that determine actual position usability.
3. Allow several competing route hypotheses and operator-like adaptation without a rigid campaign state machine.
4. Invalidate only work whose premises changed while independently healthy positions continue.
5. Preserve recovery, provenance, mode/scope isolation and unknown-outcome semantics.
6. Keep execution authority outside both models and avoid a combined campaign graph God Object.

## 3. Options considered

| Option | Benefit | Cost / disposition |
| :--- | :--- | :--- |
| **Separate Access owner and rebuildable Pathing projection** | Clear truth ownership, targeted invalidation and flexible hypothesis comparison | Requires explicit references and reconciliation. Selected. |
| One combined access/attack graph with status labels | Simple traversal and visualization | Label mistakes or stale edges can confer false access; ownership and correction become tangled. Rejected. |
| Put routes and footholds in CyberTerrain | One environmental graph | Terrain cannot own access health, candidate strategy or execution-origin validity. Rejected. |
| Treat reasoning context as the path model | Minimal stored projection | Unreviewable, non-recoverable and vulnerable to stale or privileged context contamination. Rejected. |
| Separate primary graph database for each model | Native traversal and independent scaling | No measured need; adds dual-source recovery and operational complexity. Deferred behind new evidence and an accepted ADR. |

## 4. Owned meanings

| Meaning | Owner | Required distinction |
| :--- | :--- | :--- |
| Environmental entity or relationship | CyberTerrain | Existence/connectivity claim is not access or a route decision. |
| Candidate ingress, expansion, alternate or re-entry route | AttackPathView | A hypothesis with sourced premises, not a foothold or authorization. |
| Transient access | Access validation workflow before FootholdGraph acceptance | May support bounded PROVISIONAL orientation under PRD-003; not an expansion origin. |
| Validated position | FootholdGraph | Target, identity, privilege context and bounded capability have accepted validation evidence. |
| Current operational dependency between positions | FootholdGraph | Exists only when current usability of one validated position actually depends on another validated position or access mechanism. |
| Attempt and external outcome | Execution boundary and accountable outcome records | Success, failure or unknown outcome does not decide access or path truth by itself. |
| Historical decision/action account | CampaignTrajectory | Records what happened without restoring access or route eligibility. |
| Objective eligibility/fulfillment | ObjectiveState | Cannot be inferred from a reachable node or successful transition. |

The route by which a position was acquired is historical/path evidence, not automatically a live FootholdGraph dependency. If position B remains independently usable after A is lost, B remains an independent position. If B currently requires a validated tunnel or other bounded capability through A, Access owns that explicit dependency and loss of A suspends B's use through that dependency. Current transport dependencies may be transitive across several positions. Credential validity, identity and capability eligibility are premises of the applicable position or access mechanism; discovering or acquiring them through A is historical provenance unless their current use genuinely depends on A. Dependency failure alone does not establish that B itself is presumed or confirmed lost; an alternate eligible access mechanism or fresh B evidence may restore usability under Access reconciliation. Pathing cannot invent a live dependency from topology.

## 5. FootholdGraph responsibilities

FootholdGraph accepts only positions for which Access has accepted bounded validation. A position is bound to stable target identity, operating identity/privilege context, permitted capability class, validation evidence references, campaign/engagement scope, source/mode, owner revision, freshness and health assessment. The same host under a materially different identity or privilege is a distinct access claim.

FootholdGraph owns health and usability distinctions required by PRD-003: validated and healthy, stale proof, uncertainty from conflicting evidence, presumed loss, confirmed loss and renewed validation/re-entry outcomes. Reachability, identity/context evidence, capability availability, freshness and evidence conflict remain separable dimensions; a last check-in alone does not prove process, identity, credential or capability health. These meanings need not become one global enum or rigid sequence. Stale is not lost; a timeout alone does not prove confirmed loss. A presumed-lost origin immediately suspends dependent use while allowing abbreviated recovery if contact returns and accepted validation supports it. Confirmed loss requires fresh entry validation before current use.

Freshness and the no-successful-contact threshold can expire solely because evaluation time advances. Every position use and health assessment evaluates them against the applicable engagement parameters rather than trusting the last durable label or waiting for a notification. Expired validation makes the position stale and unusable as a consequential origin until refreshed. Exceeding the accepted operational threshold supports presumed loss, not confirmed loss. When Access accepts an explicit health/loss transition, it commits the change and required publication obligation; no periodic write is required merely to make time pass.

Durable owner state lives behind Access's PostgreSQL port under ADR-005. It may retain a lost position as a sourced historical access claim, but persistence does not keep a live session or make the position currently usable. Raw credentials, authentication stores and tool/session handles do not enter FootholdGraph. An opaque bounded capability reference may identify an execution-boundary resource, but every use still requires current authority, eligibility and origin checks.

A core restart or restored database preserves accepted records, not live capability. Before consequential use, Access reconciles capability availability, position identity/context, freshness, outstanding loss/correction events and any in-flight unknown outcome. It does not mark all positions healthy from the last durable status or blindly repeat validation actions whose effects are unknown.

## 6. AttackPathView responsibilities

AttackPathView is Pathing's derived analytical projection. It may represent candidate ingress, expansion, alternate and re-entry transitions; their source and destination references; required origin condition; supporting and contradicting Terrain/Access/Capability/Objective premises; scope and exercise mode; evidence tier/freshness; expected and falsifying evidence; dependency revisions; and current projection disposition.

A path can be plausible, currently eligible for a particular proposal, blocked, stale, invalidated, attempted, unresolved, or historically demonstrated as supported by its evidence. Eligibility is action-specific: a PROVISIONAL observation may seed a hypothesis or a bounded observe-more candidate within PRD-002/003 limits, but it cannot be the sole premise for a consequential transition, validated relationship or campaign-critical action. These are analytical meanings, not a mandatory enum. “Historically demonstrated” says a bounded transition was evidenced in its earlier context. Pathing references the accountable outcome/history records; it does not own or rewrite them. Historical support does not establish that the route remains open, that the destination is a current foothold, or that retry is authorized.

AttackPathView may be rebuilt from compatible published owner views and accepted outcome references. An optional PostgreSQL materialization or in-memory view is a disposable projection and carries its input revisions, evaluation time and campaign/mode eligibility. It is never an independent source of environmental or access truth. Pathing stores no raw credentials, live execution handles, raw sensitive evidence or privileged Observer/Grader input in a blind projection.

Pathing may keep multiple competing hypotheses and recommend observe, dwell, attempt, alternate, re-entry or stop. It may expose sourced analytical attributes and candidate rankings, but intent-weighted selection belongs to the bounded reasoning/operator use case. Scoring or traversal cannot collapse uncertainty into fact, reduce epistemic tier, expand scope or choose an action by itself. A shortest or highest-ranked route is only an analytical candidate.

## 7. Transition and validation flow

1. Pathing derives a candidate from eligible current premises and records what would support or falsify it.
2. A reasoning use case may propose a bounded action. Deterministic authority checks current mission, target, origin, evidence, scope and safety; Gateway and Broker preserve the required dispatch path.
3. An execution outcome remains an attempt result. Unknown outcome stays unresolved; notification retry cannot repeat the target action.
4. Any resulting access is transient until the Access owner accepts validation of target, identity/context, repeatable bounded capability and required evidence.
5. Access commits the position change and required event obligation atomically under ADR-005. Only then may FootholdGraph publish a validated position.
6. Pathing consumes relevant accepted owner changes—including Access, Terrain, Capability, Objective and mission/scope changes—through its own contract and refreshes only affected projections. It does not publish an owner event on that owner's behalf.
7. A state-changing follow-on action or expansion from the new position requires the validated origin and current dispatch checks. Exploitation tempo may parallelize bounded orientation and validation, but cannot pre-create a foothold.

A pre-assessed chain is a set of individually bounded proposed links, not a single durable permission. At each dispatch, target, identity, scope and safety premise remain eligible; a validated origin is required for action classes that PRD-003 bars from transient positions. A newly reached transient position permits only the bounded provisional orientation defined by PRD-003 until validation succeeds. An unexpected result stops dependent links and triggers reconciliation.

## 8. Freshness, invalidation and recovery

Every path use checks the eligibility and freshness of referenced premises at the evaluation time; notification delivery alone is insufficient. Terrain expiry, origin staleness/loss, capability withdrawal, objective change, scope reduction or correction invalidates or blocks only projections that depend on those premises. Campaign-wide revocation and safety freeze retain their wider effect.

AttackPathView records dependency references precisely enough to explain invalidation and rebuild. A missing predecessor, conflicting revision or unsupported event version leaves the affected projection unavailable or unresolved; delivery order and last-write-wins cannot repair it. A late historical success cannot revive a path after a newer correction or loss.

When A becomes presumed lost, paths that require A suspend immediately. A transitive transport dependency suspends each mechanism that depends on the broken chain, without asserting that every destination position is lost. Independently healthy B and its independent paths may continue. If B has both a mechanism through A and a direct mechanism, only the A-dependent mechanism is suspended; B's position health and the direct mechanism are evaluated from their own current premises. A route whose destination is B does not make B dependent on A unless Access has accepted a current operational dependency. Re-entry is a new candidate assessment plus validation appropriate to presumed or confirmed loss; a historical path is evidence, not permission.

Projection rebuild and historical inspection never dispatch capabilities. An *as-known-then* path view preserves the premises available at that historical frontier; a retrospective view may apply later corrections under a labeled later frontier. CampaignTrajectory remains the owner of the actual decision/action narrative.

## 9. Isolation, authority and deferred choices

All position and path reads, writes, invalidation and rebuilds enforce engagement/campaign scope and source/exercise mode. Blind Pathing excludes privileged Observer/Grader material even when it is stored nearby or labeled historical. Legitimately campaign-observed telemetry follows Terrain admission; separately authorized defender-informed feedback remains labeled and evaluated separately.

Neither model exposes a direct execution handle to reasoning. A foothold reference is evidence for an origin check, not dispatch authority. AttackPathView cannot widen an approved target or convert a candidate into a command. Campaign capabilities cannot mutate either model directly or alter authoritative audit evidence.

Separating owner state from projections introduces observable projection lag. Safety and authority must not depend on a fast refresh: a consequential use checks current owner premises or defers when eligibility cannot be established. Timing SLOs, refresh budgets and fallback mechanics require workload/runtime evidence and remain deferred.

This ADR selects no SQL schema, graph crate/database, ranking formula, path algorithm, health timeout, retry count, validation command, cache budget, process topology or visualization. Domain contracts later define intent-specific inputs, outputs, errors, revision scopes and evidence requirements. Runtime isolation remains with later execution design.

## 10. Invariant compliance

| Invariant | Foothold/path obligation |
| :--- | :--- |
| INV-001 | Separate bounded owners and use cases; no universal campaign graph or manager. |
| INV-002 | Access truth and path projection remain separate from Terrain, Objectives and Trajectory. |
| INV-003 | Position/path records never authorize or directly invoke execution. |
| INV-004 | Candidates and outcomes retain evidence limits; success is not access validation. |
| INV-005 | No raw credentials, client content or authentication stores in either model/cache. |
| INV-006 | Corrections append; campaign capabilities cannot rewrite authoritative records. |
| INV-007 | Blind projections exclude privileged feeds; mode/source survive derivation and replay. |

## 11. Review cases and consequences

| Synthetic case | Required observable behavior when runtime is authorized |
| :--- | :--- |
| A PROVISIONAL observation suggests candidate D | Pathing may form a bounded hypothesis or observe-more candidate; it cannot use that observation alone for a consequential transition. |
| Path candidate reaches transient D | No FootholdGraph node until Access validation; only permitted PROVISIONAL orientation from D. |
| Tool says success but identity/privilege is unresolved | Attempt remains insufficient; no validated position or objective success. |
| A's validation freshness expires without a new event | The next position use/health assessment treats A as stale and blocks consequential origin use; no per-second write is required. |
| A exceeds the no-contact threshold without affirmative denial | Access may accept presumed loss and suspend dependencies; it cannot claim confirmed loss. |
| Position A is presumed lost while B is independently healthy | Suspend A-dependent paths; B remains usable within current bounds. |
| B uses a live tunnel through A | Access records the current dependency; loss of A suspends use through it without declaring B lost, then reconciles any alternate mechanism or fresh B evidence. |
| B has both a live mechanism through A and a healthy direct mechanism | Presumed loss of A suspends only the A-dependent mechanism; the direct mechanism remains eligible after its own current checks, and B is not declared lost. |
| Parallel transient attempts reach D1, D2 and D3 with validate, fail and unknown outcomes | Access accepts only validated D1; D2 creates no foothold; D3 remains unresolved; Pathing updates each dependent candidate without global rollback or optimistic promotion. |
| Historical route A→B succeeded yesterday | May inform a hypothesis; cannot establish current route, access or retry authority. |
| Chain link reaches a new target and returns an unexpected identity | Stop dependent links, retain bounded outcome, reconcile before another proposal. |
| Access commits B then crashes before notifying Pathing | Durable Access state and publication obligation survive; Pathing eventually rebuilds without execution. |
| Pathing cache contains a newer loss followed by a late old health event | Owner revisions prevent revival; affected projection remains invalidated or reconciles. |
| Privileged defender feedback appears in blind Pathing cache | Reject the input/view; no relabeling into campaign evidence. |
| Core restarts with a formerly healthy position and an unknown in-flight attempt | Reconcile capability and outcome; do not restore health or repeat the action automatically. |
| Same host is reached under a different identity/privilege | Treat as a distinct access claim requiring validation; do not widen the existing position. |

The selected architecture supports adaptive, human-like campaign work through competing hypotheses and bounded decisions while keeping current access explicit. It deliberately prevents consequential use from a one-shot transient position before the required validation; this is an authorized-emulation safety trade-off, not a claim that real adversaries always wait. Costs include owner-view mapping, dependency references, projection lag, invalidation and recovery reconciliation. These are future contract/integration verification obligations, not executed runtime tests; the repository remains document-only.

ADR-007 is **PROPOSED** for product-owner review. Its acceptance would complete the authored Stage 3 ADR set, but DW-FOUNDATION-001 still requires a final cross-foundation coherence check and explicit seal action. Runtime implementation remains unauthorized.

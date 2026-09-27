# ADR-002: Domain Boundaries Definition

| Metadata | Value |
| :--- | :--- |
| **Document ID** | ADR-002 |
| **Title** | Domain Boundaries Definition |
| **Status** | ACCEPTED |
| **Acceptance** | Product owner, 2026-09-27: REVISE, THEN ACCEPT; objective eligibility ownership, execution-boundary separation, intent-specific commands, and transient acquisition context clarified and checked |
| **Stage** | Stage 3 — Foundation ADRs |
| **Direct Dependency** | [ADR-001](ADR-001-modular-monolith.md) — ACCEPTED |
| **Governing Product Authority** | [PRD-000](../prd/PRD-000-product-thesis.md), [PRD-001](../prd/PRD-001-campaign-lifecycle.md), [PRD-002](../prd/PRD-002-cyber-terrain.md), [PRD-003](../prd/PRD-003-access-and-footholds.md), [PRD-004](../prd/PRD-004-expansion-loop.md), [PRD-005](../prd/PRD-005-objective-loop.md), [PRD-006](../prd/PRD-006-adaptation.md) — ACCEPTED |

## 1. Context and problem

ADR-001 selects a modular campaign core with separate trust boundaries. A deployment boundary alone does not prevent a universal campaign object, circular dependencies, or a reasoner treating a tool result as verified access. This decision assigns semantic ownership and permitted interactions before event, storage, language, and runtime contracts are designed.

The four loops — Strategic, Access, Expansion, and Objective — revisit one another as evidence changes. Adaptation overlays all four. Loops describe decisions and behavior; they do not each own a duplicate of the five operational models. A fast chain may avoid repeated reasoning, but never current dispatch authority or evidence required by its stakes.

## 2. Decision drivers

1. Give every environmental, access, route, objective, and historical claim one accountable model owner.
2. Let bounded use cases combine relevant evidence without retaining a global mutable state or all-domain service locator.
3. Keep reasoning, action authority, execution, proof derivation, and defender assessment distinct.
4. Preserve source, time, origin, tier, uncertainty, and claim limits across every boundary.
5. Support loss, re-entry, dwell, retasking, and unknown action outcomes without silently replaying effects.
6. Make prohibited dependencies and invariant violations testable once implementation is authorized.

## 3. Options considered

| Option | Benefit | Cost / disposition |
| :--- | :--- | :--- |
| One campaign aggregate with terrain, access, paths, goals, and history nested inside | Convenient reads and updates | One owner can overwrite unrelated truths; violates INV-001/002. Rejected. |
| One domain per operational loop, each with its own terrain/access/objective state | Close correspondence to workflow terminology | Duplicates truth across loops and makes adaptation a synchronization problem. Rejected. |
| **Model-owned contexts plus bounded application use cases and separate authority boundaries** | Explicit truth ownership with reusable evidence across loops | Requires narrow contracts and explicit handling of stale or partial views. Selected. |

## 4. Decision: model ownership

Each row is a separately encapsulated model context with its own domain rules and an intent-specific command interface that enforces the owning context's invariants; no generic aggregate mutation interface is permitted. They are not a shared crate/module of mutable aggregates. Concrete package names and public contract types are deferred; this is not a mandate for five services or databases.

| Context / owned model | Owns and decides | Permitted inputs / published output | Must not own or infer |
| :--- | :--- | :--- | :--- |
| Terrain / CyberTerrain | Environmental claims, relationships, provenance, freshness, epistemic status and tier; environmental Key Terrain claims | Admitted observations and origin-validation references → bounded terrain views and accepted deltas | Access/session health, mission completion, execution, or planning; OBSERVED/PROVISIONAL is not a fact |
| Access / FootholdGraph | Validated positions bound to target, identity, privilege and capability; health, staleness, uncertainty, presumed/confirmed loss and renewed validation | Bounded validation evidence and authorized position context → position assessments and changes | Candidate paths, every discovered identity, raw credentials, or execution authority; transient access is not a foothold |
| Pathing / AttackPathView | Derived route candidates, premises, dependencies, and transition views, with references to validation evidence | Selected terrain, access, capability-bound and objective-relevance views → candidate/transition projections | Independent environmental truth, access validation, objective fulfillment, or scope expansion |
| Objectives / ObjectiveState | Declared success conditions within authorized mission goals, candidate targets, objective eligibility based on referenced Terrain, Access and Mission claims, proof review, and scoped fulfillment | Mission bounds, relevant terrain/access views and derived proof → objective opportunities and review outcomes | Target-role environmental truth, foothold health, mission authorization, access grants, control-gap verdicts, or success inferred from a tool return |
| Trajectory / CampaignTrajectory | Append-only, time-ordered narrative of decisions, authorizations, attempts, outcomes, reconciliations and tempo changes | Non-sensitive records from accountable producers → historical references and bounded historical views | Current environmental/access truth or dispatch authority; history cannot restore a lost position |

Key Terrain's environmental claim stays with Terrain; Strategic reasoning assesses its mission significance from bounded views. Pathing is a derived analytical owner, not a second system of environmental truth. A historical successful transition remains evidence of the past, not proof that its route is usable now.

Objective eligibility is a bounded assessment of target suitability and proof conditions, not a copied source of target role, access health or authorization. Terrain owns target-role environmental claims, Access owns current usable access, and Mission/lifecycle retains operator-authorized mission bounds; deterministic action authority checks them at dispatch. Eligibility retains references and freshness to those premises. Stale or conflicted terrain, presumed-lost access, or withdrawn authorization blocks a dependent proof attempt until its premises are reconciled and eligibility is reassessed. Historical scoped fulfillment does not grant eligibility for a new action.

Only the owning context accepts a change to its model. Another context may publish evidence or request reconsideration through a bounded contract; it cannot mutate the owner's aggregate, persistence, or private implementation. Corrections preserve provenance and history instead of rewriting authoritative audit evidence.

## 5. Application and trust boundaries

These are responsibility boundaries, not additional universal campaign models or a service topology.

| Boundary | Responsibility | Excluded authority |
| :--- | :--- | :--- |
| Mission and lifecycle | Operator-authorized goals, scope references, exercise mode, lifecycle posture, dwell and retasking decisions under PRD-001/006 | Does not contain the five aggregates, authorize itself, or treat dwell as proof of healthy access |
| Loop-specific application use cases | Gather only relevant published views, request a bounded reasoning episode, route proposals and resulting evidence to their owners | No all-domain manager, global context, direct aggregate mutation, or adapter execution handle |
| Short-lived reasoning specialists | Compare hypotheses and alternatives; produce proposals, expected/disconfirming evidence and stopping conditions | No model writes, arbitrary execution, raw sensitive content, or privileged defender feed in blind mode |
| Deterministic action authority | Validate each proposed action against current mission bounds, evidence requirements, origin conditions and safety at dispatch | Cannot invent evidence, promote a hypothesis, or expand operator authorization |
| Capability Gateway | Accept an already-authorized request and validate eligibility against the capability contract before admission | Does not execute, grant missing authorization, expand scope, or bypass deterministic authority |
| Execution Broker | Dispatch an admitted request, correlate its outcome, coordinate cancellation and track unknown outcomes for reconciliation | Does not alter authorization or approved request semantics, bypass Gateway admission, or treat an unknown outcome as permission to replay |
| Adapter / execution environment | Execute the dispatched request within its isolated bounds and report the bounded outcome | Cannot self-authorize, self-admit, widen scope, write operational models, alter authoritative audit, or infer objective fulfillment |
| Observation admission | Check source, provenance and data eligibility before model-specific consideration | No generic reconciler that owns all model semantics; no raw sensitive intake into the core |
| Isolated ephemeral sensitive proof boundary | Process expressly authorized minimum client content only to derive approved opaque proof | No raw sensitive output to core, logs, prompts, durable or operator-facing surfaces; no model acceptance authority |
| Observer / Grader | Independent defender assessment against campaign evidence and sensor coverage | No privileged input to blind reasoning, no campaign action authority, and no substitution for target-specific objective proof |

The action path remains Reasoning → Proposal → Deterministic action authority → Capability Gateway → Execution Broker → Adapter / execution environment. For action dispatch, no component may bypass the preceding boundary: Gateway cannot execute directly, authority cannot dispatch directly to an adapter, and an adapter cannot admit its own work. These are distinct responsibilities, not a requirement for separate services or processes. Cancellation and safety-stop handling must not require a new reasoning proposal or be obstructed by action-admission sequencing; their mechanisms remain downstream decisions. Adapter translation into tool-native inputs must preserve the approved target, identity, scope and action semantics.

Observation admission establishes whether an input is eligible for consideration; the receiving model owns the domain-specific reconciliation and claim it accepts. This prevents an evidence utility from becoming a hidden authority over all five models. Detailed observation/proof contracts and containment mechanisms remain with their later authorized stages.

A narrowly scoped initial access attempt and validation may precede a foothold. Authorized transient-origin read-only orientation is permitted under PRD-003 and remains PROVISIONAL. Neither the authority boundary nor a loop use case may require an already-existing foothold for every possible action; follow-on state-changing expansion and objective work retain their validated-origin requirements.

## 6. Dependency and interaction rules

1. Domain implementations depend only on their own domain contracts and genuine primitives. They do not import sibling domain implementations, persistence clients, LLM/tool clients, HTTP clients, telemetry backends, or OS execution code.
2. A bounded application use case depends on its owning domain and only the narrow public ports/contracts needed for that behavior. Foreign inputs arrive as selected immutable views, identifiers, and evidence references, never mutable aggregates or repositories belonging to another context.
3. Adapters implement application/domain ports. Infrastructure depends inward; domain logic does not depend on infrastructure. Cross-context contract dependencies must be acyclic; recurring operational feedback does not justify source-code dependency cycles.
4. Shared primitives contain identifiers and genuinely common value semantics only. Business policy, reconciliation, routing, and collections of all model types do not migrate into a shared module.
5. Startup composition connects declared ports without retaining a runtime object or registry exposing the entire platform. Use cases receive only their declared dependencies; no universal context/provider is passed down the call chain.
6. A cross-model consequence is an explicit request/evidence notification to another owner. Consumers validate provenance and current premises before accepting their own change. Typed local calls and events may implement these boundaries; ADR-003 decides event semantics. No generic event bus is selected here.
7. A combined view is not a magically atomic snapshot. Its inputs retain freshness and provenance; stale, conflicted or unavailable dependencies block the consequential action that needs them. Dispatch rechecks current authority rather than trusting an earlier reasoning snapshot.
8. Transport success is not model acceptance. An accepted terrain observation does not imply accepted access, and an accepted access claim does not imply objective fulfillment. Failures and pending reconciliation remain visible to dependent use cases.

In blind mode, new observations require an authorized acquisition context: a validated position, authorized mission context, or PRD-003's bounded transient read-only vantage. Results from that transient vantage remain PROVISIONAL pending origin validation and tier-aware reconciliation; this permission does not extend to follow-on state-changing expansion or objective proof. Retained evidence from earlier positions may inform bounded reasoning with its original source, time, epistemic limits and current freshness; it does not establish present access. Campaign-visible effects and legitimately acquired telemetry remain ordinary sourced observations subject to PRD-000 INV-007. Privileged Observer/Grader information is excluded, including from shared caches and reused reasoning context. Separately authorized defender-informed feedback must retain its source and mode label; it cannot silently contaminate later blind context.

## 7. Failure and recovery obligations

- A position entering presumed loss suspends work dependent on that position; affirmative evidence is required for confirmed loss. Independent healthy positions may continue within their own bounds. Access owns recovery validation; Trajectory merely records it.
- If origin validation fails or is abandoned, Terrain downgrades its PROVISIONAL observations to HYPOTHETICAL and preserves provenance. Origin validation success triggers tier-aware reconsideration, not automatic corroboration. Dependent projections must invalidate unsupported premises.
- A bounded chain stops dependent links when target, identity, scope, safety premise, or origin changes. Exploitation tempo never lowers the tier required for the consequential claim.
- Missing authorization, required evidence, containment, or an unavailable required owner blocks the affected action. A campaign-wide revocation or safety freeze applies campaign-wide; an unrelated healthy position cannot override it.
- After a core failure, an issued action with an unknown outcome stays unresolved until reconciliation supports a next step. Recovery cannot infer success, failure, or permission to repeat it from a missing response. Retry and recovery mechanisms belong to downstream contracts.
- Raw sensitive content is discarded within the isolated ephemeral boundary on success, failure, interruption, or withdrawal of authorization. Only approved opaque proof and non-sensitive provenance may cross into the core.
- Audit producers may submit permitted new records; campaign capabilities receive no authority to alter, delete or disable authoritative records. The protected write mechanism is deferred, not waived.

## 8. Consequences and deferred decisions

Ownership and inward dependencies make a small modular core reviewable and allow the four loops to share understanding without sharing mutable authority. Individual contexts can evolve behind their contracts.

Costs are explicit mapping between bounded views, partial/stale cross-model knowledge, and more deliberate routing than a global object. Avoid one-field wrappers and speculative abstractions without a real boundary need. Module size and McCabe budgets complement ownership review; small functions alone cannot prove architectural separation.

This ADR does not define event schemas, delivery guarantees, transaction boundaries, database/graph storage, language-specific types, thread/process layout, queue technology, or exact package names. ADR-003 owns event semantics, ADR-004 language, ADR-005/006 storage, and ADR-007 the detailed foothold/path separation. Later execution, evidence and Observer designs must preserve the responsibility boundaries above; those details are not authorized for implementation by this ADR.

## 9. Invariant compliance

| Invariant | Boundary obligation |
| :--- | :--- |
| INV-001 | Bounded use cases and dependencies; no global manager, mutable context or universal runtime registry |
| INV-002 | Five independently encapsulated model owners; cross-model effects go through the destination owner's contract |
| INV-003 | Reasoners propose; deterministic authority gates dispatch; isolated capabilities execute |
| INV-004 | Admission is not claim acceptance; model-specific reconciliation preserves status, tier, origin and freshness |
| INV-005 | Raw sensitive material stays outside core and all persistent/operator-facing outputs |
| INV-006 | Append-only historical contribution cannot confer audit alteration authority on campaign capabilities |
| INV-007 | Blind context excludes privileged assessment inputs; campaign-visible evidence retains its limits; informed mode stays labeled |

## 10. Reviewable counterexamples and implementation verification

| Synthetic counterexample | Required observable result when implementation is authorized |
| :--- | :--- |
| Terrain imports an execution adapter or a sibling's private state | Architecture dependency check rejects the dependency |
| Two contexts depend on each other's implementation through convenience helpers | Dependency check rejects the cycle; refactor responsibilities, not just filenames |
| A use case receives all five mutable aggregates in a context object | Structural review rejects the design even if each function has McCabe <= 7 |
| An eligible objective target's terrain premise becomes stale or its origin becomes presumed lost | Suspend the dependent proof attempt; ObjectiveState cannot override Terrain or Access with a cached validated-target flag |
| Authority calls an adapter directly, Gateway executes, or an adapter admits itself | Reject the dispatch path for bypassing a required preceding boundary |
| A broker or adapter widens an approved request's target, identity, scope or action semantics | Reject the changed request; only an appropriately authorized and admitted request may execute |
| A caller tries to replace another context's aggregate through a generic setter | No such mutation interface is exposed; intent-specific commands enforce the owner's invariants |
| A transient position reports a local identity | Permit authorized read-only orientation as PROVISIONAL; do not require an already-validated foothold or grant a consequential origin |
| Origin validation succeeds while a consequential relationship lacks corroboration | Terrain reconsiders status under its tier; no automatic fact/authority promotion |
| A tool returns success without reliable context or target-specific proof | Access/objective acceptance remains unproven; transport success cannot bypass either owner |
| Position A becomes presumed lost while B is independently healthy | Suspend A-dependent work; preserve B's authorized work; confirmed loss is not inferred |
| The core restarts after dispatch but before observing its outcome | Mark the attempt unresolved and reconcile; no blind replay |
| A privileged Observer alert is accessible beside a blind campaign | Context construction excludes it; a legitimately campaign-observed alert remains a distinct sourced observation |
| Sensitive proof processing is interrupted | No raw synthetic sensitive fixture appears in durable, diagnostic or reasoning outputs |

Document review checks ownership, links, dependencies, failure semantics and cross-PRD consistency now. Future runtime packets must prove the real entrypoint-to-consumer path, illegal-transition/negative controls, permitted and forbidden dependencies, and boundary fixtures for McCabe 7 (pass) and 8 (fail). Tool selection and CI wiring are not implemented here; duplicate/dead-code signals require review of real responsibility and reachability.

ADR-002 is ACCEPTED under the product owner's 2026-09-27 REVISE, THEN ACCEPT decision, after the four requested clarifications and their counterexamples were checked. No runtime tests, CI result, implementation readiness, or foundation seal is asserted. ADR-003 is the next design dependency and remains unauthored; this revision stops at ADR-002.

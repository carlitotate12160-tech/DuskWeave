# ADR-001: Modular Monolith Architecture

| Metadata | Value |
| :--- | :--- |
| **Document ID** | ADR-001 |
| **Title** | Modular Monolith Architecture |
| **Status** | PROPOSED |
| **Stage** | Stage 3 — Foundation ADRs |
| **Direct Dependencies** | [PRD-000](../prd/PRD-000-product-thesis.md), [PRD-001](../prd/PRD-001-campaign-lifecycle.md), [PRD-002](../prd/PRD-002-cyber-terrain.md), [PRD-003](../prd/PRD-003-access-and-footholds.md), [PRD-004](../prd/PRD-004-expansion-loop.md), [PRD-005](../prd/PRD-005-objective-loop.md), [PRD-006](../prd/PRD-006-adaptation.md) — all ACCEPTED |

## 1. Context and problem

DuskWeave must maintain a long-running, goal-directed campaign while letting specialized reasoning workers change hypotheses as access, terrain, defender-visible friction, and objectives change. Deliberate and exploitation tempo differ in how quickly observations inform a next proposal. Both keep scope, safety, and epistemic limits. PRD-003 allows provisional orientation from transient access; PRD-006 allows bounded rapid reassessment. Neither gives an unvalidated position authority for consequential follow-on action.

The foundation must keep the five operational models distinct, preserve reasoning/execution separation, protect sensitive proof and audit integrity, and prevent privileged defender feedback from reaching blind reasoning. A single all-knowing campaign engine would blur those boundaries. Distributing every campaign decision among independent services from the start would add network choreography to an already time-sensitive evidence loop.

### 1.1 Observed operator patterns and design inference

| Primary incident report | Observed pattern relevant to this decision | DuskWeave design inference |
| :--- | :--- | :--- |
| [Mandiant / Google, APT41 DUST campaign (2024)](https://cloud.google.com/blog/topics/threat-intelligence/apt41-arisen-from-dust) | Prolonged access with changes in tooling and later hands-on-keyboard activity within a sustained campaign. | Preserve a continuous campaign narrative and allow bounded specialists to reassess positions and opportunities over time. |
| [Kaspersky, Lazarus Operation SyncHole (2025)](https://securelist.com/operation-synchole-watering-hole-attacks-by-lazarus/116326/) | Investigators observed several related execution chains and hands-on internal reconnaissance; they assessed, rather than proved, that later changes may have followed defensive action. | Support hypothesis revision and fast, reviewable continuation without a rigid one-pass workflow. |
| [CISA and partners, Volt Typhoon advisory AA24-038A (2024)](https://www.cisa.gov/news-events/cybersecurity-advisories/aa24-038a) | The advisory describes prolonged pre-positioning and use of legitimate native tools and processes. | Preserve campaign continuity, bounded native-environment observations, and a low unnecessary footprint. |

These reports document operations, not software architecture. The modular monolith choice below is an inference from DuskWeave's requirements; it is not attributed to these actors. Their destructive, unauthorized, or raw-data-retaining behavior is never adopted as product permission. No actor-specific playbook becomes core architecture.

## 2. Decision drivers

1. Keep a short observe → hypothesis → proposal → authorized action → outcome loop, including exploitation tempo, without an approval or full-corroboration pause at every harmless observation.
2. Make current position, epistemic status, source, and allowed claim visible at each consequential decision. An OBSERVED or PROVISIONAL entry is not automatically a fact or authorization (PRD-000 INV-004).
3. Keep CyberTerrain, FootholdGraph, AttackPathView, ObjectiveState, and CampaignTrajectory under separate ownership; no universal mutable campaign object (INV-001/002).
4. Ensure every executed action passes deterministic scope and safety authority at dispatch, including bounded chains, while cognitive workers only propose (INV-003).
5. Keep raw client material in the isolated ephemeral sensitive boundary; preserve authoritative audit evidence (INV-005/006).
6. Enforce the mode-specific defender knowledge boundary without making the Observer an invisible tactical oracle (INV-007).

## 3. Options considered

| Option | Benefit | Failure or cost |
| :--- | :--- | :--- |
| One unbounded campaign engine with shared mutable state | Simple initial call path | Mixes five models, reasoning, authority, and execution; encourages a God Object and privileged-data leaks. Rejected. |
| Independent network services for every operational model and reasoning step | Independent deployment and scaling | Adds distributed ordering, retries, and consistency problems before domain contracts exist; risks losing time-sensitive momentum. Deferred as a possible later extraction, not the starting shape. |
| **Modular monolith for the campaign core, with explicit external trust boundaries** | Local composition and short decision paths with enforceable ownership | Needs architectural checks to prevent direct cross-module state access and accidental central coordinators. Selected. |

## 4. Decision

Build the **campaign core** as one cohesive deployable application whose domain and application modules have narrow owners and explicit interfaces. Cohesive deployment is not shared authority: no module owns all five operational models, all tool adapters, or the entire workflow. Module boundaries are enforceable in code and reviewed through their public contracts. The exact domain boundaries belong to ADR-002, domain-event semantics to ADR-003, implementation language to ADR-004, and persistence/storage to ADR-005–007.

The core composes bounded decisions from separately owned model views through typed proposals and results; it does not retain a universal campaign state. Short-lived specialist reasoners receive only their relevant, sourced context; they do not receive a universal state object, arbitrary execution handle, raw sensitive content, or privileged defender feed in blind mode. An approved action travels through deterministic authority and an isolated execution boundary. The Observer evaluates outcomes independently; any defender-informed exercise has separately authorized, labeled input and evaluation.

A rapid, pre-assessed chain may avoid a new cognitive planning episode or full evidence reconciliation between every link. **Every action still faces current deterministic scope and safety checks at dispatch.** Unexpected target, identity, origin, scope, or outcome stops dependent links until reassessment. Provisional observations may promptly inform orientation and candidate hypotheses from transient positions; they cannot alone validate a foothold, scope crossing, consequential transition, or objective proof. The core keeps these epistemic distinctions without imposing a mandatory state-machine script on operator decisions.

The isolated execution environment, ephemeral sensitive proof boundary, defender assessment plane, and external tools may require process separation or independent deployment. This ADR chooses the shape of the **campaign core only**; it does not collapse trust boundaries into one process. Later ADRs specify those boundaries without turning this foundation decision into a service topology mandate.

## 5. Architectural obligations and failure handling

- Each operational model has a named owner and a bounded interface. Cross-model changes preserve their originating evidence and never write another model's truth as a side effect. ADR-002 will assign exact ownership and dependencies.
- A proposal carries the hypothesis, current position, authorization scope, expected and disconfirming evidence, and stopping condition needed for review. Its presence never grants execution authority.
- A validated, narrow Tier 1 observation may be available quickly as OBSERVED; higher-stakes claims require stronger support. Transient-origin observation remains PROVISIONAL until its origin is resolved. A source or tempo change never silently promotes the claim.
- If authorization is withdrawn, a dispatch check fails, or a chain premise changes, the relevant action or chain stops. Independent healthy positions may continue under their own authority. Timeout does not automatically mean confirmed foothold loss.
- Durable decision and proof output contains only non-sensitive provenance and approved opaque evidence. Raw client material never enters core state, prompts, traces, or reporting.
- Blind reasoning can use effects visible from its authorized current campaign position. Privileged Observer/Grader/defender feeds cannot enter its context. Defender-informed feedback requires a separately authorized and labeled exercise.

## 6. Consequences

**Positive:** Local composition avoids network round trips among the five models during ordinary reasoning. Explicit ownership and typed boundaries support rapid adaptation with reviewable evidence. A single campaign-core deployable simplifies early evolution without forcing one cognitive worker to know everything.

**Costs:** Modules in one deployable can still become tightly coupled through convenience imports or shared mutable state. Establish dependency checks and contract-focused tests as the design becomes executable. The core cannot scale individual internal modules independently until a justified extraction.

**Migration trigger:** Consider splitting one bounded context into its own deployable only after a measured isolation, reliability, or scaling need and an accepted ADR. Extraction must preserve its model owner, action authority, evidence provenance, and INV-007 mode labeling. Deployment count is not an acceptance metric.

## 7. Invariant compliance

| Invariant | ADR-001 obligation |
| :--- | :--- |
| INV-001 No God Object | No global campaign manager or context exposing all state, tools, and authority. |
| INV-002 Separate models | Five model owners with bounded interfaces; no shared mutable aggregate. |
| INV-003 Reasoning != execution | Specialists propose; deterministic authority gates every dispatch; executor remains isolated. |
| INV-004 Observation != fact | Tier and status remain explicit; provisional/observed data cannot silently become fact or authority. |
| INV-005 Sensitive zero-retention | Core receives only approved opaque proof and non-sensitive provenance; raw content remains in an isolated ephemeral boundary. |
| INV-006 Audit integrity | Core and campaign capabilities have no authority to alter authoritative audit evidence. |
| INV-007 Defender Knowledge Boundary | Blind context excludes privileged assessment feeds; campaign-visible evidence and separately labeled defender-informed mode obey PRD-000. |

## 8. Reviewable counterexamples and acceptance

1. A transient position yields a local identity query: the result is available for a sourced hypothesis without granting a validated foothold or downstream write authority.
2. A bounded chain encounters a new target or lost origin: the next dependent link stops even though the chain was pre-assessed; no planner round trip is imposed on a link whose premises still hold.
3. An Observer alert is present while a blind campaign is active: it is assessed separately and does not enter the blind reasoner's context. An authorized defender-informed exercise has a visibly different label and result set.
4. An objective uses permitted client content: the core sees only derived opaque proof; raw content is discarded within the isolated ephemeral boundary on success or failure.
5. One position becomes presumed lost while another remains healthy: only dependent work pauses; the core does not infer confirmed loss or global campaign failure.

This is a design decision and has no runtime validation yet. It neither prescribes service topology for the trust boundaries nor decides domain-event schemas, database, graph representation, language, process protocol, or tooling. ADR-002 is the next dependency; ADR-001 remains PROPOSED until separately accepted.

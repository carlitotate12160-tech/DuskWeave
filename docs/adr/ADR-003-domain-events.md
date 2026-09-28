# ADR-003: Domain Events Architecture

| Metadata | Value |
| :--- | :--- |
| **Document ID** | ADR-003 |
| **Title** | Domain Events Architecture |
| **Status** | PROPOSED |
| **Stage** | Stage 3 — Foundation ADRs |
| **Direct Dependency** | [ADR-002](ADR-002-domain-boundaries.md) — ACCEPTED |
| **Supporting Architecture** | [ADR-001](ADR-001-modular-monolith.md) — ACCEPTED |
| **Governing Product Authority** | [PRD-000](../prd/PRD-000-product-thesis.md), [PRD-001](../prd/PRD-001-campaign-lifecycle.md), [PRD-002](../prd/PRD-002-cyber-terrain.md), [PRD-003](../prd/PRD-003-access-and-footholds.md), [PRD-004](../prd/PRD-004-expansion-loop.md), [PRD-005](../prd/PRD-005-objective-loop.md), [PRD-006](../prd/PRD-006-adaptation.md) — ACCEPTED |

## 1. Context and problem

The modular core has five separate model owners. A change accepted by one owner may require another to reconsider a route, position, objective eligibility, or historical account. Direct writes into another model violate ADR-002; a global event handler with every domain dependency would recreate the same problem.

Campaigns pause, resume, dwell, and survive process failure. Notifications can be delayed, duplicated, or processed only partially before interruption. A stale projection cannot authorize a new action. A recorded successful attempt is neither a validated foothold nor objective fulfillment. Event semantics must preserve those distinctions without selecting a distributed messaging platform.

## 2. Decision drivers

1. Preserve model ownership, evidence provenance, epistemic limits and current authority.
2. Make accepted changes and their required consequences recoverable across interruption.
3. Distinguish delivery, consumer acceptance and external execution effects.
4. Preserve causal relationships without claiming a global order across independent positions.
5. Prevent replay, fan-out, retries and diagnostic surfaces from bypassing execution or sensitive-data boundaries.
6. Keep local composition simple; define guarantees before storage and runtime mechanisms.

## 3. Options considered

| Option | Benefit | Cost / disposition |
| :--- | :--- | :--- |
| Direct synchronous callbacks only, with no recoverable notification record | Minimal plumbing | Process failure can lose required consequences after the owner accepts a change. Rejected for required cross-context propagation. |
| Universal event-sourced state and a shared event bus for all operations | Uniform replay and integration model | Premature storage commitment; encourages global schemas, authority by subscription and accidental action replay. Rejected as the foundation default. |
| **Owner-published typed events, bounded consumers, recoverable required delivery** | Explicit ownership and interruption semantics; local delivery remains possible | Requires completion tracking, duplicate handling and visible unresolved dependencies. Selected. |

## 4. Semantic distinctions and ownership

| Item | Meaning / accountable producer | What it cannot establish |
| :--- | :--- | :--- |
| Observation | A sourced signal admitted for consideration under the relevant boundary | Accepted environmental truth, access, authorization, or objective success |
| Command / proposal | Intent directed to an accountable owner; a reasoner proposes and deterministic authority decides action permission | A completed transition; neither delivery nor enqueueing is acceptance |
| Domain event | An immutable statement that a named owner accepted a specific transition or assessment within its responsibility | Universal truth about its payload, another owner's acceptance, or permission to execute |
| Execution outcome record | A bounded report of an attempt and its observed or unresolved result through the execution boundary | Validated access, target-specific proof, or exactly-once external effect |
| Trajectory record | The non-sensitive historical account accepted by CampaignTrajectory from accountable producers | A dispatch instruction or a replacement for another owner's current model |
| Notification delivery / acknowledgment | Delivery and consumer-processing status | Business acceptance unless the owning consumer explicitly records its decision |

An event saying Terrain accepted an OBSERVED or PROVISIONAL claim asserts that acceptance at that status, not that the environmental claim is CORROBORATED. Access owns position validation/loss events; Objectives owns eligibility/review events; each owner publishes only within its own authority. A reasoner cannot fabricate a model event and acquire that owner's authority.

Only an accepted owner transition may be published as that transition's event. Rejected intent and incomplete processing may generate their own accountable outcome records; they must not masquerade as successful state changes. Consumers use intent-specific interfaces of their own context, never a generic setter into another owner's aggregate.

## 5. Event contracts and bounded routing

Each cross-context event family must identify its owner, semantic meaning, intended consumers and consequences before implementation. Consumer contracts state whether receipt requests reconsideration, invalidates a projection, contributes history, or supplies evidence; receipt never grants action authority.

A contract must carry or securely reference the information necessary to interpret it: stable event identity, producer/owner identity, engagement and campaign isolation scope, affected entity and owner revision, event kind/version, causal/correlation references, occurrence/recording time, and applicable evidence provenance, origin, tier, epistemic status and source/exercise-mode labels. These are semantic obligations, not a wire schema, identifier algorithm, database layout or universal payload object. Required fields and their applicability are resolved in the corresponding domain contracts.

Payloads are minimal, typed and non-sensitive. Referenced evidence remains subject to the same access and provenance checks as inline content; a reference is not permission to fetch raw sensitive material. Contracts may not carry raw adapter output, arbitrary serialized aggregates, live authority/execution handles, raw sensitive content or universal campaign context.

Consumers validate the producer's right to assert the event, campaign/engagement isolation scope, contract version and source/mode eligibility before applying it. A type name or claimed producer identifier alone is not proof of authority.

Subscriptions are explicit and bounded by producer, event family, consumer responsibility and exercise mode. Domain code does not discover consumers or know infrastructure clients. Wiring uses declared ports with no global runtime service locator. Optional presentation consumers are distinguishable from required domain/history consumers; arbitrary optional subscribers cannot become hidden acceptance dependencies.

Blind-campaign routes and their caches exclude privileged Observer/Grader feeds. Legitimately campaign-acquired telemetry remains a separately sourced observation under INV-007. Defender-informed labels and source provenance survive forwarding, derivation and replay; changing a label cannot launder privileged input into blind context.

## 6. Publication and processing guarantees

1. An accepted transition and the obligation to publish its required event must not be separated by an unrecoverable failure window. Publish only after the owner transition is accepted durably. The future persistence design must provide atomic recording of state and publication obligation, or an equivalent recoverable guarantee; best-effort publish after save is insufficient.
2. Required delivery uses at-least-once delivery semantics with stable identity across redelivery. Deduplication binds identity to its accountable producer and engagement/campaign scope; identity cannot be reused for a new semantic event. Redelivery preserves the event's semantic content; per-attempt transport metadata is not a new domain event. Acceptance records a recoverable publication obligation; a consumer records completion only after its own required effects are durable. A lost acknowledgment may cause redelivery.
3. Each required consumer must make reprocessing of the same event safe. Its local model effects and completion record must be atomically recorded or equivalently recoverable. A duplicate cannot create a second foothold, repeat a fulfillment transition, or append the same logical history item again.
4. Stable identity with conflicting content is an integrity conflict, not an ordinary duplicate. Reject or isolate processing using safe metadata, preserve the conflict evidence permitted by data rules, and block affected dependencies until resolved.
5. Producer acceptance does not wait for every consumer's completion and does not claim global atomic acceptance. Pending or failed required propagation remains visible. A dependent action must not proceed using an unrefreshed projection; it must obtain current premises from their owners or wait for reconciliation.
6. Retries are bounded per recovery attempt. Persistent failure or incompatible input produces a visible unresolved delivery obligation; no silent dropping, infinite hot retry, or false completion. Resume requires an explicit recovery path. Required history or evidence recording failure blocks dependent actions whose audit/evidence preconditions cannot be established.
7. Optional notifications may be regenerated or lost only where their contract explicitly permits it. Required invalidation, evidence and historical consequences cannot be silently reclassified as optional to improve throughput.

These guarantees do not require a message broker, background worker, queue product, or event sourcing of all domain state. A bounded local dispatcher is sufficient only if its surrounding durable publication and consumer-completion mechanisms satisfy the same recovery obligations. No claim of guaranteed eventual success under permanent failure is made.

## 7. Ordering, freshness and correction

Ordering is scoped to the producing owner's affected entity or explicitly defined consistency scope, using a monotonic revision or equivalent predecessor relationship. A publisher's clock time is not a reliable total order. Consumers that require an earlier revision must detect gaps or out-of-order delivery and defer or reconcile with the owner; unrelated healthy scopes need not stop.

An older event may contribute missing history but cannot overwrite newer accepted model state or restore revoked authority. Different contexts can accept changes independently; a combined view retains each source revision, freshness and causal references rather than pretending to be a global snapshot. CampaignTrajectory provides its recorded narrative order while preserving occurrence times, source order, gaps and causality; recording order is not proof of real-world causation.

Correction is a new accountable event referencing the affected claim; authoritative records are not edited or deleted. Terrain's downgrade after failed origin validation invalidates dependent projections. Origin validation success requests tier-aware reconsideration and never silently corroborates the claim. Objective eligibility must be reassessed when referenced Terrain, Access or Mission premises cease to support it.

Revocation, safety freeze and dispatch validity cannot depend solely on an asynchronously delivered notification. Deterministic authority checks current permission and required origin/evidence at dispatch. If current validity cannot be established, the action does not proceed. Cancellation and safety-stop handling cannot be trapped behind ordinary event backlog; mechanisms remain with the relevant execution design.

## 8. Replay, execution and sensitive-data failure

Replay is explicitly scoped to historical inspection or reconstruction of authorized derived state from compatible accepted records. It cannot dispatch capabilities, repeat reasoning with external side effects, renew authorization, restore current foothold health from historical success, or turn an unresolved attempt into an outcome. Rebuilding a projection and issuing a new target action are different operations.

A domain event may prompt a bounded use case to reconsider a proposal; an event subscription cannot directly invoke an adapter. Any resulting action follows current deterministic authority → Capability Gateway → Execution Broker → Adapter, preserving ADR-002. The original proposal/authority may be reused only where current contracts permit and current checks pass; replay itself supplies no permission.

An interrupted external attempt with unknown outcome stays unresolved until evidence supports a bounded next step. Consumer idempotency is not a guarantee of exactly-once effects on a target. Automatic retransmission of an execution request is not justified by redelivery of an event or absence of an acknowledgment.

Raw sensitive content never enters event storage, retry buffers, quarantine/dead-letter payloads, logs, traces or diagnostics. Admission rejects prohibited content without echoing it; only safe identifiers, rejection categories and approved non-sensitive provenance may survive. Authorized proof derivation remains inside the isolated ephemeral boundary and discards raw content on all required exits. Event delivery cannot extend its lifetime.

Event versions have explicit semantic compatibility rules. An unsupported version is not coerced into the nearest known type or silently skipped by a required consumer. Schema migrations must preserve provenance and meaning without rewriting authoritative audit evidence; concrete encodings, migration machinery and retention design are deferred.

## 9. Consequences and deferred decisions

The core gains explicit, recoverable cross-context consequences without distributing every domain. Owners remain authoritative and consumers can fail or recover independently within their dependency bounds. Costs include publication/completion records, stale-view handling, duplicate detection and recovery inspection.

ADR-005 must resolve persistence and atomicity mechanisms before runtime claims these guarantees; ADR-004 selects language without changing them. Later domain, evidence, audit and execution contracts resolve event families, identity/revision scope, retry policy, retention, transport authentication and execution reconciliation. No broker, database, outbox/inbox schema, distributed transaction, universal event envelope, or exactly-once target guarantee is selected here.

## 10. Invariant compliance and review cases

| Invariant | Event obligation |
| :--- | :--- |
| INV-001 | Bounded producer/consumer contracts and routing; no all-domain event manager |
| INV-002 | Only each owner accepts its model transitions; notifications do not transfer ownership |
| INV-003 | Events and replay confer no execution authority and cannot bypass the action path |
| INV-004 | Event occurrence/acceptance preserves the payload's evidence tier and epistemic limits |
| INV-005 | No raw sensitive content in payloads, recovery queues, diagnostics or evidence references accessible to the core |
| INV-006 | Corrections append new records; consumers and campaign capabilities cannot rewrite authoritative evidence |
| INV-007 | Producer source and exercise-mode boundaries survive routing, derivation, caching and replay |

| Synthetic failure case | Required observable result when implementation is authorized |
| :--- | :--- |
| Owner commits a transition then crashes before notification | Required publication obligation survives and resumes; no vanished consequence |
| Consumer commits a change but its acknowledgment is lost | Redelivery produces no duplicate logical model/history effect |
| Same event identity arrives with different content | Integrity conflict is visible; affected dependent work is blocked |
| Access loss revision arrives before an earlier health event | Preserve owner order/current validity; late health cannot revive the position |
| Required invalidation is delayed while a stale objective view looks eligible | Dispatch cannot rely on that stale view; obtain current owner premises or block |
| One required consumer fails while an unrelated position stays healthy | Pending consequence remains visible; only genuinely dependent work stops, subject to campaign-wide authority |
| Unsupported version or persistent processing failure occurs | No silent skip or unbounded hot retry; unresolved obligation has a bounded recovery path |
| Historical events are replayed during recovery | Rebuild permitted views/history without target execution or renewing access |
| Target attempt completed but outcome was never observed | Remain unresolved; notification retry does not repeat the external action |
| Provisional origin later fails validation | Downgrade remains sourced and dependent views lose their unsupported premise |
| Privileged defender event is relabeled or replayed toward blind reasoning | Source/mode admission rejects contamination |
| Rejected input contains a synthetic sensitive sentinel | Sentinel never appears in persistent events, retries, quarantine, logs or diagnostic outputs |

This is DESIGN-only. Review checks ownership, failure/recovery semantics, scope, references and consistency with accepted authority. The cases above are future contract/integration verification obligations, not executed runtime tests. ADR-003 remains PROPOSED pending review and product-owner acceptance; DW-FOUNDATION-001 remains unsealed.

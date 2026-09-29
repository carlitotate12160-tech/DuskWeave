# ADR-005: PostgreSQL System of Record

| Metadata | Value |
| :--- | :--- |
| **Document ID** | ADR-005 |
| **Title** | PostgreSQL System of Record |
| **Status** | ACCEPTED |
| **Stage** | Stage 3 - Foundation ADRs |
| **Direct Dependencies** | [ADR-003](ADR-003-domain-events.md), [ADR-004](ADR-004-rust-core-language.md) - ACCEPTED |
| **Supporting Architecture** | [ADR-001](ADR-001-modular-monolith.md), [ADR-002](ADR-002-domain-boundaries.md) - ACCEPTED |
| **Product Authority** | PRD-000..006 - all ACCEPTED; see [PRD registry](../prd/README.md) |

## 1. Context and problem

Campaign continuity needs durable accepted state, non-sensitive historical evidence and recoverable required consequences. ADR-003 requires an owner transition and its publication obligation to survive together, then each consumer's effects and completion to survive together. Memory-only callbacks and best-effort publication after saving state cannot satisfy that requirement.

Five operational models remain separately owned. A shared database must not become a universal campaign aggregate, a shortcut around intent-specific commands, or a source of execution authority. Durable storage preserves what an owner accepted with its provenance and epistemic limits; it does not turn an observation into fact or historical access into current usable access.

## 2. Decision drivers

1. Atomic owner-local changes and required publication records, with recoverable consumer processing.
2. Explicit concurrent-update protection, stable identity, constraints and visible unresolved outcomes.
3. Preserve owner boundaries and append-only authoritative history without requiring a separate database for each model.
4. Exclude raw sensitive material before it reaches any storage-related surface.
5. Support campaign/mode isolation, paused-campaign upgrades and recovery without action replay.
6. Start with one primary storage technology for permitted core records; avoid speculative distributed transactions, graph engines and messaging infrastructure.

## 3. Options considered

| Option | Benefit | Cost / disposition |
| :--- | :--- | :--- |
| **PostgreSQL with owner-local transactions and durable outbox/inbox records** | Relational constraints, transactional updates and concurrent access fit ADR-003 obligations. | Operated database, migrations, backup/restore and concurrency design remain necessary. Selected. |
| SQLite with equivalent transaction/delivery discipline | Embedded deployment and fewer operational components; credible for a constrained local application. | One writer at a time constrains write concurrency; deployment and contention assumptions would need explicit qualification. Not rejected as unsafe, but not the foundation baseline. |
| Separate primary database per context | Independent operational scaling and storage choices. | Adds recovery and cross-store coordination costs before demonstrated need. Deferred; model separation alone does not require it. |
| Event log as the sole source of every model | Uniform history and reconstruction approach. | Forces event sourcing on every domain and expands projection/versioning obligations; ADR-003 does not require it. Not selected. |
| Graph/document store as the sole core record system | May fit particular traversals or flexible documents. | Those needs do not establish a better fit for all transactional history and authority-related records. No workload evidence justifies a second primary system now; ADR-006/007 evaluate model representation. |

No throughput, latency, availability or team-operability benchmark has been run. PostgreSQL is selected for the required transaction semantics and manageable initial composition, not because the reserved ADR title pre-decides the answer. SQLite remains a credible alternative if deployment requirements later change.

## 4. Decision: scope and ownership

Use PostgreSQL as the primary durable system of record for **permitted campaign-core records**, with one logical transactional database as the initial baseline. This is not a claim that the whole product must have one database or that one physical server is sufficient for every deployment.

| Record category | Owner and meaning |
| :--- | :--- |
| Mission/lifecycle and accepted operational state | Their ADR-002 owners retain semantic authority; persistence adapters implement their bounded ports. |
| Terrain, Access and Objective claims | Preserve references, revisions, provenance, freshness and epistemic status. Physical representations remain ADR-006/007 and domain-contract work. |
| AttackPathView and other derived projections | Rebuildable only where their contracts establish sufficient retained inputs; not an independent source of terrain/access truth. |
| CampaignTrajectory and authoritative accepted records | Append-only historical content with linked corrections; historical acceptance is not current dispatch permission. |
| Required event publication, consumer effects and completion evidence | Owned transition/consumer obligations under ADR-003; infrastructure tracks delivery without deciding domain truth. |
| Approved opaque proof and non-sensitive evidence references | Store only admitted permitted material; a reference grants no permission to fetch raw client content. |

The database is not the isolated ephemeral proof boundary, campaign-scoped secret custody, an inference memory dump, a credential vault for client secrets, or the privileged defender telemetry store for blind campaigns. This ADR does not select Observer/Grader storage. Independently authorized evaluation access remains bounded by ADR-003 and INV-007.

Rust persistence implementations remain outside domain logic. No universal repository receives every aggregate or policy. Only an owning context writes its state through its intent-specific application path. Cross-owner inputs use published bounded views/contracts; table visibility does not authorize foreign writes or direct reads of private representations. SQL joins within an owner's representation are not prohibited; cross-owner read models require explicit contracts and retain source revisions and eligibility.

Schema layout, role count and package layout are deferred. They must enforce these boundaries without exposing an unrestricted database handle to reasoners, execution adapters or campaign capabilities. A shared connection pool, if used, is infrastructure and conveys no authority to bypass them.

## 5. Atomicity and recoverable delivery

Select the **transactional outbox/inbox pattern**, without prescribing table names, a queue product or a background-worker topology.

1. The producer validates its intent and records its accepted local state change, owner revision and immutable event content/publication obligation in the same PostgreSQL transaction. Rolled-back changes cannot publish as accepted transitions. Required consumer obligations must remain reconstructible across routing upgrades; a disappearing subscriber cannot erase required work.
2. Delivery reads committed obligations and submits them to the declared bounded consumers. Optional wake-up signals may reduce latency, but recovery scans durable obligations; an in-memory callback or PostgreSQL NOTIFY is not the delivery record.
3. A consumer validates source authority, scope, mode, version and owner ordering. Its local accepted effects, completion/deduplication record, and any resulting publication obligation commit together in its own transaction. No global transaction spans acceptance by all model owners.
4. Stable identity is scoped by accountable producer and campaign/engagement; consumer completion also identifies the relevant consumer obligation. A duplicate is safe only when semantic content agrees. Same identity with different content follows ADR-003 integrity-conflict resolution, not blind insert-ignore or overwrite.
5. Required consumer completion must follow durable required effects. If a consequence spans multiple owners, each accepts its own change and preserves its outstanding obligation; a consumer cannot claim foreign effects completed merely because it submitted a request.
6. Retries remain bounded, with visible pending/failed obligations and explicit recovery. Delivery bookkeeping may change, but immutable semantic event content and authoritative historical evidence cannot be rewritten through bookkeeping updates.

A transition can be locally accepted before required downstream consumers finish. It must not be reported as global acceptance. Dependent use cases obtain valid current premises or wait; projection lag is never permission to act optimistically. No distributed transaction or universal event-sourcing requirement is introduced.

Outbox delivery concerns domain-event consequences, not exactly-once target execution. External dispatch uses the ADR-002 authority path and later execution contracts. A database transaction cannot atomically commit an action on a target system.

## 6. Concurrency, freshness and ambiguous completion

Each owner persistence contract must establish a concurrency policy for the invariant it protects: expected-revision comparison, relevant constraints, explicit locking or serializable transactions as appropriate. Default Read Committed alone is not proof that a multi-row invariant is preserved. Exact isolation levels are selected per bounded operation with concurrent counterexamples, not by a universal transaction wrapper.

Conflicts, deadlocks and serialization failures produce bounded retries of database-only work after re-evaluating current premises. Transactions must be short: do not hold database locks across inference, target execution, or a wait for another context's acceptance. Domain ordering follows owner revisions/causality, not sequence allocation, wall-clock order or notification arrival.

If the connection fails while committing, the caller cannot infer rollback from the missing acknowledgment. A stable operation identity and durable outcome/publication records must permit reconciliation before reapplying a possibly committed command. This database ambiguity is distinct from an external action whose result is unknown; both require their own evidence.

A consistent database snapshot does not prove external environment freshness or current action authorization. Read replicas and caches, if introduced, cannot satisfy a consequential check without the owning contract's freshness guarantee. Revocation and safety-stop handling must not wait for ordinary delivery backlog; a current-state read alone does not solve the check-to-dispatch race. Execution contracts must bind current authority to dispatch and stopping as required by ADR-003.

## 7. Durability, failure and restoration

Required state, history, publication and completion records use durable logged storage. Deployment must retain PostgreSQL durability protections, including fsync and commit acknowledgment after the required WAL flush; asynchronous commit or unlogged storage is not acceptable for these records. This is a local durability baseline, not a promise to survive loss of every disk or node.

Replication, failover policy, backups, recovery-point and recovery-time objectives require explicit operational qualification before runtime delivery. Promotion of a lagging replica cannot silently discard acknowledged obligations while claiming continuity. A deployment must establish how its selected failure model preserves accepted records, or expose a recovery gap and block dependent continuation until reconciled; no zero-data-loss availability claim is made here.

On database unavailability, core actions requiring unavailable current authority, audit preconditions or required durable recording cannot proceed. Do not invent an in-memory fallback that later replays actions. Safety stopping of in-flight work must remain possible under the execution design; database failure is not permission to leave it unbounded.

Backup/restore must cover accepted state, event obligations, consumer completion and required historical inputs consistently. A restored snapshot may predate target effects, authority withdrawal or model corrections. Restoring it never resumes dispatch automatically: verify current mission authority, reconcile uncertain attempts and missing obligations, reassess access/freshness, and account for the recovery gap before dependent continuation. Database recovery cannot undo external effects or prove that no later actions occurred.

Logical projection rebuild is distinct from disaster restore. Rebuild follows ADR-003 causal/correction rules and never invokes capabilities; arbitrary deletion of accepted records is not a projection-rebuild procedure.

## 8. Isolation, sensitive data and audit limits

Every relevant record and access path preserves engagement/campaign scope and source/exercise-mode eligibility. No implicit cross-campaign reuse follows from shared tables or overlapping targets. Scope checks apply to reads, writes, derived views, delivery and recovery; changing a label cannot make privileged evidence eligible for blind reasoning.

Use least-privilege runtime access separated from schema ownership and migration/administrative authority. Runtime roles must not be superusers or have bypass privileges that invalidate the intended controls. Row-level security may provide defense in depth, but is not selected as the sole isolation mechanism: PostgreSQL superusers and BYPASSRLS roles bypass it, and table owners normally do too. Shared-process module boundaries are not a security sandbox.

Raw client content and operational authentication values must be excluded before SQL parameters, driver diagnostics or database intake. PostgreSQL encryption, temporary or unlogged tables, later deletion, and a short retention window do not make campaign-core storage valid secret custody. The same exclusion covers WAL, replicas, backups, retry records, query logs, crash diagnostics and export surfaces. Only approved opaque proof, opaque campaign-scoped custody references, and permitted safe non-sensitive metadata enter persistence. Any separately authorized durable secret custody required for campaign resumability remains outside this database and outside this ADR.

Campaign capabilities and runtime persistence paths cannot update, delete, truncate or disable authoritative historical evidence. Corrections append linked records; mutable current projections and delivery bookkeeping remain distinguishable from immutable history. Migration tooling must not gain a routine permission to rewrite authoritative originals under the label of schema evolution.

PostgreSQL transactions and application append-only rules do not provide tamper-proof history against a privileged administrator or compromised storage operator. This ADR establishes the required runtime restriction, not a complete audit-protection mechanism. Later audit design must qualify privileged maintenance, protection and verification under its threat model before those runtime paths are admitted. It also cannot authorize tampering with authoritative audit on target systems.

## 9. Upgrade, retention and deferred choices

An upgrade must preserve every outstanding required obligation and retained record needed for continuation or required historical inspection, including paused campaigns. Compatible readers or validated migrations/translations preserve meaning, provenance, identity and authoritative originals. Unsupported records block the affected rollout; schema parse success is insufficient. Rollback of a release is not automatically safe after data changes.

No retention period or compaction policy is selected here. Required history, deduplication evidence and publication obligations cannot be removed while their contracts still need them. A future cleanup policy must establish safe redelivery/recovery horizons and historical requirements; row age or a broker acknowledgment alone is insufficient.

ADR-006 decides Terrain storage representation; ADR-007 decides Foothold/Path separation. This ADR selects no tables, SQL migrations, Rust database library, graph extension, message broker, partitioning, deployment replica count, database version, backup product or Observer storage. Additional primary stores require a demonstrated need and accepted decision preserving these guarantees.

## 10. Invariant compliance and future verification

| Invariant | Storage obligation |
| :--- | :--- |
| INV-001 | No all-domain repository or database-driven universal campaign manager. |
| INV-002 | Model owners retain mutation authority; shared storage does not merge truth or permit private cross-owner writes. |
| INV-003 | Committed proposals/events and replay cannot authorize execution or bypass Gateway/Broker. |
| INV-004 | Persistent claims retain provenance, tier, epistemic status and freshness; storage never promotes them. |
| INV-005 | Raw client content and operational secrets are excluded before PostgreSQL intake; only approved opaque proof, custody references and safe metadata may persist. |
| INV-006 | Runtime paths cannot rewrite authoritative evidence; append-only correction and later qualified audit protection remain mandatory. |
| INV-007 | Scope/source/mode restrictions survive reads, projections, delivery, backup and restore; blind context excludes privileged feeds. |

| Synthetic failure case | Required result when runtime is authorized |
| :--- | :--- |
| Producer fails before commit, or after commit before notification | First case yields no accepted transition; second preserves state and recoverable publication together. |
| Consumer commits effects but loses acknowledgment | Redelivery does not duplicate model/history effects; completion remains durable. |
| Two consumers or requests race on the same owner revision | Constraints/concurrency policy preserve the invariant and expose the conflict. |
| Event identity is reused with different content | Integrity conflict remains visible; no silent deduplication or replacement. |
| Commit acknowledgment is lost | Reconcile the stable operation identity before reapplying a possibly committed command. |
| Database-only transaction retry surrounds a target action | Reject the design; external execution cannot be repeated by a database retry loop. |
| Ordinary delivery is delayed while authority is withdrawn | No dependent dispatch from stale premises; stopping does not wait for the backlog. |
| Storage becomes unavailable or durability settings are weakened | Block dependent acceptance/dispatch or deployment qualification; no false durable success. |
| Restore predates a withdrawal or a dispatched target effect | Recovery does not resume actions; current authority and unknown effects require reconciliation. |
| A campaign reads another campaign's records or blind reasoning receives privileged assessment | Reject in every applicable read/projection/recovery path, not only at event ingress. |
| Synthetic sensitive input is rejected | No sentinel reaches SQL parameters, diagnostics, WAL, backups or retry material. |
| Runtime role attempts history UPDATE, DELETE, TRUNCATE or protection disablement | Denied; permitted correction appends a linked record. Privileged maintenance needs separate audit qualification. |
| Paused-campaign records or pending obligations are incompatible with an upgrade | Block the affected rollout until a verified compatible path exists. |
| Projection rebuild receives corrections out of arrival order | Same accepted inputs, evaluation time and causal rules yield the same current projection without execution. |

These are future contract, concurrency, fault-injection and operational verification obligations, not executed tests. DESIGN checks cover authority, ownership, failure semantics, links and scope. No database, migration, benchmark, runtime test or CI gate is created here.

## 11. Technical references and acceptance

Primary documentation checked on 2026-09-28: [PostgreSQL isolation](https://www.postgresql.org/docs/current/transaction-iso.html), [WAL durability settings](https://www.postgresql.org/docs/current/runtime-config-wal.html), [row security](https://www.postgresql.org/docs/current/ddl-rowsecurity.html), [NOTIFY](https://www.postgresql.org/docs/current/sql-notify.html), and [SQLite deployment tradeoffs](https://www.sqlite.org/whentouse.html). These support database behavior; the storage selection and obligations are DuskWeave design judgments.

Product owner accepted ADR-005 on 2026-09-28: PostgreSQL is the sole initial campaign-core system of record; no SQLite deployment mode is selected. Graph working-view representation belongs to ADR-006, and Observer storage remains deferred to its own authorized design stage. The 2 OCPU / 12 GB deployment estimate is provisional capacity planning, not a tested guarantee or a new ADR requirement. At the time of ADR-005 acceptance, ADR-006/007 were unauthored. Both were subsequently authored and accepted. Cross-foundation reconciliation completed under DW-FOUNDATION-COHERENCE-001, and DW-FOUNDATION-001 was explicitly sealed on 2026-09-28. Runtime remains governed by the later active stage and its accepted dependencies. On 2026-09-29 the product owner authorized the INV-005 amendment clarifying that campaign-core PostgreSQL stores only opaque custody references and safe metadata, never operational secret values; the historical foundation seal is reopened pending explicit reseal.

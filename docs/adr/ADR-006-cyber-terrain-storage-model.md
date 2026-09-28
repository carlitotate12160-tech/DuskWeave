# ADR-006: Cyber Terrain Storage Model

| Metadata | Value |
| :--- | :--- |
| **Document ID** | ADR-006 |
| **Title** | Cyber Terrain Storage Model |
| **Status** | PROPOSED |
| **Stage** | Stage 3 — Foundation ADRs |
| **Direct Dependency** | [ADR-005](ADR-005-postgres-system-of-record.md) — ACCEPTED |
| **Supporting Architecture** | [ADR-001](ADR-001-modular-monolith.md), [ADR-002](ADR-002-domain-boundaries.md), [ADR-003](ADR-003-domain-events.md), [ADR-004](ADR-004-rust-core-language.md) — ACCEPTED |
| **Product Authority** | [PRD-002](../prd/PRD-002-cyber-terrain.md) and PRD-000..006 — ACCEPTED |

## 1. Context and problem

CyberTerrain owns sourced environmental claims, relationships and their epistemic limits across the seven PRD-002 dimensions. Terrain is not the validated access graph, a candidate attack path, an objective result, or the campaign history. Claims can be PROVISIONAL, OBSERVED, CORROBORATED, INFERRED, HYPOTHETICAL, STALE or REFUTED; tier, provenance and freshness affect whether a proposed decision can depend on them.

PRD-002 requires immutable point-in-time reasoning snapshots and structured terrain deltas. ADR-003 requires recoverable owner-published consequences, causal correction and replay without execution. ADR-005 selects PostgreSQL for permitted campaign-core records. Neither a durable row nor a fast graph traversal proves the external environment remains current.

The first deployment under consideration has 2 OCPU and 12 GB RAM. This is a provisional sizing assumption, not a benchmark or a required production shape. A graph engine, full resident graph, or second primary database would add operational cost before actual traversal and rebuild workloads are measured.

## 2. Decision drivers

1. Preserve Terrain's sole ownership of environmental claims and their revisions, without turning it into the other four operational models.
2. Keep provenance, tier, origin validation state, time and correction causality recoverable across crashes and pauses.
3. Provide bounded reasoning views and deltas without claiming that an old snapshot is current authority.
4. Admit low-stakes observations promptly while keeping consequential claims behind their proper tier and current evidence.
5. Limit memory and rebuild cost on small deployments without sacrificing correctness or campaign isolation.
6. Avoid a second primary graph store and an implied universal event-sourced model.

## 3. Options considered

| Option | Benefit | Cost / disposition |
| :--- | :--- | :--- |
| **PostgreSQL-owned terrain state with bounded derived graph views when useful** | One durable source, owner-local atomicity and optional fast traversal of selected relationships | Must manage view identity, invalidation, memory and rebuild. Selected. |
| PostgreSQL-only owner queries without a resident graph | Simplest operational footprint; may be sufficient for bounded exploration | Expensive recurring traversals are possible; keep as the initial valid read path and a measured alternative. |
| Complete in-memory graph as authoritative terrain | Fast local traversal for some workloads | Crash/rebuild gaps, duplicate truth, unbounded RAM and correction complexity. Rejected. |
| Separate primary graph database | Native graph queries and independent scaling | Dual writes, consistency, isolation and recovery obligations lack a demonstrated need. Deferred behind a new accepted decision. |
| Replay of all campaign events as the only terrain source | One reconstructive narrative | ADR-003 explicitly does not require universal event sourcing; conflates Terrain acceptance with trajectory/action history. Rejected. |

No performance superiority claim is made for petgraph, PostgreSQL traversal, or any representation. Library and algorithm choice require workload measurements; petgraph is a candidate, not an accepted dependency.

## 4. Decision: durable owner state and eligible claims

Terrain's accepted current claims and relationships live behind its own PostgreSQL persistence port. The owner records stable environmental identity, typed relationship meaning, engagement/campaign scope, source/mode eligibility, originating position reference and status, epistemic status, tier, evidence/provenance references, first/last observed and corroborated time as applicable, freshness bounds and owner revision as applicable. These are semantic obligations, not a universal table or serialized aggregate schema.

Terrain distinguishes the state currently applicable to a claim from immutable accepted history and linked corrections. It must retain enough permitted accepted records and causal/revision references to reconstruct required snapshots, explain an earlier decision, and apply later downgrade/refutation without editing authoritative originals. This does not make every Terrain entity event-sourced or let a snapshot replace CampaignTrajectory. Mutable owner-maintained current representation may be updated under its contract; authoritative historical records are append-only.

Only admitted non-sensitive material reaches Terrain or its storage adapter. Unfiltered tool output, sensitive client content, credentials, privileged Observer/Grader feeds for blind mode, and unrestricted environment dumps cannot be persisted as graph labels, payloads, indexes, provenance links or diagnostic copies. Admitted non-sensitive excerpts remain subject to provenance and purpose bounds. A reference cannot become a back door for retrieving raw client content.

Tier 1 acceptance remains a narrow sourced OBSERVED claim after validation and reconciliation. Tier 2/3 observations may remain OBSERVED or otherwise qualified until corroboration required for their consequential use exists. PROVISIONAL from an unvalidated origin is persisted with its origin and limitation for orientation; origin validation triggers tier-aware reconsideration, never automatic corroboration. An origin failure or abandonment causes a sourced downgrade to HYPOTHETICAL and invalidates dependent views. Tier changes require stakes-based reassessment; exploitation tempo does not lower the tier.

## 5. Snapshots, deltas and derived graph

A Terrain snapshot is an immutable *view* of claims accepted by the Terrain owner for a stated campaign, revision/causal frontier, source eligibility, and evaluation time. It retains the epistemic and temporal limits of included claims; the view is not a copy of raw observations or an authorization token. The owner may generate snapshots on demand or materialize them if necessary. Retaining every full snapshot is not required.

Historical inspection distinguishes *as known then* (only owner-accepted claims and corrections available at the past frontier, evaluated at the recorded decision time) from *retrospective interpretation* (later accepted corrections applied to the past claim under a stated later frontier and evaluation time). A later correction never rewrites what the operator knew or decided earlier. Both are Terrain views; CampaignTrajectory owns the historical action/decision account. Access to either historical view follows its own campaign/mode and purpose authorization.

A Terrain delta describes an owner-accepted, revision-linked change to eligible claims or relationships, including correction, downgrade, staleness determination or refutation as applicable. Commit of the owner change and its required publication obligation follows ADR-005; delivery order is not causal authority. An uncommitted change is not a valid delta. Downstream owners accept their own consequences under ADR-003; a Terrain delta cannot write FootholdGraph, AttackPathView or ObjectiveState directly.

Freshness can expire solely because evaluation time advances, with no new commit or delta. Every use of a claim or derived view recomputes its time eligibility against the applicable freshness bounds and evaluation time; dependent projections cannot rely only on notifications and must re-evaluate or invalidate when their premises expire. If Terrain accepts an explicit status transition, it publishes the corresponding durable delta. No periodic write is required solely to make time pass. The exact STALE status vocabulary for an expired OBSERVED claim awaits the PRD-002 decision in §10; the no-use-after-expiry rule applies regardless.

A Rust in-memory graph may materialize a *bounded selected Terrain view* for repeated traversal or hypothesis comparison. Its nodes/edges refer to stable domain identities and eligible claims; internal library indices are never persisted as entity identity or used as execution authority. The graph is a disposable cache, not a sixth operational model, authoritative access map, or sole representation of terrain. Related Access, Objective and Mission inputs for AttackPathView are obtained through their own published contracts; they are not absorbed into the Terrain graph.

Cache construction, incremental update and invalidation retain campaign/mode scope, source revisions, tier/status, correction causality and evaluation time. Lookup, invalidation and returned results enforce the same campaign and mode eligibility; shared process resources do not authorize cross-campaign reuse. Applying an out-of-order update must reconcile missing predecessors or rebuild from compatible accepted owner state; it must not overwrite a newer correction. A rebuild takes a coherent owner snapshot and reconciles revisions received during construction before publishing the view. Old graph handles are not silently reused after invalidation or restart.

A view that is only time-expired but otherwise eligible for its campaign/mode may inform bounded hypothesis work with explicit age and limits, or authorized historical inspection; it cannot supply a current consequential premise. A view with invalid campaign scope, source/mode eligibility, or unresolved correction/causal gaps must not reach active campaign reasoning, even labeled historical. Historical inspection of such material requires a separately authorized path and compatible reconstruction; a label never grants access. An absent or over-budget view is unavailable, not partial proof. The use case obtains eligible current owner evidence through a bounded read or defers the dependent decision. Deterministic authority independently rechecks mission, origin and evidence at dispatch; a graph traversal, snapshot or cache hit never confers permission.

## 6. Bounded operation, restart and upgrades

Do not require full campaign terrain to reside in memory. Select subgraphs or query windows based on a real reasoning use case; bound memory, work and concurrent builds. If a view cannot fit or its build falls behind, expose its frontier, eligibility status and reason for incompleteness to the use case and authorized operator, then take the owner-backed read path or defer dependent work, without silently dropping claims or downgrading evidence requirements. Diagnostics carry no raw sensitive or privileged blind-mode data. Concrete budgets, representation and metrics require workload measurements against the intended deployment; 2 OCPU / 12 GB is an evaluation scenario, not an ADR allocation.

On process crash, the in-memory graph disappears without losing accepted Terrain state. Recovery checks persisted owner state, pending required obligations, compatible versions and current evaluation time before recreating a view. No graph rebuild repeats target actions, declares an old foothold healthy, or revives a stale claim. Database unavailability follows ADR-005: no memory-only acceptance of new consequential state with a later blind flush.

Concurrent updates to the same applicable claim or relationship preserve sourced observations by time and vantage, enforce an owner-defined revision/constraint policy, and surface unresolved conflicts; timestamp order or last-write-wins is insufficient. Distinct sources do not automatically provide independent corroboration. A commit with an ambiguous acknowledgment is reconciled by stable operation identity before retrying database-only work. Corrections append accountable records and rebuild/reconcile affected views by owner revision and causality. Given the same compatible accepted inputs, evaluation time and rules, rebuild yields the same applicable current interpretation despite different notification arrival orders.

Upgrades affecting stored meanings or cached representations must preserve processing of required retained records, including paused campaigns, under ADR-003/005. Invalid cache formats may be discarded and rebuilt from compatible authoritative state; stored originals are not rewritten as a shortcut. Retention and compaction cannot erase provenance, deduplication or correction history still needed for owner recovery and required inspection.

## 7. Boundaries and deferred decisions

ADR-007 owns the detailed FootholdGraph/AttackPathView separation; this ADR grants Terrain no authority over validated execution positions or paths. Observer telemetry storage and optional DuckDB analytics belong to later Observer design under INV-007. Privileged Observer analytics cannot be relabeled or admitted into blind Terrain through an observation/cache path. Only telemetry genuinely obtained from an authorized current campaign position may enter as campaign-observed evidence under PRD-000; separately authorized defender-informed feedback keeps its mode/source and evaluation separate. Tool execution sandboxing and isolation belong to later execution design; this storage decision neither places untrusted tools inside the campaign core nor prescribes containers or virtual machines. The isolated ephemeral proof boundary remains outside Terrain and persistence.

This ADR selects no SQL tables, graph crate, index, traversal algorithm, full-snapshot retention, thread count, cache limit, load benchmark, migration layout or graph database. Introducing a second primary store, moving owner authority into cache, or making cache freshness equivalent to current authorization requires a separate accepted architecture decision and evidence.

## 8. Invariant compliance

| Invariant | Terrain storage obligation |
| :--- | :--- |
| INV-001 | Terrain port and optional view stay owner-scoped; no universal graph/repository manager. |
| INV-002 | Terrain environmental claims stay distinct from Access, Path, Objectives and Trajectory. |
| INV-003 | Views, replay and graph traversal cannot dispatch or authorize capabilities. |
| INV-004 | Stored/viewed claims retain epistemic status, tier, provenance, origin and time; storage cannot promote evidence. |
| INV-005 | Raw sensitive material is rejected before persistence, cache and diagnostic surfaces. |
| INV-006 | Current representation changes and linked append-only corrections cannot rewrite authoritative history. |
| INV-007 | Blind views exclude privileged feeds; source/mode labels survive cache, recovery and publication. |

## 9. Review cases and consequences

| Synthetic case | Required observable behavior when runtime is authorized |
| :--- | :--- |
| Transient D reports a service; D then fails validation | Store sourced PROVISIONAL; append downgrade, invalidate dependent views, retain historical provenance. |
| B is validated; its direct service observation is narrow Tier 1 | Accept sourced OBSERVED, not automatic CORROBORATED or global reachability. |
| A Tier 1 host becomes candidate Key Terrain | Reclassify consequential claim as Tier 3; old snapshot cannot justify critical action. |
| A correction arrives before an older relationship event | Reconcile owner causality; older delivery cannot restore the corrected current relationship. |
| A cache rebuild races with a committed downgrade | New view includes the downgrade or remains visibly incomplete; no stale dependent dispatch. |
| A claim's freshness window expires without an owner commit | At the next use, the claim and dependent view are time-ineligible for consequential use; no mandatory per-second write. |
| A blind reasoner requests an informed-mode or causally incomplete cached view | Deny active reasoning access regardless of a historical label; authorized historical inspection is separate. |
| A T2 correction changes a claim accepted at T0 after a decision at T1 | As-known-at-T1 inspection preserves the past knowledge; retrospective-T2 inspection shows the correction and labels its later frontier. |
| A graph build exceeds memory or Terrain DB becomes unavailable | No unbounded load, silent partial certainty, or memory-only acceptance; defer dependent decisions where needed. |
| Privileged defender alert is inserted into blind cache, or raw sensitive input into a graph label | Reject before admission and prevent persistence/diagnostic leakage. |
| Snapshot from yesterday shows reachable host while current Access is lost | Historical analysis remains possible; current path/action requires current owners and authority. |
| Crash occurs after Terrain commit before delta delivery | Accepted state and recoverable publication obligation survive; rebuild does not execute a target action. |

The selected design minimizes initial infrastructure while allowing measured in-memory acceleration. Its costs are owner-local revision discipline, cache invalidation, freshness evaluation and bounded rebuild. These are future contract/integration verification cases, not executed tests; the repository is document-only.

Technical references checked 2026-09-28: [petgraph graph-type trade-offs](https://docs.rs/petgraph/latest/petgraph/) and [PostgreSQL recursive queries](https://www.postgresql.org/docs/current/queries-with.html). These establish implementation options, not DuskWeave performance findings.

## 10. Blocking product vocabulary decision

Accepted PRD-002 §3.1 describes STALE as previously CORROBORATED, while §3.3 says an expired Tier 1 OBSERVED claim becomes STALE. This is an upstream product contradiction; ADR-006 cannot define it away. Recommended PRD-002 clarification: STALE applies to previously OBSERVED or CORROBORATED claims whose freshness window has elapsed without sufficient refresh; the previous evidential status and source stay in history. An expiry never itself refutes the claim, creates corroboration, or upgrades a PROVISIONAL origin. Product-owner approval of the exact PRD change is required before ADR-006 acceptance. The time-eligibility rule in §5 protects consequential use in either case.

ADR-006 is **PROPOSED** pending that decision and product-owner review. ADR-007 remains unauthored and depends on ADR-006 acceptance. DW-FOUNDATION-001 is unsealed; no runtime or execution isolation topology is selected here.

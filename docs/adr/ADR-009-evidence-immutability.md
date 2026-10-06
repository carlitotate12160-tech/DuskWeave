# ADR-009: Evidence Continuity for M1

| Metadata | Value |
| --- | --- |
| Status | ACCEPTED — product owner, 2026-10-06; M1 selected lane only |
| Date | 2026-10-06 |
| Revision | R1 — scoped architecture accepted by the product owner on 2026-10-06 |
| Authoring base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product authority | Accepted PRD-000..010 and M1 product contract; no new product breadth |
| Direct dependencies | accepted PRD-008 and ADR-003/005/008 |
| Scope | [M1 enabling lane](../contracts/M1-enabling-architecture.md); no full-stage acceptance |

## 1. Context and problem statement

M1 needs inspectable safe technical basis and claim-relative support/correction.
A digest-only ledger cannot explain why a DNS binding or response supported a decision.
Mutable current state cannot erase the basis used earlier. Evidence is not another model.

## 2. Decision drivers

Preserve accountable identity, lineage, claim-specific burden, current eligibility,
append-only history and recoverable required consequences using PostgreSQL.

## 3. Considered options

| Option | Benefit | Conflict / disposition |
| --- | --- | --- |
| Hash raw captures into a shared ledger | Apparent compactness | Unsafe input and no inspectable basis; reject |
| Universal event sourcing / evidence owner | Uniform reconstruction | Absorbs model acceptance; reject |
| Owner-local safe records plus linked corrections | Fits ADR-003/005/008 | Select; bounded duplicate and causal checks required |

## 4. Decision outcome

### Identity and technical basis

Use existing UUID operation/event conventions for newly allocated logical identities.
Identity namespace is engagement + campaign + accountable producer + record family + UUID.
Observation, material, evaluation, claim, event and attempt identities remain distinct.
Reserve attempt identity durably before an external effect. Never derive identity from raw data.

Terrain retains the admitted typed CT/DNS/HTTPS projection described in the owner contract,
plus approved request/profile, vantage, time basis, original attempt and source-family references.
This inspectable projection is the technical basis, not unrestricted request/response capture.
An integrity digest may cover only the admitted versioned safe representation.
It supports semantic duplicate comparison; it proves neither external truth nor source independence.

| Narrow claim | Minimum inspectable admitted basis |
| --- | --- |
| CT hint | Purpose-admitted hostname, source/record reference when safe, certificate times when known, retrieval/attempt context and bounded completeness |
| DNS routing | Approved query, declared resolver/vantage, transport, RCODE/truncation, admitted RRset/alias semantics and lowest relevant returned TTL with perception time |
| TLS/HEAD | Admitted endpoint/request reference and pinned address, TLS validation category, observed status/redirect category when available, effect/perception time and limitations |
| Failed/partial attempt | Last qualified phase, closed failure/disposition code, time basis, bounded byte/record/operation counters and missing dependencies; no invented response |

These fields substantiate only their stated claim; an HTTP status cannot establish an installed
version or exploitable condition. ADR-011 owns their safe egress. Trajectory may inspect the
same safe attempt references without duplicating material ownership or becoming a telemetry store.

Each evaluation binds one declared Terrain claim, purpose and burden to material references,
support/refutation/conflict/inconclusive disposition and unmet facets. Pathing separately
assesses relevance to its derived candidate. Trajectory records decisions and effects,
not a universal truth acceptance. No shared evidence-repository mutation handle is exposed.

### Current state, history and correction

Accepted historical observation/material/evaluation semantics are append-only.
Owner-current claim representation may change under expected-revision comparison.
A correction has a fresh identity, references the original, names the affected claim/use
and records an accountable safe basis. Correction history cannot silently restore stale authority.

Identical semantic redelivery has one logical effect. Conflicting content under one identity
raises an integrity conflict and blocks affected dependent use. Preserve only admitted safe
variants; unsafe variants leave category/reference only. Missing predecessors or conflicting
corrections stay unresolved. Arrival time and producer assertion cannot choose the winner.
Resolution is a new owner-accepted linked record verified against retained provenance/state.

Time records distinguish effect time when known, perception, receipt, evaluation and acceptance.
Use UTC with source origin/precision/uncertainty; owner revisions establish causal order.
Unknown source time remains unknown. Re-fetching CT cannot refresh certificate-era reality.
At-use eligibility follows the owner contract, correction/source/mode and authority limits.

### Publication and storage rights

A short owner-local transaction records accepted state/history and typed publication obligations
atomically, using ADR-005 durable logged storage. Required consumers use scoped inbox identities;
their effects and completion commit together. Notifications remain a wakeup optimization.
Pending Trajectory history and Pathing invalidation are visible; dependent execution waits
or checks current owner premises through its public port. No SQL lock spans network I/O.

Runtime roles may INSERT/read eligible history but cannot UPDATE/DELETE/TRUNCATE history,
disable protections, own schemas or gain administrative privileges. Current-state and delivery
bookkeeping updates are narrowly separate. Restore/replay cannot dispatch or erase obligations.
No tamper-proofness against a database administrator is claimed.

Late contamination revokes affected current-use eligibility before further reads/exports;
ADR-011's independent remediation authority governs prohibited-byte treatment. Safe linked
incident and correction history preserves the fact and dependencies without retaining the bytes.

### Counterexamples to qualify later

| Boundary | Required assertion |
| --- | --- |
| Lost commit acknowledgement | Fresh reconciliation finds the same identity and one durable logical effect |
| Lost consumer acknowledgement | Same material/event redelivery cannot duplicate owner/history effects |
| Older input after correction | It completes history but cannot restore superseded current state |
| Missing required technical basis | Evaluation remains insufficient; digest does not substitute |
| Copied CT record / second tool label | Same lineage contributes no independent corroboration |
| Runtime history UPDATE/DELETE/TRUNCATE | Restricted role is denied; safe correction appends |

## 5. Consequences

One system of record and owner-specific evaluation avoid a global evidence engine.
Safe technical projections have deliberate limits; missing raw artifacts cannot be reconstructed
for debugging. Full proof, external attestation, administrator-resistant audit and retention/
compaction policy remain deferred. Existing M0 historical evidence and seal are unchanged.

## 6. Invariant compliance matrix

| Invariant | Obligation |
| --- | --- |
| INV-001 | Evidence functions preserve continuity without owning sibling claims. No universal manager or workflow engine. |
| INV-002 | Terrain, Pathing and Trajectory retain distinct owners; no Access/Objective mutation. |
| INV-003 | Proposals, evidence, history and worker results never confer dispatch authority. |
| INV-004 | Narrow source-qualified claims, explicit uncertainty and owner reconciliation. |
| INV-005 | Only purpose-admitted safe semantics cross ordinary boundaries; no custody/proof implementation. |
| INV-006 | Runtime histories append; sensitive remediation has separate authority and safe accountability. |
| INV-007 | Campaign/mode/source isolation survives reads, derivation, correction and recovery. |

## 7. Verification and acceptance boundary

DESIGN checks links, ownership, dependencies and counterexamples under QUALITY_BAR section 7.
The cases above are future assertions, not executed runtime tests. The product owner
accepted R1 for the selected M1 lane on 2026-10-06. Runtime requires an issued bounded
packet; acceptance grants no target permission, qualified deployment, merge or seal.

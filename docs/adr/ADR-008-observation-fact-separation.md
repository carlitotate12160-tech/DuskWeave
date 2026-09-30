# ADR-008 — Observation & Fact Separation

| Metadata | Value |
| :--- | :--- |
| Status | PROPOSED |
| Date | 2026-09-30 |
| Authoring base | `d7297dad0f5269b3d95e2bd9dc1d28919c2ecd97` |
| Stage | 4 — Reality & Evidence |
| Direct dependencies | PRD-007; ADR-003 (ACCEPTED) |
| Governing authority | PRD-000..010; ADR-001..007 (ACCEPTED) |
| Acceptance boundary | Owner approved the outline/refinements; this authored ADR awaits explicit review and acceptance |

## 1. Context & problem statement

DuskWeave needs rapid orientation from authorized external, campaign-visible,
operator-supplied, and transient-origin inputs without treating captured output
as environmental truth, access, objective fulfillment, or action authority.
An hours-bounded campaign cannot wait for a universal reconciliation barrier.
It still needs accountable source limits, sensitive-data exclusion, and current
premises for consequential use.

[PRD-007](../prd/PRD-007-observation-model.md) defines observation eligibility.
[PRD-008](../prd/PRD-008-evidence.md) defines claim-specific evidence;
[PRD-009](../prd/PRD-009-client-proof.md) governs client proof; and
[PRD-010](../prd/PRD-010-sensitive-data-handling.md) governs safe egress and custody.
The architecture must enforce these distinctions through bounded interactions,
without creating a sixth operational model or a generic promotion lifecycle.

The [review memo](../workflows/DW-DESIGN-ADR008-REVIEW.md) separates source facts
from architectural inference. It is targeted research, not a comprehensive
operator study. The [MVP direction](../MVP_AND_DEFERRED_SCOPE.md) records delivery
scope separately; it supplies no runtime permission.

## 2. Decision drivers

- Fast, incremental owner-qualified orientation without a campaign-wide gate.
- Controlled construction/mutation and deterministic acceptance responsibility.
- Claim-relative source reliability, evidence burden, provenance, and time.
- Explicit partial, contrary, stale, conflicting, and unknown outcomes.
- Safe core semantics, bounded history, and no authority from reasoning output.
- Reuse of accepted owner/publication/storage boundaries without new infrastructure.

## 3. Considered options

| Option | Benefit | Cost or conflict | Disposition |
| :--- | :--- | :--- | :--- |
| Promote captured output directly into owner facts | Minimal apparent processing | Loses source limits, sensitive handling and acceptance burden; violates INV-004/005 | Reject |
| Universal Observation Manager with shared tiers and global approval | Centralized workflow | Competes with owners, serializes independent work, adds an unnecessary model/gate | Reject |
| Bounded input admission followed by owner-local claim reconciliation | Local accountability and rapid qualified use | Requires explicit owner/consumer contracts and failure dispositions | Select |

## 4. Decision outcome

### 4.1 Controlled input and owner boundaries

Separate raw capture, admitted Observation, inference, owner-accepted claim,
evidence, client proof, and execution authority through controlled representations
and intent-specific interfaces. Acceptance means a particular owner accepted a
bounded claim at an epistemic status; it is not a universal truth entity.

Acquisition supplies raw capture to the appropriate pre-admission boundary.
That boundary evaluates applicable current scope/authority/purpose/stops,
campaign isolation, source/mode/vantage eligibility, sensitivity, accountable
provenance, assertion limits, relevant times/uncertainty, and structural bounds.
It emits eligible non-sensitive semantics for declared consideration, or a safe
rejection/limitation outcome. Raw capture cannot go directly to reasoning,
ordinary persistence, diagnostics, or model mutation.

This is bounded eligibility evaluation with explicit context, not LLM approval.
IP membership, parsing, masking, hashing, or a serialized accepted label alone
does not satisfy admission. Sensitive candidates/intermediates remain governed
by PRD-010 until purpose-bound egress permits them; there is no general raw
quarantine. This ADR selects responsibilities, not a sanitizer or public schema.

The receiving Terrain, Access, Pathing, Objectives, or Trajectory owner evaluates
the claim against its current state/history, evidence, contrary material,
provenance, origin, freshness, intended use, and required burden. Implement
accepted owner-local rules deterministically; an LLM may propose interpretation
but cannot accept or mutate owner state. Construction/visibility restrictions
protect structural distinctions, not external truth or perpetual freshness.
Typestate is optional for a useful local transition, not a generic requirement.

### 4.2 Rapid qualification and claim-specific burden

One observation may reach multiple declared owners; each decides independently
without cross-owner mutable state or an all-owner acceptance barrier.
Admission alone cannot affect ranking. After receiving-owner qualification,
eligible material may immediately support its declared non-authoritative
hypothesis/ranking purpose. Consequential use requires the governing burden and
current premises separately.

Eligible external/pre-access observations do not require a prior foothold.
Transient-origin read-only orientation remains PROVISIONAL under PRD-003/007.
Successful origin validation triggers reconsideration, not automatic
corroboration; failure/abandonment triggers the required downgrade/invalidation.
Authentication success does not establish validated bounded execution,
target/identity/privilege context, output retrieval, or stability.

Do not map tool names to epistemic status. A sufficiently direct single item may
support a narrow OBSERVED claim when its owner contract permits; higher-stakes
burdens remain intact. Evaluate reliability and independence relative to the
claim and provenance/derivation relationships. Copies, mirrors, summaries and
different tool labels do not create independent source families. There is no
global confidence score or independence engine.

### 4.3 Time, history, corrections and propagation

Preserve applicable semantic context: campaign/purpose, source/mode,
vantage/origin, bounded assertion, effect/observation time, receipt time,
uncertainty, method limits, safe evidence/provenance and attempt references.
These are obligations by input family, not mandatory fields in one envelope.

Evaluate freshness at use from effect/observation time and applicable limits.
Receipt, admission, owner acceptance, or refreshing a feed cannot renew old
target reality. Correction, origin failure/loss, source restrictions, changed
authority, or contamination can remove eligibility before time expiry.
Apply PRD-002/ADR-006 status semantics: expired OBSERVED/CORROBORATED becomes
STALE for current use; expired PROVISIONAL retains origin uncertainty and is
ineligible for current use. Otherwise eligible historical material may support
bounded hypotheses with age/limits, not current consequential premises.
No shared TTL, timeout, scheduler, or periodic status-write requirement is chosen.

Retain material hypotheses, alternatives, no-action/proposal rationale, outcomes
and revisions required by the responsible owner/history contracts. Full LLM
scratchpad retention and a dedicated hypothesis database are not required.
Current state may change; retained historical semantics receive linked
corrections rather than silent rewriting.

Supply/reconsideration uses declared bounded interfaces under
[ADR-002](ADR-002-domain-boundaries.md). Only the accountable owner publishes
its accepted transition. Required state/history/publication and durable consumer
effects follow [ADR-003](ADR-003-domain-events.md) and
[ADR-005](ADR-005-postgres-system-of-record.md), including their owner-local
PostgreSQL and durable outbox/inbox obligations. No global transaction,
universal event sourcing, new broker, or alternate store is introduced.

A semantic duplicate has no second logical effect. Identity/content conflict,
missing predecessor, unsupported version, contradictory material and causal
gaps remain explicit until verifiable owner reconciliation; arrival order or
producer assertion cannot decide truth. Late arrivals may complete history but
cannot restore superseded current claims. Corrections do not execute compensation.
Replay/recovery cannot repeat a target action with unknown outcome or reacquire
a secret automatically.

Unresolved local dependencies block affected use. Independent eligible work may
continue only under its own current authority and absent applicable campaign-wide
withdrawal, termination or safety freeze. Stops do not wait for ordinary input
backlog. Dispatch-time checks block new work; in-flight stopping remains an
execution-design obligation, not an observation freshness mechanism.

### 4.4 Narrow effects, relationships and external knowledge

Protected-edge challenge/denial establishes only the observed interaction from
that vantage/time. It proves neither origin state, named control causality,
global absence, nor bypass permission. Another already-authorized, informative
action with smaller risk/footprint may be proposed; authority denial blocks the
affected proposal.

Dependency and identity/trust observations may supply sourced Terrain
relationships and Pathing candidates. Software dependency is not identity trust;
a candidate relation grants neither Access truth nor authority over a third party.
Separately admitted external knowledge remains source-qualified knowledge until
target evidence supports a target-specific claim. KEV presence is not client
vulnerability; missing/stale feed data is not evidence of safety. Intelligence
ingestion and executable test-artifact admission are distinct future contracts;
no CTI platform or connector is selected here.

### 4.5 Sensitive handling, defender sources and action authority

Ordinary core surfaces receive only non-sensitive semantics, opaque references
and safe metadata. Proof content remains attempt-ephemeral. Operational secret
values may exist only in separately authorized campaign-scoped custody under
PRD-010/INV-005, non-durable by default; resumability requires explicit authority.
An opaque reference grants no bearer authority or cross-campaign/retest reuse.
Expiry, withdrawal, revocation, invalidation or termination ends eligibility
and requires honest disposition.

Late contamination blocks affected ordinary use and triggers bounded downstream
reassessment/remediation through safe references. Audit accountability preserves
safe history, not prohibited bytes. Disposal uncertainty remains visible.

Blind reasoning excludes privileged defender/Observer/Grader sources through
all summaries, caches, derivations and replay. Legitimately acquired telemetry
requires an authorized current campaign position plus provenance, epistemic and
sensitive-data checks. Defender-informed exercises are separately authorized,
labeled and evaluated; missing telemetry coverage cannot prove stealth.

Admission, owner claims, evidence, CTI, events and tool results cannot dispatch.
Reasoning supplies proposals; current deterministic authority, Capability
Gateway, Execution Broker and Adapter remain necessary. Results return through
applicable admission and owner consideration, never direct model writes.

## 5. Consequences

Fast orientation and qualified hypotheses remain possible with bounded owners
and visible uncertainty. The cost is claim-specific contracts, controlled
interfaces, current-premise checks and accountable failures. Input producers
must expose source/method limits; consumers cannot infer global truth from
a successful parse, type label, event, cache or feed.

No accepted PRD or ADR is amended. This design does not require all future
subsystems before one safety-complete MVP behavior; the actual minimum contracts
and permitted implementation sequence require their own bounded decision.

## 6. Invariant compliance matrix

| Invariant | Architectural enforcement |
| :--- | :--- |
| INV-001 | Bounded input/owner interfaces; no universal manager or reconciler |
| INV-002 | Five owners retain state and burden; observation/evidence/custody is no sixth operational model |
| INV-003 | Proposals and input never dispatch; current authority and Gateway/Broker/Adapter remain separate |
| INV-004 | Admission differs from owner acceptance; claim-relative burden, time and corrections persist |
| INV-005 | Safe egress before ordinary use; opaque core references and isolated authorized custody only |
| INV-006 | Owner accountability, retained history and linked correction; no capability audit mutation |
| INV-007 | Source/mode eligibility persists downstream; blind and defender-informed results remain distinct |

## 7. Review cases and verification boundary

These are document counterexamples and future contract obligations, not executed
runtime tests or client results.

1. Authorized external denial can support a narrow owner-qualified observation without a foothold; no origin/bypass claim follows.
2. A transient origin fails, or succeeds without independent claim support: apply origin limits; never promote automatically.
3. Authentication succeeds without execution/context/stability evidence: no validated foothold.
4. Two feeds/tools repeat one acquisition/advisory: no invented source independence.
5. Old effect arrives today, or unexpired material is corrected/revoked: preserve time and current-use limits.
6. Duplicate, conflicting identity, out-of-order correction or superseded late input: no repeated effect or last-arrival truth.
7. Unknown target outcome or custody loss: no automatic action replay or secret reacquisition.
8. One local premise fails while another is eligible: isolate affected use; campaign-wide stops still govern both.
9. Unexpected sensitive input or late contamination: block propagation, reassess affected claims, preserve safe accountability.
10. Privileged defender verdict is summarized into blind context: reject the source laundering.
11. A dependency/trust candidate points at a third party: do not inherit client authority.
12. Evidence supports defer/no-action: retain bounded rationale without forcing another action.

Check references, owner/source boundaries, time/correction semantics and status.
Runtime wiring, Cargo checks and pilot proof are N/A until authorized.

## 8. Deferred decisions and next boundary

Concrete types/enums, public/wire schema, identity algorithms, retention periods,
timeouts, queues, additional storage, runtime topology, sanitizer/isolation/
cryptography mechanisms, CTI connectors and executable capability admission
remain deferred. Already accepted ADR-003/005 durability is not deferred again.

Next: product-owner review of this authored PROPOSED ADR. ADR-009..012 retain
their dependency requirements. No automatic acceptance, runtime/acquisition/
capability authorization, or change to DW-FOUNDATION-001 occurs.

# M0 — Minimum Mission, Planning Eligibility and History Contract

| Metadata | Value |
| :--- | :--- |
| Status | PROPOSED — owner requested authoring; contract and bounded sequencing exception await acceptance |
| Date | 2026-09-30 |
| Verified authoring base | `c8b9cba275dbd3bb663ae38ba0d83c0f6e690717` |
| Scope | Rust core; local mission/planning controls and durable non-sensitive history |
| Authority | Accepted PRD-000/001; ADR-001..005; INV-001..007 and QUALITY_BAR |
| Boundary | No runtime, target acquisition or milestone seal authorized by this draft |

## 1. Observable outcome and smallest scope

Through a local operator entrypoint, register an authorized mission, assess one
planning request against its bounds, withdraw authorization, and show that the
same request is refused. Restart/recovery preserves the accepted history and
withdrawal. No target action occurs anywhere in this behavior.

Choose a local CLI for this first behavior: it exposes the core without adding
a web service, UI or remote-account system. The CLI is a bounded application
caller, not an authority owner or an arbitrary command runner.

An eligible planning assessment means only that the request fits the evaluated
mission bounds at the stated revision/time. It is neither complete deterministic
action authorization nor a reusable dispatch token. Evidence, origin, capability
eligibility and the Gateway/Broker/Adapter path remain later obligations.

Preserve the campaign identity and five model owners. Implement only the
Mission/lifecycle responsibility, this narrow planning assessment, and the
Trajectory history consumer. No empty Terrain/Access/Pathing/Objective crates,
generic manager, all-domain context, LLM, graph cache or tenant framework.

## 2. Dependencies and bounded sequencing proposal

The current [stage packet](../build-order/02-reality-and-domain.md) describes
full Reality/Evidence design before full domain implementation, with Trajectory
implementation later. Propose a limited M0 lane before completing that sequence:
only this mission/history behavior. It does not claim DW-DOMAIN-001 completion
or remove dependencies from evidence, proof, acquisition or execution.

| Decision | M0 disposition | Required before |
| :--- | :--- | :--- |
| ADR-003 domain events and ADR-005 PostgreSQL | Mandatory now: required producer/consumer history durability | Any accepted M0 durable behavior |
| ADR-008 observation/fact separation | Already ACCEPTED; no target observations consumed in M0 | Observation-enabled behavior |
| ADR-009 evidence immutability | Defer evidence-specific design; M0 history still obeys ADR-003/005 and INV-006 | EvidenceEnvelope/material persistence, correction or export |
| ADR-010 proof fingerprint | Defer; no client proof derivation in M0 | Proof fingerprint/ProofEnvelope behavior |
| ADR-011 sensitive-data barrier | Defer mechanisms; safe ordinary-surface input limits remain mandatory | Raw capture/target output intake, sensitive proof or secret custody |
| ADR-012 engagement proof key | Defer; no proof key or proof cryptography in M0 | Keyed proof generation/verification |

A simple audit record is not an EvidenceEnvelope or client proof. Calling target
material a log to avoid these dependencies is prohibited. Before any acquisition,
review the actual output/sensitive surfaces and accept the necessary boundaries;
a planned non-sensitive probe does not guarantee non-sensitive output.

Rust core and Rust PostgreSQL port implementations fit ADR-004. No Go-baseline
amendment is needed for M0. A future tool adapter's responsibility must be assessed
separately. PostgreSQL remains the only core system of record; no memory/SQLite
persistence substitute or additional broker is selected.

## 3. Owners and actual call/consumer path

| Responsibility | Permitted ownership | Excluded responsibility |
| :--- | :--- | :--- |
| Mission/lifecycle | Authorized goals, scope references, mode, validity and withdrawal; owner revision | Environmental claims, access truth, objective fulfillment |
| Planning assessment | Compare the supplied bounded request with current Mission bounds; accountable result | Granting missing authority, target execution or accepting observations |
| Trajectory | Append the required sourced mission/assessment/withdrawal history and corrections | Current mission authority or writing Mission state |
| Persistence/delivery infrastructure | Implement declared owner ports and ADR-003/005 obligations | Domain acceptance, arbitrary sibling-table writes or execution |

Required path: local CLI -> bounded application use case -> Mission owner/current
bounds -> owner-local PostgreSQL acceptance/publication obligation -> declared
Trajectory consumer -> durable history/completion -> bounded operator view.
Trajectory writes through its own contract; Mission never writes its aggregate.
Selected historical views are not a global campaign snapshot.

Every runtime delivery must wire its outcome from this entrypoint to its actual
consumer/output and verify the relevant failure path. No test-only island or
speculative capability interface may substitute for that path.

## 4. Minimum command and query semantics

Names below identify intents, not chosen Rust signatures or wire schemas.

| Intent | Applicable input | Output and required effect |
| :--- | :--- | :--- |
| Register authorized mission | Stable operation identity; engagement/campaign identity; safe authorized-operator/source reference and revision; bounded goals, explicit included/excluded asset references; exercise mode; validity/window and stopping bounds | Accepted Mission revision with required history obligation, or safe rejection/conflict; registration grants no target action |
| Assess planning request | Stable assessment identity; same engagement/campaign; bounded purpose and asset reference; expected Mission revision; explicit evaluation time/context | Accountable eligibility/refusal/unresolved result with scope, reasons, evaluated revision/time and limits; required history contribution |
| Withdraw mission authority | Stable operation identity; authorized operator reference; campaign; expected revision; bounded reason | Block affected planning immediately; record owner withdrawal/publication when durable, otherwise report unresolved and remain blocked; history delay cannot postpone enforcement |
| Inspect current mission/history | Authorized same-campaign purpose and bounded frontier/range | Current Mission view from its owner plus separately labeled Trajectory view, including pending/gaps; no hidden current-state inference from history |
| Reconcile/recover local obligations | Stable operation/publication/consumer identities; current authorized local context | Verified committed/not-committed/unresolved disposition, bounded history delivery and visible completion; no target effects |

Scope references are operator-admitted asset identities with explicit inclusion
and exclusion; matching a supplied reference is not proof of ownership, origin,
reachability or resolution to a future network endpoint. Unknown, conflicting or
excluded references cannot receive a positive result. No wildcard/inferred
third-party inclusion or generic scope-language engine is required.

Use the declared validity interval/windows at evaluation time; reaching the end
makes a request ineligible without needing a periodic status write. Unavailable
or uncertain required time/authority context prevents a positive result.
No timeout value or synthetic dwell delay is introduced.

Withdrawal/restrictive changes preserve revision and history. M0 has no implicit
reactivation, retasking, scope widening or deadline extension. A future explicit
reauthorization contract must not be inferred from replay or re-registration.

## 5. Acceptance, durability and failure obligations

- Validate source authority, same engagement/campaign, mode, safe inputs, scope
  and time at the relevant boundary. A claimed operator name/type label is not
  authentication; initial deployment assumes an explicitly trusted local operator.
- Use expected Mission revision for accepted mutations; stale/conflicting
  requests cannot overwrite withdrawal. Assessments retain the actual owner
  revision/time checked and are never enduring permission.
- Producer state/assessment records and required publication obligations commit
  together in PostgreSQL. Trajectory history and consumer completion commit
  together in its own transaction. No cross-owner transaction or foreign write.
- Same scoped identity and semantic content has one logical effect. Identity
  reuse with different content is an explicit integrity conflict, never overwrite
  or insert-ignore. Missing predecessors/versions stay visibly unresolved.
- Producer acceptance may precede Trajectory completion. Show local acceptance
  and pending history honestly. A positive planning admission is not reported
  complete until required recording succeeds; refusal/withdrawal still applies
  when recording is unavailable. No global all-model gate follows from this.
- Missing commit acknowledgment is unknown, not rollback. Reconcile the stable
  operation identity before any database-only retry. Retry rechecks current
  premises and stays bounded; unresolved results remain visible.
- Database/history failure prevents any positive use needing unavailable
  authority or recording. No in-memory grant or deferred automatic action.
  Stopping/withdrawal cannot wait for ordinary history backlog. If withdrawal
  persistence fails or is unknown, do not claim durable withdrawal or reuse the
  prior grant: block affected assessment until reconciliation and current operator
  authority are established. Startup confirmation covers unrecorded withdrawal.
- Accepted semantic history is append-only; corrections link to originals.
  Normal runtime paths cannot update/delete authoritative history. Bookkeeping
  changes never rewrite semantic content. This is not administrator tamper-proof
  evidence; privileged storage failure/protection claims remain unproven.
- Restart reconciles obligations and reads current Mission time/withdrawal before
  any new assessment. Restore/replay cannot reactivate a campaign; operator
  confirmation of current authority is required before continuation, preserves
  original bounds, and never silently extends them.

## 6. Input, isolation and deployment bounds

M0 accepts only purpose-qualified non-sensitive operator metadata/references.
No raw target response, client proof content, credential value, custody resolver
or privileged defender feed exists in the allowed path. Input errors, logs,
traces, SQL/driver diagnostics and operator views must not echo rejected bytes.
Unexpected/ambiguous sensitive input is refused with safe incident metadata;
there is no archive, quarantine or invented secret custody in M0.

An allowlist or valid identifier alone is not proof that arbitrary pasted text
is safe. The first demonstration uses operator-reviewed safe mission metadata;
no general sanitization guarantee is claimed. Qualify actual ingress, diagnostics
and persistence surfaces before accepting client material.

Single tenant does not relax engagement/campaign isolation on reads, writes,
delivery and recovery. Overlapping asset references cannot cross campaign
boundaries. Domain code cannot import PostgreSQL, CLI, LLM or execution clients.
Startup composition supplies narrow ports, not an all-domain runtime object.

M0 PostgreSQL is a DuskWeave-controlled store, never the client target under
assessment; database configuration cannot select a campaign target.
M0 deployment must qualify the implemented PostgreSQL durability, runtime
append-only permissions and the tested restart/failure model. Database roles
and migration/administrative authority remain separate. No disk-loss, failover,
backup restore or universal security assurance is claimed without its evidence.
No target-facing adapter or target connection is introduced, so M0's target
footprint is zero; this does not establish later stealth or detection resistance.

## 7. Verification and demo completion

Required implementation evidence through actual caller/consumer boundaries:

1. Valid operator mission -> accepted revision -> exactly one Trajectory item.
2. Matching planning purpose/reference within validity -> recorded bounded result.
3. Excluded/unknown asset, wrong campaign/source, expired/not-yet-valid mission,
   unavailable context or withdrawal -> no positive planning admission.
4. Concurrent stale mutation/assessment versus withdrawal -> no overwritten
   withdrawal or reusable permission; historical results retain their revision.
5. Duplicate command/delivery, conflicting identity or missing predecessor ->
   one effect or explicit unresolved conflict/gap, never silent success.
6. Lost database commit acknowledgment and consumer interruption -> reconcile
   durable operation identity and outstanding history without a second effect.
7. Withdrawal with failed/unknown commit, then restart -> no reuse of a prior
   grant without reconciled outcome and current operator confirmation. Restart
   with recorded withdrawal or expiry -> history remains inspectable and new
   assessment stays ineligible; replay has no external side effect.
8. Rejected sensitive sentinel -> no propagation into the ordinary path or error
   diagnostics; no raw real-client fixture or sensitive debugging archive.

Use deterministic isolated negative/failure tests with injected time and bounded
faults. A disposable test PostgreSQL may verify real transaction/recovery behavior;
tests cannot depend on external networks, unbounded sleeps or client fault injection.
Unit ports alone do not prove PostgreSQL semantics or production reachability.

Demonstrate with explicitly authorized, operator-reviewed real-client mission
metadata: register -> assess -> withdraw -> refuse -> restart -> inspect history.
This proves local core behavior only, not target acquisition, compromise, coverage,
objective success or released client proof. No synthetic success substitutes for
client evidence, and no real target action is implied by the demo.

## 8. Delivery boundary and STOP

M0 is a milestone, not one oversized runtime packet. Split into bounded vertical
deliveries, each with a real entrypoint/consumer and meaningful tests, under
[QUALITY_BAR](../../QUALITY_BAR.md). Do not ship horizontal empty scaffolding.
Before its first IMPLEMENT packet, accept this contract/sequencing scope and
resolve the concrete public types, owner persistence/concurrency boundaries,
allowed files, actual failure tests and deployment assumptions in that packet.
The engineering partner resolves those decisions before IDE handoff; the IDE
receives an executable outcome, not an instruction to devise architecture.
Routine implementation choices cannot invent missing product/architecture.

Stop with DESIGN_DRIFT if the assigned head changes; AUTHORITY_CONFLICT for
contradictory governing decisions; SPLIT_REQUIRED for evidence/proof/custody,
acquisition/dispatch, another model, or runtime files outside the accepted map.
An M0 seal requires explicit owner action at the final verified commit with
test/demo evidence and limits. DW-FOUNDATION-001 and the full DW-DOMAIN-001
requirements remain unchanged. This PROPOSED design does not authorize code.

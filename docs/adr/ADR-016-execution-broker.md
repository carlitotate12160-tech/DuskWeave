# ADR-016: M1 Effect Admission, Cancellation and Recovery

| Metadata | Value |
| --- | --- |
| Status | ACCEPTED — product owner, 2026-10-06; M1 selected lane only |
| Date | 2026-10-06 |
| Revision | R1 — scoped architecture accepted by the product owner on 2026-10-06 |
| Authoring base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product authority | Accepted PRD-000..010 and M1 product contract; no new product breadth |
| Direct dependencies | accepted ADR-003/005/013/015; Stage 12 product-input rebind in accepted M1 enabling section 2 |
| Scope | [M1 enabling lane](../contracts/M1-enabling-architecture.md); no full-stage acceptance |

## 1. Context and problem statement

A current database read followed by later worker dispatch leaves a revocation race.
M0 eligible-v4 history is not an execution grant. Missing acknowledgements cannot justify replay.

## 2. Decision drivers

One accountable effect-start point, bounded in-flight uncertainty, durable attempt identity,
actual-operation budgets and no database lock across external I/O.

## 3. Considered options

| Option | Benefit | Conflict / disposition |
| --- | --- | --- |
| Trust old eligibility/serialized permit | Easy dispatch | Stale or forged authority; reject |
| Hold SQL transaction while contacting targets | Apparent fence | Violates ADR-005 and still lacks atomic external commit; reject |
| Serialize current effect admission and stopping, with durable attempt markers | Explicit local ordering | Select; every authority writer must participate |

## 4. Decision outcome

### Admission fence and authority writers

One live Broker owns a campaign execution session and a narrow local admission fence.
Normal dispatch, withdrawal, scope/freeze/termination mutation and renewal paths serialize
through it, including the existing withdraw CLI while an M1 session is active. The explicit
stop-only fallback below additionally serializes with release through the shared host guard.
Session fencing/generation and database constraints prevent concurrent execution owners.
Do not let an old M0 direct database writer bypass an active M1 session.

Selected owner commands that invalidate a required routing/material premise serialize their
commits with this same admission fence through a narrow application guard. Other owner work
is unaffected. Terrain retains acceptance/publication rights; Broker never writes Terrain.
At release, reevaluate time eligibility and the remaining DNS/proposal/authority deadline,
not only the revision read before database work. The worker's issued start deadline cannot
outlive any required premise. Missing coordination or expiry blocks release/renewal.

Select one local Unix-domain control socket under /run/duskweave/m1/ with directory
mode 0700, core-UID ownership and peer-credential checking. Only fixed safe stop/authority
commands from the authorized CLI are admitted; it is not a worker/tool endpoint. No socket
is exposed to a container or remote host. Active-session CLI withdrawal normally routes here.
If the Broker is absent/unreachable,
the trusted local stop-only CLI uses ADR-015's fixed host STOP action for that campaign/
generation, then the restricted Mission withdrawal function below. No generic direct-write
fallback, public service or bypass dispatch is added.

A short PostgreSQL transaction with a campaign-scoped advisory lock/expected generation
creates the unique durable session and sets its broker writer identity. Database permissions
and a guarded authority mutation function prevent ordinary/legacy writer roles from changing
an actively fenced campaign; only the distinct session-bound broker role can accept
ordinary authority mutations.
A separate restricted Mission stop-writer may invoke only the monotonic withdrawal
function: stable operation identity, authorized operator, expected Mission revision and
session generation; append withdrawal/publication under ADR-003/005. It cannot activate,
widen, clear the session/host guard, change evidence or obtain execution rights. The host
STOP latch is applied first even if SQL later fails or conflicts. All regular old/new writer
paths remain guarded; the stop-only function is the explicit narrow exception.
Release/recovery is an explicit no-effects command after attempts/stops are reconciled.
No role may clear the guard to obtain dispatch. Migration/admin roles remain outside runtime.
The session has no reusable bearer execution token. Concurrency/role qualification must prove
that a stale binary or direct ordinary-role SQL write cannot bypass the active-session fence.

Before a selected effect:
1. reserve durable original attempt identity, profile, proposal, relevant premises and counters;
2. ensure required decision/authorization/attempt Trajectory history is durable;
3. within the admission fence, obtain current committed Mission authority and owner premises,
   validate deadline and budgets, durably mark that this effect may start, then end SQL work;
4. recheck local stop/generation/deadlines; ask the fixed host launcher to release the
   one-use START/exact egress under its shared grant/STOP critical section;
5. await only safe result/disposition outside SQL and the admission sections.

The **effect-start linearization point** is host release of the one-use START with exact
egress permission under ADR-015's shared host section. Normal authority mutation also uses
the Broker fence; unreachable-Broker withdrawal uses host STOP before its stop-only write.
A planning result, enqueue, READY or SQL marker alone is not this point.
It marks a possibly starting operation, not proof that a packet reached its destination.
A withdrawal ordered before that release prevents it. A withdrawal ordered after it closes
egress/cancels the possibly in-flight effect. No claim of external rollback is made.
A crash between durable may-start marker and release conservatively leaves UNKNOWN.

The capture/egress pair must have signaled qualified READY before release. The start
permit expires after at most 1 second under ADR-015, clamped to the required deadline;
capture cannot initiate after expiry. It may perform only that request;
new direct DNS/connection/request phases require their own current authorization and reserved
counters. TLS and HEAD must check the local cancellation/permit path before starting,
not continue because the connection was previously approved.
The host watchdog renews communication only after Broker rechecks current committed
authority and original deadline under its fence, then the host guard checks STOP/generation
before renewal. Independent stop-writer revocation is preceded by host STOP, so it cannot
race a cached authority read into a later grant. Invalidation closes egress before history.

Concurrent authority mutation applies local stop before waiting on durable SQL confirmation;
commit ambiguity leaves stopping in force. Renewal/admission cannot run while a writer's
commit is unresolved. CLI/crash recovery cannot create a second execution session.
If any writer or runtime role can bypass this protocol, M1 effects are NOT READY.

The stop-only fallback returns durable/pending/unknown withdrawal receipt with the original
operation/session identity and local stop disposition. DB/history unavailability does not
clear STOP or claim durable withdrawal. Mission revision conflict leaves stopping in force
until no-effects reconciliation; recovery cannot infer missing intent as permission. The
operator preserves the safe receipt for unresolved retry/reconciliation. A restored or
restarted old session remains default-deny, and withdrawal cannot implicitly reactivate.
Broker-unreachable and DB-unavailable cases are separate qualifications, not one fake PASS.
Existing M0 functionality must be preserved and its affected writer path qualified in the
actual runtime slice; no change is implemented by this ADR.

DB unavailable, authority unavailable, required history pending or fence ownership uncertain:
no new effect/renewal. In-flight work is stopped through the independent host watchdog.
Restored DB/server state never auto-resumes a session or widens its permit.
Unsupported multi-host writers are excluded from this single-host lane.

### Durable result and recovery

Broker records safe attempt dispositions: rejected/pre-start-not-issued, may-have-started/
unknown, observed-result, cancelled-with-inflight-uncertainty, and unresolved-handling.
Worker READY or may-start persistence never claims a real observed response.
Observed safe result enters the receiving owner only after ADR-011/017 admission.

Persist result/publication obligations with stable identity before acknowledging the worker.
Worker discards raw capture regardless of result-ack loss; it does not keep raw retries.
After ack loss, core recovery inspects the durable safe result or leaves UNKNOWN.
No automatic network retry, worker restart or dispatch is part of reconcile/replay.
Campaign totals, episode reservations and consumed/possibly-consumed phase counters are
durable Broker execution records under ADR-013. New episode/restart does not refill them;
UNKNOWN stays charged. Provably unreleased reservation release is a linked accountable
record, not an erased attempt. Declared DNS follow-up or address alternative is a new
proposal/attempt that separately satisfies current authority and remaining budgets.
An intentionally new observation requires a new proposal/attempt, current authority and
remaining actual-operation budget; it cannot clear the old unknown effect or disguise replay.

Required delivery is bounded database-only recovery under ADR-003/005.
Duplicate/conflicting safe result identities follow ADR-009; scope/generation/profile
mismatches cannot become observation evidence. Failed required persistence leaves visible
pending/unknown history and blocks only dependent effects absent a broader stop.

### Required qualification counterexamples

Withdraw racing READY/release; writer commit ack lost; direct legacy writer while session
active; owner crash before/after release; DB unavailability; stale session generation;
watchdog/parent loss; Broker-unreachable withdrawal with host STOP and stop-writer receipt;
DB-unavailable withdrawal/restart; timer expiry; consumed permit reuse; safe result committed
before ack loss; forged result; recovery after restore; UNKNOWN budget across new episode;
no repeated DNS/HEAD via recovery; new explicitly admitted follow-up counts separately.
Assert actual fixture effect counts and durable original identity across fresh recovery.
A pre-commit fault is not evidence for post-effect acknowledgement loss.

## 5. Consequences

The fence is execution-specific coordination, not campaign truth or a generic scheduler.
It requires integration with actual authority writers and OS effect controls before any
acquisition slice can be ready. Limits bound stopping but cannot retract a sent request.
No exactly-once external execution guarantee is made.

## 6. Invariant compliance matrix

| Invariant | Obligation |
| --- | --- |
| INV-001 | Broker owns only issued effects, session fence, counters and stop/recovery. No universal manager or workflow engine. |
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

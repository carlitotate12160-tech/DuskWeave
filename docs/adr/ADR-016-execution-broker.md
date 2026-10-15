# ADR-016: M1 Effect Admission, Cancellation and Recovery

| Metadata | Value |
| --- | --- |
| Status | ACCEPTED — product owner, 2026-10-06; M1 selected lane only |
| Date | 2026-10-06 |
| Revision | R1 — scoped architecture accepted by the product owner on 2026-10-06; R2 adds generated-worker nested-effect admission and independent cognition/build budgets, published under the owner-approved 2026-10-07 direction. R3 defines the initial bounded inference-repair profile; scoped acceptance and unrelated amendment exact-text limits follow ENGINEERING_STATE.md |
| Inference amendment | Initial #51 profile and required R2/R3 refinements ACCEPTED by the owner on 2026-10-11 for this inference lane; no blanket R2/R3 acceptance, R1 change or seal |
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

### Generated-worker nested effects and cognition/build budgets

A generated worker submits only typed effect requests; each returns through authority,
Gateway and Broker, and trusted R1 capture/exporter code performs actual I/O. Every
nested request has its own attempt identity and passes the same current effect-start
admission; no admitted parent capability grants a batch of future contacts or bypasses
withdrawal. The reservation, may-start, UNKNOWN and no-refund semantics above apply
unchanged to each nested effect.

Cognitive and build sessions are decoupled from the 60-second acquisition episode. Model
latency or compilation must not consume a stale admission and then release a late effect.
Cognitive work may span episodes, but each effect still requires a fresh eligible episode
and current premises; all existing campaign effect totals survive restart.

Mission/deployment configuration must supply finite cognition/build limits: inference
calls and input/output size, wall time, candidate revisions, build/test CPU and memory,
artifact/output bytes and concurrent workers. Effective limits are the minimum of
campaign remainder and deployment ceilings; missing limits disable that operation. These
budgets do not increase any R1 network ceiling or authorize parallel effect episodes.
Provider timeout, malformed output and schema-repair attempts consume inference budget;
repair cannot dispatch actions. Provider loss produces a visible deferred/failed state;
there is no silent fallback to another provider or a non-cognitive success claim. Budget
reservation and uncertain consumption survive restart; uncertain effects are not refunded
or replayed. Recovery first reconciles durable state, then replans. Withdrawal stops new
inference/build/dispatch work and cancels owned workers; already-sent provider data and
network effects cannot be retroactively recalled.

### Initial inference accounting profile (scoped acceptance)

This section canonically owns the numerical rules for `deepseek-responses-v1`,
`deepseek-responses-envelope32-v1`, `m1_inference_permission_v1` and `m1_inference_permission_v2`;
the [enabling contract](../contracts/M1-enabling-architecture.md#initial-inference-contracts-scoped-acceptance)
owns field semantics, provider selection and history. The owner accepted the initial
#51 profile and required R2/R3 refinements for this lane on 2026-10-11, and the native
continuation profile on 2026-10-15, without blanket amendment acceptance or a seal.
Initial registration consent configures these ceilings; it implements no reservation,
consumption, refill, provider qualification or send.

| Resource | Deployment ceiling / accounting unit |
| --- | --- |
| Immutable ContextPack | 4096 UTF-8 bytes; at most 8 candidates per pack, not a campaign-wide hypothesis cap |
| Complete serialized request / retained configuration | Each <=16384 UTF-8 bytes per call; request includes instructions, schema, pack and any repair material |
| Response envelope | 65536 bytes per call, including reasoning items and non-streaming keep-alive whitespace |
| Calls per decision | 1 generation plus at most 1 conditional schema repair; 2 reserved slots before generation |
| Output capacity | `max_output_tokens=4096` per call, including provider reasoning tokens |
| Shared generation/repair duration | At most 20 seconds, clamped to remaining command, Mission and consent deadlines |
| Connection establishment | At most 5 seconds per call inside the same shared envelope; DNS/TCP/TLS included |
| Proposal / repair material | 8192 / 2048 UTF-8 bytes; each proposal text field <=1024 bytes; at most 8 static repair-error codes |
| Local JSON parsing | At most 8 nesting levels (depth 32 for response envelope under `deepseek-responses-envelope32-v1`) and 64 entries per collection; bounds checked before growth |
| Inference concurrency | 1 live decision/call per campaign; no parallel repair or concurrent session owner |

Response counting includes HTTP headers and entity bytes before parsing, counts leading
blank lines, and rejects compression rather than permitting unbounded decompression.
Request accounting measures the complete JSON entity, not TLS framing or the API key;
transport metadata is fixed and bounded, with no caller-supplied headers or proxy route.
These are deployment assumptions, not latency guarantees or exact token-billing estimates.
Byte counts, reserved output tokens and reported input/output/reasoning usage are distinct.
No tokenizer or byte-to-input-token conversion is claimed; input billing remains reported
usage (or unknown), not a preverified token charge. Campaign financial billing is not
inferred from these byte budgets.

Mission supplies explicit positive finite campaign totals for calls, request bytes,
response bytes, output-token capacity and inference wall-time milliseconds, plus per-pack,
per-call, per-decision and concurrency limits no wider than this profile. Missing, zero,
negative, overflowed or unsupported limits disable inference; exhausted remainder defers.
Effective limits are the minimum of campaign remainder, campaign per-operation limits and
deployment ceilings. Never silently lower the selected output cap, omit the repair reserve
or switch profile to fit: if the complete configured decision cannot be reserved, defer.
Budget identity belongs to the campaign, not the permission revision or CLI invocation;
renewed consent, a new decision, session release or restart cannot refill consumed totals.

Broker atomically reserves both call slots and their full per-call request/response/output
capacity plus the single shared wall-time envelope in a short durable transaction before
any send. Concurrent decisions compare campaign remainder and exclusive generation under
that transaction; loser gets a duplicate/conflict/busy/insufficient-capacity disposition,
not another allowance. Each slot has a stable scoped call identity and role (generation or
repair), with reserved-not-sent distinct from a call actually initiated. All transitions
retain original identity, expected state and append-only accounting evidence.

Before each call, Broker obtains current committed Mission inference authority through its
public contract, checks withdrawal/freeze/termination, revision, purpose/mode, recipient,
field classes, profile support, time and reserved capacity, and durably records `may_send`
with that call's exact admitted P/C/B bytes and locally computed binding. Generation bytes
bind before generation; repair bytes bind only after eligible repair admission, without
changing its pre-reserved identity/capacity. A slot alone cannot authorize different bytes.
All relevant authority writers serialize with this inference admission fence; local send
release and stop serialize under the same campaign/generation owner, after SQL ends.
Immediately at release recheck stop/generation, unchanged authority revision and remaining
Mission/consent/shared/command time; expired, pending-writer or unavailable authority blocks
release even after a successful marker commit. Clock uncertainty cannot extend authority.
Release is the send linearization point under that owner: a stop ordered before release
prevents transport and leaves reserved/cancelled local accounting; a stop ordered after
release means a possible send, so owned transport is cancelled and the call stays
cancelled-uncertain, never rolled back to unused. No database lock spans inference. A reservation or `may_send` marker is not proof of a
send: a crash in the marker-to-send gap conservatively means uncertain consumption.
Unconfirmed reservation/marker COMMIT blocks send even if it may have committed.
Actual send start is recorded separately when observable, never reconstructed from a
reservation. No provider idempotency/exactly-once claim substitutes for this local fence.

Consumption states distinguish reserved-not-sent, may-send/uncertain, sent-awaiting-result,
completed-with-reported-usage, failed-with-known-local-disposition and cancelled-uncertain.
Any possible send consumes the call slot and keeps its reserved byte/token capacity charged;
reported actual usage is additional evidence, not a quota refund. Wall time is measured
monotonically; after interruption its unaccounted reserved duration remains charged.
Only capacity proven never released, with the previous sender fenced from later release,
may be freed by a linked record (for example an unused repair slot); unknown never refunds.
A lost response, missing usage or ambiguous commit survives restart. Reconciliation does
not equate an absent observation with absence of the provider effect. A restored database
with a possible accounting gap blocks new calls until the gap is resolved, not reset.

The shared clock begins before generation admission/transport and includes generation,
validation, repair admission and repair transport; keep-alives, retries of database-only
work and repair do not reset it. Clamp each wait to the remaining shared/command/authority
window. The existing 30-second CommandGuard and database socket/startup/statement/lock
bounds remain unchanged; inference cannot use the C0 human-response pause to extend them.
No guaranteed history/receipt completion is promised in the residual command time. Exit
124, timeout or history failure leaves deferred/pending/unknown evidence as applicable.

Only a completed response with eligible schema defects may use the reserved repair slot;
local safe-admission must first produce bounded error codes and optional admitted fields.
Malformed/empty completed proposal text may use codes alone; raw text is never echoed.
Same question, pack and allowed references apply. Authority/scope/provenance rejection,
digest mismatch, unsupported protocol, timeout, lost response, incomplete/truncated output
or exhausted capacity cannot trigger repair. A repair result repeats complete validation;
there is no second repair, automatic transport retry, fallback or non-cognitive success.

Withdrawal blocks new sends and cancels owned in-flight transport before history waits.
Cancellation closes owned I/O and remains inside the original shared deadline; an
unconfirmed stop is reported unresolved, never successful rollback. Expiry, lost current
authority or ownership similarly blocks new work and terminates local waiting. Already
disclosed data cannot be recalled and provider-side cancellation is not guaranteed.
`prepared_no_effects` and merged prepared-only withdrawal do not implement or qualify this
inference fence/cancellation, acquisition, host STOP or target START. Their existing
no-effects release semantics must not be reused for a possibly sent inference call.

Duplicate inspection and history/accounting recovery never invoke the provider, continue
an unused repair slot, re-reserve the same decision or replenish quota. Database-only
recovery retains ADR-013's bounded invocation policy and existing wait guards. A genuinely
new decision requires explicit new identity, current consent and remaining capacity; it
cannot erase uncertainty. Concrete future assertions are in the enabling contract; all
storage, cancellation, transport and formatted runtime-fit evidence remains UNVERIFIED.

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

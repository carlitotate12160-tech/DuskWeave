# RUN_M1_SESSION_FENCE — prepared no-effects campaign session fence

## Scope and authority

`DW-IMPLEMENT-M1-SESSION-FENCE` (R1.1) adds one durable, prepared-only
campaign session fence under ADR-016 and the accepted M1 enabling
architecture. It is a bounded prerequisite for M1 execution ownership
accountability: the fence records that a distinct Broker login claimed a
campaign scope, holds ordinary authority writers out while that claim is
held, and releases only the exact prepared generation on an explicit call.

A prepared session is **not** an execution grant, a host START, a lease, a
renewal channel, a target request, a credential custody surface or an
effect-enabled state. No worker orchestration, dispatch, host control or
acquisition qualification exists in this slice. Every receipt reports:

```json
"current_permission": false,
"dispatch_granted": false,
"acquisition_qualified": false,
"host_control_qualified": false
```

## Command surface

```text
duskweave m1-session --action prepare|recover|release \
    --operation UUID --input FILE
```

`--input` is a bounded strict JSON document (≤16 KiB, unknown fields
rejected) containing exactly:

```json
{
  "engagement_id": "uuid",
  "campaign_id": "uuid",
  "operator_ref": "uuid",
  "expected_mission_revision": 1
}
```

The operation UUID is allocated deliberately beforehand with the existing
`prepare-operation` command; session commands never allocate operations.
Input must not carry endpoints, secrets, payloads or effect fields — the
parser rejects them. Unknown commands reject as `unknown_command`; all
existing commands keep their behavior.

The command uses the existing qualified database connection and the fixed
database wait policy (`RUN_DATABASE_WAIT_BOUNDS`). The login given through
the configured credential source must be a member of both `dw_runtime` and
`dw_m1_broker` to prepare or release; read-only `recover` works for an
ordinary runtime login.

## Prepare

Prepare composes the admission through the public Mission port: it reads
the Mission catalog row and the validated original registration outbox
event, binds operator/scope/original identity, requires the M1 permission
attachment, and passes the complete validated event to the guarded
function. Under the fence lock the function requires:

- `expected_mission_revision` exactly 1 and a matching operator;
- the stored contract identical to the composed event;
- an unwithdrawn Mission in its current half-open window;
- a permission attachment in its current half-open window;
- no current claim, and no prior registration/withdrawal/session use of
  the operation identity.

A successful prepare increments the generation, binds the actual login's
`pg_roles` OID (from `session_user`, never a caller field), and appends a
`prepared_no_effects` history record atomically. Replaying the same
operation, operator and login returns the original latest record
idempotently — no generation refill, no takeover. A different identity on
a used operation conflicts. While a claim is held, ordinary INSERTs into
`mission.missions`, `mission.registration_outbox` and
`mission.withdrawals` — by old binaries, direct SQL or the Broker login's
own ordinary path — are refused by the writer guard, which creates the
idle fence row when needed.

## Release

Only the original prepared operation, operator, revision and same login
release the exact current `prepared_no_effects` generation. Release clears
the fence to `idle`, appends a linked `released_no_effects` record and
preserves generation and history. Replaying release for an already
released operation returns its record without clearing a newer prepared
generation. Release never fires implicitly on process exit, timeout or
restart — an absent Broker login does not unlock a fence.

## Recover

Recovery is strictly read-only: it returns the latest durable
prepared/released record for the exact scope and operation, or `missing`.
It never claims, transfers writer ownership, increments a generation,
restarts work, retries or dispatches. A returned prepared record is
historical evidence only — it is neither a reusable permit nor proof of
present ownership.

## Receipts and outcomes

Every receipt carries the scope, operation, the lifecycle record when one
exists, and `outcome`:

| Outcome | Meaning |
| --- | --- |
| `durable` | A lifecycle record exists and bound to the request identity |
| `missing` | No record exists for the exact scope/operation |
| `refused` | A confirmed denial (invalid input, guard refusal, unqualified writer, or a known serialization abort) |
| `unknown` | Outcome unresolved — transport, commit or acknowledgment ambiguity |

`refused` covers both categorical denials and retryable aborts: the
stderr reason distinguishes them — `serialization_retry` (SQLSTATE
40001/40P01) rolled back cleanly and the identical request may be
retried, while `session_*`/input refusals are categorical. After
`unknown` or exit 124, run `recover` on a fresh connection before
any explicit retry: exit 124 means caller stop, not SQL rollback. Never
infer absence, success or rollback from missing output. The retry is the
same identity — the fenced operation replays its original record.

## Operating procedure

1. `duskweave prepare-operation --engagement E --campaign C` → new UUID.
2. Write the bounded input document; run `m1-session --action prepare`
   with the Broker credential source.
3. While prepared, ordinary authority writes on the scope are refused;
   unrelated scopes stay writable.
4. Before M0 withdrawal of the scope, run `m1-session --action release`
   with the same operation/operator/login; withdrawal of a claimed scope
   is fenced until release.
5. Any `unknown` or 124 outcome: `m1-session --action recover` first.

## Limits

The fence is a writer fence and durable record only. It does not provide
host STOP, does not stop writers through any mechanism besides INSERT
denial, and a future stop-writer fallback is neither implemented nor
qualified. It grants nothing by itself: no current permission, dispatch,
acquisition or host control ever reads true from a session receipt. The
fence does not prevent privileged administrative corruption — runtime
trust boundaries cover ordinary logins only. Advisory-lock hash collisions
serialize unrelated scopes conservatively but never confer permission.
The guard covers INSERT writers; UPDATE/DELETE/TRUNCATE and schema or
trigger changes remain denied to runtime roles outright.

Evidence for behavior belongs to the candidate's delivery report and CI;
this runbook documents semantics, not a measured claim.

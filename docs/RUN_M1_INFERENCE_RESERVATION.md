# RUN_M1_INFERENCE_RESERVATION — pre-send inference capacity reservation

## Scope and authority

`DW-IMPLEMENT-M1-INFERENCE-RESERVATION` adds one durable, Broker-owned
pre-send capacity ledger under ADR-016 and the accepted M1 inference
contracts. It is a bounded prerequisite for M1 inference accounting: a
distinct Broker login atomically validates the initial consent and Mission
publication at the SQL boundary and records a `reserved_not_sent` hold
against campaign and operation limits, before any provider adapter exists.

A reservation is **not** a send authorization. Nothing in this slice sends
data to a provider, invokes a model, builds a ContextPack, admits a
proposal, qualifies acquisition or dispatches any capability. Every
receipt reports:

```json
"current_permission": false,
"dispatch_granted": false,
"provider_qualified": false,
"acquisition_qualified": false,
"send_qualified": false
```

## Command surface

```text
duskweave m1-inference-budget --action reserve|recover|cancel \
    --operation UUID --input FILE
```

`--input` is a bounded strict JSON document (≤16 KiB, unknown fields
rejected) containing exactly:

```json
{
  "version": "m1_inference_reservation_request_v1",
  "engagement_id": "uuid",
  "campaign_id": "uuid",
  "operator_ref": "uuid",
  "expected_mission_revision": 1,
  "permission_id": "uuid",
  "goal_ref": "uuid",
  "exercise_mode": "blind"
}
```

All identities must be non-nil. `expected_mission_revision` must be
exactly 1; `exercise_mode` is `blind` or `defender_informed` and must
match the Mission's mode. The operation UUID is allocated deliberately
beforehand with `prepare-operation`; the command never allocates
operations. Unknown commands reject as `unknown_command`; all existing
commands keep their behavior.

The command uses the existing qualified database connection and the fixed
database wait policy (`RUN_DATABASE_WAIT_BOUNDS`). The login given through
the configured credential source must be a member of `dw_m1_broker` for
every action including `recover`: the guarded function checks group
membership before dispatch, and EXECUTE is granted to that group only.
The Broker test logins additionally hold `dw_runtime`. The function binds
the writer identity from `session_user` through `pg_roles` — operator
fields, SET ROLE and custom GUCs cannot supply it.

## Reserve

Reserve composes the expected source through the public Mission port: the
validated original registration outbox event, which must carry the
`m1_inference_permission_v1` consent. Recover and cancel never read the
source — reservation lifecycle after the original hold is ledger-local.
Under the scope lock the guarded function requires:

- a `m1_inference_permission_v1` consent on the exact composed
  publication, matching the request's `permission_id`, `operator_ref`,
  `goal_ref` and `exercise_mode`;
- `provider_terms_approved` true, the DeepSeek recipient and the
  `deepseek-responses-v1` capability profile, with no unknown or missing
  consent fields;
- an unwithdrawn Mission at revision 1 inside its current half-open
  window, and the consent window inside both the Mission window and the
  current instant;
- every operation limit an integer inside its bound
  (`connect_time_ms` ≤ `decision_wall_time_ms` ≤ 20000,
  `request_bytes_per_call` ≤ 16384, `response_bytes_per_call` ≤ 65536,
  `max_calls` ≤ 2, `output_token_capacity` ≤ 8192), and positive integer
  campaign totals;
- remaining campaign capacity for the computed hold of `calls=2` calls,
  `2`× per-call request/response bytes, `output_token_capacity` output
  tokens and `decision_wall_time_ms` wall time;
- no other live inference hold for the scope, and no held M1 session
  fence; symmetrically, a live inference hold refuses session `prepare`.

A successful reserve appends a `reserved_not_sent` record (generation,
revision 1, writer OID, registration provenance, distinct generation and
repair call IDs, computed capacity). Replaying reserve on the same
operation returns the original record idempotently — a second hold under
a different operation is a new generation charged separately.

## Recover and cancel

`recover` is strictly read-only: it returns the durable record for the
exact scope and operation — held or cancelled — or `missing`. It never
allocates, renews consent or revives a cancelled hold.

`cancel` flips exactly that operation's held record to
`cancelled_never_sent` (revision 2), freeing its accounted capacity. A
replayed or duplicated cancel returns the same cancelled record;
cancel on a missing operation is `refused`
(`inference_missing_reservation`), never a fabricated absence.
Cancelling one operation cannot release a newer
different hold. Mission withdrawal blocks *new* reservations while the
durable ledger stays recoverable and cancellable.

## Receipts and outcomes

Every receipt carries the scope, operation, the lifecycle record when one
exists, and `outcome`:

| Outcome | Meaning |
| --- | --- |
| `durable` | A lifecycle record exists and bound to the request identity |
| `missing` | No record exists for the exact scope/operation (recover only) |
| `refused` | A confirmed denial (invalid input, guard refusal, unqualified writer, or a known serialization abort) |
| `unknown` | Outcome unresolved — transport, commit or acknowledgment ambiguity |

`refused` covers both categorical denials and retryable aborts: the
`error=` line printed after the receipt distinguishes them —
`serialization_retry` (SQLSTATE 40001/40P01) rolled back cleanly and the
identical request may be retried, while `inference_*`/input refusals are
categorical. After `unknown` or exit 124, run `recover` on a fresh
connection before any explicit retry: exit 124 means caller stop, not SQL
rollback. Never infer absence, success or rollback from missing output.
The retry is the same identity — the ledger replays its original record.

## Operating procedure

1. `duskweave prepare-operation --engagement E --campaign C` → new UUID.
2. Write the bounded input document; run `m1-inference-budget
   --action reserve` with the Broker credential source.
3. While a hold exists, session `prepare` on the same scope refuses;
   unrelated scopes stay writable. A second inference hold on the same
   scope refuses until the first is cancelled.
4. Any `unknown` or 124 outcome: run `m1-inference-budget
   --action recover` with the same operation on a fresh connection
   before retrying; the same identity replays its original record.
5. To release capacity explicitly, run `m1-inference-budget
   --action cancel` with the original operation.

## Limits

The ledger is durable capacity accounting only. It holds no provider
credentials, no prompt or ContextPack material, no proposal and no
provider response. It grants nothing by itself: no current permission,
dispatch, provider qualification, acquisition or send ever reads true
from a reservation receipt. Capacity arithmetic reserves the declared
envelope; actual provider consumption remains UNVERIFIED until the
inference adapter exists. The ledger does not prevent privileged
administrative corruption — runtime trust boundaries cover ordinary
logins only. The guarded function is the only writer path; direct DML
by runtime roles is denied by grants and row-level security, and
TRUNCATE is blocked by the shared no-truncate trigger.

Evidence for behavior belongs to the candidate's delivery report and CI;
this runbook documents semantics, not a measured claim.

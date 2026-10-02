# RUN_B1B — Publish and Recover Nonpositive Planning History

B1b adds Trajectory recording of an **already durable** B1a decision. It remains
partial M0B: positive eligibility and withdrawal are unfinished. The v1 receipt
labels below describe `version=1` events; new `version=2` events carry the
scoped bounds snapshot and version-aware scope/window labels defined in
[RUN_B2](RUN_B2.md). M0 remains DEMO_PENDING and unsealed; no target execution
is allowed.

## Prerequisites

Apply migrations 0001, 0002 and `0003_trajectory_planning_history.sql`, in order,
using an administrative connection to the owned local PostgreSQL 17 store.
Runtime credentials belong in `DW_DATABASE_URL`, never the command line or a
committed file. The CLI requires a loopback password-bearing restricted login
with the existing `dw_runtime` grants and qualified durability. Preserve fsync,
full_page_writes and synchronous_commit. Runtime history has SELECT/INSERT,
never UPDATE/DELETE/TRUNCATE, ownership, schema CREATE, superuser or BYPASSRLS.

Use the exact original PlanningRequest file and stable operation handle from
[RUN_B1A](RUN_B1A.md). Input remains strict JSON, bounded to 16 KiB. Operator
assertion is metadata under the trusted-local-operator assumption, not authentication.

## Commands

```text
planning-history --operation <original-assessment-uuid> --input <original-request-file> --recover false
planning-history --operation <same-uuid> --input <same-file> --recover true
```

`recover=false` explicitly publishes/reconciles the original durable obligation;
it never creates an assessment. `recover=true` only reads original producer and
consumer records; it neither allocates identities nor inserts history. An absent
producer returns `not_committed`, with no consumer effect. Changed intent under
the same scoped operation fails `integrity_conflict` before consumer access.

## Honest receipts and recovery

A durable JSON receipt retains the original contract, event/operation identities,
request, decision, Mission basis/revision/mode and three producer timestamps.
Its current history view is separately labeled `history_source=trajectory`:

| History | Meaning | complete_history |
| --- | --- | --- |
| completed | Exact validated accepted contract and completion are durable | true |
| pending | Not recorded or required registration predecessor unavailable | false |
| anomaly | Identity conflict; original accepted content remains immutable | false |
| unknown | Consumer state/commit acknowledgment cannot be established | false |

`result=durable` describes the producer decision. Successful query exit is not
history completion or current permission. Every receipt retains
`complete_assessment=false` and `current_permission=false`. Version 1 receipts
show `scope=not_evaluated` and `window=not_evaluated`; version 2 receipts show
the original assessment-time labels in [RUN_B2](RUN_B2.md). The producer-only
`assess` receipt is explicitly historical; its original pending status is not
a current Trajectory query.

After unknown acknowledgment, use `recover=true` on the **same** identity before
explicitly republishing. Read-only recovery never creates the missing effect.
Consumer failures cannot erase the durable producer decision/obligation. Known
serialization aborts are bounded `serialization_retry`, not invisible retries.

## Ownership, predecessors and deployment limits

Mission remains the current authority owner. Trajectory consumes only the
validated immutable contract, never Mission tables or the mutable aggregate.
The local composition obtains that event from the producer recovery port;
neither an event label nor its UUID authenticates an arbitrary external event.

A basis-bearing event needs same-engagement/campaign accepted registration
history matching its registration operation, revision, mode and validity bounds
before first acceptance; version 2 additionally requires an exact scope snapshot
match to that predecessor's goal/included/excluded references. Missing,
mismatched, or unsupported predecessors stay pending. Deliver
registration through existing M0A reconcile before republishing the same decision.
A valid absent-Mission decision has no registration predecessor requirement.
Duplicates/read-only recovery retain the original recorded result without
consulting current Mission or requiring that predecessor again.

The consumer row is its atomic inbox, semantic history and completion record.
Conflicts append bounded identity/category only; no conflicting payload is saved.
Accepted records deduplicate each scoped event and scoped operation identity.
Anomaly markers deduplicate only on the exact conflicting event/operation
pair: a distinct pair sharing one axis is durably retained, and a marker
blocks completion through either axis, even if an earlier accepted row exists.
Producer
publication records are immutable; consumer completion does not mutate them.
Consumer timestamps describe its transaction start and do not replace producer
timestamps or establish commit wall time/current authority.

Tests require `DW_TEST_ADMIN_DATABASE_URL` and `DW_TEST_DATABASE_URL` for the
isolated owned fixture. Reuse build artifacts and the owned server during an
unchanged session, but execute fresh SQL/tests and clean coverage profiles.
Native Windows subprocess tests and Linux CI are separate evidence. No ARM64,
disk-loss, restore/failover, target behavior, full M0B/M0C or milestone seal is claimed.

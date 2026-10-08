# RUN_DATABASE_WAIT_BOUNDS — CLI database wait bounds

## Scope and authority

`DW-FIX-DATABASE-WAIT-BOUNDS` bounds the existing local CLI's PostgreSQL
waits under ADR-005 sections 6–8, the accepted M0 contract section 5 and
QUALITY_BAR. The owner approved this infrastructure hardening on
2026-10-08. It changes no PRD, ADR, invariant or seal, performs no target
action, and qualifies neither an effect-enabled M1 Broker nor its stop
mechanism. The M0 contract's no-timeout statement concerns mission
validity/dwell semantics; this policy bounds database waiting only.

## Fixed policy

| Boundary | Fixed value | Meaning |
| --- | --- | --- |
| Socket connection | 2 s | `Config.connect_timeout` overridden on every CLI connection |
| Connection startup and qualification | 5 s per connection | Independent watchdog spanning connect, authentication, wait-setting verification and `qualify_runtime` |
| Server statement | 3000 ms | Startup `options`, verified effective via `pg_settings` |
| Server lock acquisition | 750 ms | Startup `options`, smaller than the statement bound, verified effective |
| Active CLI execution | 30 s cumulative | Independent watchdog over dispatch, connections, DB operations and cleanup |

A verification or qualification failure refuses before business use as
`unqualified_database_waits`; watchdog setup failure refuses as
`database_wait_guard_failed` — there is no unguarded fallback. The 30-second
clock starts in the caller before the monitor thread exists. Only the live
C0 response-read interval pauses it; pause/resume preserves cumulative
remaining time and never refills the budget.

These are initial engineering limits for the existing local CLI under
normal OS scheduling and timer service — not throughput/latency guarantees,
not a hard real-time bound, and not a compromised-host guarantee. The
invocation envelope catches stalled server responses even when the server
has finished or aborted the query; it is not a claimed network round-trip
bound. The bounds apply to the public CLI commands; they do not cover
library consumers, migration tooling, future services, target workers or
inference, and are not their cancellation implementation. No environment
or argument knob may disable or shorten them.

## Exit status 124 — incomplete invocation, unresolved outcome

A watchdog expiry terminates the CLI process with exit status **124**
without printing a fabricated business receipt and without waiting for
driver or transaction destructors; the monitor never acquires the
stdout/stderr locks.

- Callers must inspect the exit status before interpreting output. Even an
  earlier printed receipt does not establish whole-invocation completion
  when the status is 124.
- 124 means the outcome is unresolved: no automatic retry, and no
  permission, absence or rollback may be inferred from missing output.
- A possibly committed write retains its identity: reconcile the original
  operation on a fresh connection (for example `withdraw --recover true` or
  `reconcile`) before any retry.
- A read bounded this way provides no current authority and no
  authoritative absence.

Process termination closes this client's handles but does not certify that
the server stopped work, rolled back or never committed. Unchanged
classifiers still apply: COMMIT SQLSTATE 40001/40P01 remain confirmed-abort
`serialization_retry`; other commit failures remain `commit_unknown`;
pre-COMMIT query/lock failures keep their existing categories. Server-side
statement/lock cancellation is a confirmed abort of that statement; a
withheld acknowledgment is not the same event.

## Failure and fault evidence

- Synthetic loopback peers that accept but never negotiate, or consume the
  startup packet without authenticating, terminate independently at the
  startup bound with 124 (`tests/database_wait_cli.rs`); a closed endpoint
  still refuses as `connect_failed`. No firewall/routing/backlog
  manipulation is used, and no SYN-blackhole behavior is claimed.
- A held ACCESS EXCLUSIVE lock on `mission.missions` bounds `inspect` to a
  categorical refusal; a test-owned `pg_sleep` insert trigger is canceled at
  the statement bound with zero producer/outbox/history rows
  (`tests/database_wait_postgres.rs`).
- A bounded loopback relay that completes real authentication and
  qualification then withholds business replies is bounded by the active
  watchdog at 124; a relay forwarding a real producer COMMIT but
  withholding its acknowledgment leaves a proven durable effect — durable
  contract/identity asserted before the caller stops — then a fresh direct
  recovery restores the identical contract and history completes once
  (`tests/database_wait_commit.rs`).
- Injected-Instant unit cases prove cumulative remaining time,
  pause/resume without refill, and that a late control cannot suspend an
  expired budget (`tests/faults/database_wait_faults.rs`).

Test-owned relays bind loopback only, bound frame sizes and fixture
lifetime, and never record credentials, startup/auth payloads, backend
keys or raw results. Fixtures that alter cluster roles, locks or
test-owned triggers serialize; the container and retained databases are
unchanged.

## Limits

This watchdog does not certify server-side work stopped, does not roll
back, and does not replace ADR-005 recovery obligations. It does not
harden the CLI against a compromised host, a privileged DBA, or
stdio-external supervision needs; external orchestrators still supervise
the process. The documented bounds require execution evidence on each
claimed platform; a candidate's measured results and CI status belong to
its external delivery report, not this runbook.

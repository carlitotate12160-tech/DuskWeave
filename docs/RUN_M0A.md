# RUN_M0A — Mission Registration and History (DW-IMPLEMENT-M0A)

M0A delivers one bounded vertical path: CLI → application → Mission →
owner-local PostgreSQL Mission/outbox commit → typed Trajectory consumer →
durable history → receipt → fresh-process inspect/reconcile of the same
scoped records. Registration is a declaration only: no planning, execution,
resume, assessment, or withdrawal authority exists.

## Setup (no credentials in this file)

Requirements: Rust 1.94.1 (edition 2024) and an owned, disposable PostgreSQL
17 bound to literal loopback TCP (`127.0.0.1` or `::1`), NoTls.

Environment variables (names only; see `.env.example`):

```text
DW_DATABASE_CONFIG_MODE      file (default, Linux) or explicit env-local (M0)
DW_DATABASE_URL_FILE         absolute protected-file path (file mode only)
DW_DATABASE_URL              runtime DSN (env-local mode only)
DW_TEST_DATABASE_URL         restricted LOGIN test role DSN (tests)
DW_TEST_ADMIN_DATABASE_URL   admin DSN used only for isolated test setup
```

The [database configuration contract](RUN_DATABASE_CONFIG.md) requires explicit
source selection. Windows M0 uses `DW_DATABASE_CONFIG_MODE=env-local` with
`DW_DATABASE_URL_FILE` absent; implicit env-only credentials now reject.
Linux file mode requires `DW_DATABASE_URL` absent, even when empty, and a safe
opened regular file. File mode elsewhere rejects without reading/fallback.
Refused before any connection: invalid/conflicting profile, missing/blank env,
unsafe file, malformed DSN, missing/blank credentials, non-loopback host or any
hostaddr override. Exactly one literal TCP host is permitted, `127.0.0.1` or
`::1`; no hostname, socket or multi-host alternatives. No fallback DSN, no
pgpass, no password CLI flags, no dotenv loader or TLS downgrade.

Apply the migration with the admin role; grant the runtime login role
membership in the migration-created NOLOGIN role `dw_runtime`:

```bash
psql "$DW_TEST_ADMIN_DATABASE_URL" -f migrations/0001_mission_registration.sql
# then, as admin: CREATE ROLE <rt> LOGIN PASSWORD '...'; GRANT dw_runtime TO <rt>;
```

For the current B1a-enabled CLI, also apply
`migrations/0002_planning_assessment.sql` after 0001. See [RUN_B1A](RUN_B1A.md)
for the assessment command and its permanently pending history in that slice.

## Commands

```text
prepare-operation --engagement <uuid> --campaign <uuid>
register --operation <uuid> --input <file>
inspect --engagement <uuid> --campaign <uuid>
reconcile --engagement <uuid> --campaign <uuid> --operation <uuid>
```

All declared flags are required exactly once; unknown, duplicate, or
incomplete flags are rejected before any database connection. Input files
are read bounded (16 KiB + 1 byte) and rejected bytes are never echoed or
persisted. At M0A delivery, `assess` and `withdraw` were unsupported. B1a adds
`assess`; `withdraw` remains unsupported.

## Wiring

```text
main.rs (CLI) → src/input.rs (boundary parse, bounded read)
  → src/registration.rs (application; narrow ports only)
  → src/mission.rs (Mission owner; private state, validated construction)
  → src/postgres_mission.rs (Mission+outbox one SERIALIZABLE tx; allocator)
  → src/trajectory.rs + src/postgres_trajectory.rs (consumer; history tx)
```

Every runtime connection is qualified (`qualify_runtime`) before business
use: durability settings on, no superuser/BYPASSRLS/ownership, no
CREATE/UPDATE/DELETE/TRUNCATE on `mission`/`trajectory` objects. Admin stays
outside the business CLI.

## Receipts

```text
register result=accepted engagement=<uuid> campaign=<uuid> operation=<uuid> event=<uuid> history=<completed|pending|anomaly>
register result=unknown  operation=<uuid> action=reconcile_before_retry
```

A commit whose acknowledgment is lost is UNKNOWN, not a proven rollback.
Reconcile the stable operation identity before any retry; verified absence
permits an explicit unchanged-intent retry with the same operation.

## Scenario matrix (all covered by tests)

| Scenario | Where |
|---|---|
| Real CLI register → durable Mission/outbox → history; fresh-process inspect | `tests/registration_cli.rs` |
| Rollback (explicit + conflicting commit leaves nothing) | `tests/registration_recovery.rs` |
| Same-intent dedup; conflicting identity; occupied-campaign refusal | `tests/registration_postgres.rs` |
| Producer unknown outcome with nothing committed → verified-absence retry; delivery attempt failure before consumer commit → one effect | `tests/registration_recovery.rs` |
| Producer/consumer ACK lost after real commit → reconcile Committed, original event identity, one effect | `tests/registration_commit_ack.rs` |
| Pending delivery; original-ID recovery/redelivery | `tests/registration_recovery.rs` |
| Nil engagement/campaign/operation/event rejected at owner boundary | `tests/mission_registration.rs` |
| Deterministic concurrency, single winner, visible refusal | `tests/registration_postgres.rs` |
| Wrong scope/source/version/revision; invalid construction; unsupported commands | `tests/mission_registration.rs`, `tests/registration_cli.rs`, `tests/registration_postgres.rs` |
| Sensitive sentinel; strict/bounded input; no rejected-byte persistence/echo | `tests/mission_registration.rs`, `tests/registration_cli.rs` |
| Missing/bad env, missing password, non-loopback host | `tests/registration_cli.rs` |
| Prohibited role behavior; durability settings; runtime mutation denied | `tests/registration_permissions.rs` |
| Anomaly persists; original redelivery cannot clear it | `tests/registration_postgres.rs` |

## Budgets

- Runtime diff cap: one-shot exception `M0A_RUNTIME_DIFF_EXCEPTION`
  (baseline `a9da047b2bf0bf4822536187ab3e1734ac56fc12` — protected master at
  M0A delivery, repointed from `f52cfcc` after docs-only PR #6; cap 1200,
  applies once to this initial M0A delivery only) recorded in
  `QUALITY_BAR.md`; enforced by `scripts/check_runtime_budget.py`.
- Every source/test file ≤ 400 physical lines; McCabe ≤ 7 per function.

## Coverage

CI measures line coverage for M0A production Rust code (`src/**/*.rs` only)
by running the complete suite once under `cargo llvm-cov` (pinned 0.9.1,
digest-verified musl build) on the Linux Rust job against the same real
PostgreSQL 17 setup. Fixed floors: **90% total production lines** and
**80% lines per reported production source file**, enforced by
`tests/check_coverage_gate.py` on the run's JSON report; the JSON plus a
text summary are uploaded as the `m0a-coverage` artifact (14-day
retention). Files outside `src/` in the report fail the gate; a
production source absent from the report fails unless it is
declarations-only. Line coverage is not branch coverage and does not
establish correctness; the scenario matrix assertions remain mandatory.
Windows/MSVC coverage is unverified — the gate evidence is Linux-only.

Local equivalent (requires a disposable PG17 and `cargo-llvm-cov`):

```bash
cargo llvm-cov clean --workspace
DW_TEST_DATABASE_URL=... DW_TEST_ADMIN_DATABASE_URL=... \
  cargo llvm-cov --locked --all-targets --json --output-path target/llvm-cov/m0a-coverage.json
python3 tests/check_coverage_gate.py target/llvm-cov/m0a-coverage.json
python3 tests/check_coverage_gate.py --selftest
```

## Local verification

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
DW_TEST_DATABASE_URL=... DW_TEST_ADMIN_DATABASE_URL=... cargo test --locked --all-targets
cargo audit
python scripts/check_links.py
python scripts/check_structure.py
python -m unittest discover -s tests -p test_runtime_budget.py
python tests/check_coverage_gate.py --selftest
python scripts/check_runtime_budget.py --base <expected-base-sha> --head INDEX
git diff --check && git diff --cached --check
```

## Known limitations (by design for this slice)

- Mission metadata only; **DEMO_PENDING** for real-client operational data.
- No correction/reactivation mechanism; anomalies block the obligation.
- No global recovery scan; reconcile is bounded to supplied scope+operation.
- Constructed test controls are not client success, stealth, compromise, or
  proof evidence.

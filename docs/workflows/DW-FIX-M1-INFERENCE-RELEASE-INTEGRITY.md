# DW-FIX-M1-INFERENCE-RELEASE-INTEGRITY — packet/evidence record

| Field | Value |
| --- | --- |
| Mode | FIX |
| Base | `618c885346ccf5e5b645bb63aee51b9e827be748` (`origin/master`, PR #116 merged) |
| Branch | `fix/m1-release-integrity` |
| Fixture | `dw_native_release_integrity_20261025a` on owned `dwpg-r3` (PostgreSQL 17.11, 127.0.0.1:5432), one fixture |
| Report root | `D:/DuskWeave-build/rust-1.94.1/reports/release-integrity/20261025a` (`resumed-1` for executed actions) |

## Findings

- **#76 VALID (narrower form)**: `PgBroker::release_into` used blocking
  `pg_advisory_lock` and checked the shared deadline only before that wait.
  The qualified CLI already applies `statement_timeout=3000`, `lock_timeout=750`
  and independent watchdogs via `database_wait::connect_owner`; no infinite CLI
  wait is claimed.
- **#101 VALID**: in both `DeepSeekProvider::call` and
  `AutonomousProvider::exchange_owned`, `worker.join().map_err(...)?` returned
  before the saved release error was examined.

## Changes

- `src/m1_reasoning/postgres.rs`: single `pg_try_advisory_lock` (unchanged
  key/session ownership); `false` -> `NeverReleased("current_authority_unavailable")`
  with no cleanup on the un-acquired lock; preserved early deadline check and
  post-validation `shared_deadline` recheck immediately before `Provider::call`,
  releasing the fence through the existing `pg_advisory_unlock_all` cleanup and
  returning `NeverReleased("inference_deadline")` on expiry.
- `src/m1_reasoning/transport.rs` (both paths): `worker.join()` collected
  unconditionally; saved release error leads; join failure maps to
  `provider_transport_unknown` only when release succeeded. Existing
  cancellation action on release failure preserved.
- Tests: reworked `direct_stop_wins...` (bounded completion channel, holder
  committed before worker join, durable withdrawal assertion retained); added
  held-fence contention refusal (zero provider calls, no leaked session lock,
  `never_released` -> `failed_never_released_reserved` through restricted-owner
  inspect); added deadline-expiry-during-validation refusal (re-acquirable
  fence afterward); added worker-panic + failing/successful release precedence
  cases for both concrete transports; reworked `m1_terrain_context_postgres.rs`
  block 3 only (prompt refusal under held anomaly fence, `NeverReleased(
  "current_authority_unavailable")`, durable anomaly + `reference_ineligible`
  premise checks; `wait_for_advisory_waiter_or_done` retained for blocks 1-2).

## RED -> GREEN (same fixture custody, `--test-threads=1`)

- RED `m1_native_release`: 3 new/reworked tests failed on blocking
  implementation (`exit 101`).
- RED `m1_terrain_context_postgres release_fence_...`: failed (`exit 101`).
- RED `--lib m1_reasoning::transport`: both panic-precedence cases failed with
  `provider_transport_unknown` returned instead of `inference_release_unknown`.
- GREEN `m1_native_release`: 4/4. GREEN `m1_terrain_context_postgres
  release_fence_...` (all original blocks): 1/1. GREEN `m1_native_continuation`:
  4/4. GREEN `--lib m1_reasoning::transport`: 41/41.

## Fixture provisioning recovery (failed attempt, not a FIX cycle)

The first `--session` bootstrap exited on stdin EOF after printing `ready`,
leaving `dw_native_release_integrity_20261025a` provisioned but its
process-held credentials unrecoverable. Owner-authorized exception: dropped
only that disposable database and its 15 proven roles (admin suffix
`dc8d41360af6` via `datdba`; negative set `e6f10a64db7b` via in-DB ACLs and
column privileges — no wildcard, no other fixture touched), verified absent,
then re-provisioned the identical name through unchanged
`m1_native_bootstrap.py` with stdin held open for multi-action custody.
Original report directory preserved; executed evidence lives in `resumed-1`.

## Checks

- `cargo fmt --all -- --check`: pass after `cargo fmt --all`.
- `cargo clippy --locked --all-targets --features fixture-supervisor --
  -D warnings`: pass.
- `check_complexity.py --base 618c885` (rust-code-analysis-cli 0.0.25): Gate
  PASS; `release_into` own=7 cap=7 compliant, `exchange_owned` own=1, zero
  regressions, 10 retained debt items unchanged; advisory REVIEW_TRIGGER
  `release_into` span 56>50 retained for cohesive release fencing.
- `check_structure.py`, `check_links.py`: pass (advisory file-size WARNs only).
- `check_runtime_budget.py --base ... --head HEAD`: 0 committed lines at
  measurement; worktree diff is +14 net production lines (within the 10-30
  estimate) plus focused test assertions.
- Affected instrumentation: `cargo llvm-cov` on `--lib`,
  `--test m1_native_release`, `--test m1_terrain_context_postgres` into
  `D:/DuskWeave-build/rust-1.94.1/coverage/release-integrity`, JSON at
  `resumed-1/affected-coverage.json`. The required 90% total / 80% per-file
  gate remains the Linux full-suite CI check on the candidate.

## LOC before/after (physical)

postgres.rs 545 -> 560; transport.rs 530 -> 533; m1_native_release.rs 215 ->
348; m1_native_transport.rs 504 -> 564; m1_autonomous_transport.rs 1078 ->
1125; m1_terrain_context_postgres.rs 1466 -> 1475. Growth is only the
fix/assertion corrections; retained size debt not repaid.

## Review

Author self-review (QUALITY_BAR §7) and one distinct adversarial review are
recorded in the PR body; findings and dispositions are tracked there.

## Deletion disposition (owner-local)

No whole-file deletion assigned or performed. `m1_native_live.py`,
`m1_terrain_live.py`, `m1_schema_validate.py` retain documented consumers;
historical migrations/evidence stay. Nothing retired.

## Out of scope

No live provider/target/resolver traffic, no migration/dependency/profile/CI
change, no seal or M1 completion claim. #76/#101 remain open until delivered
scope is verified after merge.

# DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON

MODE: IMPLEMENT
PROJECT: DuskWeave
WORKSPACE: `D:/DuskWeave-ci-complexity`
BRANCH: `feat/m1-autonomous-skeleton`
BASE: `00e5667606fbc549aa5593997d9f88316ebc62e1`

This records the bounded owner-directed execution resumed after PRs #72, #71
and #73 merged, including the exact implementation/consumer file map established
during execution. It does not grant new architecture or target permission.

## Outcome and authority

Prove one autonomous orchestration episode through real application supervision,
Mission/Broker admission, Cognition, Terrain and Trajectory: one start produces
Decision 1, one explicitly isolated fixture DNS result, a new pack consumed by
Decision 2 and durable history/inspect, without intermediate operator input.
This supports objective-led authorized validation and evidence-led adaptation;
APT41, Lazarus and Volt Typhoon remain behavioral lenses, not runtime playbooks.

Governing authority: [PRD-000](../prd/PRD-000-product-thesis.md),
[ADR-004](../adr/ADR-004-rust-core-language.md),
[ADR-005](../adr/ADR-005-postgres-system-of-record.md), accepted
[M1 section 7](../contracts/M1-enabling-architecture.md#7-proposed-autonomous-run-amendment-2026-10-19)
and [ADR-016 section 4a](../adr/ADR-016-execution-broker.md#4a-proposed-bounded-run-supervision-2026-10-19)
at exact text `0615fb72c946fd6abf0823692be621096091377c`,
[roadmap sections 1.2–1.4](../MVP_AND_DEFERRED_SCOPE.md#12-autonomous-campaign-delivery-order-owner-direction-2026-10-19),
[QUALITY_BAR](../../QUALITY_BAR.md) and build-order packets 00/07.
Foundation/M0 seals and all historical qualification evidence remain unchanged.

## Resources and custody

Verify identity/HEAD/worktrees before resuming; preserve unrelated dirty worktrees
and never access or propagate local Cargo configuration. Git owns candidate custody.
The editing mirror is `/workspace/scratch/a764b1824bf0/skeleton-work`; Windows is
the qualified runtime platform, Rust 1.94.1 x86_64 MSVC. One verification owner uses
ordinary artifacts `D:/DuskWeave-build/rust-1.94.1/normal`, instrumented artifacts
`D:/DuskWeave-build/rust-1.94.1/coverage/skeleton`, and reports
`D:/DuskWeave-build/rust-1.94.1/reports/skeleton-c`, process-scoped only.

Reuse owned `dwpg-r3`, PostgreSQL 17 image digest
`sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f`.
Initial iterations retain `dw_native_skeleton_20261020a`/`20261020b`; final fresh
migration qualification uses `dw_native_skeleton_20261020c`. Restricted generated
runtime logins perform assertions; admin is fixture setup/fault injection only.
No automatic service/database/role deletion; no provider credential or external egress.

## Exact editable map
- `.github/workflows/doc-check.yml`
- `docs/BUILD_ORDER.md`
- `docs/ENGINEERING_STATE.md`
- `docs/MVP_AND_DEFERRED_SCOPE.md`
- `docs/RUN_M1_AUTONOMOUS_FIXTURE.md`
- `docs/workflows/DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON.md`
- `migrations/0016_autonomous_inference_profile.sql`
- `migrations/0017_autonomous_fixture_run.sql`
- `migrations/0018_issued_fixture_terrain.sql`
- `scripts/check_dependencies.py`
- `src/database_wait.rs`
- `src/lib.rs`
- `src/m1_autonomous_context.rs`
- `src/m1_cli.rs`
- `src/m1_context.rs`
- `src/m1_fixture_dns.rs`
- `src/m1_inference_budget.rs`
- `src/m1_inference_permission.rs`
- `src/m1_reasoning/fixture.rs`
- `src/m1_reasoning/mod.rs`
- `src/m1_reasoning/request.rs`
- `src/m1_reasoning/request_autonomous.rs`
- `src/m1_reasoning_cli.rs`
- `src/m1_run.rs`
- `src/m1_run_cli.rs`
- `src/m1_run_effect.rs`
- `src/m1_run_inference.rs`
- `src/m1_run_supervision.rs`
- `src/m1_terrain_cli.rs`
- `src/m1_terrain_input.rs`
- `src/m1_terrain_issued.rs`
- `src/m1_terrain_view.rs`
- `src/main.rs`
- `src/postgres_m1_run.rs`
- `src/postgres_m1_terrain.rs`
- `tests/m1_autonomous_permission.rs`
- `tests/m1_autonomous_wire.rs`
- `tests/m1_run.rs`
- `tests/m1_run_cli.rs`
- `tests/m1_run_failures.rs`
- `tests/m1_run_postgres.rs`
- `tests/support/m1_native_bootstrap.py`
- `tests/support/m1_run.rs`
- `tests/test_dependencies.py`
- `tests/test_m1_native_helpers.py`

Read-only: accepted PRDs/ADRs/contracts, AGENTS, QUALITY_BAR, old migrations,
legacy command consumers and repository skills. No existing acceptance/seal edit.
Tracking disposition: UPDATE ENGINEERING_STATE and BUILD_ORDER to merged
predecessors/current implementation; update the existing MVP roadmap for touched
debt and explicit remaining work. The runtime guide is operational documentation,
not another tracking/evidence ledger.

## Decisions and observable acceptance

- Run only `autonomous-dns-fixture-v1` / `controlled_fixture`; fresh v3 consent,
  all nine disclosure classes and full two-decision capacity before work starts.
  Legacy v1/v2 serialized shape/limits and operational commands remain compatible.
- Foreground supervisor launches the same release's fixed child phases with a
  fixed infrastructure environment. Independent reaper, persisted short lease,
  150 s run / 50 s inference phase / 60 s acquisition phase, clamped to remainder.
  Generation/repair share 40 s active, 45 s reserved; 20 s per call. These are policy
  ceilings, not calibrated latency promises. Child failure/parent loss is UNKNOWN;
  recovery never resumes execution or creates another provider/DNS call.
- Bind immutable P/C/B and complete UTF-8 escaped generation/repair before reserve:
  pack ≤8192 B; configuration/request each ≤16384 B; fixed typed references.
  Preserve selected seed and current usable Terrain ahead of optional candidates.
  This producer omits the optional hypothesis summary with `not_required`;
  it never invents or accepts an alternate summary as committed history.
- Broker reserves the single fixture attempt, produces a result with an explicit
  source/mode/release and durable intake obligation before ACK. Terrain independently
  checks the versioned issued reference, admitted scope/source/time, reconciles and
  commits atomically; Trajectory records required history before Decision 2.
  Deterministic snapshots retain the Broker-issued perceived time; no worker-clock
  restamping or timestamp replacement is used to manufacture freshness.
- Decision 2 must cite the actual first committed decision and new qualified fixture
  Terrain. Its local labels resolve through the freshly fitted immutable pack.
  The deterministic provider is a plumbing double; it proves no LLM cognition.
- Duplicate start and repeated fresh recovery preserve original identities and
  effect counts; conflicting inputs reject. Withdrawal before release stops;
  lease loss cannot renew; child failure persists UNKNOWN. A physical post-COMMIT
  ACK-loss test asserts durable result before recovery and zero replay afterward.
- Repay Terrain record_dns/load_existing by cohesive transaction/decoding refactor,
  preserving old SQL authority, conflicts and ACK-unknown behavior. New/worsened
  Rust functions ≤7; retain untouched debt explicitly. Check owner dependencies
  with actual baseline plus alias/re-export/qualified-path negative controls.

## Validation and boundary

Focused TDD and real restricted PostgreSQL/CLI scenarios precede final fmt,
Clippy, full all-targets coverage (90% total / 80% each production file), pinned
McCabe ratchet against BASE, dependency/structure/link/hygiene checks and final
current-head CI. Serialize cluster-role fixtures; reuse compilation, not test PASS.
A full coverage run replaces an identical standalone all-targets rerun.

Apply canonical QUALITY_BAR §7 author self-review and a distinct adversarial
review; the author cannot claim independence. Record actual commands/platform,
results, remaining proof gaps and advisory cohesion dispositions in the PR.
No automatic merge, next provider slice, target operation or M1 seal.

STOP for contradictory accepted authority, unsealed required dependency, violated
invariant or material scope expansion, naming the smallest missing resolution.
Routine defects and advisory LOC crossings are corrected/dispositioned in this PR.

## Author review disposition

QUALITY_BAR §7: the supervisor composes narrow owner ports; run bookkeeping does
not own operational-model truth. Domain/contract modules have a CI dependency
check with negative controls. Legacy inference-basis construction is reused,
existing APIs retain consumers, and new production symbols are reachable from
m1-run through fixed children to owner persistence and inspect/history. Restricted
SQL source binding and current authority gate both decisions/effect release;
physical result ACK loss, withdrawal, child failure, hard parent loss, lease
expiry and independent reaping have focused scenario evidence. This is author
self-review; distinct review and final exact-head gates remain required.

Advisory cohesion: the cumulative runtime diff includes three new migrations and
one complete vertical behavior, rather than disconnected precursor PRs. Retain
`m1_context.rs` (537 lines) for pure fitting/selection, `m1_inference_permission.rs`
(451) for one versioned disclosure contract, existing `m1_terrain_input.rs` (465)
for strict intake and `m1_terrain_view.rs` (595) for pure Terrain view/dependency
validation. The new bounded feedback projection/request template and application
supervision/effect/inference composition are separate responsibilities. Migrations
0017 (423) and 0018 (452) retain atomic run/Broker lifecycle and Terrain admission
respectively; old migrations remain immutable. PostgreSQL Terrain orchestration
is refactored without moving its existing acceptance authority merely to lower
McCabe. Untouched large suites and old named hotspots remain explicit debt, not
closed by allowances. No public CLI/M0 removal lacks a tested compatibility replacement.

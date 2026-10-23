# DW-FIX-M1-RUN-DIAGNOSTICS — IDE execution packet

PROJECT: DuskWeave
MODE: FIX
WORKSPACE: D:/DuskWeave-ci-complexity
BRANCH: fix/m1-run-diagnostics
BASE: b439661f82c4cebf1d615629f07eb176dd3a53e8

The issued instructions below are translated into English without changing their accepted outcome. Later bounded amendments govern their explicit deltas. Explain provider-decision failures from a fresh receipt/inspect process without sensitive output, replay, a generic supervisor, or weaker safeguards. This supports the MVP objective → hypothesis → authorized action → evidence → adaptation loop. APT41, Lazarus and Volt Typhoon are behavioral references, not playbooks or execution permission.

## 1. Preflight and authority

Read AGENTS, ENGINEERING_STATE, BUILD_ORDER/00/07, QUALITY_BAR section 7, build-duskweave, ADR-011/016 and the relevant M1 contracts. The section 7 / section 4a acceptance at `0615fb72` remains unchanged. Verify worktrees, identity, branch, HEAD, status and origin; reconcile changed authority/source before proceeding. Preserve unrelated worktrees and do not work on master.

Requalify and reuse `dwpg-r3`: PostgreSQL 17, image `postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f`, loopback binding. Bootstrap fresh `dw_native_run_diagnostics_20261021a` with unique restricted logins. Refuse collisions; use a fresh helper-generated `dw_native_` name when necessary. Preserve existing databases, cluster-wide passwords, roles, container, volume, `.env` and global environment.

Use Rust 1.94.1 MSVC and ordinary target `D:/DuskWeave-build/rust-1.94.1/normal`, owned by one verification process across builds/tests/children. Reports: `D:/DuskWeave-build/rust-1.94.1/reports/m1-run-diagnostics-20261021a`. If local coverage is required, use the separate `D:/DuskWeave-build/rust-1.94.1/coverage-m1-run-diagnostics-20261021a` target. No cargo clean; never read/copy/edit `.cargo/config.toml`, dump environment, or print keys/DSNs.

All work is offline. Remove `DEEPSEEK_API_KEY` from bootstrap/test/probe environments and descendants before launch. No provider contact, target DNS, paid retry or restart of old runs.

## 2. Evidence and limits

- Original episode `c9c2d22d-945a-409c-95bc-9f3e6d683616`: started→claimed 1280 ms, claimed→UNKNOWN 4751 ms, total 6031 ms. These are parent recording times, not a claim-query benchmark.
- Retained launch configuration contains all DSNs; missing-env is not proved for that episode.
- A key-free probe of the same child binary exited 124 at 5148 ms without READY. Multiple watchdogs can produce that code; it does not identify the winner.
- Diagnostic copy `dw_diag_20261021a` has one completed call and one decision, terminal `owner_view_requested`, zero fixture DNS and zero second decisions. It does not prove the full loop.
- Confirmed defects: read_ready discarded terminal output and finished flattened unsuccessful exits. Preserve historical evidence.

## 3. Issued editable map

Production:

- src/database_config.rs
- src/m1_run.rs
- src/m1_run_cli.rs
- src/m1_run_supervision.rs
- src/postgres_m1_run.rs
- migrations/0020_m1_run_diagnostics.sql (new)

Tests/support:

- tests/m1_run_cli.rs
- tests/m1_run_diagnostics.rs (new cohesive diagnostic scenarios)
- tests/m1_provider_fixture.rs (affected assertions/regressions only)
- tests/support/m1_run.rs
- tests/support/m1_native_bootstrap.py
- tests/test_m1_native_helpers.py

Packet/tracking/operator:

- docs/workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md (issued instructions and concise evidence)
- docs/ENGINEERING_STATE.md
- docs/MVP_AND_DEFERRED_SCOPE.md
- docs/RUN_M1_AUTONOMOUS_FIXTURE.md

Read-only: main.rs, database_wait.rs, m1_run_control.rs, m1_run_inference.rs, m1_inference_control.rs, m1_reasoning_cli.rs, transport/transport_control, ContextPack/proposal/Terrain producers and consumers, migrations 0001–0019, CI, accepted PRD/ADR/contracts, tests/support/database_wait_proxy.rs, tests/m1_run_postgres.rs and tests/m1_run_failures.rs. State a concrete need before expanding the map.

## 4. Implementation decisions

### A. Configuration preflight

After request validation and before store.start/hash/spawn, parse required core, Broker, Cognition, Trajectory and Terrain configuration through the existing env-local/file selection. Do not require URL environment variables for file deployments, read both sources, add fallback, or open additional preflight connections. Missing/invalid/conflicting configuration yields a closed category/owner label without values.

Check Windows SystemRoot and provider-profile control-root configuration. Actual root qualification remains at its current boundary. Preflight does not replace connection/role qualification or current-use authority checks. Start must not require an API key: preserve durable offline live_key_missing. Inspect/recover/stop must not require the key or every start configuration. Preserve argument-validation precedence.

### B. Existing stdout pipe and safe terminal errors

Successful provider children must emit exact READY first. Never skip prelude, log, stage-marker, receipt or incorrect frames to find READY. Distinguish matching READY, pre-READY terminal error (failure), permitted post-READY terminal error/EOF, and refused invalid/oversized/late/missing frames.

Bound each frame to 128 bytes including LF and retain the existing total drain bound. Explicit error allowlist: missing_env, invalid_database_config_mode, database_config_conflict, missing_database_url_file, database_file_platform_unqualified, invalid_dsn, non_loopback_host, missing_credentials, connect_failed, unqualified_database_waits, run_control_unqualified, run_control_stopped, run_child_refused, inference_authority_unavailable, run_inference_pending, child_failed. Map unknown values to child_failed without retaining/reflecting the original. Regex shape alone is not evidence that a value is safe.

Reuse the reader/watchdog; add no diagnostic service/thread. Discard stderr and do not retain full stdout, panic/SQL text, provider content or private reasoning. Keep observed exit status, child-reported category and parent-observed cause distinct. Exit 124 without a cause marker is not preparation_timeout. Parent-observed readiness/phase deadline or local stop may be reported as that observation, without claiming which child watchdog fired.

Settle exit/reader races within the original remaining deadline. No unbounded join or new grace clock; unavailable information stays unavailable. Pipe/diagnostic failure never grants READY or release.

### C. Restricted immutable diagnostics

Publish one final append-only/idempotent record per (run_id, phase) through PgRunStore and forward migration 0020. The internal `m1_run_phase_diagnostic_v1` payload contains run_id; existing phase enum; cause (child_error, child_exit, readiness_refused, readiness_deadline, phase_deadline, local_stop, supervisor_unavailable, unavailable); cause_source (child_report, parent_observed, unavailable); allowlisted child_error or null; signed 32-bit exit_code or null; boolean readiness_observed; elapsed_ms (parent time since launch, 0..150000 or null). Receipt publication disposition is committed, pending_or_unknown or unavailable. Preserve Windows exception codes outside 0..255.

Maximum payload: 1024 UTF-8 bytes with no free text. Child categories are reports, not authority facts. Final cause describes observation, not an unmeasured root cause. SQL checks owning Broker/run/started phase, fixed search_path, closed enums, bounds and unknown-key rejection. No direct-write grants, foreign scope, record replacement or role expansion. Identical publication is idempotent; conflicting publication refuses.

Post-expiry diagnosis records existing obligations only; it cannot alter admission, charges, phase state, history or effect permission. Publish after child termination using the existing wait/run budget. No SQL/logging in reaper/callback/watchdog. Publication failure cannot delay STOP/reap or convert failure to success.

Extend summary/inspect optionally without changing existing field meanings. Old runs report unavailable; no invented backfill. Preserve UNKNOWN where effects/commit are uncertain. Recovery is bounded database-only work, without relaunch, inference or effect replay. Bootstrap includes 0020 (18 initial / 9 refresh migrations); refresh must not replace the diagnostic summary with the old producer. Use a narrow new function instead of copying an old migration.

### D. Preserve independent failure owners

No supervisor framework, scheduler, event bus, global pool, tracing subsystem, async conversion or thread per sample. Retain independent parent reaper, readiness guard, local STOP observer, child preparation cover, authority coordinator/transport permit, parent EOF detection and original decision envelope. Record their lifetimes, blocking work and failure responsibilities here.

Do not replace PowerShell ACL checks, remove child hashing, share connections across roles/threads, delete the second prepared_basis or increase deadlines in this FIX. The two TerrainBasis instances have distinct consumers; naive deduplication can couple authority checks.

Measure launch→claim→READY/exit and parent total separately. Missing READY is censored, not internal-stage timing. Warm samples are not cold benchmarks; provider duration does not establish exact overhead or startup below 500 ms. Record concrete profiling gaps without adding production debug modes.

## 5. Acceptance and verification

Use a genuine failing assertion for lost diagnostic behavior; do not fabricate production faults. Synthetic parser/child controls complement actual m1-run configuration/SQL tests. Required assertions:

1. Missing/malformed/conflicting core/history/Terrain config gives safe owner/category with zero new runs/reservations/calls/effects. Missing Windows SystemRoot fails before SQL; invalid arguments retain invalid_args precedence.
2. Fresh child failure exposes allowlisted terminal category in receipt/fresh inspect when SQL is available. Reader/exit races preserve evidence before and after READY.
3. Prelude, wrong identity/phase, padding, CRLF, invalid UTF-8, oversized/truncated/late frames and EOF refuse READY. Unknown-code secret sentinel never reaches output or storage.
4. Exit 124 alone is not claimed as preparation_timeout. Parent deadline facts remain distinct. Blocked pipe/SQL and stop tests preserve independent reap and callback purity.
5. Restricted SQL rejects foreign writers, wrong run/phase, unknown keys/codes, oversize and conflicting duplicates. Identical publication preserves history, state and charges.
6. Unavailable diagnostic SQL follows child stop with honest pending_or_unknown/unavailable and no recovery replay.
7. Keep the two-decision deterministic skeleton through real CLI, Broker, Terrain and history. Missing-key provider fixture remains durable live_key_missing without fallback; fixtures do not prove real cognition.
8. owner_view_requested remains distinct from completed; no forced DNS choice or operator continuation.

Run fmt; Python native-helper unit tests; m1_run_cli; then binary, m1_run_diagnostics, m1_provider_fixture, m1_run_postgres and m1_run_failures serially in one fresh restricted fixture (--test-threads=1). Use --locked and no key. Run Clippy --all-targets -- -D warnings, links, structure and the verified analyzer 0.0.25 McCabe ratchet against the issued base. No global tool updates or cached PASS for unavailable tools. Reuse fixture refresh only as designed, never a demo DB.

CI runs full tests/coverage and the Linux reservation-stall regression. Preserve 90% total / 80% per-production-file floors, the ratchet and all failure assertions. Missing legacy local CREATEROLE qualification is not Windows full-suite PASS. Do not duplicate a full run without a concrete reason or parallelize role-mutating fixtures.

## 6. Tracking, review and STOP

Update ENGINEERING_STATE with actual FIX status/next action, roadmap section 1.4 with touched debt disposition, this packet with candidate-bound RED/GREEN/checks/uncertainty, and the operator guide with configuration/diagnostic/UNKNOWN/recovery meanings. Never claim this candidate merged/sealed. ACL latency, owner connections, internal preparation timing and owner-view continuation remain OPEN unless verified. Reference historical evidence instead of duplicating full reports.

Use QUALITY_BAR section 7, architecture before style. LOC/diff/span are advisory; McCabe is enforced. No threshold changes, helper fragmentation, or consumer removal without replacement. Perform author self-check and the assigned distinct review honestly, fix valid findings in the same PR, validate the final candidate, and submit without auto-merge. The owner need not adjudicate every code detail.

STOP for unresolved authority/base drift, required out-of-map dependency, invariant violation or a blocker remaining after two FIX cycles. Routine defects stay in scope. Bring measured preparation failure to architecture review; do not raise timers or replace fixtures to force GREEN.

Completion proves entrypoint→child observation→restricted immutable diagnosis→fresh inspection with safeguards, skeleton and required checks intact. It does not prove the historical root cause, real provider loop, target authority or M1 completion.

## 7. Implementation evidence (2026-10-21)

Verified repository: `carlitotate12160-tech/DuskWeave`, actual worktree
`D:/DuskWeave-ci-complexity`, branch `fix/m1-run-diagnostics`, issued base
`b439661f82c4cebf1d615629f07eb176dd3a53e8`. GitHub master matched that base at
preflight. The current committed head, PR state and final CI come from Git/GitHub;
this candidate does not contain its own head SHA or claim its own merge.

Windows x86_64 MSVC / Rust 1.94.1; owned `dwpg-r3` retained, pinned image and
127.0.0.1:5432 binding requalified, PostgreSQL 17.11. No competing cargo/rustc/
duskweave verification owner was running at preflight. Ordinary target is the
issued `D:/DuskWeave-build/rust-1.94.1/normal`, held by this execution across
builds and native subprocess tests. No local coverage was run. Reports are under
`D:/DuskWeave-build/rust-1.94.1/reports/m1-run-diagnostics-20261021a`.

The first bootstrap retained `dw_native_run_diagnostics_20261021a` after a new
0020 function syntax error. This was setup failure, not product RED. Credentials
were not recovered. After correction, the helper generated the fresh fixture
`dw_native_fixture_ebc5531ddb0afa40`; its restricted credentials remain only in
the live bootstrap and child environments. Initial migration count is 18 and
refresh count 9. `DEEPSEEK_API_KEY` was removed explicitly before bootstrap,
tests and probes, and the bootstrap removes it again. No provider/target contact
or historical run restart was performed. All retained databases/roles survive.

**RED:** before production edits, the added assertion
`m1_run_supervision::tests::child_terminal_error_before_ready_preserves_the_safe_code`
ran through `cargo test --locked --bin duskweave` and failed because the baseline
returned `run_readiness_refused` for `error=missing_env` instead of preserving
the admitted child code. Compilation/setup succeeded; one test failed at the
behavior assertion. The same assertion then passed after the reader fix.

**Wiring:** `main` existing dispatch → `m1_run_cli::start` → parse-only owner
preflight → existing run admission → `PhaseChild` reaper/readiness observation →
stop/reap and original-deadline-bounded terminal drain → `PgRunStore` → restricted
`execution.run_phase_diagnostic` → existing summary plus narrow diagnostic
projection → fresh `m1-run-inspect`. No production command/export exists only
for tests. Inspect/recover/stop retain the existing Broker-only composition;
recovery does not publish guessed diagnostics or restart children.

**Byte and trust boundaries:** frames are at most 128 bytes including LF and
the entire reader consumes at most the existing 16384-byte drain ceiling.
Successful readiness still requires the exact first frame. One terminal error
is admitted before or after READY; extra/malformed frames refuse. Unknown codes
become `child_failed` at the pipe boundary. Only the closed 9-field record is
retained; the SQL boundary rejects unknown keys/codes and payloads over 1024
UTF-8 bytes. SQL checks signed 32-bit status and elapsed 0..150000/null, writer
identity and an existing started phase. The only runtime write route is the
security-definer publication function with fixed search_path. Identical retry
is idempotent; conflicting publication and direct mutation refuse. Separate
trust-boundary validation is deliberate, rather than duplicate authority.

| Supervisor owner | Lifetime / potentially blocking operation | Failure covered |
| --- | --- | --- |
| Parent independent reaper | Phase launch to stop/drop; owned child kill/wait, no SQL/pipe read | Parent SQL loop or output reader stall |
| Readiness reader/watchdog | Provider decision launch to exact READY/refusal/original preparation bound; reader may block on stdout | Prelude, invalid/absent/late frame; terminal output projection |
| Parent local STOP observer | Provider child lifetime; qualified local control check, no SQL | Local stop before/while parent SQL waits |
| Child preparation cover | Child entry to armed provider/envelope; monotonic deadline only | Blocked hash/claim/owner setup before READY |
| Authority coordinator | One bounded decision; one restricted SQL check at a time | Missing/late current authority or premises |
| Transport-local permit | Actual transfer; latch and monotonic expiry only in callback | Coordinator unavailable/stalled, local revocation; no SQL/history in callback |
| Parent EOF / decision envelope | Child inference lifetime / original decision deadline | Parent death and blocked cooperative abort |

The parent publishes only after owned child termination. Publication failure
cannot postpone STOP/reap or turn a failed child into success. Local pending
records retain each phase independently; a later successful publication cannot
erase an earlier pending phase. Fresh durable summary marks missing records
unavailable, including old runs. Existing effect/charge/history/status fields
retain their producer; 0020 wraps the old summary without copying its body.

**Focused local checks:** format, 28 Python helper tests, 2 CLI tests, 4 diagnostic
tests, 12 provider-fixture tests and 2 real-entrypoint PostgreSQL skeleton tests
passed on the working candidate. Binary tests passed 51/51 before the final
lifecycle refactor and are repeated below through the retained bootstrap.
Failure-target, final committed-head checks and CI results must be read from
their actual subsequent runs; earlier local results are not transferred to a
later commit automatically. Clippy all-targets passed; link/structure checks
passed with advisory size warnings. The pinned analyzer 0.0.25 ratchet passed
against the issued base: ten retained debt items, none worsened, no cap change.
The final working-source serial run also passed binary 51/51 and failure target
14/14. Windows legacy full-suite CREATEROLE remains unqualified; Linux CI supplies the
required full-suite/coverage and focused reservation evidence for its candidate.

Commands executed (each Rust process has the issued ordinary target; Python was
`C:/Users/ekoku/AppData/Local/Programs/Python/Python312/python.exe`):

```text
cargo fmt -- --check
python -m unittest discover -s tests -p test_m1_native_helpers.py
cargo test --locked --test m1_run_cli
cargo test --locked --bin duskweave -- --test-threads=1
cargo test --locked --test m1_run_diagnostics -- --test-threads=1 --nocapture
cargo test --locked --test m1_provider_fixture -- --test-threads=1 --nocapture
cargo test --locked --test m1_run_postgres -- --test-threads=1 --nocapture
cargo test --locked --test m1_run_failures -- --test-threads=1
cargo clippy --locked --all-targets -- -D warnings
python scripts/check_links.py
python scripts/check_structure.py
python scripts/check_complexity.py --analyzer D:/DuskWeave-build/rust-1.94.1/tools/rca-0.0.25-pr22/bin/target/release/rust-code-analysis-cli.exe --base b439661f82c4cebf1d615629f07eb176dd3a53e8 --report-dir D:/DuskWeave-build/rust-1.94.1/reports/m1-run-diagnostics-20261021a/complexity
```

The five DB-dependent targets above ran serially inside one retained bootstrap
session. Reports `diagnostic-final.log`, `provider-final.log`,
`run-postgres-final.log`, `run-failures-final.log`, `bin-confirmed.log` and the
complexity report contain the actual local results, including prior setup/test
corrections rather than presenting those attempts as PASS. Clippy initially
reported collapsible conditionals and the ratchet caught three new regressions;
those were corrected within the map and the subsequent gates passed.

**Observed timings (one warm sample each, milliseconds):** offline missing-key
parent total 3009, launch-to-terminal observation 1933, READY observed; publication
lock failure parent total 3276, child observation 1323, pending receipt and fresh
no-replay recovery passed. Parked claim reply: parent launch-to-barrier 678,
parent total 5572, phase observation 5003, no READY/claim observed, with parent
`readiness_deadline`. Parked reservation: decision anchor 507 after child launch
(3 ms query), parent total 46096, child observation 45623, READY observed,
exit 124 retained as `child_exit`, internal cause unavailable. These samples
are observations, not cold-start benchmarks, query latency or exact overhead.
READY timestamps and internal hash/ACL/owner/context/coordinator decomposition
remain unavailable; an absent READY is censored. Safe phase timestamps are in
the retained receipts. No claim of startup <500 ms or historical root cause.

**Author review, QUALITY_BAR section 7:** Q1/Q2: lifecycle observation stays in
supervision, configuration in its parser, immutable metadata in the Broker
adapter; no sibling model mutation or new dependency. Q3/Q4: the existing reader,
watchdogs, admission and summary producer are retained, without a framework or
new timer; process settlement and typed projection are separate responsibilities.
Q5/Q6: the explicit map and real-entrypoint/fresh-inspect assertions cover the
requested path. Q7/Q8: direct SQL negatives, code sentinel, signed Windows exit,
framing/race/deadline/reap and no-replay controls exercise the changed boundaries.
Distinct reviewer evidence and exact-head CI remain separate from this author
assessment. The enlarged supervision file retains one lifecycle owner plus its
cohesive parser/process scenario tests; existing provider/support test files
retain their shared fixture responsibility. LOC/span warnings are retained for
review, not bypassed through unrelated helpers or threshold changes.

Tracking UPDATE: ENGINEERING_STATE points to this active FIX/next action; roadmap
section 1.4 records its bounded disposition and remaining OPEN debt; the operator
guide explains configuration, diagnostics/UNKNOWN and database-only recovery.
No BUILD_ORDER edit is needed: stage/navigation and accepted scope are unchanged.
PowerShell ACL latency, owner-connection optimization, internal timing gaps,
owner-view continuation, real provider loop and live qualification remain OPEN.

## 8. Distinct review and correction R1

One distinct agent `/root/diagnostics_review` reviewed committed `ca95f56`
against the issued base, using the repository adversarial-review skill. It
performed source/report review only (no build, DB or edits); the author ran the
reported local tests. The pass found one VALID blocking issue: reader completion
could precede the readiness watchdog verdict. A fast child could exit zero after
exact READY while the parent still had no final deadline-qualified verdict;
`protocol_valid` could accept that pending state. The original source therefore
remained NOT READY despite its passing focused tests. No other source defect
survived the full pass; final CI remains separately required.

**R1 RED:** `cargo test --locked --bin duskweave
reader_completion_without_a_watchdog_completion_cannot_finalize_success --
--test-threads=1` failed its actual assertion: `ChildExit` was returned instead of
`Unavailable` when only reader completion was available. This is a synthetic
completion-order fixture, without a fault added to production.

**One cohesive correction:** both existing reader and watchdog send completion;
settlement consumes both within the remaining original phase deadline before
snapshot/protocol acceptance. A required readiness verdict must be positively
observed; missing completion cannot qualify success. No new thread, grace clock,
deadline or SQL in a watchdog was added. The gated late-watchdog test uses the
actual reader, actual successful child, queued exact frame and production
`readiness_watch`; it proves pending acceptance refuses and the final diagnostic
is `readiness_deadline` when the watchdog processes the frame after its bound.
The test first needed a fixture correction to wait for actual process exit after
pipe EOF; that setup failure is not claimed as product RED.

The R1 focused supervisor run passes 10/10; current-head broader regressions,
review of this delta and Linux CI/coverage are the remaining delivery evidence.
The previous committed head's local checks passed 85 Rust/28 helper assertions;
they are historical candidate evidence, not qualification of this correction.

The same distinct reviewer inspected only the correction delta on `f9a8ae0`
and reported R1 RESOLVED / READY for that source scope. Its conclusions for the
unchanged boundaries remain in force; it did not rerun tests or claim final CI.
The committed corrected binary run passed 53/53, including both new readiness
regressions. PR #109 remains OPEN; ENGINEERING_STATE now points to conditional
owner review after all current-head required checks, without recording a merge.
This tracking-only follow-up changes no runtime or test source. Final evidence
is read from the actual GitHub candidate and local `r1-head-*` reports.

## 9. Resume, correction R2 and qualification limits (2026-10-22)

The interrupted bootstrap ended; its credentials were neither recovered nor
reused. The owned service/image/version/binding and candidate were reverified.
The helper created fresh `dw_native_fixture_33acfea53b064b38`, retaining its
restricted credentials only in the new bootstrap and child environments.
The key was explicitly removed again. Reports for this session are under
the issued report directory's `resume-20261022` subdirectory.

Before R2, candidate `59ee86d` passed all nine jobs in Linux CI run
`39631255466`, including the focused reservation stall and full coverage suite:
654 passed assertions, zero failed/ignored; total coverage 96.79% (10900/11262),
with every production file above 80%. The PR merge checkout was `69ac97c`.
These are that historical candidate's Linux results, not current-head Windows
qualification or evidence that an unexamined scanner has no findings.

The fresh Windows serial run passed binary 53/53, diagnostics 4/4, PostgreSQL
skeleton 2/2, CLI 2/2, Clippy and format. However, the full provider target
failed `production_stop_withdraw_and_manual_terrain_signal_before_blocked_authority_sql`
at its authoritative `withdraw` writer assertion (11/12 passed), and the full
failure target failed `hard_parent_loss_during_child_publication_leaves_unknown_and_zero_effect_replay`
at `store.renew(r).is_err()` (13/14 passed). The latter also failed in the earlier
R1 run. Focused parent-loss and eight-seed reference runs on the new fixture
passed, while the earlier full provider run had failed reference eligibility.
These inconsistent results remain UNVERIFIED root causes and block readiness;
a focused pass does not erase a failing required target. No assertions, sleeps,
production policy or read-only fixtures were changed to obtain a pass.

**R2 finding, independently validated:** the CodeRabbit advisory on
`record_diagnostic` was VALID. The phase deadline also guarded publication,
so observing a phase timeout necessarily skipped durable publication despite
remaining run time. This contradicts section 4C's post-termination accounting.

**R2 RED:** added real-entrypoint regression
`expired_acquisition_phase_publishes_diagnosis_within_remaining_run_budget`
parked acquisition reservation SQL using the existing reply relay. On the
unchanged production candidate it failed the fresh-inspect assertion:
`diagnostic_publication=unavailable`, expected `committed`. The local receipt
retained `phase_deadline`, `parent_observed`, elapsed 60022 ms as pending.
The first decision was durable, with no fixture effect. Compilation and the
barrier succeeded; this is behavioral RED, not setup failure. Command:
`cargo test --locked --test m1_run_diagnostics expired_acquisition_phase_publishes_diagnosis_within_remaining_run_budget -- --exact --test-threads=1 --nocapture`.

**R2 correction:** pass the existing absolute run deadline through `wait_phase`
to diagnostic publication, including launch failure. Child polling, independent
reaping and terminal settlement retain their original phase deadline; SQL occurs
after the child is stopped. No deadline, SQL grant, production thread or timer
was added. The same regression requires a durable parent-observed phase timeout,
matching fresh inspect/receipt, one decision, zero effects and no recovery replay.
The publication-start guard uses the run deadline; synchronous SQL retains the
existing qualified database wait bounds. It does not prove SQL completion before
that Instant. Accounting cannot revive effect authority. R2 GREEN: diagnostic
target 8/8 passed; acquisition observation 60018 ms, durable `phase_deadline`.
The test then gained explicit pre-recovery effect/decision counts and accurate
parent-only timing. The final committed assertion run is reported separately in
`resume-20261022/r2-head-phase-expiry.log`. The working R2 source also passed
binary 53/53, CLI 2/2, PostgreSQL skeleton 2/2, 28 Python helper tests, format,
Clippy, links, structure and pinned McCabe ratchet (ten unchanged debt items).
These local working-source results do not automatically qualify a later commit.
Delta review and final-head CI are read from their actual candidate-bound runs.

Author delta review under QUALITY_BAR section 7: Q1-Q4 preserve the CLI's existing
composition owner, one run clock and unchanged SQL authority; no abstraction or
production dependency is added. Q5-Q6 are exercised through actual `m1-run`,
restricted publication and a fresh inspect process after the fixed phase bound.
Q7-Q8 retain failed parent exit, immutable diagnostics and zero replay assertions;
the broader Windows safety-target evidence remains UNVERIFIED and blocking.
The distinct reviewer inspected only this delta after the original full pass;
it agreed the accounting-start correction preserves child lifetime and requested
explicit pre/post recovery counts, now included. Its final SHA-bound disposition
is separate from author execution and final CI. The new cohesive test stays below
the test-file size reference point; no count-driven helper split is warranted.

This is the second cohesive FIX cycle. Any retained required-gate failure means
NOT READY and STOP for bounded root-cause review; green Linux CI cannot waive
the explicit Windows gate. No merge, seal or provider execution is authorized.

## 10. Amendment R3: withdrawal fixture correction and clock qualification (2026-10-22)

The bounded root-cause review produced the retained report
`pr109-rootprobe-20261022a` (outside Git; source evidence references it by
name). This third bounded amendment executes only the assigned test-fixture
correction inside its editable map: `tests/m1_provider_fixture.rs` plus this
packet, ENGINEERING_STATE, the roadmap and the operator guide. `src/`,
migrations, `tests/support/` and `tests/m1_run_failures.rs` stayed read-only.
No production retry policy, role grant, deadline, lease or assertion was changed. The
amendment ran on the same branch/worktree at reviewed head `836cf8a` on base
`b439661`, verified clean at preflight.

**Withdrawal root cause — now evidence-bound.** The retained classification
`historical-withdrawal-classification.json` shows the failing provider
withdrawal died from a concurrent update on `INSERT INTO mission.withdrawals`,
mapped to `serialization_retry`. The mechanism is in accepted production code:
`execution.guard_mission_writer()` (migrations 0008/0009) takes
`pg_advisory_xact_lock(scope)` then bumps `session_fences.epoch`; a concurrent
epoch bump while the child waits on that lock aborts the serializable
transaction inside the INSERT trigger (statement-path `store_err`) or at
commit (`commit_err`); both mappings classify serialization/deadlock as `serialization_retry`.
Other statement errors map to `storage_error`; other commit errors map to
`commit_unknown`. The old test submitted once, discarded its receipt, and
asserted unconditional exit success. It did not retry with a new UUID, so a
known rejection surfaced without its category. That is a test-harness defect, not a production defect; production
behavior remains locked and unchanged. The historical episode's specific
concurrent updater is inferred (the epoch UPDATE inside the trigger is the
INSERT's only row update), not directly evidenced.

**Correction.** The provider-fixture withdrawal writer is now a receipt-aware
harness: one bounded 16384-byte stdout collector, a strict withdrawal receipt
parser, and a disposition classifier. The operation UUID is minted once and
resubmitted only on a failed exit whose receipt names the same operation with
result `rejected`/category `serialization_retry`, with the premise re-validated
by the resubmitted command's own authority path; all other outcomes (unknown,
storage errors, conflicts, foreign receipts, unreadable output) are terminal
failures. ADR-005 bounded resubmission is exercised, never assumed. The
injection scenario additionally asserts at least one retry occurred, so the
test cannot silently degenerate to durable-on-first-attempt.

**Fault injection and controls.** New scenario
`fenced_epoch_bump_serializes_first_attempt_then_bounded_resubmission_is_durable_once`
injects an admin epoch bump so attempt 1 is provably serialized. RED before the
correction: attempt 1 exited 1 with a 216-byte `serialization_retry` receipt and
the durable assertion failed. GREEN after: attempt 1 `serialization_retry`,
attempt 2 durable (1058 bytes), same operation UUID. Added unit controls
`withdrawal_receipt_parser_bounds_shape_and_size` and
`withdrawal_retry_classifier_accepts_only_bounded_serialization_rejection`.
The existing
`production_stop_withdraw_and_manual_terrain_signal_before_blocked_authority_sql`
regression passes unchanged through the new harness (attempt 1 durable,
1029 bytes), proving no behavior was weakened for the pass.

**Clock qualification — ENVIRONMENT_UNQUALIFIED.** `dwpg-r3` was requalified
(same pinned image, 127.0.0.1:5432, PostgreSQL 17). The bootstrap helper
generated fresh databases `dw_native_pr109_withdrawal_requal_20261022d` and
`..._20261022e`; the prescribed `..._a` name was refused on collision and the
helper-generated names are per-packet fresh isolation, honestly recorded rather
than renamed. The retained lease-clock probe binary
(`probe_unsupervised_lease_host_vs_database_clock`, source
`pr109-rootprobe-20261022a/probe-source/tests/lease_clock_control.rs`) was rerun
against the new fixture without a key. Iterations 1-8 were clean; iteration 9
recorded `clock_timestamp()` stepping back 856 ms and 1701 ms mid-observation
and `renew` was then accepted after 2253 ms host-monotonic with the lease
unchanged — the same signature as the root-probe summary. Verdict:
ENVIRONMENT_UNQUALIFIED. The `m1_run_failures.rs:109` renewal-expired failure
is therefore attributed to the environment, not verified as a product defect,
and remains unqualified until the environment clock is recovered. Clock
recovery (WSL2/Docker time sync) is outside this map: no restart, clocksource
change or service activation was performed; evidence is in
`reports/pr109-withdrawal-requal-20261022a/lease-clock-requal-summary.json`.
The required final serial run `--test m1_provider_fixture --test m1_run_failures`
is deliberately not executed until clock qualification exists; it will run
once, with failures classified from receipts/clock evidence, not retried.

**Environment flake observation:** during post-review verification on the same
unqualified host, the unchanged pre-existing `signal_before_sql` path failed
twice consecutively (`writer exited before signal`, once on `stop`, once on
`terrain`) — the writer child exited before its local marker appeared, which
localizes failure before marker creation but does not identify its cause.
Control-root qualification and the store path remain hypotheses. A diagnostic
rerun that only routed child stderr (no semantic change) passed attempt-1
durable in 14.97 s. Classification: cause UNVERIFIED on an unqualified environment; unchanged
source and a passing rerun do not establish an environmental cause; the withdrawn writer's own scenario passed deterministically
throughout. Recorded as evidence, not chased; consistent with the
ENVIRONMENT_UNQUALIFIED verdict.

**Checks on the amended working source:** `cargo fmt -- --check`,
`git diff --check`, `cargo clippy --tests`, `python scripts/check_links.py`,
`python scripts/check_structure.py` and the pinned analyzer 0.0.25 McCabe
ratchet (`--base b439661`) all pass — ten retained debt items, none worsened,
no cap change. Production `src/` is untouched so the ratchet measures only
unchanged functions. These are working-tree local results; they do not qualify
a later commit automatically.

**Remaining uncertainty / NOT READY:** the delivery stays NOT READY pending
(1) environment clock recovery and re-qualification, (2) the single final
two-target serial run, and (3) final-head CI. No merge, seal, provider
execution or timer change is authorized.

**Author R3 self-review, QUALITY_BAR section 7:** Q1 ownership: the harness
lives in the owning provider-fixture test file as private functions; the SQL
injection uses existing admin helpers, not production seams. Q2 dependencies:
no new dependency; the writer drives the real CLI and reads its real receipt.
Q3 simplicity: spawn/settle/parse/classify are separate small functions; the
~430-line harness is proportionate to the closed receipt contract it enforces.
Q4 single truth: the classifier mirrors the producer's two-line receipt
contract; duplication across that trust boundary is deliberate. Q5 requirement
fit: same-operation UUID minted once, retry only on a proven same-operation
serialization rejection, bounded attempts inside one shared deadline,
unknowns terminal — matching the amendment §4/§5 text. Q6 reachability:
exercised through `m1-run` CLI spawn → local marker → held scope fence →
restricted withdraw → Trajectory history, with fresh DB assertions. Q7 failure
integrity: collector, deadline and attempt bounds verified; forged/foreign/
oversized receipts and all non-serialization outcomes are terminal; the
replay proof asserts one withdrawal and one history row. Q8 trust/claims:
the injection proves the rejection mechanically rather than by timing luck;
the retry-occurrence assertion prevents silent degenerate GREEN. N/A:
production reachability, migrations and role grants are unchanged by design.

**Amendment-delta review (2026-10-22, distinct reviewer):** one distinct agent
performed a source/report review of only the working-tree delta versus
committed `836cf8a` (no builds or database reruns; scope verified by file
inspection). Verdict: READY-for-delta modulo one finding. The reviewer
independently verified the serialization mechanism against migrations
0008/0009 and the error mapping, the classifier's same-operation
serialization-only gate, the bounded collector, the shared attempt deadline,
end-to-end UUID identity, the fence-block attribution check, and the unchanged
caller's preserved semantics. Its one blocking-grade finding — the injection
scenario did not assert that a retry actually occurred — was fixed in-place
(`attempt >= 2` when `bump_epoch`); three wording nits (error-mapping site,
premise re-check phrasing, "proven" strength for the historical inference)
were corrected above. It confirmed ENVIRONMENT_UNQUALIFIED is the honest
verdict and that the deferred final serial run follows the assignment.

## 11. Closure simplification (2026-10-22)

The owner requested direct correction and simplification after reviewing R3.
This is a bounded re-scope following the existing root-cause review, without
resetting FIX-cycle counts or changing the accepted outcome. The editable
delta is AGENTS.md, this packet, ENGINEERING_STATE, MVP_AND_DEFERRED_SCOPE,
tests/m1_provider_fixture.rs and its private supervision.rs/withdrawal.rs
submodules. Production, SQL, CI and expiry assertions remain read-only.

One integration target keeps the existing database mutex and test inventory.
Feedback/admission, startup/STOP/deadline supervision, and receipt-aware
withdrawal have separate private source modules; no generic fixture framework,
new integration binary, production API or duplicate scenario is introduced.
The child module split preserves behavior. Separate deadline regressions use
a terminal child and a delayed collector; these are new failing assertions
against the R3 helper, not fabricated historical failures.

Repository packets/prose are English; owner conversation may be Indonesian.
The issued instructions above are translated without changing their scope.
The false historical fresh-UUID assertion is corrected, and the unclassified
stop/Terrain exits remain UNVERIFIED instead of a proved environment flake.

At this historical checkpoint, final qualification remained blocked on the
measured database-clock rollback; section 12 records the later disposition.
Preserve historical uncertainty; closure requires a qualified current clock,
the unchanged two-target scenarios and final-candidate CI, not an impossible
reconstruction of every lost historical error. No host/WSL setting is changed
by this source correction.

**Verification:** the two new deadline assertions failed against the R3 helper
(0/2), then passed after the shared-deadline correction (2/2). On the final
working source, six selected scenarios passed: the two deadline controls, two
receipt/classifier controls, the forced-serialization CLI scenario, and the
original STOP/withdrawal/Terrain-before-SQL scenario. The fault scenario
returned a 216-byte serialization rejection, then one 1029-byte durable
receipt with the same operation. The original pre-SQL scenario passed in
7.83 seconds; no historical failure is relabeled from that pass.

The original 15 fully qualified test names are retained, with two new deadline
tests. Thin test entrypoints preserve CI's existing --exact consumer and
recorded commands; the CI reservation filter still discovers exactly one
test. These are compatibility entrypoints, not forwarding helpers introduced
to game runtime metrics. The source sizes are 622 lines for feedback and
stable test entrypoints, 538 for supervision and 486 for withdrawal.
Advisory crossings retain those cohesive responsibilities.

Windows Rust 1.94.1, fresh restricted database
dw_native_pr109_simplify_20261022a, no provider key: fmt, Clippy --all-targets
-D warnings, links (223 valid / zero broken), structure and diff hygiene pass.
Evidence is retained outside Git in reports/pr109-simplify-20261022a.
No src/ or migration bytes changed relative to the previously reviewed source.
The prior head's Linux CI passed 661 tests; it is not final-candidate CI.
The analyzer is not on this session's PATH, so no fresh local numerical
McCabe result is claimed; final CI retains the enforced ratchet.

Author review: Q1/Q2/Q6 preserve owner ports and one integration target;
Q3 separates failure responsibilities without a new runner or production
abstraction; Q4 corrects language and false historical claims; Q5/Q7 prove
the original deadline and stable-operation withdrawal through actual CLI/SQL;
Q8 keeps key-free fixture evidence separate from cognition, target permission
and the unresolved clock environment. Production wiring changes, new grants
and serialization-capacity changes are N/A because production is unchanged.
At this historical checkpoint, distinct final-delta review and final-candidate
CI remained pending. Windows expiry qualification was not waived. Section 12
records the later qualification and owner merge; no M1 seal is claimed.

## 12. Merge reconciliation (2026-10-22)

PR #109 was squash-merged to master at
`ef23262184965b81e7103b12371000619e503caa` on 2026-10-22T07:39:31Z,
from reviewed head `83f33386eff95eb65cf7e3c60df5fc42a61fafc7` against base
`b439661f82c4cebf1d615629f07eb176dd3a53e8`. This supersedes earlier pending
delivery statuses while preserving historical failures and FIX-cycle history.

**Final Windows gate:** direct log inspection at
`D:/DuskWeave-build/rust-1.94.1/reports/pr109-final-serial-20261022a/` confirms:

| Log / target | Result | Duration |
| --- | --- | --- |
| provider-fixture.log / m1_provider_fixture | 17 passed, 0 failed/ignored/filtered | 145.52 s |
| run-failures.log / m1_run_failures | 14 passed, 0 failed/ignored/filtered | 189.39 s |

The owner reports first-attempt serial execution on fresh restricted fixture
`dw_native_fixture_8d6efa1f3a997171`, migrations 0001-0020, after W32Time
automatic startup / WSL restart. Those invocation/fixture/maintenance details are
owner attestation; the two logs independently establish their test results.
The source was clean at the reviewed head when checked, and Windows reported
successful NTP synchronization. Both parent-loss cases, expired-lease rejection,
forced serialization and both settlement controls pass. This satisfies the
recorded two-target gate, not physical clock-cause proof, continuous stability,
a Windows full-suite PASS or OCI application qualification. Original discarded
stop/Terrain errors remain UNVERIFIED. No production timer or assertion changed.

**Exact-head CI and review:** [run 39735029731](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39735029731)
passed nine jobs with 96.79% production coverage (10902/11263), meeting 90%/80%
floors. Its Rust checkout `274ac41638542b62459bc11f77c92bba7f535535` binds
head `83f33386...` to base `b439661f...`. The pinned 0.0.25 analyzer retained ten
debt items with zero worsening. GitGuardian, CodeQL and ci-ok succeeded; both
threads were resolved before merge. A separate agent reviewed
`836cf8a9fa987fbdc17ca6c92387b1f39a3d6eae..83f33386eff95eb65cf7e3c60df5fc42a61fafc7`
against QUALITY_BAR section 7 and returned READY-for-delta, without blockers.
It inspected receipt/retry/deadline identity, real consumers and Linux CI evidence;
Windows log verification was separate. This is distinct agent review, not human
or institutional independence. Production, SQL and CI were unchanged after R2.

**Tracking - UPDATE:** reconcile this packet, ENGINEERING_STATE, BUILD_ORDER and
the existing roadmap. Correct nonblocking wording/size nits above; retain the
test-comment encoding nit for its owning file's next change. This reconciliation
is documentation-only: runtime wiring, behavior TDD, compiler/coverage reruns and
fresh local McCabe measurement are N/A. Its own document checks/review and
required current-head CI belong to its PR; prior CI is not its current-head PASS.

**Next boundary:** prepare one fresh real-provider fixture demonstration through
m1-run with fresh explicit disclosure/current terms/window and the infrastructure
key. Decision two must consume the original owner-qualified fixture result;
legal stop/defer/view outcomes are not forced into success. DNS remains a fixture
and recovery remains database-only. Linux live-boundary / OCI application
qualification remain unfinished and do not newly block this controlled demo.
No provider/target contact, authority acceptance, timer extension or M1 seal is
authorized by this reconciliation.

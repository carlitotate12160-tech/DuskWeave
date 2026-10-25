# DW-FIX-M1-FIXTURE-SUPERVISOR-RUST — R1

Mode: FIX. Workspace `D:/DuskWeave-ci-complexity`, branch
`feat/m1-inference-lifecycle` (existing [DRAFT PR #116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116)),
base `7e1926c2f666aa16cd914f20209a6a1400bae3f2` (original PR base
`79b0e3ee1bea44b5e5639c1142af45b6c44918b4` unchanged). Delivered 2026-10-24 as
the first seam of the owner-requested
[Rust tooling direction](../MVP_AND_DEFERRED_SCOPE.md#15-rust-tooling-direction-owner-request-2026-10-24).
This is a supporting-tooling slice: it adds no campaign capability, execution
authority, permit, timer or migration change, and preserves the APT41/Lazarus/
Volt Typhoon behavioral lenses as reference context only.

## Outcome

The 2026-10-25 bounded correction below supersedes the original cleanup,
exit-normalization and service-capture claims. The original seam and its
evidence remain historical; neither the production-runtime FIX-cycle count
nor this tooling outcome's review history is reset.

One opt-in Rust development executable, `dw-fixture-supervisor`, now owns
fixture action supervision and the fixed local Docker/PostgreSQL service
qualification. It closes the three reviewed CodeRabbit defects:

1. **Nullable Docker port metadata** — `service.rs` decodes absent/null
   `NetworkSettings.Ports` and absent/null `5432/tcp` without panicking and
   refuses them through the closed `native_service_unqualified` category.
2. **Lost timeout attribution** — `harness_timeout` with exit 124 stays the
   primary disposition even when cleanup is unknown, cleanup is a separate
   closed field, an ordinary child exit 124 remains `action_failed`, and the
   qualifier's defensive legacy `TimeoutExpired` mapping retains the timeout
   attribution instead of collapsing to `action_failed`/1.
3. **Exited parent leaving a descendant holding pipes** — the action is
   assigned to a Windows Job Object before initial execution (or a Unix process
   group); the owned tree stays reachable after the direct parent exits,
   termination goes through the wrapper's `start_kill`, and collection is
   bounded instead of waiting for pipe holders.

## Original seam changed files (historical)

Created: `src/bin/dw-fixture-supervisor/{main,action,report,service}.rs`
(137/355/272/353 physical lines; all under the 400 tooling reference),
`tests/fixture_supervisor.rs` (482), this ledger.
Modified: `Cargo.toml` (opt-in `fixture-supervisor` feature, `process-wrap
=10.0.1` optional dependency, explicit `default-run = "duskweave"`),
`Cargo.lock`, `tests/support/m1_native_bootstrap.py` (bridge),
`tests/support/m1_lifecycle_qualification.py` (service-check delegation,
supervisor prerequisite, TimeoutExpired mapping),
`tests/test_m1_native_helpers.py`, `tests/test_m1_lifecycle_qualification.py`,
`tests/test_m1_bootstrap_progress.py` (bridge semantics),
`.github/workflows/doc-check.yml`, `docs/RUN_M1_AUTONOMOUS_FIXTURE.md`,
`docs/RUN_M1_NATIVE_INFERENCE.md`, `docs/RUN_M1_TERRAIN.md`,
`docs/ENGINEERING_STATE.md`, `docs/MVP_AND_DEFERRED_SCOPE.md`,
`docs/workflows/DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE.md` (frozen-history
pointer). All within the issued file map.

## Mechanism

- `main.rs` owns closed command/request parsing: `run-action` reads one private
  stdin JSON request (version, argv, cwd, label, report root, integer action
  budget ms, redaction values) bounded to 128 KiB, 128 argv items, 256
  redactions, 8192 bytes per string and 1..=3,600,000 ms; unknown fields and
  invalid limits are refused without reflecting input. `service-check` takes
  the closed flag set.
- `action.rs` owns child/group lifetime and capture. The action clock starts at
  complete private-request admission before spawn (`timing_origin:
  request_admission`). Only the direct child's disposition is the action
  result. The single wrapper remains owned while `inner_mut().try_wait()`
  polls the native parent; the outer wrapper's `start_kill` settles the owned
  boundary on every terminal parent event, including closed-pipe descendants.
  Wrapper wait/completion-port messages are never empty-job evidence. One
  2,000 ms deadline covers native termination acknowledgment, native parent
  disposal and reader collection inside an unscoped worker; a pending worker
  is never joined. Reader EOF and read errors remain distinct. Each stream
  retains at most 16 MiB, and incomplete/overflowed streams are suppressed.
- `report.rs` owns closed result/progress serialization and publication assembly,
  and redacts assembled streams (secrets split across read chunks are
  still removed), suppresses any stream that overflowed or could not finish
  with a closed omission reason, refuses report collisions before launch, and
  makes a report-write failure non-success.
- The Python `Fixture` keeps provisioning, restricted roles, schema setup and
  process-scoped credentials; `Fixture.run` delegates every action to the
  helper selected by the absolute `DW_TEST_FIXTURE_SUPERVISOR` path (no PATH
  fallback, no on-demand build, no Python supervision fallback — the old
  `stop_action_tree`/`wait_supervised`/`wait_with_progress` are deleted) and
  validates status, supervised exit, child exit, capture, cleanup and omissions
  against the published metadata, including their types and result consistency. Legacy no-ceiling
  calls receive the finite 3,600,000 ms development-tool policy ceiling
  (selected policy, not measured fit). Both bootstrap and qualifier delegate
  the Docker inspect/port/version qualification to `service-check`; their
  duplicated Python port iteration is gone. Credentials stay in the inherited
  process environment and the private stdin request, never argv, output or
  reports. T8 and CI inherit the same helper path.
- Own code keeps `forbid(unsafe_code)`; native calls stay inside the reviewed
  pinned dependency. This tooling does not replace ADR-015, adds no kill-on-
  close claim, and does not supervise the supervisor itself.

## Original seam RED/GREEN evidence (historical mechanism)

- Exited parent with a six-second pipe-holding descendant, 0.5 s ceiling:
  old `Fixture.run` returned after **6.125 s** (`AssertionError: 6.125 not
  less than 4`), waiting for the descendant's pipes. GREEN through the bridge:
  bounded return under 4 s with the owned descendant terminated.
- Null/absent `Ports` and null `5432/tcp`: old code crashed with
  `TypeError: 'NoneType' object is not iterable` (and `AttributeError` for
  null `Ports`); the closed refusal is now owned by Rust and surfaced through
  the bridge.
- Timeout attribution: old qualifier mapped a raised `TimeoutExpired` to
  `action_failed` exit 1 (`AssertionError: 1 != 124`); the defensive mapping
  now reports `harness_timeout`/124.

## Original seam verification (historical, superseded mechanism)

- Helper unit tests 16/16 and integration tests 13/13 (real OS processes;
  success, child failure, child exit 124, live-parent timeout, exited parent
  with pipe-holding descendant, output overflow suppression, secret split
  across writes, protocol/report refusals, collision-before-launch,
  write-failure non-success, unrelated sentinel survival, service-check
  refusals, closed progress-line emission, plus the Unix-only setsid-escape
  `cleanup=unknown` case which cannot run on Windows). `cargo fmt --check`,
  feature-enabled and default `cargo clippy --all-targets -D warnings` pass.
- Real T8 through the existing staged-upgrade consumer:
  `cargo test --test m1_inference_lifecycle t8_populated_legacy_owner_records_survive_forward_migration_and_repeatable_refresh --exact`
  passed 1/1 in the ordinary target via bootstrap → helper → nested worker,
  fresh staged database on owned `dwpg-r3` (report
  `dw-t8-upgrade-1ce0_18e187e5c523df5c` retained).
- Focused instrumentation, cargo-llvm-cov 0.9.1, isolated
  `coverage/fixture-supervisor` target (workspace/profile cleanup only there;
  `--no-report` never combined with `--no-clean`; normal and lifecycle
  coverage caches untouched): per-file line coverage `main.rs` 100%,
  `report.rs` 100%, `action.rs` 92.6%, `service.rs` 98.1% — all above the 80%
  per-file floor. This run does not substitute for the four lifecycle actions.
- Python suites: `test_m1_bootstrap_progress.py` 12/12, `test_m1_native_helpers.py`
  35/35, `test_m1_lifecycle_qualification.py` 17/17, `test_m1_terrain_live.py`
  39/39, `test_m1_terrain_live_contract.py` 5/5.
- Pinned rust-code-analysis-cli 0.0.25 ratchet against the packet base: Gate
  PASS, 10 retained debt items unchanged, zero regressions, zero new over-cap
  functions. Structure checker: warning membership identical to base (35),
  zero forbidden paths/markers/hygiene findings; `doc-check.yml` measured 440
  lines (≤441); the old lifecycle ledger remains at its 1221-line freeze.
  Dependency direction 22 protected modules, 0 violations; link check clean.
- Runtime diff (production/manifest paths, lockfile and tests excluded): about
  1,120 added lines across the four new tooling modules and `Cargo.toml` —
  above the 600 advisory reference point. Retained deliberately: the four
  modules are single-responsibility owners of the issued decisions and include
  inline unit tests; splitting them further would separate the closed protocol
  from its validation rather than separate responsibilities. No threshold was
  raised and no warning member added.

## CI wiring

The `rust` job builds/tests the helper into a separate `fixture-supervisor`
target before the Python bridge tests (moved from the `structure` job, which
has no Rust toolchain) and before the bootstrap consumers, exporting only its
safe absolute path. Clippy, the full all-targets coverage consumer and the
geiger diagnostic now run with `--features fixture-supervisor` so the helper is
linted, executed and measured under the retained 90%/80% floors; the pinned
dependency checks and audit read the updated lockfile. No required check
identity changed; no Windows service-container job was added.

## Original seam review and tracking (historical)

The original distinct adversarial review of the complete candidate diff returned READY
for this packet's scope with five non-blocking findings; all were addressed or
recorded here: progress-emission coverage was added (unit), the service probes
were aligned inside the bridge's 60 s bound (25 s each), the candidate tree was
staged as one custody unit, the old-ledger line count was corrected to its
1221 freeze, and two residuals are recorded below. This review does not waive
the pending gates listed at the end of this section.

Residual limits accepted for a later roadmap-1.5 seam: (1) the bridge's Unix
last-resort unresponsive fallback kills only the helper's own group, not the
action's separately-led group — reachable only if the helper itself exceeds its
internal bounds, and the qualified platform is Windows; (2) the supervisor
hard-death containment boundary is not claimed (no kill-on-close), matching
decision 14.

Exact-head correction (2026-10-25): the first pushed candidate failed its
Linux CI in the prerequisite unit test, which used a Windows-style path that
`Path.is_absolute()` rejects on Linux; both controller tests now derive the
helper path from the platform's own temporary directory. No production code
changed; the same 17 controller tests pass locally and the correction is
re-verified by the new exact-head CI run.

Original author self-review under QUALITY_BAR section 7 (cleanup/failure claims superseded below): Q1 ownership PASS (four
single-responsibility modules; the bridge keeps provisioning only); Q2
dependencies PASS (the helper imports no crate owners; dependency direction 0
violations); Q3 simplicity PASS (no runner framework; injection limited to two
docker probes); Q4 single truth PASS (port/image/version enforcement lives only
in `service.rs`; the Python digest constant is a record, not a check);
Q5 requirement fit PASS (three defects closed with RED/GREEN evidence; no
campaign semantics touched); Q6 reachability PASS (bootstrap session/one-shot,
qualifier, T8, live wrappers and CI all consume the helper through the same
Fixture API; real T8 executed); Q7 failure integrity PASS (bounded waits,
first-failure stop, honest `cleanup=unknown`/omission dispositions, exit 124
distinction, report-write failure non-success); Q8 trust PASS (closed
categories, no input reflection, credentials confined to env/stdin, redacted
publication — with the residuals above stated honestly).

Tracking disposition UPDATE: `ENGINEERING_STATE.md` (phase and
active-entry status), `MVP_AND_DEFERRED_SCOPE.md` section 1.5 (first seam
delivered), the three run guides (helper build/path prerequisite), this
ledger, and the frozen-history pointer in the old lifecycle ledger.
`BUILD_ORDER.md` navigation is unchanged.

Not delivered here: a fresh full four-action Windows qualification on the
replaced mechanism (its mandatory final disposition remains pending; no gate
waiver or READY claim), native Linux execution of the helper tests (expected
from the exact-candidate CI, including the setsid-escape case), any live
provider/target contact, issue or thread closure, merge, acceptance or seal.
The four `5d769d2` Windows PASS results keep their original provenance under
the superseded Python supervision.


## Bounded correction, 2026-10-25

Authority: owner-issued PR116 bounded correction, Amendment A, base
`60e89d1c0f31c6c902b20076c1a769ce68124c74`. This corrects the current tooling
outcome; the two historical production-runtime FIX cycles remain unchanged.
The instruction-conflict STOP preserved clean source and `base-build.log`;
Amendment A replaced the unsafe accessor instruction with the verified safe
single-wrapper path. No unsafe, dependency, manifest, main, CI, ACL, SQL or
production runtime change is made.

### Corrected mechanism and evidence basis

`run-action` and both fixed service probes call `action::capture_process`.
The service's duplicate spawn/read/join/kill/wait implementation is removed.
Its 25-second per-probe policy and the bridge's 60-second bound are unchanged;
private transient capture never publishes Docker inspect content. Service
probes emit no action progress and refuse every non-completed result.

`cleanup=completed` means a successful native owned-boundary termination
acknowledgment (or Unix ESRCH specifically establishing an absent owned group),
disposed native parent, and finished capture readers within the shared 2,000 ms
deadline. A detached cleanup worker owns blocking termination/drop and is
observed through a deadline-bounded channel. Missing/failed evidence is
`cleanup=unknown`; EOF alone is insufficient. This is neither a native empty-job
query nor universal descendant reaping, hard-death or adversarial-escape containment.
Independent recorded-PID assertions still require actual descendant termination.

Rust normalizes completed results to exit 0 only with child exit 0, complete
capture and qualified completed cleanup. Supervision failure after child exit 0
is `action_failed`/1 while preserving `child_exit=0`. Actual nonzero child exits
are retained; child exit 124 stays action failure. An original harness timeout
stays `harness_timeout`/124 even with unknown cleanup. Helper transport exit 0
still means successful result publication; `Fixture.run` returns the supervised
exit. Both protocol and metadata must satisfy the bridge contract.

### Candidate-bound RED to GREEN

The retained unchanged-base build was reused. New assertions failed against
that mechanism before implementation (`correction-1/red-*.log`):

- Closed-pipe descendant: parent exit 0 left the recorded owned PID alive.
  GREEN requires return below four seconds with a 1,000 ms action budget,
  completed cleanup, terminated six-second descendant and surviving sentinel.
  Pipe-holding and live-parent timeout assertions also remain.
- Actual `Fixture.run` overflow: 17 MiB, child exit 0, `action_failed` and
  incomplete capture nevertheless returned 0. GREEN returns nonzero, preserves
  child exit 0 and suppresses stdout. Real one-shot bootstrap and qualifier
  consumers observe failure; qualifier retains three `NOT_RUN` actions.
- Simulated termination failure incorrectly reported completed cleanup; reader
  error incorrectly became finished capture. GREEN leaves cleanup unknown and
  non-success, suppresses failed reads, and exercises the actual pending cleanup
  worker within the settlement deadline with both original success and timeout.
- Actual service capture waited 6.252 seconds for a six-second pipe holder and
  accepted overflow. GREEN returns below four seconds, independently checks the
  recorded descendant's termination, and refuses overflow/nonzero probes.

An ordinary successful child remains a control. Exit-124 distinction, cross-write
redaction, collision-before-launch, report write failure, nullable-port refusal,
original scenario inventory and first-failure/no-replay behavior are retained.

### Windows correction checks

Environment: Windows AMD64/MSVC, Rust 1.94.1, Python 3.12.10,
cargo-llvm-cov 0.9.1, owned PostgreSQL 17.11 on `dwpg-r3` requalified through
the corrected service-check. The report root is
`D:/DuskWeave-build/rust-1.94.1/reports/fixture-supervisor/closure-20261025a`;
`base-build.log` is retained and all new evidence is under `correction-1`.

- Dedicated helper target: unit 20/20, integration 14/14, serial. The Unix-only
  escape assertion awaits exact-head Linux CI; no Windows substitution is claimed.
- Python suites: bootstrap_progress 15/15, native_helpers 35/35,
  lifecycle_qualification 18/18, terrain_live 39/39, terrain_live_contract 5/5.
- Focused instrumented helper: action 94.57%, report 99.21%, service 97.81%,
  main 100%; all exceed the 80% per-file floor. Profile cleanup was confined to
  `coverage/fixture-supervisor`; no normal/lifecycle cache was cleaned and
  `--no-report` was not combined with `--no-clean`. This is focused helper
  coverage, not the full production coverage gate or four-action matrix.
- Exact ordinary T8 selector ran once through its real staged bootstrap and
  nested worker: 1/1 PASS (56.14 s test time). T8 provisioned its own fresh
  predecessor database/restricted roles and exercised forward migration and
  repeatable refresh. No outer fixture or historical credential recovery.
- fmt, feature-enabled all-target Clippy, structure/hygiene, dependency direction,
  links, whitespace and pinned rust-code-analysis-cli 0.0.25 ratchet PASS.
  Ten historical complexity debts remain unchanged; no new over-cap function.

Physical LOC before/after: action 384/399, report 272/369, service 354/187,
main 137/137; integration 482/364; bootstrap 364/393; bridge-progress tests
170/216; qualifier tests 292/321. Moved invocation/guard support is 162 lines;
private service tests were 178 in the external file (inlined by Amendment B below). Structure warning membership is exactly the same
35 files as the base, with no new or removed member. The frozen lifecycle ledger
remains 1221 lines. Retained historical debt is not claimed as repaid.

Publication owns result/progress/metadata serialization; process ownership stays
in action and fixed service admission stays in service. No circular owner import
or second runner was introduced. The only two new files are the permitted test
moves. The cumulative source-path diff exceeds the advisory runtime reference;
the changes remain cohesive across process custody, closed result publication
and their real consumers, with no exception or threshold adjustment.

### Review, disposition and remaining qualification

Author section-7 delta review: Q1/Q2 ownership/dependencies are explicit above;
Q3 removes the duplicate supervisor; Q4 Rust normalization and independent bridge
validation preserve one result contract; Q5 the three issued findings have RED
and GREEN assertions; Q6 run-action -> capture -> report -> Fixture.run ->
one-shot/qualifier/T8 and service-check -> the same capture path are exercised;
Q7 failure/UNKNOWN/124/NOT_RUN behavior is tested; Q8 closed omissions, private
capture, redaction and the stated cleanup-evidence limits preserve truthful claims.
No changed runtime island remains. No new campaign capability is delivered.

One distinct independent section-7 delta review inspected committed `c46b3cc`
against correction base `60e89d1`, including real callers and retained RED/GREEN
evidence. It returned one VALID blocking finding R1: an obsolete Unix-only
`Path` import remained in the integration file after its harness moved. Linux
doc-check run `40156456763` independently failed Clippy on that exact import,
before Linux runtime tests; the other seven component jobs passed. The review
found no further surviving runtime defect. R1 is corrected by deleting only the
unused import/attribute, without a lint allowance or behavior change; this is
the one cohesive review correction, not another tooling/runtime FIX cycle.
The review's cleanup, closed-pipe, supervised-exit and shared-service challenges
and Q1-Q8 dispositions remain attached to the existing outcome. The corrected
head receives narrow delta verification under the same review and fresh Linux CI.
The first failed CI remains historical evidence, not a qualified Linux run.

Current committed head, exact-head Linux CI and the review's final delta
disposition are read from PR #116 before returning READY_FOR_FINAL_QUALIFICATION.
That disposition never means READY_TO_MERGE: a fresh full four-action Windows
matrix on this replaced mechanism and final qualification remain mandatory and
were not started here. Old `5d769d2` matrix results retain their original Python
supervision provenance. No provider/target/resolver contact, merge, issue/thread
closure, review-history reset, acceptance or M1 seal is authorized or recorded.

Tracking UPDATE is confined to this existing ledger and ENGINEERING_STATE's
active-correction/next-qualification pointers. The frozen lifecycle ledger,
roadmap and all other authority remain untouched.


## Amendment B: coverage inventory correction, 2026-10-25

Owner-issued continuation base `276335b263cd432d4e6a57bc4f7d144d94e994d6`.
The preceding STOP is retained: exact-head Linux doc-check `40157033166`, Rust
job `121060533080`, passed Clippy, helper/Python tests, focused qualification
and the full instrumented suite but failed the actual coverage inventory gate:
`production source missing from report: src/bin/dw-fixture-supervisor/service/tests.rs`.
Reported coverage was 96.81% (12915/13341), with all reported files above 80%;
that was not a complete gate PASS. Seven other component jobs and the separate
parser-mutation run `40157033152` passed. The same independent review confirmed
the coverage blocker after correcting its earlier R1 obsolete-import finding.

Amendment B authorizes only relocation of the existing private tests. The
187-line `service.rs` now contains the complete formerly external 178-line
module inside its original `#[cfg(test)] mod tests`, preserving nested
`#[cfg(test)] mod scenarios`, `use super::super::*`, every assertion, fixture,
comment and `service::tests::scenarios::*` identity. The obsolete external file
is deleted. Its content matches after normal indentation and rustfmt line-wraps;
the production prefix before the test-module declaration is byte-for-byte
identical to the base. There is no new source/helper/test file, runtime change,
checker exclusion, fabricated entry, threshold/CI/dependency change or cycle reset.

The dry relocation was 366 physical lines; normal rustfmt introduces five
line-wraps, giving an actual formatted `service.rs` count of 371, below 400.
No comment/blank deletion or formatting compression is used. Structure warning
membership remains the same 35 retained files; no new member or repaid-debt claim.

Local Windows/Rust 1.94.1 evidence uses the exclusively owned dedicated
`fixture-supervisor` target and fresh
`closure-20261025a/coverage-inventory-1` report directory. All earlier logs and
STOP/RED/GREEN evidence remain retained; no behavioral RED is fabricated for
this already-passing test relocation:

- Before/after `--list`: the exact same 20 helper unit-test names, zero missing,
  added or renamed tests; `relocation.json` records the comparison, prefix and
  content checks, absent external file and physical LOC.
- Serial helper unit 20/20 and integration 14/14 PASS, with zero ignored or
  filtered required cases. Existing process assertions are preserved qualification,
  not a new defect discovery.
- fmt check, feature-enabled all-target Clippy with `-D warnings`, coverage
  checker `--selftest` (all seven existing controls), structure/hygiene,
  pinned rust-code-analysis-cli 0.0.25 ratchet, document links and whitespace PASS.
  Ten historical complexity debts remain unchanged, without a new/worsened violation.

The existing distinct reviewer inspects only this relocation/evidence delta
under the same review, checking test ownership/inventory and unchanged production
prefix. Exact committed head, narrow review disposition and fresh Linux full-suite
coverage/actual inventory-gate results are read from PR #116. Old coverage totals
alone never qualify this relocation. READY_FOR_FINAL_QUALIFICATION requires the
new candidate's inventory, 90% total / 80% per-file gate and other required checks
to pass; it never means READY_TO_MERGE.

No local T8, provisioning, Python bridge matrix, local coverage instrumentation
or full Windows lifecycle matrix is repeated for this test-only delta. Earlier
Windows evidence keeps its original provenance; the mandatory full four-action
Windows qualification on the replaced supervisor remains the next separately
authorized gate. No provider/resolver/target contact, issue/thread closure,
merge, acceptance or seal. Tracking UPDATE is confined to this ledger and
ENGINEERING_STATE's active-correction/next-qualification pointer.


## CodeRabbit process-observer correction, 2026-10-25

The owner explicitly authorized the validated CodeRabbit finding
[`4369845429`](https://github.com/carlitotate12160-tech/DuskWeave/pull/116#discussion_r4369845429)
after Amendment B. Base `193354b90f6fa04b2a82e7305beb55619d203316` was clean,
published, and qualified by Linux doc-check `40166781426` and parser-mutation
`40166781445`; its complete inventory/coverage gate passed at 96.78%
(13053/13487). That evidence and the earlier STOP/review history remain retained.
This focused test-harness correction does not reset production-runtime FIX cycles.

Finding VALID: the non-Windows `process_alive` path used only proc-file existence.
A terminated zombie can retain `/proc/<pid>/stat` until reaping, creating false
failure in independent descendant-termination assertions. The correction reads
the state after the final closing parenthesis of comm, which can contain spaces
and parentheses. State Z is terminated, recognized non-Z states remain alive,
and a missing file is absent. Other read errors, undecodable contents and missing
or unrecognized state fail the observation instead of manufacturing death.
The Windows tasklist path and every existing PID/sentinel/escaped-process
assertion remain unchanged. No supervisor, product runtime or timer changed.

Genuine RED at the extracted private proc-file boundary retained the original
existence predicate: all three new regression functions failed assertions for
Z misclassification, unreadable stat and malformed state. This is deterministic
synthetic proc-content evidence, not a claim that a flaky CI zombie race was
observed on the base. GREEN uses the same private boundary, exercised through
the real Linux liveness caller and directly by host-independent controls:
Z dead; R/S/D/T alive; comm with spaces/parentheses or embedded `) Z ` handled;
missing entry dead; directory/invalid-UTF-8 read and malformed/unknown state
rejected. The real sleeping escaped process remains an independent positive
control in the existing Unix suite.

Local Windows/Rust 1.94.1 checks in the exclusive helper target: unit 20/20 and
integration 17/17 PASS (14 existing plus 3 new observer regressions); fmt,
feature-enabled all-target Clippy, structure/hygiene, dependency direction,
links, whitespace and pinned McCabe ratchet PASS. Structure warning membership
remains 35; the ten historical source complexity debts are unchanged. Harness
size is 248 physical lines. Evidence is retained under
`closure-20261025a/zombie-observer-1`; prior report directories are preserved.

Tracking UPDATE is confined to this existing ledger and ENGINEERING_STATE's
active correction/next qualification pointers. The only code change is in
`tests/support/fixture_supervisor.rs`; its three regressions remain inline in
that existing test-support file. No source/CI/checker/manifest/bridge change,
new source file, database provisioning, local T8/Python matrix or full Windows
matrix is introduced. The existing reviewer checks only this observer delta;
current committed head, new exact-head Linux CI and final review disposition
are read from PR #116. The mandatory full four-action Windows qualification
on the replaced supervisor remains a separate pending gate. No merge,
automatic thread closure, provider/resolver/target contact, acceptance or seal.

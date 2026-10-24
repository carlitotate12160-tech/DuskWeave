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

## Changed files

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
  result; wrapper `try_wait`/`wait` group state is never taken as whole-tree
  completion proof (the pinned process-wrap 10.0.1 `JobObjectChild::try_wait`
  returns the inner parent status). Reader threads drain both pipes into
  16 MiB bounded transient memory, overflowing streams are drained-but-dropped,
  and after the stopping event cleanup/collection is bounded to 2,000 ms;
  an unfinished reader is abandoned (`UNKNOWN` cleanup), never joined forever.
- `report.rs` redacts assembled streams (secrets split across read chunks are
  still removed), suppresses any stream that overflowed or could not finish
  with a closed omission reason, refuses report collisions before launch, and
  makes a report-write failure non-success.
- The Python `Fixture` keeps provisioning, restricted roles, schema setup and
  process-scoped credentials; `Fixture.run` delegates every action to the
  helper selected by the absolute `DW_TEST_FIXTURE_SUPERVISOR` path (no PATH
  fallback, no on-demand build, no Python supervision fallback — the old
  `stop_action_tree`/`wait_supervised`/`wait_with_progress` are deleted) and
  validates the protocol line against the published metadata. Legacy no-ceiling
  calls receive the finite 3,600,000 ms development-tool policy ceiling
  (selected policy, not measured fit). Both bootstrap and qualifier delegate
  the Docker inspect/port/version qualification to `service-check`; their
  duplicated Python port iteration is gone. Credentials stay in the inherited
  process environment and the private stdin request, never argv, output or
  reports. T8 and CI inherit the same helper path.
- Own code keeps `forbid(unsafe_code)`; native calls stay inside the reviewed
  pinned dependency. This tooling does not replace ADR-015, adds no kill-on-
  close claim, and does not supervise the supervisor itself.

## RED/GREEN evidence (captured against the unchanged mechanism first)

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

## Verification actually run (Windows 10 x86_64, Rust 1.94.1 MSVC)

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

## Review, tracking and remaining gates

One distinct adversarial review of the complete candidate diff returned READY
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

Author self-review under QUALITY_BAR section 7: Q1 ownership PASS (four
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

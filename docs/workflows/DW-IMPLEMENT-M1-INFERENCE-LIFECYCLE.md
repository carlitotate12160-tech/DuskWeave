# DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE — historical R2 delivery

Delivered in [#116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116),
merged `618c885346ccf5e5b645bb63aee51b9e827be748`. The accepted
[contract revision](../contracts/M1-enabling-architecture.md#provider-neutral-lifecycle-and-freshness-revision-2026-10-22)
and [ADR timing table](../adr/ADR-016-execution-broker.md#selected-successor-timing-policy-reasoning-bounded-v1)
own semantics/policy; this closed assignment is not executable next work.

The production path is `main -> m1_run_cli -> m1_run_inference -> restricted
owner preparation/reservation -> selected guarded transport -> codec/admission
-> Cognition/Trajectory -> first_step/inspect`. Migration 0021 independently
validates bindings and current-use eligibility. Real external qualification is
separate from controlled local Responses/Chat Completions protocol fixtures.

## R2 completed runtime and qualification ledger

| Assertion | Retained qualification scope |
| --- | --- |
| T1 | Both real CLI protocol fixtures: two decisions/history, one issued DNS/Terrain and bound successor pack |
| T2 | Concrete bounded transport, blocked callback/full reporter controls; original call clocks include pre-send work |
| T3 | Original 60s Terrain TTL, post-expiry historical recording, zero stale-dependent release and direct SQL controls |
| T4 | STOP/withdrawal/control/parent loss, original deadlines and stopped factory permit |
| T5 | Three-profile Rust/SQL binding matrix; missing DeepSeek key gives `live_key_missing`, zero fallback/calls/DNS |
| T6 | Actual counters, conditional repair only, uncertain accounting and key-free no-replay recovery |
| T7 | Diagnostic-v2 closed reports, competing/lost/full channels; missing reports never invent causes |
| T8 | Fresh 0001-0021 bootstrap and real populated-0020 staged upgrade, original records/identities retained |
| T9 | 24 producer-derived wire cases: both protocols, feedback/eight seeds, UTF-8/escaping, exact/over bounds and SQL admission |

The early working-source table and samples retain their own provenance in the
historical text. At P=8192 bytes, the largest tested repair bodies were
14596/14621 bytes; these samples are not every legal input or latency percentiles.
Final ordinary/instrumented Windows actions and T8 on Rust supervision are
recorded by the [supervisor closure](DW-FIX-M1-FIXTURE-SUPERVISOR-RUST.md#final-windows-qualification-2026-10-25).
Final Linux checks/review belong to #116; earlier-head PASS never substitutes.

## PR review and FIX cycle 1

The full review/correction history remains linked below, including both runtime
FIX cycles, readiness failures and source-bound evidence. Archiving the chronology
does not reset cycles, resolve unmeasured historical causes or discard valid findings.

## Native qualification closure (2026-10-24)

Python-supervised `5d769d2` passed a four-action matrix, but an exited-parent /
descendant-held-pipe counterexample invalidated its general cleanup guarantee.
The pinned llvm-cov `--no-report`/`--no-clean` conflict was an invocation blocker,
not a product test failure. Both incidents remain historical; the
[Rust supervisor replacement](DW-FIX-M1-FIXTURE-SUPERVISOR-RUST.md) supersedes that
mechanism. Old Windows results are not relabeled as qualification of the replacement.

## 8. Execution record: blocked uncommitted work

Historical anchor retained for ADR-016 readers. The old WIP status is superseded
by the merged delivery and final qualification above; source snapshots and earlier
blocked observations remain in the immutable record below.

No real-provider feedback completion, live acquisition, target permission or M1
seal is established. The later [release-integrity FIX](DW-FIX-M1-INFERENCE-RELEASE-INTEGRITY.md)
changes only its stated release/error-precedence boundary.

[Full historical assignment, corrections and evidence](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/workflows/DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE.md).

# DW-FIX-M1-FIXTURE-SUPERVISOR-RUST — historical delivery

Delivered within [#116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116).
`src/bin/dw-fixture-supervisor/` owns trusted fixture-action trees, bounded output /
wait/cleanup, redacted publication and fixed service checking. Windows Job Object
and Unix process-group ownership replace Python Popen/taskkill action supervision;
bootstrap, qualifier, T8 and CI use the same seam. This is development tooling,
not a production Broker or qualification of Windows acquisition.

Dependency safety placement retained core `forbid(unsafe_code)`. The private
service tests moved inline after coverage inventory correctly rejected an
unreported `src/.../tests.rs`; no coverage exclusion or threshold change was used.
Null-port and descendant-cleanup findings remain valid corrected history.

## CodeRabbit process-observer correction, 2026-10-25

Linux proc-file existence misclassified zombies. The test observer now parses
state after the final comm parenthesis; Z is dead, known active states are alive,
missing is absent and unreadable/malformed/unknown state is an observation error.
Three deterministic regressions plus the escaped live-process control preserve
independent termination assertions. Earlier observer/coverage STOPs and reviews
remain in the full record; no product-runtime FIX-cycle reset is implied.

## Final Windows qualification, 2026-10-25

Tested source `ef49780bd32ca19383cf6f7bd49c87aa5fd94f96`; documentation-only
successor `cc0d0896fca6996613381c71e4cebcf8624a9591` changed no executable inputs.
Windows AMD64 / Rust 1.94.1 MSVC / llvm-cov 0.9.1 / PostgreSQL 17.11, owned
`dwpg-r3`, database `dw_native_lifecycle_rust_final_20261025a`.

| Action | Result | Elapsed |
| --- | --- | --- |
| ordinary_binary | 57/57 PASS | 32.8s |
| ordinary_lifecycle | 18/18 PASS | 277.7s |
| instrumented_binary | 57/57 PASS | 49.5s |
| instrumented_lifecycle | 18/18 PASS | 288.0s |

First attempt, aggregate exit 0; zero failed/ignored/filtered required cases.
All four metadata records: `capture: complete`, `cleanup: completed`,
`omissions: []`, `timing_origin: request_admission`, within the 900s action policy.
T8 passed inside both lifecycle actions. `qualification.json` binds the tested
head and rebuilt helper's absolute path. Report root:
`D:/DuskWeave-build/rust-1.94.1/reports/inference-lifecycle/qualification-rust-final-20261025a`.
Earlier logs live under `reports/fixture-supervisor/closure-20261025a`.

Final #116 Linux checks/review and resolved threads remain on the PR. This proves
the stated Windows fixture source/environment only, not historical startup cause,
a latency distribution, a real provider, target permission or an M1 seal.

[Full historical assignment, corrections and evidence](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/workflows/DW-FIX-M1-FIXTURE-SUPERVISOR-RUST.md).

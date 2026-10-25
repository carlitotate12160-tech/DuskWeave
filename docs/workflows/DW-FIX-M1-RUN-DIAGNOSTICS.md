# DW-FIX-M1-RUN-DIAGNOSTICS — historical delivery

The FIX delivered configuration preflight, bounded closed child observations,
restricted immutable diagnostics and fresh inspect through `src/m1_run*`,
`src/postgres_m1_run.rs` and migration 0020. Cause/source/exit/guard observations
are distinct; exit 124 never identifies the historical killer by itself.
Current operation belongs to [the operator guide](../RUN_M1_AUTONOMOUS_FIXTURE.md).

## 12. Merge reconciliation (2026-10-22)

[#109](https://github.com/carlitotate12160-tech/DuskWeave/pull/109) merged at
`ef23262184965b81e7103b12371000619e503caa`, reviewed head
`83f33386eff95eb65cf7e3c60df5fc42a61fafc7`. R1/R2 and closure corrections preserve
original settlement budgets, same-operation known-serialization-rejection handling,
UNKNOWN outcomes and database-only recovery. Production/SQL/CI were unchanged
after R2; the fixture corrections create no production retry policy.

| Recorded gate | Result / provenance |
| --- | --- |
| Windows `m1_provider_fixture` | 17/17, 145.52s |
| Windows `m1_run_failures` | 14/14, 189.39s |
| Exact-head Linux [39735029731](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39735029731) | Nine jobs passed; 96.79% production coverage; ten retained complexity debts, no worsening |
| Review | Separate closure-delta reviewer returned READY-for-delta; PR retains findings/dispositions |

Windows logs: `D:/DuskWeave-build/rust-1.94.1/reports/pr109-final-serial-20261022a/`.
Owner-reported W32Time recovery / WSL restart is environment attestation, not
proof of the old clock cause or continuous safety. Original discarded stop/Terrain
errors remain UNVERIFIED. This is not Windows full-suite or live-provider proof.
Earlier failures/correction cycles remain historical and are not reset or relabeled.

[Full historical assignment, corrections and evidence](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md).

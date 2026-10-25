# DW-FIX-M1-INFERENCE-RELEASE-INTEGRITY — historical delivery

[#117](https://github.com/carlitotate12160-tech/DuskWeave/pull/117) merged at
`7d7e6964abd628278c56021e1d77752122e77a07`, reviewed source
`f9ad970760acdd8f3a306ccf996074909ff84d1d`, base `618c885`.

`src/m1_reasoning/postgres.rs::PgBroker::release_into` tries the campaign session
fence once; contention refuses `current_authority_unavailable` with zero provider
calls. It preserves early expiry and rechecks the original shared deadline after
validation, before `Provider::call`. Acquired-fence cleanup stays on the existing path.
Both concrete paths in `src/m1_reasoning/transport.rs` always collect the worker;
saved release error precedes join failure. No timer, retry or authority expansion.
This removes release-admission waiting, not all later SQL/accounting waits.

Real RED/GREEN records are under
`D:/DuskWeave-build/rust-1.94.1/reports/release-integrity/20261025a/resumed-1`:
native release 4/4, Terrain fence scenario 1/1 (other blocks retained), continuation
4/4 and transport 41/41 passed. The deadline test proves entry before expiry and
refusal after validation expiry; lock reacquisition proves cleanup.
The Terrain writer-first block asserts prompt refusal, zero calls and durable
anomaly/reference ineligibility after owned transaction cleanup.

Exact-head Linux [40185684159](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/40185684159)
passed required gates: 96.79% production coverage, all files >=80%; McCabe
`release_into`=7, `exchange_owned`=1, ten retained debts unchanged. Distinct review
and thread dispositions live on #117. The original intermediate LOC table was
stale; use the actual Git diff rather than that historical estimate.

Provisioning EOF/recovery and failed attempts remain in the full historical record;
no fixture/evidence deletion or retry is authorized by retaining this summary.
No provider/target contact, legacy removal, M1 completion or seal is claimed.

[Full historical assignment, corrections and evidence](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/workflows/DW-FIX-M1-INFERENCE-RELEASE-INTEGRITY.md).

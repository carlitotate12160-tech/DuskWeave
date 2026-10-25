# DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE — historical design

Owner-accepted 2026-10-22; delivered by
[#115](https://github.com/carlitotate12160-tech/DuskWeave/pull/115), merged at
`79b0e3ee1bea44b5e5639c1142af45b6c44918b4`. Current architecture belongs to the
[M1 revision](../contracts/M1-enabling-architecture.md#provider-neutral-lifecycle-and-freshness-revision-2026-10-22)
and [ADR-016 revision](../adr/ADR-016-execution-broker.md#provider-neutral-lifecycle-and-freshness-revision-2026-10-22).
The R0/R1 assignments are historical, not pending design work.

## Retained accepted assertions

These remain requirements, not a claim that all future providers are implemented:
- User/organization credential references cannot be substituted; secrets stay
  outside ordinary context/history. Connection, protocol, model and routing remain distinct.
- Compatible model profiles reuse a protocol without changing domain behavior.
  Approved model/upstream/routing must be enforced; an unsatisfied policy refuses
  instead of switching route. OpenAI/Claude/Mistral/OpenRouter remain unqualified.
- A single selected profile stays pinned. Finite inner/outer timings fit the
  original admission window; insufficient remainder defers rather than truncates.
- TTL-only expiry may preserve historical reasoning; it yields zero stale-dependent
  effect release, including direct backend admission. Withdrawal, STOP, integrity
  change and control loss still revoke affected inference.
- If streaming is introduced, fragmented events/UTF-8/split keys cannot bypass
  screening or admit partial output. Streaming is not selected by this record.
- Slow renewal or scheduling gaps retain honest control-unavailable outcomes;
  diagnostics cannot delay stopping. Parent/child/publication failures retain
  original identities and uncertainty; inspect/recover causes zero replay or refund.

[Runtime T1-T9 evidence](DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE.md#r2-completed-runtime-and-qualification-ledger)
qualifies its recorded source/environment, not every assertion for every provider.
The unsuccessful provider run `4a4ce477-5734-4b92-bb6a-dce74f430114` remains at
`D:/DuskWeave-build/rust-1.94.1/reports/provider-fixture-demo-20261022a/immediate/demonstration-report.md`.
It committed Decision 1 and fixture Terrain; Decision 2 ended exit 124 with
`may_send_uncertain`, with the exact killer unestablished. No inherited consent,
live acquisition or M1 seal follows from this design.

[Full historical assignment, corrections and evidence](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/workflows/DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE.md).

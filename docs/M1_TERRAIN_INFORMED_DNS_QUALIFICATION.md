# M1 Terrain-Informed DNS — historical qualification

These operator-DNS -> Terrain -> inference attempts predate the autonomous
successor. They are retained failures/blocks, not current qualification or consent.

| Episode | Tested source | Verdict / established result | Fixture |
| --- | --- | --- | --- |
| 20261018-1 | `fd73340c60c84d3fff49e459141a3b4f378631fd` | LIVE_FAIL: sole eligible Terrain premise omitted by byte fitting; reserved IDs compared incorrectly with actual calls. One generation/decision committed, zero repair. | `dw_native_live_da649bfb0f34bf1d` |
| 20261019-1 | `aadd5401a332a0b43b028c0a94eec2e92cfe59f1` | LIVE_BLOCKED: stdin EOF before observation; zero DNS/provider invocation; unknown durable counts remain null. | `dw_native_live_466203c294c1c597` |
| 20261019-2 | Same `aadd540` | LIVE_BLOCKED: one operator DNS observation admitted; 11s remaining vs >20s required, so no provider invocation. No refreshed timestamp or retry. | `dw_native_live_ddc8eab9e28534f0` |
| 20261019-3 | `ff9c3703c375990e4a55db9e68527e4dcd34c96f` | LIVE_FAIL: premise retained in P=3899; generation completed in 10084ms but shape required repair; repair remained may_send_uncertain, zero durable decisions/history/closures. | `dw_native_live_43b4dc45f93d682e` |

## Findings and evidence limits

F1/F2 code-only remediation (#67) retained the last supported premise or refused
before send, separated reserved/actual call identities, and moved setup ahead of
observation aging. Producer/SQL/consumer regressions qualified that code only;
the historical LIVE_FAIL did not become LIVE_PASS.

For episode 20261019-3, sent P retained `terrain_0`, generation usage was recorded,
and uncertain repair remained charged. A may-send marker proves neither entry
into `release_into` nor a send/provider receipt. Exact live exit/terminating guard
was not retained. Offline deadline diagnosis demonstrated a consistent mechanism,
not the historical cause: real transport with an in-memory Broker, two accepted
loopback connections, no proof of production SQL durability/request delivery.
Its 27-case schema/decoder matrix established only tested-case agreement.
The then-current shared deadline was not reset or widened by that diagnosis.

The qualified foreground handoff and synthetic one-console rehearsal did not
qualify application latency. The final operator handoff took 12.844s; the earlier
~48s post-response interval was not decomposed into human versus tooling costs.
An accidental second helper invocation was refused by existing report collision /
exclusive-write guards before fixture, registration, DNS or provider effects.

## Retained evidence

Local root: `D:/DuskWeave-build/rust-1.94.1/reports/dw-demo-m1-terrain-informed/`.
Each `episode-<id>/result/terrain_live_qualification.json` retains its verdict;
transport proof and one-console timing evidence remain alongside their original
directories. Corrected offline diagnosis is under
`reports/dw-diagnose-m1-schema-deadline/cycle-2`; cycle-1 is retained.
Exact binary hashes, original identities, detailed measurements and corrected
schema subtotals remain in the immutable text below. Raw provider/private
reasoning and DNS bytes were not retained in the report.

These episodes prove no autonomous acquisition, admitted adaptation loop, access,
cognitive MVP completion or M1 seal. DNS was operator-performed; report
`target_activity=false` describes platform activity, not absence of that DNS query.
Use [current state](ENGINEERING_STATE.md#6-current-slices-and-next-action) for the
next outcome; a new episode requires fresh authority and does not overwrite these.

[Full episode records and diagnostic qualifications](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/M1_TERRAIN_INFORMED_DNS_QUALIFICATION.md).

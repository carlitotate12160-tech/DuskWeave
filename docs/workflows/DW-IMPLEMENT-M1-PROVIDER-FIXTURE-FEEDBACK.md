# DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK — historical delivery

[PR #107](https://github.com/carlitotate12160-tech/DuskWeave/pull/107) merged
reviewed head `17480ed` at `2af7d01c9aa29c799d159d127a29b5766c60cd5f` on
2026-10-21. `m1-run` gained real-provider inference while retaining a deterministic
Broker DNS effect, owner-qualified Terrain, second-pack feedback and durable history.

Implementation lives in `src/m1_reasoning/transport.rs`, `src/m1_inference_control.rs`,
`src/m1_run*` and owner persistence. `m1_provider_fixture`, `m1_autonomous_transport`
and `m1_inference_control` exercise producer/wire binding, bounded revocation,
original clocks, uncertain accounting and key-free no-replay recovery.
The test-only relay's missing TCP_NODELAY caused the Linux readiness blocker;
its corrected sockets preserve framing/fault ordering. Windows affected tests
and Linux focused/full coverage qualified the merged source. F-B/C1-C4 and
FIX-cycle provenance remain in the historical text; no cycle count resets.

The merge delivered wiring/offline qualification, not a real-provider episode.
Later episode `4a4ce477-5734-4b92-bb6a-dce74f430114` did not complete feedback;
[current evidence/next action](../ENGINEERING_STATE.md#6-current-slices-and-next-action)
and the [current operator guide](../RUN_M1_AUTONOMOUS_FIXTURE.md) supersede old
profile instructions. No live DNS, provider retry consent or M1 seal is implied.

[Full historical assignment, corrections and evidence](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/workflows/DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK.md).

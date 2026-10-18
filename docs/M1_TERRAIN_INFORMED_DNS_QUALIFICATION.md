# M1 Terrain-Informed Live DNS Qualification — Episode 20261018-1

Bounded DEMO qualification of the merged Terrain-informed inference path
(PR #66 runner, commit `fd73340`). One operator-reviewed real DNS observation
for the operator-selected asset domain was admitted to `CyberTerrain`,
projected into an immutable `ContextPack`, sent once to the approved provider
profile, and reconciled through fresh-process inspect and two recoveries.

This episode proves only a bounded connected inference attempt. It does not
prove adaptation, DuskWeave target acquisition, access, cognitive MVP
completion, or an M1 seal. `dispatch_granted = false` on every receipt.

## Outcome: LIVE_FAIL (truthful completed attempt)

| Check | Result |
| --- | --- |
| Tested commit | `fd73340c60c84d3fff49e459141a3b4f378631fd` (BASE = origin/master) |
| Binary SHA-256 | `14c0e33bb0a9ee345106c622e2b23ac0e8fdd505a370049285b8363deec03c70` |
| Operator DNS | one UDP `A/IN` question to the declared public resolver, 3 s timeout, no retry/TCP/EDNS/follow-up; `noerror`, complete, 1 answer, TTL 1799 s |
| Operator review | projected fields reviewed and approved before intake, inside the 60 s freshness window |
| Terrain intake | accepted, `fresh`/`observed`/`tier_1`, `routing_usable`, no integrity hold; `perceived_at` ≤ `evaluated_at` < `valid_until` |
| Inference | 1 generation call, 0 repairs; 1 decision, 1 outbox, 1 accepted history, 1 closure; reserved 2 call slots |
| Historical checks | inspect + 2 recoveries: `pack_equal` true on all three; zero extra call/reservation records campaign-wide |
| Failure categories | `pack_terrain_absent`, `inference_calls_changed` ×4, `terrain_proof_absent` |

No automatic retry, second decision, fallback provider/model, or replacement
identity was attempted.

## Finding F1 — `pack_terrain_absent` (byte-bound omission)

The admitted claim was the campaign's sole eligible Terrain premise, yet the
ContextPack projected `shown_claims = 0` with `omitted_by_byte_bound = 1`. The
stored P is 3264 bytes against the fixed `context_bytes = 4096` profile
ceiling, and the fitting logic still omitted the sole claim entry — the exact
margin is not measured, but the headroom was at most 832 bytes. The fitting
order (drop an operator-only candidate before the sole eligible premise) had
no droppable unlinked candidate, so the premise itself was omitted. In this
configuration the mandated 4096-byte budget did not carry one claim entry
alongside the base pack.

Effect: the connected inference ran without any `terrain_*` reference, so no
proposal could cite the Terrain premise. Bounded finding for Work — the byte
budget/fitting behavior needs review outside this packet's file map.

## Finding F2 — `inference_calls_changed` (reserved vs actual call identities)

The durable decision event and all four receipts carry `call_ids =
[generation_call, repair_call]` — the two reserved call identities — while the
runner compares them to actual `execution.inference_calls` rows (1 row on a
no-repair decision, the unused repair slot leaving no call row). The check
cannot pass on any zero-repair decision: receipt `call_ids` names reserved
identities, not performed calls. Bounded finding for Work — runner check or
receipt semantics need reconciliation outside this packet's file map.

## Reasoning assessment

The admitted `validated_proposal` cites `[mission, asset_0]` (the only pack
references available). Its rationale explicitly identifies that the
`cyber_terrain` summary showed zero claims with one omitted by byte bound,
treats `asset_0` as a declared-only premise with unknown freshness, and its
`next_intent = observe_more` requests a bounded `cyber_terrain` view selected
to `asset_0` — the evidence-led direction the loop intends: it asked for the
missing premise rather than inventing it. `proposal_cites_terrain` is
UNVERIFIED, not refuted: no `terrain_*` identifier existed in P to cite.

Safe P exposes references, time, and status — not hostname or address values —
so it cannot establish semantic inference about the actual resolved address or
backend. A DNS answer is no proof of origin, ownership, reachability,
vulnerability, or access.

## Evidence and limits

- Evidence kind `real_operator_dns`; report JSON (local):
  `D:/DuskWeave-build/rust-1.94.1/reports/dw-demo-m1-terrain-informed/episode-20261018-1/result/terrain_live_qualification.json`;
  disposable fixture `dw_native_live_da649bfb0f34bf1d` retained on owned
  `dwpg-r3` (PostgreSQL 17.11, pinned image, loopback 5432).
- Controlled checks actually run: `cargo build --locked --bin duskweave`
  (toolchain 1.94.1, shared normal target) and `python -m unittest`
  `test_m1_terrain_live` (39) + `test_m1_native_helpers` (26) — all PASS.
  These are synthetic boundaries, not live evidence.
- The provider key was read only by the inference child; no raw provider
  output, CLI stdout, private reasoning, input copies, or DNS bytes are
  retained in the report.
- Unqualified and unchanged: acquisition, adaptation, target contact, access,
  M1 seal. `target_activity = false` inside the runner report refers to the
  platform's own network activity; the operator's single declared DNS query
  is recorded above.

## Next action

Work review of F1 and F2. A repeated episode is a separate operator decision;
this packet performs no retry.

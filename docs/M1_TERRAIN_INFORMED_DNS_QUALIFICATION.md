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

---

## Code-Only Remediation & Qualification (`DW-FIX-M1-TERRAIN-QUALIFICATION` R3)

The historical `LIVE_FAIL` episode (`dw_native_live_da649bfb0f34bf1d`) is
preserved unchanged. `DW-FIX-M1-TERRAIN-QUALIFICATION` (R3) repairs the
reproducible code and tooling defects above using deterministic synthetic
fixtures, the real restricted PostgreSQL database, and an injected provider,
with zero external DNS queries or live provider calls:

1. **Prior audit measurements vs R3 lossless deduplication and last-premise floor (`src/m1_context.rs`)**:
   - Prior audit measurements established that reconstructing the historical
     2-limitation claim (`cache_remaining_ttl`, `single_vantage_only`) added
     835 bytes (799-byte reference + 36-byte linkage) to the 3264-byte base
     pack for `4099` UTF-8 bytes against `4096` (while a 1-limitation fixture
     fit at `4081` bytes).
   - `m1_context::prepare` now omits the 4× repeated per-entry
     `reason: "not_loaded_by_this_profile"` on `foothold_graph`,
     `attack_path_view`, `objective_state`, and `campaign_trajectory` and
     replaces the three duplicated `pending_unknown` strings with
     `["other_owner_views_not_loaded_by_profile"]` (saving 200 UTF-8 bytes
     while preserving all six owner entries, timestamps, Terrain unavailable
     reasons, operator premise metadata, and capacity
     `reservation_granted = false` / `shared_call_clock = "not_started"`).
   - **Last-premise sufficiency policy**: when the validated,
     disclosure-permitted loaded Terrain view has at least one fresh usable
     `Observed` claim for the originally selected candidate prefix under
     consent, successful preparation must retain at least one such claim, its
     linked candidate, its pack reference, and its `TerrainDependency`. Byte
     fitting drops optional unlinked operator candidates first and may omit
     surplus claims, but fails closed with
     `Fail::State("context_bytes_insufficient")` rather than dropping the final
     supported premise (preserving `context_size_deferred` when no eligible
     claim exists and mandatory metadata itself cannot fit).
   - **Measured R3 capacity and bindings** (`tests/support/m1_terrain_capacity.rs`):
     - Historical 2-limitation shape: `P = 3899` bytes (`197`-byte margin under
       `4096`), `configuration = 5462` bytes (`10922`-byte margin under
       `16384`), generation `body = 8891` bytes (`7493`-byte margin), 7-code
       repair `body = 9111` bytes (`7273`-byte margin; `repair_bytes = 147`
       under `2048`).
     - All 8 routing-eligible `DnsLimitation` subsets fit `4096` (`P <= 3932`
       bytes, margin `164` bytes).
     - Combined worst-case numeric-width boundaries at PostgreSQL `timestamptz`
       max epoch (`9_224_318_015_999`, 13 digits), `i64::MAX` epoch and
       5-dimension campaign capacity, `300` s uncertainty, `86400` s TTL, 3
       limitations, `medium` priority, and 64 supplied candidates:
       `P = 4075` bytes (`21`-byte margin; generation `body = 9069`, repair
       `body = 9289`) at intake max `claim_revision = 10001`, and `P = 4094`
       bytes (`2`-byte margin; generation `body = 9088`, repair `body = 9308`)
       at pure-view `u32::MAX` `claim_revision` and `omitted_by_claim_limit`.
   - **Native zero-send floor disposition** (`tests/support/m1_terrain_live_contract.rs`,
     `tests/m1_terrain_context_cli.rs`): under smaller consent (`3500` bytes),
     `command_with("m1-infer", ...)` fails at `m1_context::prepare` with
     `Fail::State("context_bytes_insufficient")`, preserves the allocated
     decision identity in `execution.native_input_bindings`, and produces zero
     provider prepare/call invocations, zero `inference_reservations`, zero
     `inference_calls`, zero `reasoning_decisions`/outbox/history/closures, and
     zero quota charge; `m1-context-prepare` emits its `deferred` receipt with
     `pack_utf8: null`, all false grant flags, and
     `error=context_bytes_insufficient`.
2. **Reserved vs actual call verification (`tests/support/m1_terrain_live.py`)**:
   - Reads the scoped initial `execution.inference_reservations` row
     (`record_revision = 1`, `kind = 'reserved_not_sent'`), validates canonical
     distinct non-nil UUIDs `[generation_call_id, repair_call_id]`, compares
     receipt `call_ids` to that exact ordered pair without sorting,
     independently verifies actual `execution.inference_calls` rows (generation
     required, repair optional, no duplicates or foreign IDs), and reports
     `reserved_call_ids` and `actual_call_ids` separately.
   - Real Rust-to-Python contract regression
     (`terrain_connected_command_with_receipt_and_sql_pass_python_verifier`)
     passes the actual `command_with` receipts and PostgreSQL rows over stdin to
     the Python verifier and confirms `LIVE_PASS` for 2 reserved IDs / 1 actual
     generation row plus rejection of role-reversed `call_ids`.
3. **Setup before observation ageing (`--await-reviewed-observation`)**:
   - Validates static inputs while requiring observation initially absent,
     provisions the fixture and registers Mission once, emits flushed safe
     `READY` with no open SQL transaction or campaign lock, reads one bounded
     `continue` line (`<= 16` UTF-8 bytes), verifies static snapshots unchanged,
     admits the observation preserving original timestamps, and requires
     `remaining_lifetime_seconds` (`valid_until - now`) to exceed the consented
     `decision_wall_time_ms` before invoking `m1-infer`.
   - Note on the historical episode timing: the observation was 56 seconds old
     when Terrain was recorded (~3 seconds of freshness remaining before
     ~8.175 seconds of provider latency); the portions of that 56-second
     interval were not decomposed and are not attributed solely to provisioning.
4. **Limits**: this remediation is code-only qualification. It does not
   reclassify Episode `20261018-1` as `LIVE_PASS`, execute a new live DNS or
   provider episode, or qualify DuskWeave target acquisition, adaptation,
   cognitive MVP completion, or an M1 seal.

---

## Episode 20261019-1 — LIVE_BLOCKED (truthful completed attempt)

Bounded DEMO retest at BASE `aadd5401a332a0b43b028c0a94eec2e92cfe59f1`
(origin/master after the PR #67 merge; `demo/m1-terrain-retest-20261019`) of the
same operator-reviewed DNS -> Terrain intake -> immutable ContextPack ->
approved provider -> fresh-process inspect/two recoveries lane for the
operator-selected asset domain.

| Check | Result |
| --- | --- |
| Tested commit | `aadd5401a332a0b43b028c0a94eec2e92cfe59f1` (origin/master = BASE) |
| Binary SHA-256 | `9129c7d9a6a82855e21c3198181143c7c58ca7074201e67f2ac86ac914995023` |
| Setup | `dwpg-r3` requalified (pinned PostgreSQL 17 image, loopback 5432, running); `cargo build --locked --bin duskweave` on toolchain 1.94.1, shared ordinary target; 70 deterministic tests PASS (`test_m1_terrain_live` 39, `test_m1_terrain_live_contract` 5, `test_m1_native_helpers` 26); fresh fixture `dw_native_live_466203c294c1c597`; Mission registration accepted once (`registration_operation_id` `47fc6f6d-f24a-4ce1-ad85-1ee9afe00a47`); safe `READY` emitted with no open transaction or campaign lock |
| Operator DNS | none — the observation phase was never reached; zero datagrams sent |
| Provider | none — `m1-infer` was never invoked; no attempt, so no possible send |
| Outcome | `LIVE_BLOCKED`, failure `operator_continue_eof`; counts unknown (`null`, not measured zero) |

The runner's single bounded stdin `continue` read for the reviewed-observation
handoff received EOF after READY: this execution environment does not retain an
interactive writable stdin for a backgrounded runner process. Per packet the
blocker is reported before DNS; no setup-after-observation substitution was
used, `inputs/observation.json` was never created, the static inputs remained
byte-frozen, and no second runner invocation was permitted or attempted.
`dispatch_granted = false`, `automatic_retry = false`, `target_activity = false`
(the report's flag covers the platform's own activity; no operator DNS query
was made in this episode). The fixture database and the registered Mission are
retained as evidence; `dw_native_live_da649bfb0f34bf1d` and the historical
`LIVE_FAIL` record remain unchanged.

No mechanical-path, freshness, or reasoning conclusion is established: Terrain
intake of a new observation, connected inference, proposal citation, and the
premise-retention and reserved-vs-actual identity checks are unqualified live
for this episode. This is operator_input admission tooling only — no
DuskWeave acquisition, adaptation loop, target contact, access, cognitive MVP
completion, or M1 seal.

### Next action

A further live episode requires an execution environment able to retain
interactive stdin for the runner's `continue` handoff (or a separately
authorized bounded handoff mechanism) and remains a fresh operator decision
with current consent; this packet performs no retry.

---

## Stdin transport qualification — 20261019 (pre-episode proof)

Before any new live invocation the stdin transport was qualified with the
packet-supplied `stdin-probe.py` (no fixture, database, DNS, credential,
provider, or production CLI involvement):

| Mechanism | Result |
| --- | --- |
| IDE agent exec tool (persistent shell session, non-PTY) | `PROBE_BLOCKED` — `readline` hit `stdin_eof` at launch (pid 27156); a later interaction cannot deliver input |
| Operator foreground interactive PowerShell | `PROBE_ACK` — `READY` (pid 16496), process stayed alive on `readline`, operator later typed the printed challenge, exit 0 |

Qualified transport: **operator foreground terminal**. Evidence:
`D:/DuskWeave-build/rust-1.94.1/reports/dw-demo-m1-terrain-informed/transport-probe-20261019/transport-proof.json`
(local). Two earlier same-console probe invocations ended `invalid_input`
(operator typed placeholder/a space) while still demonstrating stdin delivery.

## Episode 20261019-2 — LIVE_BLOCKED (truthful completed attempt)

Same bounded lane for the operator-selected asset domain, executed through the
qualified foreground terminal at BASE `aadd5401a332a0b43b028c0a94eec2e92cfe59f1`.

| Check | Result |
| --- | --- |
| Tested commit | `aadd5401a332a0b43b028c0a94eec2e92cfe59f1` (origin/master = BASE) |
| Binary SHA-256 | `9129c7d9a6a82855e21c3198181143c7c58ca7074201e67f2ac86ac914995023` (unchanged BASE binary) |
| Setup | `dwpg-r3` requalified (pinned PostgreSQL 17 image, loopback 5432, running); fresh fixture `dw_native_live_ddc8eab9e28534f0`; Mission registration accepted once (`60f2ae07-849d-4f71-99b5-65b4764765f9`); `READY` emitted; runner alive with qualified stdin |
| Handoff | PASS — `continue` delivered through the qualified terminal; `operator_continue_eof` did not recur |
| Operator DNS | exactly one UDP `A/IN` datagram to the declared resolver, 3 s timeout, no retry/fallback/follow-up; `noerror`, complete, 1 answer, TTL 1799 s, RTT ~78 ms, `uncertainty_seconds` 1; operator reviewed the projected fields and approved |
| Terrain intake | accepted — `observed`/`fresh`, `claim_revision` 1, no integrity hold, `perceived_at` 1792383493, `valid_until` 1792383553, `temporal_frontier` 1792383493, `evaluated_at` 1792383541; full original lineage (operation `41bdf8f3`, event `05806b06`, observation `7a0046d6`, material `f4f61076`, evaluation `fa0abae6`, claim `b7e3e2c2`) |
| Provider | none — blocked in preflight before `m1-infer`; no attempt, so no possible send |
| Outcome | `LIVE_BLOCKED`, failure `terrain_freshness_insufficient`; `remaining_lifetime_seconds` 11 < 20 required; counts unknown (`null`, not measured zero) |

The combined post-response interval — DNS response receipt through review,
observation write, `continue` handoff, and Terrain intake evaluation — was
~48 s of the observation's 60-second capped lifetime (`valid_until =
perceived_at + min(lowest_ttl, 60)`), leaving 11 s against the required
`> 20 s` inference window. The individual review/write/handoff/intake
durations were not separately measured; the interval is the combined
post-response span, not human review time alone. Per packet the margin loss is
reported, not rescued: no recollection, timestamp rewrite, validity extension,
or second runner invocation.
`dispatch_granted = false`, `automatic_retry = false`, `target_activity = false`;
the fixture and registered Mission are retained as evidence, and all prior
reports/databases remain unchanged.

Mechanical proof advanced but the loop is still unqualified live: the qualified
stdin transport and real Terrain intake of a current observation now pass;
ContextPack send, connected inference, proposal citation, and recoveries remain
unestablished. No acquisition, adaptation, access, cognitive MVP completion,
or M1 seal.

### Next action

Any further episode needs a review-to-continue path inside the freshness
margin (for example a faster operator review loop or a separately authorized
bounded handoff) plus fresh operator consent; this packet performs no retry.

---

## One-console review helper — 20261019 (synthetic qualification)

To remove avoidable post-DNS agent/file/terminal round trips while preserving
actual operator review, a one-use local helper `one_console.py` was built
outside the repository (external evidence dir; not committed): the foreground
parent spawns the unchanged `tests/support/m1_terrain_live.py` with
`stdin=PIPE` held open, performs the bounded collection, displays the safe
observation projection, requires one explicit `approve` token, then
automatically writes that exact approved observation with exclusive creation
and flushes `continue` into the same runner pipe. EOF, rejection, invalid
input, timeout, pre-existing file, write failure, or any safety check failure
sends no `continue` and closes the owned child so it can record BLOCKED.

- 14 synthetic unittest cases against a real subprocess pipe
  (`rehearsal_child.py`): READY-before-collection, file-absent-before-review,
  `continue` only after a complete approved file, rejection/EOF/invalid/
  timeout/write-failure/existing-observation paths, exact field/TTL/timestamp
  preservation, a sensitive sentinel never reaching report surfaces, and
  no-sleep 30 s budget/expiry boundaries — all PASS.
- One representative foreground rehearsal with actual operator review of a
  clearly synthetic observation (documentation-only `example.invalid` data; no
  DB/DNS/provider/credentials): `REHEARSAL_PASS`, measured
  receipt→approval→write→continue flush = **9.156 s** ≤ 30 s target;
  `REHEARSAL_ACK` verified by the child. Evidence:
  `D:/DuskWeave-build/rust-1.94.1/reports/dw-demo-m1-terrain-informed/one-console-20261019/rehearsal-proof.json`
  (local). The rehearsal proves the human/controller handoff only — not
  production DB/provider latency or a live outcome.
- Exact-master CI run 39200545621 at BASE `ff9c370` reached SUCCESS before any
  live invocation.

## Episode 20261019-3 — LIVE_FAIL (truthful completed attempt)

Same bounded lane at BASE `ff9c3703c375990e4a55db9e68527e4dcd34c96f`
(origin/master after the PR #68 merge on 2026-10-19T04:39:13Z; branch
`demo/m1-one-console-20261019`), executed through the qualified foreground
console helper.

| Check | Result |
| --- | --- |
| Tested commit | `ff9c3703c375990e4a55db9e68527e4dcd34c96f` (origin/master = BASE; CI 39200545621 SUCCESS) |
| Binary SHA-256 | `23ed9b0f7b1414a455e0b2ffc96a6ca2c1966319bfeeacbb3315f2ee2555dab4` |
| Setup | `dwpg-r3` requalified (pinned PostgreSQL 17 image, loopback 5432, running); fresh fixture `dw_native_live_43b4dc45f93d682e`; Mission registration accepted once (`a3f7426c-234e-4786-889c-a806bde7d1cf`); `READY` emitted; observation absent until approved |
| Handoff | PASS and measured — DNS response receipt → operator `approve` → complete `observation.json` write → `continue` flush = **12.844 s** ≤ 30 s target (helper `timing-summary.json`); the ~48 s combined post-response interval of episode `20261019-2` was eliminated |
| Operator DNS | exactly one UDP `A/IN` datagram to the declared resolver, 3 s timeout, no retry/fallback/follow-up; `noerror`, complete, 1 answer, TTL 3600 s, `perceived_at` 1792385928, `uncertainty_seconds` 1; operator reviewed the displayed fields and typed `approve` |
| Terrain intake | accepted — `observed`/`fresh`, `claim_revision` 1, no integrity hold, `evaluated_at` 1792385941, `valid_until` 1792385988 (60 s cap), `temporal_frontier` 1792385928, `remaining_lifetime_seconds` 47 > 20 required; full original lineage (operation `8162a737`, event `7af2a79d`, observation `3b99631d`, material `4b0ad5ca`, evaluation `983c8750`, claim `fcce48ce`) |
| Inference | `m1-infer --operation new` ran once; decision `05be8971-0735-43b9-88ac-ec6dd39c6e07`; P 3899 bytes, `pack_sha256` `7b159b65…`; `terrain_reference_ids` `["terrain_0"]` carried in the original sent P; `reserved_call_ids` == `actual_call_ids` exactly (`4fab9f56…`, `a2ec3b8a…`); `generation_repair_pack_equal` true; 1 reservation record / 2 reserved calls; `extra_call_records` 0, `extra_reservation_records` 0 |
| Provider | generation call `4fab9f56…` **completed** (usage: input 2653 / output 2012 / reasoning 1552 tokens, `wall_time_ms` 10084, requested and reported model `deepseek-flash`, `response_bytes` 15943); admission `Repairable`, code **`proposal_shape`** (retained repair body); repair call `a2ec3b8a…` remained **`may_send_uncertain`**, usage NULL — the process exited while the repair send was in flight; whether it reached the provider is unknown (honest uncertain accounting) |
| Durable completion | `decisions`/`outbox`/`history`/`closures` all 0 — no committed decision, no receipt; inspect + 2 recoveries all `pack_equal` true but `verified` false (`identity_invalid`: no committed decision identity) |
| Outcome | `LIVE_FAIL` — failures `inference_exit`, `inference_receipt_absent`, `identity_invalid` ×3, `durable_completion_absent`; `dispatch_granted` false, `automatic_retry` false, `target_activity` false |

## Finding F3 — `inference_exit` (repair died inside the provider send; shared deadline is the leading hypothesis)

### Facts (directly retained evidence)

- The reservation's shared `decision_wall_time_ms = 20000` covers the
  generation call AND the one permitted schema repair.
- Generation call `4fab9f56…` is `completed` with usage `wall_time_ms` 10084 —
  ~10.1 s of the shared 20 s window consumed; admission was `Repairable`
  with code **`proposal_shape`** (the retained repair body carries
  `{"repair_error_codes":["proposal_shape"]}` — the generation response did
  not decode to the required proposal shape).
- Repair call `a2ec3b8a…` passed `prepare`/`may_send` (row →
  `may_send_uncertain`) and entered `release_into`/`p.call`. Its state never
  reached `usage_unknown`, `failed_never_released`, or `completed`, and usage
  is NULL: the process exited inside the send window — after the may-send
  marker committed and before any post-send state transition.
- `m1-infer` exited nonzero within the runner's 40 s timeout (no
  `inference_timeout` recorded); the exit code itself was not retained in the
  report. No receipt, no decision/outbox/history/closure rows.
- The uncertain accounting is correct: whether the repair request reached the
  provider is unknown — neither claimed nor denied.

### Hypothesis (consistent with all evidence, not directly observed)

The deadline watchdog in `src/m1_reasoning/transport.rs` (`Deadline::arm` →
`std::process::exit(124)`) fires `process::exit` when the shared deadline
elapses. With ~10.1 s consumed by generation, a repair response of comparable
provider latency (~10 s) would still be in flight at deadline expiry, and the
forced exit produces exactly the observed state (persisted
`may_send_uncertain`, no receipt, nonzero exit). Alternative explanations
(e.g., a transport-level abort) would ordinarily transition the row to
`usage_unknown` or `failed_never_released`; a mid-call `process::exit`/fatal
abort is the mechanism consistent with the persisted marker.

### Offline diagnosis (DW-DIAGNOSE-M1-SCHEMA-DEADLINE)

- **Schema contract**: the emitted provider schema (real `request::bind`
  over a `m1_context::prepare` pack) and the local decoder were checked on
  27 synthetic cases via `jsonschema` 4.26.0 (Draft 2020-12) — zero
  expectation disagreements, no supported-shape defect. Every schema-valid
  refusal comes from an intentionally stricter native bound (byte vs
  character `maxLength`, trim, premise/eligibility); the only local-side
  tolerance is omission of required-but-nullable keys, a harmless
  direction. The live generation's `proposal_shape` violation field remains
  unknown because provider text is not retained.
- **Deadline reproduction**: the real `decide` + concrete transport against
  loopback reproduced the observed shape exactly — generation `completed`,
  repair `may_send` marker committed, then `process::exit(124)` inside
  `provider.call` while the repair send was in flight (unscaled child:
  exit 124 at 20027 ms with the server observing 2 requests; scaled child
  identical at 1549 ms). Controls: an expired shared deadline before
  release yields `NeverReleased` with no second request, and a disarmed
  deadline never exits.
- **Attribution limit**: the `database_wait` command monitor also produces
  exit 124 with no transport involvement, so the code alone cannot attribute
  the historical killer — the reproduction proves the mechanism can produce
  the exact ledger shape, not that it fired in episode `20261019-3`.
- Report: `D:/DuskWeave-build/rust-1.94.1/reports/dw-diagnose-m1-schema-deadline/cycle-1`.

### Bounded finding for Work

The reproduction confirms the failure shape is reachable through the real
transport: the shared wall budget across generation+repair leaves
insufficient headroom for a repair of comparable provider latency, and
deadline expiry terminates the process before accounting can settle.
Candidate corrections (raise `shared_wall_time_ms`, give the repair its own
deadline window, or make deadline expiry close accounting gracefully before
exit) are Work/engineering decisions, not in-demo patches.

## Reasoning assessment (20261019-3)

No admitted proposal exists: the decision was never durably committed, so
`proposal_cites_terrain` is UNVERIFIED, not refuted. Notably, the original sent
P did carry `terrain_reference_ids` `["terrain_0"]` — the premise-retention
fix from PR #67 held in live use — but whether the model's output would have
cited it cannot be assessed from an uncommitted generation.

## Evidence and limits (20261019-3)

- Evidence kind `real_operator_dns`; runner report (local):
  `D:/DuskWeave-build/rust-1.94.1/reports/dw-demo-m1-terrain-informed/episode-20261019-3/result/terrain_live_qualification.json`;
  helper `timing-summary.json` alongside; disposable fixture
  `dw_native_live_43b4dc45f93d682e` retained on owned `dwpg-r3`.
- After the `LIVE_FAIL`, one accidental second helper `live` invocation was
  attempted; the unchanged runner's report-path collision guard stopped it
  before any fixture, registration, DNS, or provider activity, and the helper's
  exclusive write refused to overwrite `timing-summary.json`. No retry effect
  occurred; all episode evidence derives from the single authorized
  invocation.
- The provider key was read only by the inference child; no raw provider
  output, private reasoning, or DNS bytes are retained in the report.
- Established live for the first time: real Terrain-informed ContextPack send
  with premise retained (`terrain_0` in P), reserved==actual call identity, a
  completed provider generation with recorded usage, and correct uncertain
  accounting for an interrupted repair. Still unqualified: committed
  decision/receipt, admitted proposal, citation, adaptation, acquisition,
  access, cognitive MVP completion, M1 seal.

### Next action

Work review of F3 (shared decision wall-time vs generation+repair latency).
Any further live episode is a separate operator decision with fresh consent
and, per the finding, likely needs the wall-budget decision resolved; this
packet performs no retry.

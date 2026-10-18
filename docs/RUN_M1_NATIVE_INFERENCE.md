# Running M1 Native Inference

## Overview

DuskWeave provides production-native initial reasoning inference via the `duskweave` CLI.
The native inference path executes the authorized sequence:

```text
ContextPack
  -> Broker Reservation
  -> Bound HTTP/TLS Request (DeepSeek Responses API)
  -> Proposal Validation & Reasoning Discard
  -> Cognition Atomic Decision Publication
  -> Trajectory Historical Record
  -> Hold Closure & Capacity Accounting
```

This workflow supports evidence-led hypotheses, adaptation, and restrained footprint
using APT41, Lazarus, and Volt Typhoon as behavioral lenses—never as execution
playbooks or execution authority. In all cases, `dispatch_granted = false` is preserved
on every receipt; no target contact or automated dispatch is authorized.

## Configuration & Role Separation

Native inference requires four distinct PostgreSQL role connections configured via
`DW_DATABASE_CONFIG_MODE` (e.g. `env-local`):

| Environment Variable | Database Role | Permissions |
| --- | --- | --- |
| `DW_DATABASE_URL` | Core (`dw_runtime`) | Mission registration and core authority |
| `DW_INFERENCE_BROKER_DATABASE_URL` | Broker (`dw_m1_broker`) | Reservations, capacity snapshots, hold closure |
| `DW_COGNITION_DATABASE_URL` | Cognition (`dw_m1_cognition`) | Atomic decision and outbox publication |
| `DW_TRAJECTORY_DATABASE_URL` | Trajectory (`dw_m1_trajectory`) | Narrow reasoning history append and inspection |

Transport reads `DEEPSEEK_API_KEY` exclusively inside its private transport boundary.
API keys and raw DSNs are never reflected, logged, or emitted in receipts.

## Consent & Profile Specification

New native campaigns require explicit consent:

- **Permission Version**: `m1_inference_permission_v2`
- **Profile**: `deepseek-responses-envelope32-v1`
- **Bounds**: Envelope JSON depth 32; ContextPack and proposal JSON depth 8.
- **Provider Target**: Fixed endpoint `https://api.deepseek.com/responses`, model `deepseek-flash`, `stream=false`, `reasoning={"effort":"high"}`, `tools=[]`.
- **Budgets**: At most 2 provider calls per decision (1 generation + at most 1 conditional schema repair).
- **Repair disclosure**: `admitted_repair_fields_codes` is mandatory for native v2.
- **Fixed settings**: Envelope32, collection64, response65536, proposal8192, text1024,
  repair2048, repair-code-count8, connect5000ms, calls2, output4096 and concurrency1.
  Smaller positive context/candidate/request/configuration settings remain supported.

Historical `m1_inference_permission_v1` campaigns are preserved and never silently upgraded.

## CLI Commands

### 1. New Inference Attempt

Initiate a new LLM reasoning attempt for an admitted ContextPack request:

```bash
duskweave m1-infer --input /path/to/request.json --operation new
```

The request JSON must contain an admitted `m1_context_prepare_request_v1` payload with
valid campaign scope and current consent. Upon successful completion, `m1-infer` outputs
an `m1_inference_receipt_v1` JSON object to stdout.

### 2. Inspect Existing Decision

Inspect the durable state of a previously issued or in-flight decision without making provider calls:

```bash
duskweave m1-infer-inspect --input /path/to/request.json --operation <DECISION_UUID>
```

Inspection verifies Cognition publication and Trajectory history without network interaction.

### 3. Recover Interrupted Decision

Recover from lost publication or history acknowledgments in a fresh process:

```bash
duskweave m1-infer-recover --input /path/to/request.json --operation <DECISION_UUID>
```

Recovery completes pending Trajectory obligations or reconstructs terminal state.
Recovery never re-generates proposals or initiates provider calls.

The complete validated input, including candidates, is bound immutably to the
core-allocated decision before reservation. Keep that identity after a binding
acknowledgment failure. Inspect/recover compares the complete input hash even when
only the binding exists; it cannot retrofit a binding onto a historical reservation.

## Terrain Evidence Premises, Current-Use Rechecks & Historical Inspection

1. **Exact Pack-Local References (`m1-initial-inference-v3`)**:
   - When `owner_qualified_summaries` is disclosed and eligible fresh Terrain DNS
     claims exist for selected candidate assets, `ContextPack` assigns deterministic
     pack-local IDs (`terrain_0..terrain_7`) alongside `mission` and `asset_0..`.
   - Every `terrain_*` entry represents narrow `OBSERVED` (`Tier1`,
     `corroborated = false`) evidence from `operator_reviewed_dns`
     (`dns_hostname_ipv4_resolution`), never corroborated fact or target permission.
   - Proposals must cite exact pack-local IDs (`mission`, `asset_*`, `terrain_*`);
     substituting raw UUIDs, JSON paths, or omitted/unknown IDs is rejected.
2. **Pre-Release Launch Fence vs Cited-Reference Current-Use Validation (`TerrainBasis`)**:
   - **Before provider release (`before_provider_release`) and launch ordering**:
     inside `PgBroker::release_into`, the Broker acquires the campaign session
     advisory lock (`hashtextextended(engagement_id::text || ':' || campaign_id::text, 0)`),
     verifies current Mission authority, `owner_qualified_summaries` disclosure,
     and every serialized `TerrainDependency` in the `ContextPack` via
     `terrain.current_dns_context_view` filtered by exact `claim_ids` (which
     validates both canonical payload lineage and every projected scalar/index
     column across `terrain.current_claims`, `terrain.claim_revisions`,
     `terrain.evaluations`, `terrain.materials`, `terrain.observations`, and
     `terrain.outbox`), and holds that session fence only until actual transport
     launch (`release()`). Narrow transaction-scoped write-guard triggers on
     `terrain.current_claims`, `terrain.integrity_conflicts`, and
     `trajectory.terrain_history` acquire `pg_advisory_xact_lock` on the same
     campaign key, ordering Terrain supersession and integrity-hold writes with
     the release check-to-launch interval. If an invalidation commits before
     release admission, release returns `NeverReleased` (`reference_ineligible`
     or `current_authority_unavailable`) and zero provider calls are sent; if
     release wins the fence, a concurrent Terrain writer cannot commit before
     transport launch and proceeds immediately after `release()` unlocks the
     session fence (before LLM response waiting, so no transaction, row, or
     campaign advisory lock is held across provider latency).
   - **During transfer (`current_authority_and_premises`)**: progress callbacks check
     current Mission authority and the operation deadline without re-querying all
     Terrain dependencies, so an uncited independent claim changing mid-flight does
     not cancel an in-flight call.
   - **After response (`current_authority_for_references`)**: `proposal::admit`
     rechecks current Mission authority and only the exact cited `terrain_*`
     dependencies (union of `proposal.references`, `next_intent.references`, and
     `view_selectors`). If a cited Terrain premise became stale, superseded, held,
     or conflicted during transfer, admission rejects the proposal with durable
     `reference_ineligible` (non-repairable); if only an uncited premise changed,
     the supported proposal is admitted.
3. **Historical Pack & Proposal Inspection (`pack_state`, `pack_utf8`, `proposal`)**:
   - Every `m1_inference_receipt_v1` reports `pack_state` (`"available"`,
     `"not_bound"`, or `"unavailable"`), `pack_utf8` (the exact original UTF-8
     `ContextPack` bytes when durably bound in `execution.inference_calls`), and
     `proposal` (the admitted stored `Proposal` when present).
   - `PgBroker::inspect_pack` reads at most the two original call rows for the
     decision, verifies each row's stored `context_pack` / `configuration` / `body`
     against its `binding` via `request::verify_binding`, enforces byte-for-byte
     equality between generation and repair `context_pack` payloads, and checks
     that any committed decision's `bindings` match the call bindings. Tampered or
     mismatched bytes fail closed (`inference_decision_integrity`).
   - On continuation (`m1-infer-inspect`, `m1-infer-recover`, and repeated
     `m1-infer` for a known decision identity), a transient pack-read failure
     (`inference_inspect_unavailable`, such as unreadable pack columns while
     decision history and call-state accounting remain readable) yields an
     explicit receipt with `pack_state: "unavailable"`, `pack_utf8: null`,
     preserved historical `decision`/`event`/`proposal`/`history`, honest
     `consumption`, `recovery_category: "no_provider_replay"`, and
     `dispatch_granted: false`. Hard integrity failures
     (`inference_decision_integrity`) and input-binding conflicts remain
     fatal errors and are never mapped to `"available"` or `"not_bound"`.
   - `m1-infer-inspect`, `m1-infer-recover`, and repeated `m1-infer` for a known
     decision identity never construct a provider, query current Terrain state, or
     re-serialize `ContextPack` bytes.

## Hold Lifecycle & Anomaly Dominance

1. **Automatic Closure**: Terminal decision completion automatically closes the exclusive
   session hold via `execution.close_inference()`, charging 2 call slots to cumulative capacity.
2. **Campaign Continuation**: Subsequent decisions in the same campaign succeed only after prior
   holds close and remaining campaign budget permits.
3. **Anomaly Dominance**: Conflicting identity or content produces durable anomaly records in
   `trajectory.reasoning_history`. Under anomaly dominance, hold closure is refused, blocking
   further unprincipled use of disputed identities.

A late verified conflict also invalidates dependent continuation despite a prior
stored closure. Historical charges remain; no recovery refunds uncertain consumption.

## Native fixture qualification

Use `tests/support/m1_native_bootstrap.py` before launching Rust tests. It qualifies
the retained owned PostgreSQL service and creates a disposable database in the
`dw_native_` test namespace with unique random-password logins via
`createuser --pwprompt`. Credentials remain in the bootstrap/child process
boundary. Login or database collisions fail closed; an existing database name is
always refused — the helper never drops, overwrites or silently reuses one, and
no retained role password is reset. Rust fixtures require explicit native DSNs,
validate the `DW_NATIVE_FIXTURE_DATABASE` namespace, and cannot synthesize
credentials or mutate the process environment.

```bash
# Provision a fresh generated fixture database, then run a focused native test:
python tests/support/m1_native_bootstrap.py --container dwpg-r3 \
  --report <report-dir> -- cargo test --test m1_native_cli -- --test-threads=1
```

- `--database fixture|live` selects the provisioning kind (the `fixture` kind adds
  the negative-principal controls); the default is `fixture`.
- `--database-name <name>` selects an explicit database name. When omitted a
  unique name is generated (`dw_native_<kind>_<random>`). Every name must match
  `dw_native_[a-z0-9_]+` within PostgreSQL's 63-byte identifier limit; validation
  runs before any provisioning call.
- The selected name is passed to Rust tests through `DW_NATIVE_FIXTURE_DATABASE`
  in the child environment.

Run focused native tests locally. CI runs the complete suite and coverage on its
isolated disposable service; legacy cluster-role bootstrap is not authorized locally.

## Terrain-Informed Qualification Runner

Use the dedicated [Terrain runner](../tests/support/m1_terrain_live.py), not the
inference-only helper, to qualify operator-reviewed DNS -> Terrain intake ->
immutable ContextPack -> native provider decision/history -> historical recovery.
This is qualification tooling, not a collector or production workflow engine.

```bash
python tests/support/m1_terrain_live.py \
  --binary <committed-candidate-duskweave-binary> \
  --report <fresh-report-directory> \
  --registration <operator-approved-registration.json> \
  --observation <operator-reviewed-dns-observation.json> \
  --context-request <operator-approved-context-request.json> \
  --evidence-kind real_operator_dns \
  [--env-file .env] [--database-name <dw_native_live_...>]
```

All three files are mandatory, remain operator-owned, and are never copied into
the report. Each orchestration read is bounded to 16 KiB. Keep them unchanged
during the run. The binary remains authoritative for scope, consent, disclosure,
evidence admission and current-use eligibility:

- Registration uses the existing `register` format: an approved Mission with its
  M1 DNS purpose/vantage/resolver policy plus current `m1_inference_permission_v2`,
  `deepseek-responses-envelope32-v1`, recipient/terms approval, finite limits and
  explicitly approved `owner_qualified_summaries`. The runner never adds consent,
  approval references, assets or scope.
- Observation uses [the existing Terrain DNS format](RUN_M1_TERRAIN.md):
  `m1_terrain_dns_observation_v1`, `source_kind = operator_reviewed_dns`, the same
  engagement/campaign and selected asset, and independently reviewed non-sensitive
  DNS material. Preserve its original perception time, uncertainty and TTL.
- Context request uses [the existing preparation format](RUN_M1_CONTEXT_PREPARE.md):
  `m1_context_prepare_request_v1`, matching Mission/permission and a candidate
  containing the observation's asset. No caller-supplied accepted view or pack.

Use `--evidence-kind synthetic_input_provider_qualification` only for a separately
authorized provider episode with clearly synthetic operator inputs. The required
label is an operator declaration, not automated proof of observation provenance.
Neither evidence kind is DuskWeave DNS acquisition or permission for target contact.
No real Mission/observation/context files or current provider consent accompanied
this tooling delivery: actual qualification is `LIVE_NOT_EXECUTED`.

### Sequence And Proof Criteria

The existing bootstrap qualifies the retained owned `dwpg-r3` service (pinned
PostgreSQL 17 image, running, loopback port 5432), provisions one fresh `live`
database and distinct restricted owner logins, and applies existing migrations.
Omitting `--database-name` generates a unique `dw_native_live_*` name. Existing
databases/logins are refused, never reset, dropped or reused. Serialize this
role-mutating run with other fixture owners; retain interrupted databases/roles.

The runner registers exactly the supplied Mission, records the observation once
through `m1-terrain-record --operation new`, and requires accepted history, a fresh
usable OBSERVED claim and no integrity hold before inference. Freshness is the
existing Terrain window, capped at 60 seconds from the original `perceived_at`;
input preparation, provisioning and inference all consume that window. Expiry
blocks/defer honestly. Never edit the timestamp to pass: a fresh observation
requires a separate operator action and a fresh decision. Production release and
cited-reference admission checks still handle expiry during the attempt.

It invokes `m1-infer --operation new` exactly once, with the selected key passed
only to that child. The production decision permits one generation plus at most
one eligible schema repair, with no orchestration retry, fallback or model switch.
Fresh-process `m1-infer-inspect` and two `m1-infer-recover` invocations use that same
decision and unchanged context request, without the key. Timeout retains the
partial/durable scoped identity when available; post-start uncertainty is
`LIVE_FAIL`, never relabeled `LIVE_BLOCKED` because later inspection failed.

`terrain_live_qualification.json` contains safe scoped identities, source/time/status,
tested Git commit and binary SHA-256, counts, exact reference IDs, P digest/equality
assertions, fixed failure categories, and only a successfully admitted/verified
proposal. It contains no input copies, pack copy, raw command output, stderr,
provider response, private reasoning or credentials. A pre-existing report is
never overwritten. `null` counts mean unknown, not measured zero.

`LIVE_PASS` requires all of the following: one admitted current-profile provider
decision, outbox, accepted history and closure; one generation and zero/one repair;
an eligible reference from this exact observation present in the original sent P;
equal generation/repair P bytes; exact original P on inspection and both recoveries;
unchanged call identities, decision/event/proposal and reservation/call counts;
and `dispatch_granted = false` on every receipt. The original P comes directly
from scoped `execution.inference_calls`; production inspect verifies its binding.
A separate preparation allocates a different decision: its bytes are not a
substitute or an equality baseline.

`proposal_cites_terrain` separately reports whether the actual admitted proposal,
next intent or view selectors cite the retained Terrain reference. The operator
must assess the preserved hypothesis, alternative, expected/falsifying observation,
uncertainty, rationale and next intent for useful evidence-led access-validation
planning. Ignoring Terrain is reported honestly and never triggers another send.
Connected inference is not adaptation, action-outcome feedback, acquisition,
validated access, a complete cognitive MVP or an M1 seal. APT41, Lazarus and Volt
Typhoon remain behavioral reference lenses, never playbooks or execution permission.

`LIVE_BLOCKED` covers pre-send input/consent/fixture/freshness failures.
`LIVE_FAIL` covers a started/possibly sent attempt failing qualification, including
missing/tampered historical P or unknown counts. No automatic retry is performed.
Build the binary from the tested commit under the process-owned ordinary target
`D:/DuskWeave-build/rust-1.94.1/normal`; the report records its hash but cannot itself
prove binary-to-source provenance. Use fresh subdirectories under
`D:/DuskWeave-build/rust-1.94.1/reports/dw-m1-terrain-live-runner`.
No ordinary/instrumented cache mixing, cleanup, configuration changes or new live
provider/target authority follows from this command.

Deterministic checks (synthetic controlled boundaries, not live evidence):

```bash
python -m unittest discover -s tests -p test_m1_terrain_live.py
python -m unittest discover -s tests -p test_m1_native_helpers.py
```

Production wiring is unchanged: runner -> real Mission/Terrain/inference CLIs ->
existing owner contracts -> provider/history -> verified historical receipts.

## One Bounded Inference-Only Episode

```bash
python tests/support/m1_native_live.py --binary <path-to-duskweave> \
  --report <new-report-dir> [--env-file .env] [--database-name <dw_native_...>]
```

The helper provisions a fresh `live` database, registers one blind engagement,
runs `m1-infer` once against the fixed DeepSeek profile (at most one generation
plus one eligible schema repair), then runs fresh-process `m1-infer-inspect` and
two `m1-infer-recover` invocations. There is no automatic retry, model change,
fallback or target contact; `live_qualification.json` records `LIVE_PASS`,
`LIVE_FAIL` or `LIVE_BLOCKED` for the committed binary under test. A failure
after inference starts keeps the original decision identity and the durable
call/decision/outbox/history/closure counts; it is never relabeled as a
preflight block.

This older helper supplies inference-only consent and no Terrain observation.
Its `LIVE_PASS` cannot establish Terrain-informed cognition; it remains unchanged.

### Helper `.env` loading vs direct CLI environment

`--env-file` is a helper-only convenience: it reads exactly `DEEPSEEK_API_KEY`
from the selected file (default this repository's `.env`), validates it before
any provisioning, and passes it only to the `m1-infer` child process. Other
lines are ignored, shell expressions are never executed, and there is no
fallback to any other key source; inspect and recover children run without the
key. The production `duskweave` binary itself reads only the process
environment — it has no dotenv loader. When invoking `duskweave m1-infer`
directly, export `DEEPSEEK_API_KEY` plus the four role DSNs
(`DW_DATABASE_URL`, `DW_INFERENCE_BROKER_DATABASE_URL`,
`DW_COGNITION_DATABASE_URL`, `DW_TRAJECTORY_DATABASE_URL`) in the calling shell
yourself.

# Run the controlled autonomous fixture

The original deterministic lane uses the production supervisor and restricted
PostgreSQL owners with fixture inference and DNS. SDK qualification adds loopback
provider HTTP. The separately bound provider-fixture lane replaces only inference;
live acquisition remains a later, independently qualified slice. Fixture success
proves the connected workflow and makes no live target claim.

The SDK lane below uses fresh v5 permission / v3 run bindings. Existing v4
permission / v2 run bindings retain their explicitly selected legacy transport. [ENGINEERING_STATE](ENGINEERING_STATE.md#6-current-slices-and-next-action)
owns delivery/readiness; [the lifecycle record](workflows/DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE.md#r2-completed-runtime-and-qualification-ledger)
indexes tested assertions. Neither a guide nor old qualification grants new consent.

## SDK loopback qualification and fresh consent

The SDK lane pairs `m1_inference_permission_v5`,
`m1_autonomous_run_request_v3` and `m1_context_pack_autonomous_v3` with
`autonomous-dns-fixture-v2`, `controlled_fixture` and the existing
`reasoning-bounded-v1` timers. Register fresh consent with the exact selected
profile/recipient; old permissions and old run versions cannot select SDK.
Keep the existing registration/context/seed fields and finite campaign limits.
The request explicitly names the same `inference_profile` as its permission.
SDK v5 requires the fixed 16384-byte per-call request budget because the SDK
performs serialization/counting internally. Lower request budgets refuse;
preflight is not an estimate authorizing a larger transfer. Context and intent
still obey their separately admitted caps.

| Explicit SDK profile | Fixed recipient | Infrastructure credential |
| --- | --- | --- |
| `fixture-genai-responses-v1` | `http://127.0.0.1:18083/responses` | Fixed synthetic fixture auth; no provider key |
| `deepseek-genai-responses-low-v1` | `https://api.deepseek.com/responses` | `DEEPSEEK_API_KEY`; separate fresh authorization required to call |

Only the fixture profile permits loopback HTTP. Port 18083 has no override;
if occupied, refuse and preserve its owner. The owning `m1_genai_integration`
test target starts the synthetic HTTP provider, registers fresh consent and
invokes the actual CLI. It uses explicit schema revision 0022 through the
existing restricted native bootstrap; its default remains 0021 for legacy
qualification. The test custodian must hold the dedicated Cargo target and
cluster-role fixture ownership for the whole action.

```text
cargo test --locked --test m1_genai_integration sdk_real_cli_completes_feedback_and_fresh_key_free_recovery -- --exact --test-threads=1
```

Inside that authorized fixture session, the operator CLI remains:

```text
duskweave m1-run --input <fresh-registered-sdk-run.json> --timing-profile reasoning-bounded-v1
duskweave m1-run-stop --run <run-id>
duskweave m1-run-inspect --run <run-id>
duskweave m1-run-recover --run <run-id>
```

Inspect/recover/stop require their existing Broker configuration and no inference
key. They preserve durable identities and never repeat inference or DNS.
Possible sends remain charged; incomplete/unknown responses do not publish a
usable proposal or open repair. SDK repair is limited to the existing eligible
codec errors and one reserved repair call. Raw HTTP/provider content and private
reasoning do not enter ordinary output or persistence. Historical decisions
still require current owner admission before any consequence.

The offline episode is **synthetic provider + DNS fixture**. It qualifies
orchestration and executed adapter boundaries, not real-provider cognition,
paid-provider quality, live target facts, Linux acquisition or production
readiness. The installed DeepSeek profile is not invocation authority.

### Dependency and distribution obligations

The vendored SDK is pinned at `a9504f1fe3641b4d21a8c44bea4379bed4394f4b` with
the five identified boundary/evidence files patched and one exact trailing-
whitespace correction in `src/adapter/adapters/zai/mod.rs`. Source and combined
patch identities are in `third_party/genai/source-manifest.json`. Snapshot
hashes identify canonical Git LF text; the provenance guard also accepts a CRLF
checkout without changing content, membership or conflict-marker checks. Retain its
MIT/Apache notices and modified-source notices. The locked TLS graph also
contains permissive ISC, BSD-3-Clause, Zlib and CDLA-Permissive-2.0 components.
The manifest retains the exact `webpki-root-certs` certificate-data LICENSE
text/provenance. Redistributing that data, including in an embedding binary,
must make its license text available alongside the data. A Cargo cache alone
does not fulfill distribution obligations. This packet does not implement or
qualify release packaging. Dependency curation/audit still applies to the
complete graph and final candidate; no additional license is implicitly allowed.

## What the owner should see

The successor qualification uses `m1_inference_permission_v4`,
`m1_autonomous_run_request_v2` and the provider-independent
`autonomous-dns-fixture-v2` run profile in `controlled_fixture` mode. The run
request explicitly selects `fixture-responses-v1` or
`fixture-chat-completions-v1`, bound to the registered permission. Its closed
CLI bootstrap hint is `--timing-profile reasoning-bounded-v1`; it grants no
authority. Local protocols require no provider key and cannot fall back to a
network provider. Numerical policy lives in
[ADR-016](adr/ADR-016-execution-broker.md#selected-successor-timing-policy-reasoning-bounded-v1).

Successor inspect/recover includes `inference_profile` and
`recorded_decisions`. Each retained proposal keeps its actual intent,
`disposition`, original times and separate `current_use` limitation. A
`recorded_stale_premises` first decision defers with zero DNS; a stale second
decision may complete historical recording. Neither grants current dispatch.
Diagnostic-v2's optional `reported_guard` records a delivered closed guard
report, separately from the parent's observed cause and exit code. Missing
reports and exit 124 do not establish an exact terminating guard.

One start completes two decisions and one fixture action, with no `continue`,
copying results or intermediate approval. The terminal JSON contains:

| Field | Completed fixture |
| --- | --- |
| `status` | `completed` |
| `execution_mode` | `controlled_fixture` |
| `decision_count`, `decision_history_count` | `2`, `2` |
| `effect_count`, `terrain_history_count` | `1`, `1` |
| `dispatch_granted` | `false` (proposals grant no execution permission) |
| Decision/attempt/result identifiers | Stable across inspect, duplicate start and recovery |

The regression inspects committed owner records and asserts that Decision 2
cites the actual first decision and newly qualified Terrain, rather than merely
printing these counts. An eight-seed scenario checks real producer/consumer fit.

## Isolated demonstration on the owned Windows fixture service

### Successor Windows qualification

The [final source-bound matrix](workflows/DW-FIX-M1-FIXTURE-SUPERVISOR-RUST.md#final-windows-qualification-2026-10-25)
records the completed four-action Rust-supervised Windows qualification. Do not
repeat it for documentation changes or reuse its evidence names. A future assigned
qualification uses fresh database/report names, one owner for normal/instrumented
targets and cluster-role fixtures, the rebuilt native helper's absolute path and
the existing `tests/support/m1_lifecycle_qualification.py` consumer.

Follow [native helper setup](RUN_M1_NATIVE_INFERENCE.md#native-fixture-qualification).
Clean only selected coverage workspace profiles, preserve compiled artifacts and
historical evidence, then run the qualifier with the assigned `--database-name`
and `--report`. It requires all four full action summaries and nested T8, stops
on first failure and preserves redacted logs/metadata and truthful cleanup/omissions.
The 900s Cargo-action policy is not a product timer or measured latency guarantee.
Pinned llvm-cov 0.9.1 refuses `--no-report` with `--no-clean`; the qualifier's
current tested commands own that invocation, not historical snippets.

### Previously delivered skeleton demonstration

Use the Rust 1.94.1 / PostgreSQL 17 [native setup guide](RUN_M1_NATIVE_INFERENCE.md#native-fixture-qualification).
[The skeleton record](workflows/DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON.md) is historical implementation evidence.
No production/client database or old live consent is usable for this fixture.
With no other shared-target or role-fixture owner running, in PowerShell:

```powershell
Set-Location D:/DuskWeave-ci-complexity
$env:CARGO_TARGET_DIR='D:/DuskWeave-build/rust-1.94.1/normal'
Remove-Item Env:DEEPSEEK_API_KEY -ErrorAction SilentlyContinue
python tests/support/m1_native_bootstrap.py --database-name dw_native_skeleton_owner_demo_20261020 --report D:/DuskWeave-build/rust-1.94.1/reports/skeleton-owner-demo -- cargo test --locked --test m1_run_postgres -- --test-threads=1 --nocapture
```

The bootstrap qualifies/reuses owned `dwpg-r3`, creates a fresh disposable database
and narrow logins, applies actual migrations and keeps credential values private.
An existing database/report name is refused: choose a new owned name for a later
run; do not delete retained evidence to rerun. It prints a sanitized progress
heartbeat while building/testing, then the test result and terminal receipt.

## Operational commands

For new v2 runs use v4 permission and explicit `inference_profile`, as exercised
by `tests/m1_inference_lifecycle.rs` and its private support. Launch with
`m1-run --timing-profile reasoning-bounded-v1 --input <registered-v2-run.json>`;
the timing hint is a CLI flag, not a request/context JSON field. Keep the registered
profile/recipient/settings and request binding identical. The following v1/v3
shape remains a supported legacy lane, not a v2 example.

A deployed legacy fixture run requires an already registered Mission with current
contact permission, all nine v3 disclosure classes, fixture profile/mode and
finite budgets for both decisions. Infrastructure connections remain the
existing [native-inference configuration boundary](RUN_M1_NATIVE_INFERENCE.md).
The run request version is `m1_autonomous_run_request_v1`; it carries `run_id`,
`run_profile`, `execution_mode`, absolute `deadline_at_ms`, the existing typed
`context` request and 1–8 ordered `seed_names` (`asset_ref`, `name`). Seeds must
match the admitted candidates/contact policy; names are never extracted from
LLM prose. Maximum request size is 16384 bytes. `m1-run-phase` is a private fixed
child protocol, not an operator step.

```text
duskweave m1-run --input <registered-fixture-run.json>
duskweave m1-run-inspect --run <run-id>
duskweave m1-run-stop --run <run-id>
duskweave m1-run-recover --run <run-id>
```

Stop prevents subsequent release; withdrawal continues to use the existing Mission
command. Hard parent death may prevent a final terminal receipt. Fresh inspect
reports a lost supervisor lease as UNKNOWN. Recovery makes at most three database
redelivery attempts; it does not restart phases, refill quota, repeat inference,
reacquire DNS or promise a successful outcome. Committed history remains historical.
An ACK outage can leave a durable result before the caller knows it committed.

Withdrawal and other fenced writers distinguish a *known transaction rejection*
from an unknown outcome. A failed exit whose receipt names the same operation
with a serialization rejection (`rejected` / `serialization_retry`) means the
transaction provably did not commit — for example when a concurrent
`session_fences` epoch bump aborts the serializable writer. Only that known
rejection may resubmit the same operation identity, bounded, after the premise
is re-checked. `commit_unknown`, storage errors, conflicts and unreadable
output are unknown: the durable state is not proven and resubmission must not
be assumed. Re-minting the operation UUID turns a known rejection into a false
new operation and must never be used to force a green result.

Lease and deadline qualification also depends on the database clock agreeing
with host monotonic time. On this Windows/WSL2 lane `clock_timestamp()` has
been observed stepping backwards by hundreds of milliseconds, which can admit
a lease renewal after the host-observed expiry. Such a host is UNQUALIFIED for
final serial qualification; environment clock recovery is an owner-managed
operations task, not a test or product change.

### Start configuration and terminal diagnostics

After input validation, start parses the selected configuration for core, Broker,
Cognition, Trajectory and Terrain before opening a connection, hashing the binary
or creating a run. Keep the existing `file` / `env-local` selection: file deployments
do not need URL environment variables, and conflicting sources refuse. Failure
prints only `config_owner` from the closed owner vocabulary and `error` category.
Windows also requires `SystemRoot`; the provider profile requires the configured
control root, whose actual ownership/ACL qualification remains at its existing boundary.
An API key is not a start prerequisite: an offline provider-profile decision can
durably report `live_key_missing`. Inspect/recover/stop need their existing Broker
configuration, without the other start configurations or an inference key.

The [diagnostics FIX](workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md) adds optional
`diagnostics` containing closed `m1_run_phase_diagnostic_v1` records. Each phase
has at most one immutable record. `cause_source=child_report` labels an allowlisted
child error; `parent_observed` labels a parent observation. `exit_code` is the actual
signed process status, when available. Exit 124 alone does not identify a watchdog.
`elapsed_ms` measures parent time from child launch, including bounded terminal
observation; it does not measure a provider stage or prove an effect.

`diagnostic_publication=committed` means the started phases' diagnostic records are
available. `pending_or_unknown` means the current receipt has safe
`pending_diagnostics` whose SQL acknowledgment is unavailable; fresh inspect may
resolve a committed record. `unavailable` means durable diagnosis is absent or
incomplete. Old runs are never backfilled with inferred causes. If the summary
itself cannot be read, a local pending receipt reports `status=unavailable` and
does not invent effect/history counts. Existing UNKNOWN effect/commit semantics
remain; a diagnostic record cannot prove success or authorize replay.

After child termination, publication may use the remaining original run budget
even when the phase deadline has expired. If that budget or the database is
unavailable, the pending/unavailable distinction above still applies; recovery
does not reconstruct a missing record.

A single completed provider decision followed by `owner_view_requested` remains
distinct from `completed` with two decisions, one fixture DNS result, Terrain and
history. Recovery is database-only and never fabricates missing diagnostics or
continues an owner view, provider call, repair or fixture effect.

Existing v1/v2 inference, manual operator-reviewed Terrain intake, M0 assessment,
withdrawal and inspect/recovery commands remain supported. Fixture source labels
cannot promote a result into live authority. Foundation/M0 seals are unchanged;
there is no new M1 acceptance, deployment qualification or target authorization.

## Provider-fixture lane: opt-in, not inherited consent

For the delivered successor, bind `m1_inference_permission_v4` and
`m1_autonomous_run_request_v2` to `autonomous-dns-fixture-v2`, `controlled_fixture`,
`inference_profile: deepseek-responses-low-v1`. Pass the CLI timing hint
`--timing-profile reasoning-bounded-v1` at start. Only inference contacts
`https://api.deepseek.com/responses`. The closed profiles select `deepseek-flash`,
text-only non-streaming output, no tools and 4096 output tokens; `low` selects
`reasoning.effort="low"` while `deepseek-responses-reasoning-v1` remains the
explicit `high` variant. Prefer `deepseek-responses-low-v1` for this measured
lane: one authorized episode observed the high variant exhausting all 4096
reasoning-inclusive tokens on Decision 2 while the low variant completed with
3343 reasoning + 460 visible tokens. That is one observation, not a latency or
reliability calibration; both profiles keep honest `incomplete` handling and
there is no endpoint/model fallback. DNS stays `dns-routing-fixture-v1`
with no target/resolver socket; Terrain source stays `fixture_dns_acquisition`.
Local protocol profiles never consume the key or fall back to DeepSeek.

Before any provider contact:
- Finish offline qualification on the actual candidate/platform and review the
  packet's measured limits, uncertainty and unresolved checks.
- Register a new campaign with explicit `m1_inference_permission_v4` binding that
  exact run/profile/mode/recipient, all nine permitted classes, current non-nil
  terms reference and explicit terms approval, finite capacity and UTC window.
- Review the actual initial Mission/seeds, disclosure and window. This guide,
  a key, accepted architecture and an old episode supply no consent. Operational
  campaign identities/window/terms are not prefilled or fabricated by tests.
- Provision/qualify the private operator-owned local control root before start.
  Set `DW_M1_RUN_CONTROL_ROOT` in the owning process to
  `D:/DuskWeave-build/rust-1.94.1/provider-fixture-control`. Existing roots require
  inspection and qualified reuse, never deletion or clearing stop markers.
  Windows ownership/DACL must restrict access to the operator, SYSTEM and
  Administrators; reparse/remote/traversal paths refuse. Do not change host policy.
- Supply `DEEPSEEK_API_KEY` only through the infrastructure environment boundary.
  The supervisor explicitly forwards it only to inference children after run
  admission; acquisition children, deterministic runs, inspect and recovery do
  not receive or need it. Never put it in input JSON, arguments or reports.

Use the same single `m1-run --input` entrypoint. There is no phase-by-phase
operator input. Keep the same trusted control-root configuration for separate
stop, withdrawal and manual Terrain-writer commands. Run-stop writes the UUID
run marker before database lookup; withdrawal and manual premise replacement
signal the campaign before slow SQL. Markers/EOF only revoke local work: they do
not commit authenticated SQL authority and cannot resume a run. Direct SQL
writers still require their common fence; their fallback is permit loss, not an
assumed shared in-process flag. Inspection/recovery does not emit a new stop.

The canonical finite successor allocations and independent clocks are in
[ADR-016's timing table](adr/ADR-016-execution-broker.md#selected-successor-timing-policy-reasoning-bounded-v1),
implemented by `src/m1_inference_profile.rs`. Bind that profile and full two-decision
capacity before start; insufficient original campaign/window/run remainder refuses.
No lease, repair, cancellation or accounting step renews permission or refreshes TTL.
Legacy v1/v3 profiles keep their original allocations; never mix their consent
or timer values into a successor request.

The second immutable pack must resolve to the actual first decision and issued
owner-qualified fixture result, retaining original perception/expiry. A genuine
retain, defer, stop, view request or capability gap is legal; do not force DNS or
restart a paid episode to obtain a preferred strategy. A second proposed effect
is recorded, not executed. Two HTTP responses alone do not demonstrate feedback.

Inspect durable generation/repair identities and states when attributing calls:
`completed_charged_unused_repair_reserved` alone does not prove repair was unused.
Unknown consumption remains charged. Inspect/recover with the key absent must
make zero further provider/effect requests; recovery is database-only.

For current demonstration evidence and missing prerequisites, read
[ENGINEERING_STATE](ENGINEERING_STATE.md#6-current-slices-and-next-action).
Offline fixtures prove owner wiring/failure semantics, not LLM cognition,
live target facts or an M1 seal. A failed/partial paid episode stays recorded;
do not replay it to obtain a preferred strategy or a successful-looking report.

## Accepted SDK transport transition (not delivered)

An accepted amendment selects a trusted `rust-genai` SDK adapter to replace
this lane's owned direct-libcurl inference transport under the semantic
`dw-inference-intent-v1` binding; see
[ADR-016 section 4b](adr/ADR-016-execution-broker.md#4b-candidate-rust-genai-sdk-transport-amendment-2026-10-28)
and
[enabling contract section 8](contracts/M1-enabling-architecture.md#8-candidate-rust-genai-sdk-adapter-amendment-2026-10-28).
It is not delivered: there is no SDK CLI, no new command and no changed
behavior in the commands, profiles, bindings or qualification above. Adoption
arrives only through a bounded runtime packet, qualified fixture/provider-
fixture evidence, new versioned permission/run bindings and fresh consent —
never by reusing an existing episode or approval.

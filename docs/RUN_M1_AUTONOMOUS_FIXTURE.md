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

The retained legacy v4/v2 successor qualification uses
`m1_inference_permission_v4`, `m1_autonomous_run_request_v2` and the provider-independent
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

For runs on the retained legacy v4/v2 binding, use v4 permission and explicit
`inference_profile`, as exercised by `tests/m1_inference_lifecycle.rs` and its
private support. Launch with
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

Retained legacy instructions: this lane keeps the v4/v2 bindings on its
explicitly selected legacy transport. New SDK episodes instead use
`m1_inference_permission_v5`, `m1_autonomous_run_request_v3` and
`m1_context_pack_autonomous_v3` from the SDK section above; never mix the
bindings or their consent and timer values.

For this retained legacy successor lane, bind `m1_inference_permission_v4` and
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

## Accepted SDK transport transition (delivered)

The accepted amendment's trusted `rust-genai` SDK adapter replaced this lane's
owned direct-libcurl inference transport under the semantic
`dw-inference-intent-v1` binding; see
[ADR-016 section 4b](adr/ADR-016-execution-broker.md#4b-candidate-rust-genai-sdk-transport-amendment-2026-10-28)
and
[enabling contract section 8](contracts/M1-enabling-architecture.md#8-candidate-rust-genai-sdk-adapter-amendment-2026-10-28).
The SDK runtime was delivered by PR #132 (merge `a85afcb`). Its bindings,
profiles, consent, commands and qualification are owned by the
[SDK section](#sdk-loopback-qualification-and-fresh-consent) above and by
[ENGINEERING_STATE](ENGINEERING_STATE.md#6-current-slices-and-next-action);
the v4/v2 lanes above remain supported as retained legacy instructions.

Current SDK support: real inference may use the consented
`deepseek-genai-responses-low-v1` profile, but acquisition remains
`controlled_fixture`. Real target acquisition still requires a separately
delivered and qualified Linux runtime. Each episode still requires fresh
consent — no existing run, episode or approval carries over.

## Linux LAB build storage (owned Oracle ARM64)

The LAB deployment on `pica-arm-01` uses `boundary/qualify/qualify.sh`
(`service`, `release`, `install-bundle`, `lab-database`, `lab-bind`,
`build-storage`, `build-only`).
For the local exporter-attach candidate, Docker starts the exporter as PID1 with
its fixed UID and network disabled. The launcher connects to both containers
before READY, sends the exporter its independent REQUEST and START, and stages
one bounded capture frame through capture stdout EOF before forwarding it on
the exporter's parent-owned attach input. The exporter returns only the safe
RESULT; the launcher sends ACK after the core has durably handled that result.
The input stays open through ACK. Historical unresolved `transport.pending`
markers still block confirmed disposal, while new attempts dispose through
container and namespace custody.

Qualify that candidate from the fresh source with `build-storage`,
`lab-database`, `release`, then `lab-bind`, recording each exit status.
Preserve the prior installed binaries, manifest bytes and metadata, and image
digest before `release`; restore and verify them after the packet-owned tests
and workers finish. Do not run `prepare-host.sh` for this refactor. This
procedure does not make a candidate the permanent installed release.

The frozen final-release bundle instead uses explicit fresh destinations; its
no-argument legacy forms above stay reserved for the retained attach fixture.
The bundle and the SHA-tagged image name the frozen release source
(`8c6f752e27c8d47671d3e71a86bf55c9aae6f2db`); the checked-out driver carrying
`qualify.sh`, this guide and the test harness is a separate later commit —
never describe the bundle as a build of the driver. After the prior installed
binaries, exact manifest bytes/metadata and image digest are preserved in a
protected root:root/0700 backup:

The `lab-database` SOURCE_DIR is an operator-verified Git export. Before root
imports the export's bootstrap helper or executes its migrations, the operator
pins the intended committed driver revision by full SHA — the later
deployment-driver commit, a different identity from frozen release source
`8c6f752e27c8d47671d3e71a86bf55c9aae6f2db` — exports it with
`git -c core.autocrlf=false -c core.eol=lf archive --format=tar --output=<owned-path> <revision>`
(per-invocation options; never a machine or global Git configuration change),
transfers the archive as bytes (for example `scp`, never text redirection),
and compares the complete extracted tree against `git ls-tree -r <revision>`:
exact membership plus every file's raw blob OID via `git hash-object
--no-filters` or the equivalent Git blob SHA-1 over raw bytes — including
`tests/support/m1_native_bootstrap.py` and every migration input. Missing,
extra, type/mode or byte mismatches refuse the source; the pinned revision and
comparison result stay in the existing execution evidence. `tar -d` proves
only extracted-file/archive equality — useful for transfer, never Git
identity. This operator comparison is the Git-byte verification `lab-database`
requires; the command itself enforces only the separate custody checks below
(protected ancestry, root-owned regular files, link and write rules) and never
inspects Git content.

- `install-bundle <bundle-dir>` verifies the exact seven-file membership
  (four binaries plus the three `third_party/genai` notices), protected bundle
  directories/ancestry, root-owned regular members with nlink=1 and no
  group/other write, and all seven pinned digests before installation.
- `lab-database <source-dir> <database> <role-prefix> <config-dir>` requires
  Git-byte-verified source with protected root-owned directories/ancestry,
  bootstrap helper and migration inputs, all without group/other write. It
  provisions a fresh `dw_native_` schema-0030 fixture on the owned service and publishes
  protected configuration: root-owned/group-`duskweave-core` 0750 directory,
  six runtime DSNs and `environment.json` 0400 owned by core, and a
  root:root/0400 administrative `test-environment.json` core cannot read.
- `lab-bind <image-ref> <database> <report-dir>` binds only when the installed
  hashes equal the frozen bundle, reuses the image-worker byte check and
  writes one LAB qualification row plus the same manifest schema and finite
  86400000-ms validity.

The physical harness then requires absolute `DW_M1_LAB_CONFIG_ROOT` and
`DW_M1_LAB_REPORT_ROOT` in its own environment; absent or partial values
refuse before credentials are read or a connection is made. Run selected
physical cases one at a time with `--ignored --exact --test-threads=1
--nocapture`, requiring exactly one executed test per case.

For release preparation without deployment side effects, run
`bash boundary/qualify/qualify.sh build-only <name>:<tag>` from the verified
exported source tree. It reuses the same locked/offline build pipeline and
fixed worker Dockerfile as `release`, but derives its source from the
script's own export (`boundary/qualify/../..` resolved to a physical path)
rather than the retained attach export, and requires an explicit unqualified
`name:tag` image reference that must not already exist. It stops after the
image build: `install_release` and the service/lab-database/lab-bind steps
are never reached, so installed binaries, `deployment.json`, sudoers and
database state stay unchanged, and Go module resolution stays offline. The
image identity is bound to the exported candidate — for example
`dw-final-release-prep-20261109:<full candidate SHA>`. A build-only image is
a source-bound artifact for review, not an installed or qualified release;
binding, service/database provisioning and final qualification remain
separate later operations under their own fresh identities.

Build output, caches and temporary writes must land on a separately
mounted ext4 block volume at `/srv/duskweave-build`; source, reports,
toolchains, Docker, containerd and PostgreSQL stay on the boot
filesystem. The `release` and `build-storage` commands fail closed
before any compiler or Docker write when the mount is absent, not a
mountpoint, a symlink, shares a device with `/`, `/var/lib/docker` or
`/var/lib/containerd`, is not ext4, or carries foreign content —
including names beginning with dots. Each existing fixed output/cache/
temporary component (`normal`, `go-cache`, `tmp`, `tmp/go`, `tmp/build`,
`go-release`, `boundary-release`, `coverage`) must itself be a real
root-owned directory on the same filesystem: no symlink, file, nested
mount or group/world write. Newly created fixed directories receive the
same validation before consumption. The check emits only mount identity
and fixed failure categories.

Prerequisite: an operator-provisioned owned volume at the fixed path
(UUID fstab entry with `_netdev`; iSCSI auto-login). Rollback: the
volume holds only rebuildable outputs — losing it fails closed; the
retained `/srv/duskweave/build` tree is untouched: its retired
build outputs are never consulted, while the explicitly retained Cargo
dependency cache under it may still be read by builds.
Limits: the separate filesystem removes the demonstrated build-writeback
stall of containerd temporary-layer unmount; it is not a readiness
guarantee against other device/VM coupling. The original 5000-ms
preparation deadline is unchanged and re-measured, never raised.

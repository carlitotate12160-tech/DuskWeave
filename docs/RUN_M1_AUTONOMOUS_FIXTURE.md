# Run the controlled autonomous fixture

This first Walking Skeleton runs the production supervisor and restricted
PostgreSQL owners. Inference and DNS are deterministic, explicitly labeled
fixtures. It sends no provider or target request. A passing run proves the
connected workflow. The separately bound provider-fixture lane below replaces
only inference; live acquisition remains a later, independently qualified slice.

## What the owner should see

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

Use the qualified Rust 1.94.1 / PostgreSQL 17 setup described in
[the implementation packet](workflows/DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON.md).
No production/client database or old live consent is usable for this fixture.
With no other shared-target or role-fixture owner running, in PowerShell:

```powershell
Set-Location D:/DuskWeave-ci-complexity
$env:CARGO_TARGET_DIR='D:/DuskWeave-build/rust-1.94.1/normal'
python tests/support/m1_native_bootstrap.py --database-name dw_native_skeleton_owner_demo_20261020 --report D:/DuskWeave-build/rust-1.94.1/reports/skeleton-owner-demo -- cargo test --test m1_run_postgres -- --test-threads=1 --nocapture
```

The bootstrap qualifies/reuses owned `dwpg-r3`, creates a fresh disposable database
and narrow logins, applies actual migrations and keeps credential values private.
An existing database/report name is refused: choose a new owned name for a later
run; do not delete retained evidence to rerun. It prints a sanitized progress
heartbeat while building/testing, then the test result and terminal receipt.

## Operational commands

A deployed fixture run requires an already registered Mission with current
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

Existing v1/v2 inference, manual operator-reviewed Terrain intake, M0 assessment,
withdrawal and inspect/recovery commands remain supported. Fixture source labels
cannot promote a result into live authority. Foundation/M0 seals are unchanged;
there is no new M1 acceptance, deployment qualification or target authorization.

## Provider-fixture lane: opt-in, not inherited consent

[The provider-fixture packet](workflows/DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK.md)
governs `autonomous-dns-provider-fixture-v1` / `controlled_fixture`. Only inference
may contact `https://api.deepseek.com/responses`, under
`deepseek-responses-autonomous-v1`. The existing fixed request selects
`deepseek-flash`, high reasoning effort, text-only non-streaming output, no tools
and 4096 output-token capacity. There is no endpoint/model fallback.
DNS remains `dns-routing-fixture-v1`, with no target resolver/socket. Terrain
source is `fixture_dns_acquisition`, never a live observation.

Before any provider contact:
- Finish offline qualification on the actual candidate/platform and review the
  packet's measured limits, uncertainty and unresolved checks.
- Register a new campaign with explicit `m1_inference_permission_v3` binding that
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

The application reserves four call slots, 65536 request bytes, 262144 response
bytes, 16384 output-token capacity and 90000 inference wall-time ms. Each decision
has one original 40000 ms generation/conditional-repair clock and 45000 ms full
reservation. Each actual call is capped at 20000 ms including <=5000 ms connect;
preparation <=5000 ms, inference phase <=50000 ms and run <=150000 ms, all clamped
by original current authority, premise and run deadlines. Cancellation/accounting
never extends permission to send. These are policy ceilings, not latency fits.

The second immutable pack must resolve to the actual first decision and issued
owner-qualified fixture result, retaining original perception/expiry. A genuine
retain, defer, stop, view request or capability gap is legal; do not force DNS or
restart a paid episode to obtain a preferred strategy. A second proposed effect
is recorded, not executed. Two HTTP responses alone do not demonstrate feedback.

Inspect durable generation/repair identities and states when attributing calls:
`completed_charged_unused_repair_reserved` alone does not prove repair was unused.
Unknown consumption remains charged. Inspect/recover with the key absent must
make zero further provider/effect requests; recovery is database-only.

Current operational disposition: **REAL PROVIDER EPISODE NOT EXECUTED**.
Fresh disclosure/key and complete candidate/platform qualification are separate
prerequisites. Offline scripted inference tests prove owner wiring and failure
semantics, not actual LLM cognition, live target facts or an M1 seal.

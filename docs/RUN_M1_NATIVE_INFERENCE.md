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

## One bounded live episode

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

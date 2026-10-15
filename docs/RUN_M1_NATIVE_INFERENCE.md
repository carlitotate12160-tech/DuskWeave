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

## Hold Lifecycle & Anomaly Dominance

1. **Automatic Closure**: Terminal decision completion automatically closes the exclusive
   session hold via `execution.close_inference()`, charging 2 call slots to cumulative capacity.
2. **Campaign Continuation**: Subsequent decisions in the same campaign succeed only after prior
   holds close and remaining campaign budget permits.
3. **Anomaly Dominance**: Conflicting identity or content produces durable anomaly records in
   `trajectory.reasoning_history`. Under anomaly dominance, hold closure is refused, blocking
   further unprincipled use of disputed identities.

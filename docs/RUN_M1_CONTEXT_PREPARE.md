# RUN_M1_CONTEXT_PREPARE — locally assembled safe inference context preparation

## Scope and authority

`DW-IMPLEMENT-M1-CONTEXT-PREPARE` adds local assembly and inspection of an
immutable, safe inference input (`m1_context_pack_v1`) under the accepted M1
inference contracts and ADR-016. It qualifies:

```text
main/CommandGuard -> m1 CLI dispatch -> closed input admission -> core allocation
-> Mission committed-publication qualification + Broker public capacity snapshot
-> Cognition ContextPack assembly -> CLI safe preparation receipt
```

Preparation is **not** an inference call, an action selector, an acquisition
grant, or durable history. It prepares the safe input bytes (future P) for
inspection without sending data to any provider. Every receipt reports:

```json
"current_permission": false,
"dispatch_granted": false,
"provider_qualified": false,
"acquisition_qualified": false,
"send_qualified": false,
"reservation_granted": false,
"decision_published": false,
"history_completed": false,
"request_binding_qualified": false
```

## Command surface

```text
duskweave m1-context-prepare --input FILE
```

`--input` is a bounded strict JSON document (≤16 KiB, unknown/duplicate fields
rejected) containing:

```json
{
  "version": "m1_context_prepare_request_v1",
  "mission": {
    "version": "m1_inference_reservation_request_v1",
    "engagement_id": "uuid",
    "campaign_id": "uuid",
    "operator_ref": "uuid",
    "expected_mission_revision": 1,
    "permission_id": "uuid",
    "goal_ref": "uuid",
    "exercise_mode": "blind"
  },
  "candidates": [
    {
      "asset_ref": "uuid",
      "priority": "high"
    }
  ]
}
```

Rules:
- `candidates`: at most 64 typed candidate premises; nil UUID and duplicate asset
  references are rejected.
- Priority must be one of `"high"`, `"medium"`, or `"low"`.
- No caller-supplied question, observations, URLs, summaries, or manifests.
- Missing `--input` or unrecognized flags fail closed under `CommandGuard`.

## Database and role requirements

- Uses a single read-only Repeatable Read transaction.
- Connects using the configured database connection (env-local or file mode).
- The connecting role must be a member of `dw_m1_broker` to execute
  `execution.inference_capacity_snapshot(uuid, uuid)`.
- Requires migration 0011 (`0011_m1_context_capacity.sql`).

## ContextPack assembly

Cognition compiles the accepted M1 mission question:

> "Which already-authorized, mission-relevant HTTPS candidate is the best next
> subject for access-validation planning within the permitted contact envelope,
> and which routing or response premise still needs observation, a different
> candidate, or deferral?"

- **Views**: Mission view is available with its publication lineage and revision;
  environmental views (`cyber_terrain`, `foothold_graph`, `attack_path_view`,
  `objective_state`, `campaign_trajectory`) are explicitly marked unavailable
  with reason `"not_loaded_by_this_profile"`.
- **Candidates**: displayed up to the consent candidates limit and byte bounds
  (ceiling 4096 bytes). Prefixes shrink to fit without truncating JSON or metadata;
  excess candidates are omitted with visible omission counts.
- **Capacity**: informational snapshot of remaining capacity across all five
  dimensions, fence epoch, and full-decision requirements.

## Receipt and outcomes

For successful ingress parsing and operation allocation, the command outputs a single-line JSON receipt:

```json
{
  "kind": "m1_context_preparation_receipt_v1",
  "decision_id": "uuid",
  "outcome": "prepared|deferred|refused|unavailable",
  "pack_utf8": "...",
  "current_permission": false,
  "dispatch_granted": false,
  "provider_qualified": false,
  "acquisition_qualified": false,
  "send_qualified": false,
  "reservation_granted": false,
  "decision_published": false,
  "history_completed": false,
  "request_binding_qualified": false
}
```

Outcomes:
- `prepared`: success; `pack_utf8` carries the exact immutable JSON string.
- `deferred`: consent window not yet open or expired, consent missing required
  field classes, or candidates supplied without `admitted_operator_premises`.
- `refused`: out-of-scope candidate assets or basis mismatch.
- `unavailable`: post-allocation reader failures (e.g. database query error,
  second reader-connect failure, lock wait timeout, or missing Broker role). Emits
  a receipt with allocated `decision_id`, `pack_utf8: null`, all false flags, and exits non-zero.

Early pre-allocation input/config failures (e.g. missing/malformed `--input` flag,
unrecognized arguments, duplicate keys, or invalid JSON syntax) fail closed under
`CommandGuard` and ingress validation with static error output and non-zero exit code
without emitting a receipt (no operation or decision ID was allocated).

## Explicit snapshot and non-durable limits

- Prepared bytes (`pack_utf8`) are strictly as-of-snapshot and non-durable.
  Snapshot values reflect database state at read-time under Repeatable Read.
- They do not constitute a send-time fence, do not reserve capacity, grant no
  provider send qualification, and write no durable Trajectory history or decisions.
- Preparation creates no reservation ledger record, refunds no capacity, and
  neither accesses external inference providers nor contacts targets.
- Re-preparation is a fresh operation; this slice includes no automatic replay.

## Future dependencies

The following remain unqualified by this slice:
- Send admission and transport adapter (`deepseek-responses-v1`)
- Immutable request binding (P/C/B digest)
- Provider response parsing, repair, and error handling
- Durable Trajectory decision publication
- Target contact, acquisition, feedback, retrieval, and synthesis

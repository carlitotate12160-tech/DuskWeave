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

- Uses a single read-only Repeatable Read transaction across Mission publication,
  Broker capacity snapshot, and the restricted Terrain public DNS owner view.
- Connects using the configured database connection (env-local or file mode).
- The connecting role must be a member of `dw_m1_broker` (and free of sibling
  `dw_m1_cognition`, `dw_m1_trajectory`, or `dw_m1_terrain` memberships and
  private Terrain table grants) to execute
  `execution.inference_capacity_snapshot(uuid, uuid)` and
  `terrain.current_dns_context_view(jsonb)`.
- Requires migrations 0011 (`0011_m1_context_capacity.sql`) and 0015
  (`0015_m1_terrain_context_view.sql`).

## ContextPack assembly

Cognition compiles the accepted M1 mission question:

> "Which already-authorized, mission-relevant HTTPS candidate is the best next
> subject for access-validation planning within the permitted contact envelope,
> and which routing or response premise still needs observation, a different
> candidate, or deferral?"

- **Views**:
  - `mission`: available with its publication lineage and revision.
  - `cyber_terrain`: controlled by `owner_qualified_summaries` disclosure and
    selected candidate assets (at most 8 included, non-excluded assets under the
    consent candidate limit):
    - When `owner_qualified_summaries` is absent from `field_classes`, records
      `availability: "unavailable"` with `reason: "disclosure_not_permitted"` and
      performs no Terrain query.
    - When `owner_qualified_summaries` is disclosed but no candidate assets are
      selected, records `availability: "unavailable"` with `reason: "not_requested"`.
    - When queried inside a bounded savepoint (`SAVEPOINT terrain_context_view`),
      returns `availability: "available"` with `shown_claims` and `omissions`
      (`stale_expired`, `withheld`, `held_or_conflicted`, `omitted_by_claim_limit`,
      `omitted_by_byte_bound`), or degrades on port/lineage failure to
      `availability: "unavailable"` with `reason: "incompatible_lineage"` or
      `"owner_view_unavailable"`.
  - Remaining environmental views (`foothold_graph`, `attack_path_view`,
    `objective_state`, `campaign_trajectory`) remain explicitly marked
    `unavailable` with `revision: null`, `frontier: null`, and `evaluated_at`;
    their repeated per-entry `reason` is omitted in favor of the single
    `pending_unknown` entry `["other_owner_views_not_loaded_by_profile"]`
    (which applies to exactly those four owners, while operator reference
    `source`/`status`/`freshness`/`completeness`, candidate `status`, and
    capacity `reservation_granted: false` / `shared_call_clock: "not_started"`
    retain the remaining uncertainty and admission explanations).
- **Terrain references and safe projection**: only current, unheld, fresh
  `observed` claims (`operator_reviewed_dns`, `dns_hostname_ipv4_resolution`,
  `tier1`, `corroborated: false`, `routing_usable: true`) become active
  `references` (`terrain_0..terrain_7`) and are linked in the corresponding
  candidate's `terrain_references` array. Hostnames, resolver/answer IPv4
  addresses, and raw DNS packets are never projected into `ContextPack`.
- **Candidates, byte fitting, and minimum-premise floor**: first applies the
  consent `candidates` ceiling (at most 8) to the input prefix; candidates
  outside that authorized prefix are never pulled in. Complete claim entries and
  metadata are never truncated to fit within `context_bytes` (ceiling 4096 bytes,
  JSON depth ≤8).
  - If the validated, disclosure-permitted loaded Terrain view has **no** fresh
    usable `observed` claim for the originally selected candidate prefix under
    consent, existing unavailable/stale/withheld/conflicted/limited-reasoning
    behavior is preserved, and `context_size_deferred` is returned only when
    mandatory metadata itself cannot fit `context_bytes`.
  - If **at least one** fresh usable `observed` claim exists for that prefix,
    successful preparation must retain at least one such claim, a linked
    retained candidate, its pack reference, and its `TerrainDependency`. Under
    byte pressure, assembly drops optional unlinked operator candidates first
    (wherever they occur in the authorized prefix, preserving the remaining
    candidates' relative input order and priorities) and may omit surplus claims
    (`omitted_by_byte_bound`), but never removes the final supported premise or
    its last linked candidate. If required metadata plus the minimum retained
    premise cannot fit `context_bytes`, `m1_context::prepare` fails closed with
    `Fail::State("context_bytes_insufficient")` before any inference reservation
    or provider work.
  - All pack-local IDs (`asset_0..`, `terrain_0..`), candidate
    `terrain_references` links, `PreparedContext::terrain_dependencies()`, and
    omission counts are built from that final selection (`selection` reports
    `"input_order_prefix_with_count_and_byte_bounds"` when the final candidates
    form an input prefix and `"input_order_with_count_and_byte_bounds"` when an
    earlier operator-only candidate was omitted under byte bounds).
- **Measured capacity and request-binding margins**:
  - Historical 1-candidate, 1-claim shape with `cache_remaining_ttl` and
    `single_vantage_only`: `P = 3899` UTF-8 bytes (`197`-byte margin under
    `4096`), `configuration = 5462` bytes (`10922`-byte margin under `16384`),
    generation `body = 8891` bytes (`7493`-byte margin), and 7-code repair
    `body = 9111` bytes (`7273`-byte margin; `repair_bytes = 147` under `2048`).
  - All 8 routing-eligible `DnsLimitation` subsets (`[]` through
    `["cache_remaining_ttl", "single_vantage_only", "resolver_recursion_variance"]`)
    fit at `4096` (`P <= 3932` bytes, minimum margin `164` bytes).
  - Combined worst-case numeric-width boundaries at PostgreSQL `timestamptz`
    maximum epoch (`9_224_318_015_999`, 13 digits), `epoch = i64::MAX`, all five
    `campaign` remaining-capacity fields at `i64::MAX`, `uncertainty_seconds = 300`,
    `lowest_ttl_seconds = 86400`, all 3 eligible limitations, `priority = "medium"`,
    and `64` supplied candidates (`candidates = 1`): `P = 4075` bytes (`21`-byte
    margin; generation `body = 9069`, repair `body = 9289`) at intake-bounded
    `claim_revision = 10001` / `omitted_by_claim_limit = 0`, and `P = 4094` bytes
    (`2`-byte margin; generation `body = 9088`, repair `body = 9308`) at pure-view
    `u32::MAX` `claim_revision` and `omitted_by_claim_limit`.
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
- `deferred`: consent window not yet open or expired (`context_consent_deferred`),
  consent missing required field classes, candidates supplied without
  `admitted_operator_premises` (`context_disclosure_deferred`), mandatory metadata
  exceeding `context_bytes` (`context_size_deferred`), or required metadata plus
  the minimum available eligible Terrain premise exceeding `context_bytes`
  (`context_bytes_insufficient`). Emits a `deferred` receipt with `pack_utf8: null`
  and all false grant flags, prints `error=<code>`, and exits non-zero.
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

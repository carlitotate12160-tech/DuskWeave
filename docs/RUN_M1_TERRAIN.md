# Running M1 Terrain DNS Intake

## Overview

DuskWeave supports operator-submitted, pre-reviewed DNS observation intake for `CyberTerrain` via the `duskweave` CLI:

```text
CLI (m1-terrain-record / m1-terrain-inspect / m1-terrain-recover)
  -> Strict Intake Validation (m1_terrain_dns_observation_v1, depth <= 8, <= 16 KiB)
  -> Mission Scope, Purpose, Vantage, Resolver & Operating Window Verification
  -> Terrain Atomic Acceptance (observations, materials, evaluations, claim_revisions, current_claims, outbox)
  -> Trajectory Narrow Consumer Verification & Append-Only History (trajectory.terrain_history)
  -> Bounded Receipt (m1_terrain_receipt_v1)
```

Governing authority: [M1 enabling architecture §3](contracts/M1-enabling-architecture.md), [ADR-006](adr/ADR-006-cyber-terrain-storage-model.md), [ADR-008](adr/ADR-008-observation-fact-separation.md), [ADR-009](adr/ADR-009-evidence-immutability.md), [ADR-011](adr/ADR-011-sensitive-data-barrier.md), and [ADR-013](adr/ADR-013-capability-contract.md).

In all cases:
- The only admitted source class is `operator_reviewed_dns`. Callers cannot label an observation as DuskWeave-acquired, provider-qualified, or defender-derived.
- Every receipt reports `dispatch_granted = false`, `acquisition_qualified = false`, and `corroborated = false`.
- A later operator-supplied observation remains labeled operator input (`operator_reviewed_dns`); it is not DuskWeave target-acquisition evidence and does not complete or seal M1.
- APT41, Lazarus, and Volt Typhoon remain behavioral reference lenses only, never execution permission.

## Role Separation & Configuration

M1 Terrain CLI commands require three distinct, non-overlapping PostgreSQL role connections configured via `DW_DATABASE_CONFIG_MODE` (for local runs, `env-local`):

| Environment Variable | Database Role | Permissions |
| --- | --- | --- |
| `DW_DATABASE_URL` | Core (`dw_runtime`) | Mission registration, withdrawal, and M1 policy read projection |
| `DW_TERRAIN_DATABASE_URL` | Terrain (`dw_m1_terrain`) | `terrain.*` append-only acceptance + `terrain.current_claims` current projection + narrow `mission.*` read |
| `DW_TRAJECTORY_DATABASE_URL` | Trajectory (`dw_m1_trajectory`) | Narrow `terrain.*` verification read + `trajectory.record_terrain_history(jsonb)` append-only execution |

Each connection is qualified before use:
- `fsync=on`, `full_page_writes=on`, `synchronous_commit=on`, `statement_timeout=3000ms`, `lock_timeout=750ms`.
- No superuser, `BYPASSRLS`, `CREATEDB`, `CREATEROLE`, schema `CREATE`, or table ownership.
- `dw_m1_terrain`, `dw_runtime`, `dw_m1_broker`, `dw_m1_cognition`, and `dw_m1_trajectory` are mutually exclusive for Terrain and Trajectory writes.

## Closed Intake Profile (`m1_terrain_dns_observation_v1`)

Input files must be valid UTF-8 JSON (`<= 16384` bytes, nesting depth `<= 8`) with `#[serde(deny_unknown_fields)]`:

```json
{
  "version": "m1_terrain_dns_observation_v1",
  "engagement_id": "00008301-0000-0000-0000-000000000001",
  "campaign_id": "00008301-0000-0000-0000-000000000002",
  "operator_ref": "00000000-0000-0000-0000-000000000011",
  "expected_mission_revision": 1,
  "goal_ref": "00000000-0000-0000-0000-000000000013",
  "exercise_mode": "blind",
  "correlation_id": "00000000-0000-0000-0000-000000000701",
  "source_kind": "operator_reviewed_dns",
  "vantage_ref": "00000000-0000-0000-0000-000000000031",
  "resolver_ipv4": "192.0.2.53",
  "subject_name": "www.example.invalid",
  "asset_ref": "00000000-0000-0000-0000-000000000021",
  "expected_claim_revision": 0,
  "perceived_at": 1760000100,
  "uncertainty_seconds": 1,
  "transport": "udp",
  "qtype": "A",
  "qclass": "IN",
  "rcode": "noerror",
  "truncated": false,
  "completeness": "complete",
  "limitations": ["single_vantage_only"],
  "answers": [
    { "ipv4": "198.51.100.10", "ttl_seconds": 120 }
  ]
}
```

### Intake Rules
- **Closed source**: `source_kind` must be `"operator_reviewed_dns"`.
- **Forbidden caller flags**: caller-supplied `accepted`, `fact`, `claim_status`, `trust_tier`, or `corroborated` fields are rejected before persistence or diagnostic output.
- **No unsafe/raw material**: raw packets, TXT records, HTTP headers, URLs, credentials, and free-form annotations are rejected; rejected values are never echoed on stdout or stderr.
- **Bounds**: `answers` has `<= 4` distinct IPv4 records (`ttl_seconds <= 86400`), `limitations` has `<= 4` distinct codes (`cache_remaining_ttl`, `single_vantage_only`, `partial_alias_chain`, `resolver_recursion_variance`), and `uncertainty_seconds <= 300`.

## Narrow Claim & Freshness Semantics

1. **Meaning**: A supported claim (`dns_hostname_ipv4_resolution`) means only that the declared `operator_reviewed_dns` source reported this `subject_name -> IPv4` binding from `(vantage_ref, resolver_ipv4)` at `perceived_at`. It proves no ownership, origin discovery, backend identity, reachability, foothold, vulnerability, or execution permission.
2. **Distinct identities**: `observation_id`, `material_id`, `evaluation_id`, `claim_id`, and `event_id` are deterministically derived and distinct per `(engagement_id, campaign_id, operation_id)`.
3. **Trust tier & status**:
   - Complete `noerror` material with non-empty answers, `lowest_ttl_seconds > 0`, and no `partial_alias_chain` limitation yields `trust_tier = "tier_1"`, `claim_status = "observed"`, `corroborated = false`.
   - Incomplete, truncated, partial-alias, non-`noerror`, empty-answer, or `ttl_seconds = 0` material is recorded with `claim_status = "withheld"` and `usable_routing_premise = false`. Safe historical material retains its explicit limitations.
4. **Read-time freshness**:
   - `valid_until = perceived_at + min(lowest_ttl_seconds, 60)` when `lowest_ttl_seconds > 0`.
   - `ttl_seconds = 0` sets `valid_until = perceived_at` (`freshness = "expired"`, `routing_usable = false`): valid historical material, never usable for later routing/contact.
   - `routing_usable` is computed at read/use time from current Mission authority (`authority_active`) and `perceived_at <= evaluated_at < valid_until`; there is no background timer or expiry writer.
5. **Rotation vs. conflict**:
   - Every revision retains a monotonic `temporal_frontier`: the highest accepted perception time, including any affected integrity hold. Late conflicting material cannot lower it.
   - A strictly later observation (`perceived_at > prior.temporal_frontier`) with matching `expected_claim_revision` may establish new applicability. Routing still requires qualified material and current freshness/authority. This allows normal RRset rotation and later qualified recovery without erasing historical conflicts.
   - A duplicate at the frontier must match the full material semantics (transport, query, RCODE, truncation, completeness, limitations and answers/TTLs), asset and time uncertainty, and have an unconflicted predecessor. Repeating a disputed variant under a new operation cannot select a winner. Earlier material, or incompatible material at the frontier, remains `conflicted` with `routing_usable = false`.
   - Accepted revisions retain their original identities and predecessor revision. Integrity holds are separate append-only references; fresh current views overlay holds without rewriting accepted source records. A superseded historical claim cannot be used as the current routing premise.

## Operator CLI Commands

### 1. Record a Reviewed DNS Observation

With an explicit core-allocated operation ID or `--operation new`:

```bash
duskweave m1-terrain-record --input /path/to/dns_observation.json --operation new
```

or with a pre-allocated operation UUID:

```bash
duskweave m1-terrain-record --input /path/to/dns_observation.json --operation <OPERATION_UUID>
```

Idempotent redelivery of the exact same request under the same `<OPERATION_UUID>` returns the durable acceptance without creating duplicate revisions or history rows. Conflicting admitted content under an existing `<OPERATION_UUID>` is refused with `reason = "input_mismatch"`, preserving the original committed identity. An eligible variant in the affected applicability durably holds its dependent use. Unattributed, ineligible or different-applicability variants cannot hold unrelated routing premises. Independent later qualified applicability remains eligible.

Trajectory verifies the committed Terrain source through its restricted role. Mismatched or unsafe variants never enter anomaly contracts: those contain only a static category and the committed affected claim reference. Anomalies dominate replay, inspection and recovery of the affected history; accepted history stays immutable.

If the COMMIT acknowledgment is lost, the initial receipt reports `result = "unresolved"`, `publication = "unknown"`, `history = "unknown"`, and preserves the allocated `operation_id`, including with `--operation new`. It does not claim rollback. Reconcile that same operation through inspect/recover; do not allocate a replacement. A verified pre-commit refusal reports `publication = "not_committed"`. Failed inspection reports uncertainty, never a verified absence.

### 2. Inspect an Existing Terrain Operation

Inspect durable Terrain state and Trajectory history in a fresh process without mutating state or performing network activity:

```bash
duskweave m1-terrain-inspect --input /path/to/dns_observation.json --operation <OPERATION_UUID>
```

Inspection re-evaluates `authority_active`, `freshness`, and `routing_usable` at current read time (`evaluated_at`). If Mission authority has been withdrawn, `authority_active` and `routing_usable` are `false`.

### 3. Recover Pending Trajectory History

Complete pending `trajectory.terrain_history.v1` obligations after a post-commit acknowledgment loss or transient Trajectory outage:

```bash
duskweave m1-terrain-recover --input /path/to/dns_observation.json --operation <OPERATION_UUID>
```

Recovery is strictly database-only: it verifies the committed `terrain.*` source rows through Trajectory's narrow port, records the accepted history row once, and allocates no replacement operation identity.

## Running Focused Native Tests

Provision a fresh disposable `dw_native_fixture_<random>` database on the owned `dwpg-r3` service and run the four Terrain test suites:

```bash
python tests/support/m1_native_bootstrap.py --container dwpg-r3 \
  --report <new-report-dir> -- cargo test \
  --test m1_terrain --test m1_terrain_postgres \
  --test m1_terrain_commit_ack --test m1_terrain_cli -- --test-threads=1
```

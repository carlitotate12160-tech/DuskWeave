# RUN_B1A — Durable Nonpositive Assessment

B1a adds a Mission-owned local assessment decision and an immutable required
publication obligation. It does **not** deliver that contract to Trajectory.
Every assessment receipt reports the original decision-time producer view:
`history=pending`, `history_reason=not_published_at_decision`, and
`history_view=producer_receipt_as_of_decision`, including recovery. For current
Trajectory publication status, use [RUN_B1B](RUN_B1B.md). Neither a successful exit nor a stored decision grants current
permission or target execution authority.

## Prerequisites

- Rust 1.94.1 and an owned PostgreSQL 17 database with durability enabled.
- Apply `migrations/0001_mission_registration.sql`, then
  `migrations/0002_planning_assessment.sql` with an administrative role.
- Use a restricted runtime login with membership in `dw_runtime`; it has
  `SELECT`/`INSERT` on the new table, not update/delete/truncate or schema DDL.
- Set `DW_DATABASE_URL` to a password-bearing local loopback runtime DSN.
  Tests additionally require `DW_TEST_ADMIN_DATABASE_URL` and
  `DW_TEST_DATABASE_URL` for their isolated fixture. Do not print or save values.

The CLI qualifies durability and runtime privileges before business use.
Migration/administration is separate from the runtime process. No target
database or client material belongs in this configuration.

## Command

First obtain a stable handle using the existing command:

```text
prepare-operation --engagement <uuid> --campaign <uuid>
```

Create a JSON file of reviewed, non-sensitive operator metadata:

```json
{
  "engagement_id": "00000000-0000-0000-0000-000000000001",
  "campaign_id": "00000000-0000-0000-0000-000000000002",
  "purpose_ref": "00000000-0000-0000-0000-000000000003",
  "asset_ref": "00000000-0000-0000-0000-000000000004",
  "expected_mission_revision": 1,
  "current_authority_confirmed": true
}
```

Then invoke:

```text
assess --operation <prepared-uuid> --input <file> --recover false
assess --operation <same-uuid> --input <same-file> --recover true
```

All flags are mandatory exactly once. The file is bounded to 16 KiB, with
unknown or missing fields rejected. The boolean is an operator assertion under
the trusted local operator assumption; it is not authentication or proof that
current authority is valid. `recover=true` looks up only the original durable
operation and exact request; it does not read current Mission or reevaluate.
After `commit_unknown`, recover the **same** handle before any fresh retry.

## Receipt and limits

A durable receipt is one JSON object containing `result=durable`, the validated
original `PlanningAssessed` contract, `decision_origin=durable_record`, the fixed
`publication_obligation=trajectory.planning_history.v1`, and explicit
`scope=not_evaluated`, `window=not_evaluated`, `complete_assessment=false`,
`current_permission=false`. `basis_status=unavailable` means no current Mission
row was found when the original decision was made; it never defaults an exercise
mode. A known basis retains its typed mode and registration provenance in the
contract. All three event timestamps use the producer transaction's epoch
seconds, not commit wall time or a current-window verdict.

The only decisions are `unresolved_mission_basis`,
`refused_revision_mismatch`, `unresolved_authority_unconfirmed`, and
`unresolved_evaluation_incomplete`. They follow that precedence and are all
nonpositive. A verified absent recovery returns `result=not_committed`; database
or contract errors are not absence. An unknown commit emits `result=unknown`
with `action=recover_before_retry` and a failing exit.

The Mission row and required publication are atomic and durable, while the
Trajectory assessment effect count remains zero in B1a. Registration history,
`inspect`, and `reconcile` retain their M0A behavior. No purpose/asset
eligibility, current scope/window, positive permission, withdrawal, target
action, restore/failover, ARM64 qualification, or M0 seal is claimed.

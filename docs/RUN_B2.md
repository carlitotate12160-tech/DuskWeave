# RUN_B2 — Scoped Current-Mission Bounds on Assessment

B2 extends the B1 assess/planning-history path so a **new** assessment compares
the unchanged PlanningRequest against the scoped current Mission: registered
goal reference, included/excluded asset references, and validity interval. The
durable result is still always nonpositive; no positive eligibility, current
permission, or target execution exists. The fully matched outcome remains
`unresolved_evaluation_incomplete` because full admission and M0C withdrawal
are unfinished.

## Version distinction

New `PlanningAssessed` events carry `version=2` and, when a scoped Mission
basis exists, a `basis.scope` snapshot: `goal_ref`, `included_assets`, and
`excluded_assets` (each a bounded list of nonnil references; inclusion
nonempty; inclusion/exclusion overlap is legal and exclusion wins). Version 1
records retain their exact B1 semantics: `scope=None`, no purpose/asset/window
evaluation, and no scope key in stored JSON. Old rows are read without
rewriting contracts, timestamps, or identities.

## Prerequisites

Apply migrations 0001 through `0004_planning_assessment_v2.sql` in order as an
administrative role with application clients quiescent; 0004 only widens the
`planning_history` version check to `CHECK(version IN (1,2))` and is safe to
reapply. Runtime credentials stay in `DW_DATABASE_URL` (loopback, restricted
`dw_runtime` login); tests use `DW_TEST_ADMIN_DATABASE_URL` and
`DW_TEST_DATABASE_URL` against an isolated owned database. Do not print or
save DSN values.

## Commands and decisions

Use the same `prepare-operation`, `assess`, and `planning-history` commands as
[RUN_B1B](RUN_B1B.md); flags and request shape are unchanged. The producer
reads `mission.missions` inside its existing SERIALIZABLE transaction after
the duplicate/recovery lookup, never from registration history or Trajectory.
New v2 decisions, in precedence order:

| Condition | Decision |
| --- | --- |
| No current scoped Mission basis | `unresolved_mission_basis` |
| Expected revision differs from owner revision | `refused_revision_mismatch` |
| Current-authority assertion is false | `unresolved_authority_unconfirmed` |
| Request purpose differs from current `goal_ref` | `refused_purpose_mismatch` |
| Asset in `excluded_assets` (even if also included) | `refused_asset_excluded` |
| Asset absent from `included_assets` | `refused_asset_unknown` |
| `evaluated_at < starts_at` | `refused_not_yet_valid` |
| `evaluated_at >= ends_at` | `refused_expired` |
| All checks pass | `unresolved_evaluation_incomplete` |

Validity is `[starts_at, ends_at)` against the existing whole-second producer
transaction-start timestamp; there is no caller-controlled clock. Malformed
owner scope, decode errors, and unsupported revision/mode fail safely rather
than becoming absent-Mission or empty permissive scope.

## Historical labels and recovery

Both `assess` and `planning-history` receipts expose version-aware labels for
the original assessment time. For v1, `scope`/`window` remain `not_evaluated`.
For v2, `scope` is `not_evaluated` before the scope guards, then
`purpose_mismatch`/`excluded`/`unknown` on the matching refusal or `matched`
once they pass; `window` is `not_evaluated` until scope guards pass, then
`not_yet_valid`/`expired`/`within_window`. Both receipts keep
`complete_assessment=false` and `current_permission=false`.

Duplicate submission and `recover=true` return the original durable event —
version, decision, scope snapshot, timestamps, and identities — even after the
current Mission row disappears or changes; a genuinely new evaluation needs a
new operation handle. First v2 acceptance in Trajectory additionally requires
the scoped registration predecessor to match the event's exact scope snapshot;
missing or mismatched predecessors stay pending. Upgrading an old schema
preserves every v1 row and its semantics unchanged.

## Limits

No withdrawal, restriction, re-authorization, positive admission, target
connection, or M0 seal is claimed. The consumer never reads Mission tables.
See [RUN_B1A](RUN_B1A.md) for original v1 assess semantics.

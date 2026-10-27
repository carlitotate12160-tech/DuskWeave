# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave. Verify actual worktree, branch and HEAD before delivery.
- **Current Phase**: M1 is unsealed. Current work is the Windows developer
  entrypoint; its candidate and checks come from GitHub.
- **Active seals**: `DW-FOUNDATION-001` and `DW-M0-001`; evidence below is unchanged.
- Scope/priorities belong to [the roadmap](MVP_AND_DEFERRED_SCOPE.md); dependency
  navigation belongs to [BUILD_ORDER](BUILD_ORDER.md). Neither duplicates PR state.

## 2. Stage 0 Artifact Checklist & Verification

Bootstrap verification is historical; the full checklist remains in
[the pre-retirement record](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/ENGINEERING_STATE.md).
Current contributor protocol and quality rules live in [AGENTS](../AGENTS.md)
and [QUALITY_BAR](../QUALITY_BAR.md).

## 3. Exit Criteria Evaluation for `DW-BOOTSTRAP-001`

Completed bootstrap evidence remains in the historical record above; it is not
a new acceptance or a claim that every current artifact was checked then.

## 4. Active Invariants Enforced

Use [PRD-000](prd/PRD-000-product-thesis.md) and
[the invariant index](build-order/00-authority-and-invariants.md).

## 5. Foundation Seal Evidence

- **Authority completeness**: PRD-000..006 and ADR-001..007 are `ACCEPTED`.
- **Coherence**: Historical F1–F7 reconciliation remains recorded. The accepted 2026-09-29 INV-005 amendment preserves five operational models and ordinary-surface zero exposure while allowing bounded campaign-scoped operational-secret custody. Refreshed cross-foundation terminology and status scans found no new contradiction.
- **Document verification**: The reseal check covered 41 tracked Markdown files and 87 relative links with zero broken links; all documents remain below 400 lines, current-status scans are coherent, and `git show --check` passes for the sealed baseline.
- **Review**: One refreshed adversarial review covered initial access without a foothold, authorized reuse, resumable restart, custody loss, cross-campaign isolation, PostgreSQL/evidence contamination, safety freeze, termination/withdrawal, in-flight uncertainty, and the distinction between disposing DuskWeave-held copies and revoking the client-side credential. No blocking contradiction remains.
- **Scope**: This seal accepts the foundation design at baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0`. No runtime, tool integration, payload implementation, or CI result is claimed.
- **Product-owner action**: The product owner explicitly authorized the `DW-FOUNDATION-001` reseal on 2026-09-29 after accepting PRD-010.

---

## 5a. M0 Seal Evidence (`DW-M0-001`)

- **Scope**: accepts the bounded local M0 lane at verified commit `669f2f36bf2909f3c28f4018a6ae69e76364b286` — M0A registration/history, M0B planning/history, M0C withdrawal/refusal/recovery including C0 fresh-authority confirmation and the C1a/C1b corrections, plus the assurance tooling merged alongside (parser mutation qualification, dependency hardening).
- **Evidence**: owner-accepted `DW-DEMO-M0-TWO-ASSETS` — register → assess → withdraw → refuse → restart → inspect on two operator-reviewed real-asset scopes with live C0 exchanges; bounded Work REVIEW_PASS recorded through the PR #31 reconciliation; merged runtime deliveries #21–#30 each with exact-master CI; parser boundary qualification 33/17/0/16 (PR #32, run 37297574836); dependency gate with real negative controls (PR #33); exact-master run 37339827937 SUCCESS on the sealed commit.
- **Limits**: local loopback core only — no target observation, acquisition, execution, compromise, reachability or ownership proof is claimed; retained demo database `dw_m0_demo_two_assets_20261005` remains evidence, not authorization. The scoped historical RED-evidence gap remains UNVERIFIED-not-PASS; the DSN rotation/reference sync remains IDE attestation. The seal grants no `DW-DOMAIN-001`, no target runtime, no LLM integration and no five-model scaffolding. `DW-FOUNDATION-001` is unchanged.
- **Product-owner action**: the product owner explicitly accepted the Work-reviewed demo evidence and authorized the M0 seal on 2026-10-05.

---

## 6. Current slices and next action

**Next product action:** a fresh authorized provider-fixture demonstration on
the merged source, preferably through `deepseek-responses-low-v1` per the
[operator guide](RUN_M1_AUTONOMOUS_FIXTURE.md#provider-fixture-lane-opt-in-not-inherited-consent),
then Linux acquisition-boundary qualification.
`DW-FIX-M1-DNS-SELECTOR-CONTRACT` merged as `e11f62f`
([#120](https://github.com/carlitotate12160-tech/DuskWeave/pull/120)): an
admitted `observe_more` + `dns_udp` proposal carrying contextual references
alongside a single canonical `asset_N` seed now selects and reserves that seed
identically in Rust and restricted SQL, and pack-local `asset_N` ordinals
resolve through entity identity to the correct `seed_names` entry even when
ContextPack omission shifts them. Fresh disclosure, current terms/window,
credential and actual platform qualification are required before provider
contact. Historical consent is unusable.

Relevant merged implementation/evidence pointers, not a rolling merge log:

| Outcome | Durable reference |
| --- | --- |
| Controlled skeleton | [#74](https://github.com/carlitotate12160-tech/DuskWeave/pull/74), [record](workflows/DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON.md) |
| Real-provider wiring and diagnostics | [#107](https://github.com/carlitotate12160-tech/DuskWeave/pull/107), [#109](https://github.com/carlitotate12160-tech/DuskWeave/pull/109), [diagnostics evidence](workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md#12-merge-reconciliation-2026-10-22) |
| Successor lifecycle / Rust fixture supervision | [#116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116), merged `618c885`; [final Windows qualification](workflows/DW-FIX-M1-FIXTURE-SUPERVISOR-RUST.md#final-windows-qualification-2026-10-25) |
| Nonblocking release and transport error precedence | [#117](https://github.com/carlitotate12160-tech/DuskWeave/pull/117), merged `7d7e696`; [record](workflows/DW-FIX-M1-INFERENCE-RELEASE-INTEGRITY.md) |
| Conditional maintenance and qualified skill-doc CI routing | [#118](https://github.com/carlitotate12160-tech/DuskWeave/pull/118), merged `d29a1fb` |
| DNS selector contextual references and entity-based ordinals | [#120](https://github.com/carlitotate12160-tech/DuskWeave/pull/120), merged `e11f62f` |
| Rejected-provider diagnostic metadata with uncertainty-only accounting | [#122](https://github.com/carlitotate12160-tech/DuskWeave/pull/122) |

**Unproved:** the 2026-10-22 provider run `4a4ce477-5734-4b92-bb6a-dce74f430114`
committed Decision 1 and fixture Terrain; Decision 2 ended exit 124 with
`may_send_uncertain`. Exact terminating guard and complete cognitive feedback
remain unproved. [Retained evidence](workflows/DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE.md)
does not authorize replay or establish a successful successor episode.

**Live acquisition remains unqualified:** the 2026-10-19 WSL inventory showed
active 2 GiB swap and a `wsl-capture-crash` core handler; it did not qualify
ADR-015 no-swap/no-dump, egress, host STOP or disposal. Qualify the actual Linux
boundary before live DNS, then HTTPS. Windows fixture qualification is not Linux
acquisition evidence. No host reconfiguration or target contact is authorized here.

**Debt remains open:** use [roadmap sections 1.3-1.5](MVP_AND_DEFERRED_SCOPE.md#13-audit-disposition-attached-to-these-deliveries)
and current candidate metrics. Legacy CLI/M0 behavior remains supported until
tested replacements and compatibility disposition exist. No M1/full-domain seal,
new provider support or blanket Windows full-suite PASS is implied.

**Active CI work:** `DW-FIX-CI-RUST-TWO-WORKERS` — the instrumented Rust suite
moves to one shared build, two supervised workers and a single merged coverage
report behind the unchanged `rust` check; candidate and evidence stay in the
open draft PR on GitHub.

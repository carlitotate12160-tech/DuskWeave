# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave
- **Workspace**: `D:/DuskWeave-ci-complexity` (selected reusable M1 source worktree; verify actual branch/HEAD per packet)
- **Current Phase**: M1 Terrain-informed qualification lane at merged [PR #69](https://github.com/carlitotate12160-tech/DuskWeave/pull/69) (squash merge commit `20671c64e706df570c6e00b673655fbfe4455d31` on 2026-10-19T08:51:52Z; predecessor PR #68 merged `ff9c3703c375990e4a55db9e68527e4dcd34c96f`, exact-master CI run 39200545621 SUCCESS). Delivered by `demo/m1-one-console-20261019`: a one-use foreground console helper removed the post-DNS review round trips (14 synthetic tests PASS; representative rehearsal `REHEARSAL_PASS` at 9.156 s); episode `20261019-3` then recorded **LIVE_FAIL** — review→write→`continue` measured 12.844 s, Terrain intake accepted a fresh `observed` claim with 47 s remaining, `m1-infer` ran with `terrain_0` carried in the sent P and reserved==actual call identity; the generation call **completed** (10.084 s provider transfer, real usage; admission `Repairable`, code `proposal_shape` per the retained repair body) and the repair call persisted `may_send_uncertain` after the inference process exited nonzero — send-path attribution UNVERIFIED; the shared 20 s `decision_wall_time_ms` watchdog `process::exit(124)` mid-send mechanism was demonstrated offline through the real transport (finding F3; exact historical killer unverified, 124 ambiguous with the outer command guard). Prior episodes `20261019-1`/`20261019-2` (**LIVE_BLOCKED**: `operator_continue_eof`; `terrain_freshness_insufficient` after a ~48 s combined post-response interval) and the historical 2026-10-18 **LIVE_FAIL** remain preserved; see [M1_TERRAIN_INFORMED_DNS_QUALIFICATION.md](M1_TERRAIN_INFORMED_DNS_QUALIFICATION.md). `dispatch_granted = false`; no M1 seal.
- **Active Seal**: `DW-FOUNDATION-001`; `DW-M0-001`
- **Seal Status**: **SEALED — EXPLICITLY RESEALED** (product-owner authorization, 2026-09-29; coherence verified); **M0 SEALED** (product-owner authorization, 2026-10-05)
- **Sealed Authority Baseline**: `f93087b52c480822544bad0fb5d99d17eedf8ac0`
- **Prior Sealed Authority Baseline**: `5883fa52cd063083350a41a293e0bd654d500d63`
- **M0 Sealed Commit**: `669f2f36bf2909f3c28f4018a6ae69e76364b286` (tree `114271ed4f8131c0abf8b9999533e0edcb4853ca`; exact-master CI run 37339827937 SUCCESS)
- **Target Full-Domain Seal**: `DW-DOMAIN-001`, after accepted Stage 4 design and Stage 5 domain contracts; the M0 seal does not claim that seal
- **Date Historically Sealed**: 2026-09-28
- **Date Reopened**: 2026-09-29
- **Date Resealed**: 2026-09-29
- **Date M0 Sealed**: 2026-10-05

---

## 2. Stage 0 Artifact Checklist & Verification

| Document | Path | Status | Authority / Role |
| :--- | :--- | :--- | :--- |
| **Build Order** | `docs/BUILD_ORDER.md` + `docs/build-order/*.md` | VERIFIED | Thin index defines navigation; separate packets define stages, dependencies, and implementation bounds. |
| **Agent Protocol** | `AGENTS.md` | VERIFIED | Defines the authority hierarchy, invariants INV-001 through INV-007, and agent rules. |
| **Reasoning Skill** | `.agents/skills/build-duskweave/SKILL.md` | VERIFIED | Defines the reasoning workflow, PRD/ADR authoring guidance, and domain contracts. |
| **Quality Bar** | `QUALITY_BAR.md` | VERIFIED | Defines module/diff LOC budgets, the testing bar, and the God Object prohibition. |
| **Engineering State** | `docs/ENGINEERING_STATE.md` | VERIFIED | Repository tracking status for seals, milestones, and active gaps. |
| **PRD Registry** | `docs/prd/README.md` | VERIFIED | Registration index for PRD-000 through PRD-021 plus writing rules. |
| **ADR Registry** | `docs/adr/README.md` | VERIFIED | Registration index for ADR-001 through ADR-024 plus architecture conventions. |

---

## 3. Exit Criteria Evaluation for `DW-BOOTSTRAP-001`

- [x] **Authority hierarchy documented**: Established hierarchically in `AGENTS.md` and `docs/BUILD_ORDER.md` (PRD > ADR > Domain Contract > Quality Bar > AGENTS.md > SKILL.md > Implementation).
- [x] **Build order documented**: `docs/BUILD_ORDER.md` is the canonical index; the details of the 22 stages are split into bounded packets under `docs/build-order/*.md`.
- [x] **God-object rules documented**: INV-001 and the prohibition on monolithic managers are documented in `AGENTS.md` and `QUALITY_BAR.md`.
- [x] **Module-size rules documented**: The `< 300 LOC` ideal bound, the `400 LOC` review reference point (historically hard-capped), and the responsibility-based split rules are documented in `QUALITY_BAR.md`.
- [x] **ADR/PRD conventions documented**: Format, content constraints, and governance are described in `docs/prd/README.md` and `docs/adr/README.md`.

---

## 4. Active Invariants Enforced

All interactions and plans are subject to:
- **INV-001**: No God Object
- **INV-002**: Separate Operational Models (`CyberTerrain`, `FootholdGraph`, `AttackPathView`, `ObjectiveState`, `CampaignTrajectory`)
- **INV-003**: Reasoning != Execution (Proposal → Deterministic Validation → Capability Gateway → Executor)
- **INV-004**: Observation != Fact
- **INV-005**: Sensitive Data Isolation and Campaign-Scoped Secret Custody
- **INV-006**: Audit Integrity
- **INV-007**: Defender Knowledge Boundary (PRD-000 §6; mode-specific authority)

---

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

- **Active work / PR**: none open — latest merged delivery `demo/m1-one-console-20261019` (`DW-DEMO-M1-ONE-CONSOLE-REVIEW-RETEST` + `DW-DIAGNOSE-M1-SCHEMA-DEADLINE` cycle-2 correction — documentation and test-only diagnostics) delivered as [PR #69](https://github.com/carlitotate12160-tech/DuskWeave/pull/69), squash-merged 2026-10-19T08:51:52Z as `20671c64e706df570c6e00b673655fbfe4455d31` at BASE `ff9c3703c375990e4a55db9e68527e4dcd34c96f` — records the one-console review helper qualification (14 synthetic tests PASS against real child pipes; representative foreground rehearsal `REHEARSAL_PASS`, receipt→approve→write→continue 9.156 s ≤ 30 s target) and episode `20261019-3` **LIVE_FAIL** (fixture `dw_native_live_43b4dc45f93d682e`; measured handoff 12.844 s; one operator-reviewed DNS datagram admitted fresh/`observed` with 47 s remaining; `m1-infer` ran once — `terrain_0` in sent P, reserved==actual call IDs, generation `completed` with real usage (input 2653 / output 2012 / reasoning 1552 tokens, 10084 ms; admission `Repairable`/`proposal_shape` per retained repair body), repair persisted `may_send_uncertain` after the inference process exited nonzero (send-path attribution UNVERIFIED) — the shared 20 s `decision_wall_time_ms` watchdog `exit(124)` mid-send mechanism was demonstrated offline through the real transport with a synthetic in-memory Broker (unscaled child exited 124 at 20047 ms with 2 accepted loopback connections; does not prove the historical send or durable ledger state); zero decisions/outbox/history/closures; failures `inference_exit`, `inference_receipt_absent`, `identity_invalid` ×3, `durable_completion_absent` — finding F3). Predecessor `demo/m1-terrain-retest-20261019` merged as [PR #68](https://github.com/carlitotate12160-tech/DuskWeave/pull/68) (`ff9c370`, 2026-10-19), recording retest episodes `20261019-1`/`20261019-2` (**LIVE_BLOCKED**: `operator_continue_eof`; `terrain_freshness_insufficient`) and the stdin transport qualification; PR #67 previously delivered the premise-retention/receipt-identity remediation. Details in [M1_TERRAIN_INFORMED_DNS_QUALIFICATION.md](M1_TERRAIN_INFORMED_DNS_QUALIFICATION.md).
- **Next action**: Work decision on finding F3 — the offline diagnosis is merged (PR #69); fact: the repair call persisted `may_send_uncertain` with no final accounting; whether it entered `release_into`/`p.call`/a network send is UNVERIFIED (ADR-016 permits a crash between marker commit and send; live exit code and phase not retained). `DW-DIAGNOSE-M1-SCHEMA-DEADLINE` (+cycle-2 correction) delivered: emitted-schema vs decoder matrix (27 synthetic cases, jsonschema 4.26.0 Draft 2020-12; 11 valid = 8 accepted + 3 refused, 16 invalid = 2 tolerated nullable-key omissions + 11 repairable + 3 rejected; no demonstrated shape defect), real `decide`+transport demonstration of the shared-deadline `exit(124)` mid-repair-send through a synthetic Broker (accepted-connection evidence, not durable-ledger proof), a corrected oracle that rejects fast/early/missing-terminal false success, and controls proving exit 124 is ambiguous between the transport watchdog and the outer command guard. Disposition per current evidence: bounds preserved (shared 20 s, outer 30 s, no repair clock reset), no demonstrated contract violation, no authorized budget change; any policy amendment requires separate Work resolution. Any subsequent live episode requires fresh consent and the wall-budget decision first. The live episode established a real Terrain-informed ContextPack send (premise retained as `terrain_0`), a completed provider generation, and correct uncertain accounting — but no committed decision, admitted proposal, or citation. Acquisition, adaptation, cognitive MVP completion, and the M1 seal remain unqualified.
- **Bounded McCabe hotspot disposition**: Static caller/consumer review at `BASE` (`4d938e7`) confirmed no runtime defect across the 13 retained named Terrain/CLI/history hotspots (`PgTerrainStore::record_dns` 24, `load_existing` 17, `read_current_claim` 10, `inspect_dns` 9; `verify_mission_permission` 13; `TerrainReceipt::from_event` 10, `map_policy_reason` 10; `connect_qualified` 8, `run_record` 8, `command` 8; `EvaluationOutcome::as_str` 8; `record_terrain_history` 12, `inspect_terrain_history` 13) and the 1 retained ambiguous `src/m1_terrain_cli.rs::command::<anonymous>` closure group. All 13 named items and the ambiguous closure owner remain untouched read-only dependencies; all new and modified functions in `feat/m1-terrain-context` satisfy McCabe `<= 7`.
- **Advisory LOC disposition**: 12 file warnings (9 pre-existing files: `migrations/0014_m1_terrain.sql` 523, `src/m1_context.rs` 462, `src/m1_reasoning/postgres.rs` 542, `src/m1_terrain.rs` 601, `src/m1_terrain_input.rs` 446, `tests/m1_terrain.rs` 589, `tests/m1_terrain_postgres.rs` 992, `tests/support/m1_native_cli.rs` 653, and `tests/test_complexity_ratchet.py` measured at 803 lines at BASE; plus 3 delivery additions: `src/m1_terrain_view.rs` 575, `tests/m1_terrain_context.rs` 887, `tests/m1_terrain_context_postgres.rs` 1,466) and 2,046 runtime changed lines (336 lines in `migrations/0015_m1_terrain_context_view.sql` plus 1,710 lines across 13 `src/` files). Each module retains a single cohesive owner responsibility (`m1_terrain_view.rs` holds pure Terrain view/query/dependency contracts and validation; `m1_context.rs` holds pure ContextPack assembly and byte-fitting; `m1_reasoning/postgres.rs` holds Broker/Cognition PostgreSQL adapters and role qualification; test suites hold cohesive scenario matrices sharing one harness).
- **Reconciled predecessor merges (GitHub master)**:
  - [PR #68](https://github.com/carlitotate12160-tech/DuskWeave/pull/68) (`ff9c370`): retest records — episodes 20261019-1/-2 LIVE_BLOCKED and stdin transport qualification; reviewed head `7a292e3` tree `4cb47b3`, merged 2026-10-19, exact-master CI 39200545621 SUCCESS
  - [PR #67](https://github.com/carlitotate12160-tech/DuskWeave/pull/67) (`aadd540`): Terrain premise retention/receipt-identity qualification fix preserving the historical LIVE_FAIL episode; reviewed head `887e0ad` tree `6d19c4e`, merged 2026-10-19, exact-master CI 39194044884 SUCCESS
  - [PR #66](https://github.com/carlitotate12160-tech/DuskWeave/pull/66) (`fd73340`): bounded Terrain-informed live qualification runner (Python orchestration, synthetic entrypoint tests, operator guide); reviewed head `0b94c03` tree `e5f02a0`, merged 2026-10-18, exact-master CI 39134848423 SUCCESS
  - [PR #65](https://github.com/carlitotate12160-tech/DuskWeave/pull/65) (`a5f5f76`): Terrain-informed ContextPack/native inference, canonical lineage/release guards and historical pack/proposal inspection; reviewed tree matches `4efef0a`, merged 2026-10-18, exact-master CI 39126574504 SUCCESS
  - [PR #64](https://github.com/carlitotate12160-tech/DuskWeave/pull/64) (`4d938e7`): fix(quality): enforce McCabe ratchet against validated base
  - [PR #63](https://github.com/carlitotate12160-tech/DuskWeave/pull/63) (`10051c2`): feat(m1): implement safe DNS observation intake, Terrain claim qualification, and Trajectory history
  - [PR #62](https://github.com/carlitotate12160-tech/DuskWeave/pull/62) (`cd7e482`): fix(m1): repeatable native live testing + one bounded live episode
  - [PR #61](https://github.com/carlitotate12160-tech/DuskWeave/pull/61) (`b4be03e`): chore: simplify delivery policy and advisory metrics enforcement
  - [PR #60](https://github.com/carlitotate12160-tech/DuskWeave/pull/60) (`304b809`): feat(m1): implement production-native inference and continuation
  - [PR #59](https://github.com/carlitotate12160-tech/DuskWeave/pull/59) (`1a0a4c3`): build(deps): bump actions/upload-artifact from 7.0.1 to 7.0.2
  - [PR #58](https://github.com/carlitotate12160-tech/DuskWeave/pull/58) (`dabb4c3`): build(deps): bump the codeql-action group across 1 directory with 2 updates
  - [PR #56](https://github.com/carlitotate12160-tech/DuskWeave/pull/56) (`2000e2d`): build(deps): bump serde_json from 1.0.151 to 1.0.154
  - [PR #57](https://github.com/carlitotate12160-tech/DuskWeave/pull/57) (`4c76b80`): build(deps): bump uuid from 1.26.1 to 1.28.0
- **Governing accepted decisions & milestone evidence**:
  - Canonical engineering policy: [QUALITY_BAR.md](../QUALITY_BAR.md)
  - Architectural authority & invariants: [AGENTS.md](../AGENTS.md)
  - Foundation seal evidence: Section 5 above; baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0`
  - M0 seal evidence: Section 5a above; commit `669f2f36bf2909f3c28f4018a6ae69e76364b286`
  - M1 enabling contracts: [M1 enabling architecture](contracts/M1-enabling-architecture.md), [M1 external orientation decision loop](contracts/M1-external-orientation-decision-loop.md), and [ADR registry](adr/README.md)
  - Historical delivery details: preserved in Git commit history and GitHub pull request records.

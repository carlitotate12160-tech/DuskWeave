# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave
- **Workspace**: `D:/DuskWeave-ci-complexity` (selected reusable M1 source worktree; verify actual branch/HEAD per packet)
- **Current Phase**: Lifecycle design PR #115 is merged at `79b0e3ee1bea44b5e5639c1142af45b6c44918b4`. Runtime [DRAFT PR #116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116) remains NOT READY. The [qualification closure](workflows/DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE.md#native-qualification-closure-2026-10-24) retains the prior flag-conflict STOP. The issued coverage-command amendment removes only redundant `--no-clean`; next: one fresh committed-source Windows matrix, then exact-candidate Linux CI and distinct review if it passes. Historical startup causation remains UNVERIFIED; no acceptance or seal changes.
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

- **Historical provider feedback remains unqualified**: run `4a4ce477-5734-4b92-bb6a-dce74f430114` committed the first decision and fixture DNS/Terrain; Decision 2 ended exit 124 with `may_send_uncertain`. Its exact terminating guard remains unproved. Preserve [the diagnostics reconciliation](workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md#12-merge-reconciliation-2026-10-22); this runtime packet grants no replay or inherited consent.
- **Delivered skeleton**: [PR #74](https://github.com/carlitotate12160-tech/DuskWeave/pull/74) (`feat/m1-autonomous-skeleton`, [DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON](workflows/DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON.md)) was squash-merged to `master` at `94c240b` on 2026-10-20T10:01:59Z. The controlled autonomous skeleton on master covers one `m1-run` start → first decision → Broker-admitted fixture DNS → issued-result Terrain intake → causally linked second decision → durable history/inspect under real restricted PostgreSQL principals, with the campaign advisory fence and `session_fences.epoch` ordered ahead of `run_current` and mutable control/admission writes across `reserve_fixture_dns`, `commit_fixture_dns`, `run_stop`, `run_unknown`, `run_terminal` and `run_phase_complete` (`migrations/0017_autonomous_fixture_run.sql`), `source` omitted on zero-claim `cyber_terrain` owner views (`src/m1_context.rs`), the 400 ms test-owned lease custodian for positive supervised fixture work, and the [M1 enabling contract](contracts/M1-enabling-architecture.md) top-level `Autonomous-run amendment` metadata row reconciled with accepted section 7. Final-head CI on `059a34b` (run `39400100099`) passed every required check including the full-suite coverage gate at the 90% total / 80% per-production-file floors. Fixture success proves orchestration plumbing only — no real LLM cognition, live acquisition, target permission or M1 seal is claimed. Windows full-suite legacy `CREATE/ALTER ROLE` execution remains BLOCKED under the `NOCREATEROLE` native-local admin; its absence is a local-environment limit, not a Windows full-suite PASS.
- **Delivered packet / PR**: [PR #107](https://github.com/carlitotate12160-tech/DuskWeave/pull/107) (`feat/m1-provider-fixture-feedback`, [DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK](workflows/DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK.md)) was squash-merged to `master` at `2af7d01c9aa29c799d159d127a29b5766c60cd5f` on 2026-10-21T09:30:54Z. F-B/C1-C4 and their FIX-cycle history are retained in the packet, including the Linux readiness blocker that was root-caused to the test-only reply relay's missing TCP_NODELAY. The bounded relay amendment enables TCP_NODELAY on both relay sockets before forwarding/cloning, refuses failed socket configuration and retains framing/fault ordering. Windows focused regressions and Linux focused/full coverage passed on the merged source without changing production timing or assertions; detailed measurements and review evidence remain in the packet.
- **Active runtime / next action**: [PR #116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116) stays DRAFT / NOT READY. The [existing ledger](workflows/DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE.md#native-qualification-closure-2026-10-24) preserves the sampler removal, 18 scenarios and prior Windows 57/57 + 18/18 / instrumented flag-conflict STOP. The issued amendment authorizes only removal of `--no-clean`, its regression assertion and one fresh `20261024d` qualification. Next: commit the bounded correction, run once and stop on first failure; publish and qualify exact-candidate Linux/review only after success. Historical 5,068 ms causation remains UNVERIFIED; no FIX-cycle reset, provider/target/resolver contact, merge or seal.
- **Delivered diagnostics FIX**: [PR #109](https://github.com/carlitotate12160-tech/DuskWeave/pull/109), [DW-FIX-M1-RUN-DIAGNOSTICS](workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md), merged at `ef23262`. Start preflight, safe closed child observations, restricted immutable diagnostic publication and fresh inspection are delivered. R1/R2 correct readiness settlement and post-phase-expiry accounting within remaining run time. Root-cause amendments fix the withdrawal fixture's discarded serialization-rejection receipt, retain same-operation bounded resubmission, separate fixture responsibilities and enforce the original settlement deadline; production and SQL are unchanged by those fixture amendments. After owner-reported W32Time automatic startup / WSL restart, final Windows targets passed 17/17 and 14/14. Exact-head [Linux run 39735029731](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39735029731) passed all nine jobs with 96.79% production coverage; the separate closure reviewer returned READY-for-delta. This closes the required gate for that run, not platform-wide clock safety. The physical cause of the old rollback and discarded historical stop/Terrain errors remain unverified. Accepted section 7 / ADR-016 section 4a text `0615fb72c946fd6abf0823692be621096091377c` and all seals remain unchanged.
- **Debt follow-up**: the skeleton refactors Terrain transaction orchestration/row decoding: `record_dns` 24 → 6 and `load_existing` 17 → 2 in local pinned analysis against the packet base. That skeleton left 11 named hotspots and the ambiguous CLI closure open. Final-head PR #109 CI retains ten unchanged hotspots; PR #107 resolved Terrain CLI command debt (original PR-base 8 to 5); remaining debt stays OPEN under [roadmap section 1.4](MVP_AND_DEFERRED_SCOPE.md#14-delivery-performance-and-debt-closure-owner-direction-2026-10-19). Shared inference-basis construction is reused. Existing operational CLI/M0 consumers remain until replacement/compatibility evidence exists; deleting them is not counted as delivered debt repayment.
- **Deployment evidence (2026-10-19, read-only)**: Ubuntu WSL2 is running; its inspected environment exposes active 2 GiB swap and a `wsl-capture-crash` core handler with crash-dump enablement. Rust/Go/nft executables were not found on that shell PATH. Docker Desktop runs Linux/cgroup v2 and the owned `dwpg-r3` PostgreSQL 17 digest-pinned service remains running. This inventory does not qualify capture/exporter no-swap/no-dump/egress/STOP/disposal, does not prove captured-data leakage, and authorizes no host reconfiguration. Current acquisition readiness remains UNVERIFIED/BLOCKED pending actual qualified deployment.
- **Historical Terrain-context McCabe disposition (before this skeleton)**: Static caller/consumer review at `BASE` (`4d938e7`) confirmed no runtime defect across the 13 retained named Terrain/CLI/history hotspots (`PgTerrainStore::record_dns` 24, `load_existing` 17, `read_current_claim` 10, `inspect_dns` 9; `verify_mission_permission` 13; `TerrainReceipt::from_event` 10, `map_policy_reason` 10; `connect_qualified` 8, `run_record` 8, `command` 8; `EvaluationOutcome::as_str` 8; `record_terrain_history` 12, `inspect_terrain_history` 13) and the 1 retained ambiguous `src/m1_terrain_cli.rs::command::<anonymous>` closure group. All 13 named items and the ambiguous closure owner remain untouched read-only dependencies; all new and modified functions in `feat/m1-terrain-context` satisfy McCabe `<= 7`.
- **Historical Terrain-context LOC disposition**: 12 file warnings (9 pre-existing files: `migrations/0014_m1_terrain.sql` 523, `src/m1_context.rs` 462, `src/m1_reasoning/postgres.rs` 542, `src/m1_terrain.rs` 601, `src/m1_terrain_input.rs` 446, `tests/m1_terrain.rs` 589, `tests/m1_terrain_postgres.rs` 992, `tests/support/m1_native_cli.rs` 653, and `tests/test_complexity_ratchet.py` measured at 803 lines at BASE; plus 3 delivery additions: `src/m1_terrain_view.rs` 575, `tests/m1_terrain_context.rs` 887, `tests/m1_terrain_context_postgres.rs` 1,466) and 2,046 runtime changed lines (336 lines in `migrations/0015_m1_terrain_context_view.sql` plus 1,710 lines across 13 `src/` files). Each module retains a single cohesive owner responsibility (`m1_terrain_view.rs` holds pure Terrain view/query/dependency contracts and validation; `m1_context.rs` holds pure ContextPack assembly and byte-fitting; `m1_reasoning/postgres.rs` holds Broker/Cognition PostgreSQL adapters and role qualification; test suites hold cohesive scenario matrices sharing one harness).
- **Reconciled predecessor merges (GitHub master)**:
  - [PR #114](https://github.com/carlitotate12160-tech/DuskWeave/pull/114) (`56039de`), merged 2026-10-22: tracking reconciliation of the PR #109 merge and final qualification; documentation only.
  - [PR #74](https://github.com/carlitotate12160-tech/DuskWeave/pull/74) (`94c240b`), merged 2026-10-20T10:01:59Z: controlled autonomous skeleton — fenced authority/admission writes, zero-claim view `source` omission, test-owned lease custodian; orchestration evidence only, not cognition or target qualification.
  - [PR #73](https://github.com/carlitotate12160-tech/DuskWeave/pull/73) (`00e5667`), merged 2026-10-20T01:46:50Z: existing roadmap now schedules delivery performance and legacy/debt closure.
  - [PR #71](https://github.com/carlitotate12160-tech/DuskWeave/pull/71) (`dca2564`), merged 2026-10-20T01:25:36Z: accepted autonomous-run DESIGN; synthetic examples remain capacity evidence, not runtime qualification.
  - [PR #72](https://github.com/carlitotate12160-tech/DuskWeave/pull/72) (`ca7d4ef`), merged 2026-10-20T01:04:00Z: conservative document-only classification, sanitized progress and separate normal/coverage caches. Current final checks are read from GitHub; no cached PASS or blanket test parallelism is implied.
  - [PR #70](https://github.com/carlitotate12160-tech/DuskWeave/pull/70) (`535eab5`): tracking reconciliation after the PR #69 merge; the autonomous DESIGN base
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

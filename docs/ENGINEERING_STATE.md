# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave
- **Workspace**: `D:/DuskWeave-ci-complexity` (selected reusable M1 source worktree; verify actual branch/HEAD per packet)
- **Current Phase**: Active delivery on `fix/native-live-repeatability` (repeatable native live testing: fresh `dw_native_` database selection, explicit `--env-file` key loading, honest live outcomes). Base `b4be03ebd5a4165cb20f4dbe4dca048c754bf21f`. Predecessors #61, #60, #59, #58, #56, #57 merged. Next work: a sourced real observation feeding an LLM hypothesis/next decision through the owning model and existing history path.
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

- **Active work / PR**: `fix/native-live-repeatability` — Make native live testing repeatable (fresh `dw_native_` database names, explicit `--env-file` key loading, honest `LIVE_*` outcomes) and execute one bounded live DeepSeek episode. Base `b4be03ebd5a4165cb20f4dbe4dca048c754bf21f`. Candidate head, CI and review status are tracked directly in Git and GitHub.
- **Next action**: Review and merge this delivery. Follow-up: a sourced real observation for niagamas.com informs an LLM hypothesis/next decision, retained through the owning model and existing history path.
- **Reconciled predecessor merges (GitHub master)**:
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

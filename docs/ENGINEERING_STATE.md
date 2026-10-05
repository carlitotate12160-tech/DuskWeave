# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave
- **Workspace**: `D:/DuskWeave`
- **Current Phase**: Accepted bounded M0 implementation lane — M0A registration/history MERGED; M0B partial (B1a, B1b, B2 and R3 MERGED); M0C-C0 fresh-authority confirmation MERGED through PR #21; baseline planning-CLI complexity FIX MERGED through PR #23; M0C-C1a withdrawal/history MERGED through PR #22; baseline-complexity convergence FIX (`DW-FIX-M0-BASELINE-CONVERGENCE`) MERGED through PR #29 at `90f2ebd` with the pinned content of sibling candidates #26–28 incorporated (#26–28 are externally CLOSED and superseded); CI McCabe gate qualification MERGED through PR #24 at `5859f16`; C1b versioned durable refusal MERGED through PR #25 at `e030e88`; `DW-IMPLEMENT-M0-POSITIVE-ADMISSION` MERGED through PR #30 at `51aaddd`; the bounded two-asset M0 demo (`DW-DEMO-M0-TWO-ASSETS`) executed locally and its evidence is delivered for distinct Work review; `DW-IMPLEMENT-M0-PARSER-MUTATION` is a parallel parser-verification candidate awaiting distinct Work review; M0 remains DEMO_PENDING and unsealed. Stage 4 design dependencies outside this lane remain deferred.
- **Active Seal**: `DW-FOUNDATION-001`
- **Seal Status**: **SEALED — EXPLICITLY RESEALED** (product-owner authorization, 2026-09-29; coherence verified)
- **Sealed Authority Baseline**: `f93087b52c480822544bad0fb5d99d17eedf8ac0`
- **Prior Sealed Authority Baseline**: `5883fa52cd063083350a41a293e0bd654d500d63`
- **Target Full-Domain Seal**: `DW-DOMAIN-001`, after accepted Stage 4 design and Stage 5 domain contracts; bounded M0 delivery does not claim that seal
- **Date Historically Sealed**: 2026-09-28
- **Date Reopened**: 2026-09-29
- **Date Resealed**: 2026-09-29

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
- [x] **Module-size rules documented**: The `< 300 LOC` ideal bound, the `400 LOC` hard cap, and the responsibility-based split rules are documented in `QUALITY_BAR.md`.
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

## 6. Current slices and next action

Latest verified delivery base: `51aaddd9d372cb4cd5563a517ce15693f42f6a01` on live remote `master`, the 2026-10-05 merge of [PR #30](https://github.com/carlitotate12160-tech/DuskWeave/pull/30), `DW-IMPLEMENT-M0-POSITIVE-ADMISSION`; its tree `4e7062922fa268950fcd78a6f70b43bf380921c0` is byte-identical to the reviewed head `b9918445df4df96c41e90e1ee186f340695b23ed`. Exact-master check runs on `51aaddd` (`ci-ok`, `structure`, `rust`, `links`, `codeql`) completed SUCCESS, verified live 2026-10-05; the packet-pinned run is [37281203009](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37281203009). Prior base: PR #25 merged at `e030e88f84d6ba1f2d16811e7bb9b4b969f64db9` with exact-master [CI run 37269346971](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37269346971) SUCCESS. The owner's external acceptance note retains one scoped historical RED-evidence gap as **UNVERIFIED-not-PASS**; it granted no exception to the positive packet's TDD or final-head gates. Unchanged predecessor reviews/evidence remain historical, not new candidate verification.

### Current work

| Slice | State | Pinned evidence | Blocker / next action |
| --- | :--- | :--- | :--- |
| `DW-FIX-M0-BASELINE-CONVERGENCE` (PR #29) | MERGED through PR #29 at `90f2ebd` on 2026-10-04; merged tree byte-identical to the reviewed integration head `b600c47293612be949f6d79c9c66817df3ab9daf`; baseline converged to zero violations | predecessor local evidence from the reviewed #29 delivery (136 tests passing, 96.95% production coverage — reusable, not a new local run); head-bound report under `D:/DuskWeave-build/rust-1.94.1/reports/DW-FIX-M0-BASELINE-CONVERGENCE/` | none for master; incorporated #26–28 content needs no further runtime merge |
| PR #24 CI McCabe gate | MERGED through PR #24 at `5859f16` on 2026-10-05: history-preserving merge of `90f2ebd` into the reviewed `97dcca8` gate candidate; refreshed checker and gate qualified on the converged baseline | exact-master [CI run 37257453380](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37257453380) SUCCESS | none for master |
| PR #25 C1b durable refusal | MERGED at `e030e88` on 2026-10-05; tree identical to accepted `c5f3dbf` | exact-master CI run 37269346971 SUCCESS; external acceptance records the scoped historical RED gap as UNVERIFIED-not-PASS | none for predecessor; no new TDD exception |
| PRs #26/#27/#28 complexity candidates | externally CLOSED as observed live on 2026-10-05; superseded — their pinned content is incorporated into master through #29 | incorporation pinned by the #29 delivery | no further runtime merge |
| `DW-IMPLEMENT-M0-POSITIVE-ADMISSION` | MERGED through PR #30 at `51aaddd` on 2026-10-05; tree identical to reviewed `b991844` | exact-master check runs on `51aaddd` SUCCESS | none for master |
| `DW-DEMO-M0-TWO-ASSETS` | executed locally 2026-10-05 on branch `demo/m0-two-assets` at base `51aaddd`; register/assess/withdraw/refuse/restart/inspect for two operator-reviewed real-asset scopes completed with live C0 exchanges; retained demo database `dw_m0_demo_two_assets_20261005` on `dwpg-r3` | receipts/inputs/report under `D:/DuskWeave-build/rust-1.94.1/reports/DW-DEMO-M0-TWO-ASSETS/`; final candidate SHA stays in the external delivery report | distinct Work review of the demo evidence; M0 stays unsealed |
| `DW-IMPLEMENT-M0-PARSER-MUTATION` | parallel parser-verification candidate on `test/m0-parser-mutation` in `D:/DuskWeave-ci-complexity` (PR #32); verification tooling and test assertions only, runtime diff 0 | base `51aaddd`; baseline scan 33 mutants / 8 meaningful survivors, all addressed by `tests/parser_mutation.rs`; candidate rescan and Linux CI qualification run with the delivery | distinct Work review, then separately authorized integration |
| M0 seal | pending; M0 DEMO_PENDING and unsealed | demo evidence alone grants no seal | explicit owner seal action after accepted demo review |

### Historical deliveries

| Date | Delivery | Evidence |
| :--- | :--- | :--- |
| 2026-10-05 | PR #30 merged `DW-IMPLEMENT-M0-POSITIVE-ADMISSION` at `51aaddd9d372cb4cd5563a517ce15693f42f6a01` (tree `4e706292` identical to reviewed head `b991844`) | exact-master check runs on `51aaddd` SUCCESS; packet-pinned [CI run 37281203009](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37281203009) |
| 2026-10-05 | PR #25 merged C1b durable refusal at `e030e88f84d6ba1f2d16811e7bb9b4b969f64db9` (tree identical to accepted `c5f3dbf`; scoped RED gap remains UNVERIFIED-not-PASS) | [CI run 37269346971](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37269346971) |
| 2026-10-05 | PR #24 merged `DW-FIX-CI-MCCABE-FINALIZE` gate qualification on the converged baseline at `5859f16e7f69a1c44103f2d8cfba6efa7ccec03f` | [CI run 37257453380](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37257453380) |
| 2026-10-04 | PR #29 merged `DW-FIX-M0-BASELINE-CONVERGENCE` at `90f2ebdc3851d19aba1786b55a66284fc2de8466` (tree identical to reviewed head `b600c47`; incorporated the pinned #26–28 refactors and closed the remaining baseline violations) | [CI run 37218404785](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37218404785) |
| 2026-10-03 | PR #22 merged the rebased M0C-C1a withdrawal/history correction at `73b0b6ff49a2d5ec08f8a71c5ace5d2144e015cc` (superseded published head `0be98a78a8f141d50cb2c9b745eb49fd24ae3726`) | [CI run 37134662240](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37134662240); local repair metrics `D:/DuskWeave-build/rust-1.94.1/reports/DW-M0C-C1A-LOC-REPAIR` |
| 2026-10-03 | PR #23 merged `DW-FIX-M0-PLANNING-CLI-COMPLEXITY` at `22230eb7458e9ca708db9ffa9df0f339c9bfc038` (cmd_assess 18→2, cmd_planning_history 16→4; `run`=9 qualified pure dispatch at 222 runtime lines) | [CI run 37126501314](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37126501314) |
| 2026-10-03 | PR #21 merged `DW-IMPLEMENT-M0C-C0-FRESH-AUTHORITY`: fresh bounded challenge/response for new affirmative assess; durable duplicates and recover=true stay read-only | [CI run 37092184903](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37092184903) |
| 2026-10-02 | PR #18 merged the R3 planning-history journal extraction; PR #19 merged workflow corrections; PR #20 merged the QUALITY_BAR section 7 review-policy documentation | [CI run 36992649673](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/36992649673); [CI run 37022757041](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37022757041) |
| 2026-10-01 | PR #5 merged M0A registration/history; PR #9 coverage gate and PR #10 CodeQL maintenance merged; verified master `387910c06e7d26c6b81da9fd2a0001e974bf9aa7` | [CI run 36806405110](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/36806405110): 29 tests, coverage 94.43% (695/736) |

Preserved owner decisions and C1a verification evidence (historical, superseded by the PR #22 merge):

- The owner rejected a runtime-ceiling increase and authorized direct inspection/refactoring; that instruction superseded the earlier FIX packet's mandatory binary CLI-child partition without relaxing owner, behavior, file, complexity or cumulative-budget rules.
- C1a local evidence: the corrected all-targets instrumented run passed 120 tests with zero ignored on Windows x64/MSVC Rust 1.94.1 + PostgreSQL 17.11 under the restricted candidate login; production coverage 96.24% (1996/2074), every executable file >=80%; measured C1a functions/closures all <=7 with `run` <=10 as qualified pure dispatch; runtime 594/600 cumulative; fmt, Clippy, links, structure, checker selftests, audit and whitespace passed.
- C0 requires a fresh bounded dialogue for new affirmative assessments; historical duplicate/recovery paths stay read-only. C1a returns bounded `authority_withdrawn` before allocation/insertion and does not implement the C1b durable refusal event.
- Merged partial M0B: B1a, B1b, B2 and R3; see [RUN_B1A](RUN_B1A.md), [RUN_B1B](RUN_B1B.md) and [RUN_B2](RUN_B2.md).

`DW-FOUNDATION-001` remains explicitly resealed against authority baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0`. PRD-000..010, ADR-001..008, and the [minimum M0 mission/planning/history contract](contracts/M0-mission-authority-history.md) are ACCEPTED under their recorded owner decisions. The accepted bounded sequencing permits local M0 delivery before full domain/evidence coverage; it grants no target acquisition/execution or full `DW-DOMAIN-001` seal.

Next: distinct Work review of the delivered `DW-DEMO-M0-TWO-ASSETS` evidence (register -> assess -> withdraw -> refuse -> restart -> inspect on two real-asset scopes) and of the `DW-IMPLEMENT-M0-PARSER-MUTATION` verification candidate (PR #32), then a separately authorized owner decision on the M0 seal. Eligible v4 records only the Mission bounds checked at the original revision/time; assessment completion also requires durable Trajectory history and never grants current permission or dispatch. Legacy v1/v2/v3 meanings remain unchanged. M0 is DEMO_PENDING and unsealed. No acceptance or seal is granted by this reconciliation. ADR-009..012 remain deferred until their named evidence/proof/sensitive/key behavior requires them; target execution, LLM integration and five-model scaffolding remain outside M0.

The local `.cargo/config.toml` is untracked configuration. Preserve it; do not stage, delete or overwrite it automatically.

Per `docs/BUILD_ORDER.md`, PRD-000, PRD-001, and PRD-002 are `ACCEPTED`. The product-owner revision on 2026-09-27 established the INV-007 Defender Knowledge Boundary in PRD-000 §6 (the "current campaign position" wording was aligned across documents); PRD-002 was expanded with tiered epistemic confidence (Tier 1/2/3) and PROVISIONAL status; control-gap assessment keeps distinguishing conclusive evidence from incomplete telemetry.
- **DW-DESIGN-002**: PRD-003 Access & Footholds, PRD-004 Expansion Loop, PRD-005 Objective Loop, and PRD-006 Adaptation were accepted by the product owner on 2026-09-27 after corrections to observation bounds, initial access, sensitive proof, and synthetic examples. All four are `ACCEPTED`; this is not a foundation seal.
- **Cross-document reconciliation**: PRD-000 INV-004 asserts validation and reconciliation for every observation with corroboration burden proportionate to impact; PRD-002 distinguishes OBSERVED, PROVISIONAL, and evidence-supported facts. INV-007 retains `current campaign position`. All PRD-001..006 references, initial-access bounds, sensitive proof, and traces were checked on 2026-09-27.
- **ADR-001 acceptance**: The product owner accepted the modular campaign core on 2026-09-27 along with the core + worker alternative and the shared process failure/reconciliation consequence before retry. ADR-001 is `ACCEPTED`. PRD-001 §3.3 was aligned with presumed/confirmed loss in PRD-003 under the same approval.
- **ADR-002 acceptance**: The product owner's decision on 2026-09-27 was REVISE, THEN ACCEPT. Four clarifications have been applied and verified: objective eligibility refers to Terrain/Access/Mission claims; Gateway, Broker, and Adapter are separated with a prohibition on bypass dispatch; the command interface is intent-specific; and transient read-only acquisition remains PROVISIONAL. Historical evidence remains bounded by provenance/freshness and does not prove current access. ADR-002 is `ACCEPTED`; this acceptance is neither a seal nor runtime evidence.
- **ADR-003 acceptance**: ADR-003 Domain Events was written and checked through document review plus one adversarial pass as `PROPOSED` based on the ACCEPTED ADR-002. The review clarified identity/deduplication scope and the separation of redelivery metadata from event semantic content. Scope: domain-owned events, bounded consumer contracts, recoverable publication/delivery, duplicate handling, ordering/freshness, replay without execution, plus sensitive-data bounds and exercise mode. The 2026-09-28 revision addressed B1–B7 with explicit consumer effects, required/optional obligation classification, verifiable integrity-conflict resolution, upgrade compatibility, shutdown without waiting for backlog, campaign isolation, and correction by revision/causality. Seven review cases were added; rebuild equivalence compares evaluation time under the same reconciliation rules. The product owner accepted this revision on 2026-09-28; ADR-003 is `ACCEPTED`. ADR-004 became the next design dependency.
- **ADR-004 acceptance**: ADR-004 Rust Core Language was written as `PROPOSED`: Rust for the core, Go for integrations per baseline; types only guarantee local invariants, not permanent freshness/authorization. The language boundary preserves authority, event recovery, sensitive proof, and exercise mode. QUALITY_BAR examples were aligned with PRD-003 and ADR-002. The 2026-09-28 brainstorming revision clarified the separation of Rust use cases from the not-yet-chosen inference runtime, reasoning depth, untrusted proposal admission, bounded correction/retry, per-component Python promotion through an ADR, and empirical evaluation without assuming identical output. The product owner accepted the revision on 2026-09-28; ADR-004 is `ACCEPTED`. This approval locks the ADR-004 decision, not the foundation seal; build-order section 30 still requires ADR-005..007 and whole-foundation coherence.
- **ADR-005 acceptance**: The product owner accepted ADR-005 on 2026-09-28: PostgreSQL becomes the sole initial campaign-core system of record with no SQLite mode. Owner-local transactions use durable outbox/inbox, explicit concurrency/recovery, and audit/sensitive-data restrictions. In-memory graph evaluation is the ADR-006 design task; DuckDB for Observer was not selected and remains pending the Observer stage. The 2 OCPU / 12 GB host estimate is provisional planning, not a benchmark result or deployment guarantee. ADR-005 is `ACCEPTED`; this is not a schema, a runtime implementation, or a tamper-proof audit claim. At the time of ADR-005 acceptance, ADR-006/007 had not been written.
- **ADR-006 acceptance**: The product owner accepted ADR-006 on 2026-09-28 after STALE/PROVISIONAL alignment and view-feasibility review. PostgreSQL remains the durable Terrain state owner; a bounded Rust graph is only a disposable derived view after workload evidence. Expiry is evaluated at use time, views whose mode/scope/causality are ineligible are rejected, and as-known snapshots are distinguished from retrospective interpretation. Acceptance did not select a graph crate, did not accept ADR-007, and did not authorize runtime.
- **ADR-007 acceptance**: The product owner accepted FootholdGraph and AttackPathView Separation on 2026-09-28. Access owns validated position, condition/loss, and genuinely live operational dependencies; Pathing owns candidates and transition projections that are rebuildable from source revisions. The review clarified transitive transport dependency versus credential provenance, per-dimension/mechanism health, invalidation from all relevant owner premises, projection lag that must not ground safety, and direct fallback. The parallel case distinguishes D1 (validated), D2 (reached transient access but failed validation and underwent lifecycle downgrade), and D3 (unknown external outcome that forms no access claim). Operator-authorized priorities only parameterize bounded reasoning; intent-weighted selection belongs to the reasoning use case, not Pathing owner state. Transient access, tool success, historical routes, and path caches create neither foothold nor dispatch authority. ADR-007 is `ACCEPTED`; the following coherence check was completed and the foundation was then explicitly sealed on 2026-09-28.
- **PRD-007 acceptance**: The product owner accepted the Observation Model on 2026-09-28 after revision review. Observation is distinguished from reconnaissance; raw capture is distinguished from foundation-level raw observation; admission is bounded without a global barrier; only owner-qualified results may influence non-authoritative uses such as hypothesis ranking; freshness is rooted in effect/observation time; and protected-edge denial remains a narrow result without circumvention permission. This acceptance does not seal `DW-DOMAIN-001` and does not authorize runtime or acquisition tooling.
- **PRD-008 acceptance**: The product owner accepted the Evidence Model on 2026-09-28 after bounded revision. The model distinguishes Observation, admitted technical evidence material, claim-specific evaluation, EvidenceEnvelope, owner claim, and client proof; it sets burden per owner contract, stable logical identity with bounded semantic lineage, reviewer authority that does not mutate owner state, time-basis uncertainty, current-use eligibility, and late-contamination handling that does not protect prohibited bytes. Acceptance did not select schema, custody manager, cryptography, storage, sensitive-remediation mechanism, or runtime; it does not change the seal.
- **PRD-009 acceptance**: The product owner accepted Client Proof on 2026-09-29 after refinement of actionable remediation, stable client-facing finding traceability, bounded scanner-signal reporting, attack-narrative composition, and completion-scope distinctions. ProofEnvelope remains a least-disclosure, reviewable client-facing derivation; verified result, inferred root cause, prospective impact, severity, and the client risk decision remain separate; release sign-off stays at the disclosure boundary. Acceptance did not select a report engine, portal, scoring system, cryptography, storage, sensitive-remediation mechanism, or runtime; it does not change the seal.
- **PRD-010 acceptance / INV-005 reconciliation**: The product owner accepted Sensitive Data Handling on 2026-09-29 after review of real assessment practice and sanitization assurance. Proof content remains attempt-ephemeral, while separately authorized operational authentication material may be reused only inside isolated campaign-scoped custody. Core, PostgreSQL, event/retry, observation/evidence/proof, operator, and reasoner receive only an opaque reference and safe metadata. Custody is non-durable by default, may survive pause/restart only for explicit resumability, must not cross campaigns/retests, every use requires current authority, and termination/withdrawal triggers disposal plus honest disposition. Storage, cryptography, recovery, sanitization, and runtime mechanisms remain deferred.
- **Quality enforcement**: McCabe policy is 7 for business/other functions and 10 only for qualified pure dispatch under QUALITY_BAR.md; function length above 50 triggers evidence-based review. Numerical measurement/enforcement must be reported separately from policy. Runtime now exists through merged M0A; formatting, Clippy, PostgreSQL integration tests, cargo audit, CodeQL, source/diff budgets and fixed production line-coverage gates are configured. Current execution evidence is the exact-master CI run recorded above; analyzer availability does not imply every architectural obligation is mechanically enforced.
- **DW-FOUNDATION-COHERENCE-001 reconciliation**: F1–F5 were fixed in PRD-000/001, QUALITY_BAR.md, and the ADR-002/005 footers. Cross-checks of Position/transient/validated, current state/history/correction, stop/freeze/termination, crate/module ownership, foundation relative links, statuses, and INV-001..007 were repeated; one adversarial review of the changed result sharpened the safety-freeze scope. F6 was then accepted by the product owner: rapid, bounded, and persistent engagement uses one campaign model with an operator-authorized envelope; extended duration is not a requirement for every engagement, there is no artificial delay, and expiry produces a bounded-completion report plus residual uncertainty. Concrete duration presets and the scheduler remain deferred. F7 was then accepted by the product owner: continuity does not depend on target-side persistence; arbitrary malware and unmonitored implants are prohibited; a temporary managed artifact is only a future optional capability with explicit authorization, lease/capability bounds, a non-sensitive manifest, revocation, expiry, a cleanup plan, opaque cleanup evidence, and honest residual reporting. Format, signing, isolation, transport, and cleanup mechanism remain deferred to Capability/Runtime design. This F1–F7 reconciliation itself does not authorize runtime; the seal is recorded separately through explicit product-owner action.
- **Foundation / Stage 4 boundary**: `DW-FOUNDATION-001` was explicitly resealed at authority baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0` after INV-005 reconciliation; the earlier historical baseline remains `5883fa52cd063083350a41a293e0bd654d500d63`. Stage 4 authoring remains permitted; the accepted bounded local M0 lane permits issued runtime packets. Target capability execution and tool integration remain unauthorized.

## 7. Engineering setup maintenance

Project Instructions and the three separate skills were prepared through
user-requested configuration work on 2026-09-27. This does not change the product seal.
- Architecture/packet preparation: `.agents/skills/duskweave-engineering/SKILL.md`.
- Packet execution: `.agents/skills/build-duskweave/SKILL.md`.
- Distinct adversarial review: `.agents/skills/duskweave-adversarial-review/SKILL.md`.
- Setup navigation: `docs/workflows/START_HERE.md`.
- GitHub/CI hardening (user-requested, 2026-09-30): default branch is `master`; squash-only merge with auto-delete of head branches; `default-branch` ruleset requires a PR, conversation resolution, the `ci-ok` check (aggregator of all gated jobs), and an up-to-date branch with no routine bypass; secret scanning + push protection + Dependabot alerts/security updates are enabled; `dependabot.yml` covers github-actions; `scripts/check_structure.py` enforces the category-specific file budgets, forbidden `utils/helpers/common/misc/managers` paths, conflict markers, and Rust hygiene (active once `.rs` files exist). Workflows run on GitHub-hosted runners with minimal token permissions, actions pinned to full SHA, and no client credentials. M0A and PRs #9/#10 now provide Rust formatting/Clippy/full tests under coverage, PostgreSQL 17 integration, cargo audit, CodeQL and fixed 90% total / 80% per-file production line-coverage enforcement. Dependabot cargo is configured. Mutation testing, parser fuzzing, benchmarks, ARM64 matrix, restore/failover qualification and SBOM/signing remain deferred until the behavior requires them.
- Historical seal `DW-BOOTSTRAP-001` still refers to the earlier baseline. `DW-FOUNDATION-001` is now the active seal; Stage 4 authoring runs separately from runtime authorization.


## 8. Owner-authorized LOC-policy calibration

Owner-authorized on 2026-10-01 through DW-FIX-LOC-POLICY-001: production/tooling
files remain <=400; test/benchmark files <=500; Markdown <=600 subject to narrower
document rules. Runtime diffs prefer <300, trigger explicit cohesion/ownership
review above 400 and stop above 600 or the packet's lower declared ceiling.
Physical counting and SQL inclusion remain. ADR-004 section 6 now references
the canonical delivery budgets without duplicating a stale runtime ceiling;
language ownership and the recorded foundation seal baseline are unchanged. The existing distinct review must
resolve the trigger; numerical CI success is insufficient. Packet sizing includes
formatted measurement and correction room. Reassess after five runtime deliveries.

Owner approval applies to engineering policy, not a new product acceptance or
seal. Delivery/review/merge and candidate CI must be verified from current
GitHub evidence; this entry does not establish those outcomes. Historical policy execution base:
`a072b2e8ccf7d3311214d5be0d34543179e5f4bf`
(PR #11 R1 merged); the earlier master/CI entry in section 6 is historical evidence.
M0A remains implemented, M0B/C pending, M0 DEMO_PENDING and unsealed.

## 9. Owner-authorized review-policy clarification

On 2026-10-03 (Asia/Jakarta), the product owner authorized the proposed quality
guardrails and their application through review. QUALITY_BAR.md section 7 owns
the eight-question evidence-based review; source skills and packet/PR templates
reference it. The 50-line function threshold is a review trigger; McCabe stays 7
for business/other functions with 10 only for qualified pure dispatch. Existing
file and cumulative runtime budgets remain. No analyzer or CI gate is added here.

Historical review-policy candidate snapshot: verified preflight master was `0df83ae73065f38f46605d5ed077a638329aac0c`; PR #19's
workflow correction is merged in that baseline. This document-only review-policy
candidate is authored for delivery, not recorded merged or runtime-verified.
The local C0 implementation worktree was observed in progress at that baseline;
no C0 PR or merge is claimed. Complete its issued packet and review before the
subsequent withdrawal slice. Full positive admission, M0 demo and seal remain pending.
No product acceptance, capability authorization or historical seal changes.

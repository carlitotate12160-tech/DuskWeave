# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave
- **Workspace**: `D:/DuskWeave`
- **Current Phase**: Accepted bounded M0 implementation lane — M0A registration/history MERGED; M0B partial (B1a, B1b, B2 and R3 MERGED); M0C-C0 fresh-authority confirmation MERGED through PR #21; baseline planning-CLI complexity FIX MERGED through PR #23; M0C-C1a durable withdrawal/history MERGED through PR #22; C1b versioned durable refusal draft PR #25 (F2/F3 reviewed, F1 blocked); registration-complexity draft PR #26 (scoped review passed, unmerged); Trajectory-complexity draft PR #27 (F1 CLOSED on original evidence provenance, scoped review reused); CI McCabe gate draft PR #24 (unchanged checker sibling); partial FIX DW-FIX-M0-MISSION-DECODING-COMPLEXITY in delivery; Mission/planning baseline remainder, PR #24-27 integration and positive admission pending; M0 DEMO_PENDING and unsealed. Stage 4 design dependencies outside this lane remain deferred.
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

## 6. Next Immediate Action

Latest verified delivery base: `73b0b6ff49a2d5ec08f8a71c5ace5d2144e015cc` on remote `master`, verified live on 2026-10-04 as the squash-merge of [PR #22](https://github.com/carlitotate12160-tech/DuskWeave/pull/22), which merged the reconciled M0C-C1a durable local withdrawal/history candidate at reviewed head `640968028d7871c9e98e3227cae20cac8f75e1a2`: local withdrawal acceptance records an immutable Mission marker and publication obligation, immediately denies new affected assessments, writes separate Trajectory history/completion and keeps fresh read-only recovery. Exact-master [CI run 37134662240](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37134662240) succeeded on that merge. Earlier verified delivery base: `22230eb7458e9ca708db9ffa9df0f339c9bfc038` on remote `master`, verified live on 2026-10-03 as the squash-merge of [PR #23](https://github.com/carlitotate12160-tech/DuskWeave/pull/23), which merged the baseline-only FIX `DW-FIX-M0-PLANNING-CLI-COMPLEXITY` at reviewed head `9edce0c586bfedc4930479d8d537770ecf05e4f8` (tree-identical to the merge result). Exact-master [CI run 37126501314](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37126501314) succeeded on that merge: 103 passed, zero failed/ignored, production line coverage 95.92% (1624/1693), `planning_cli` 96.63% (86/89), all fixed coverage floors pass. The two planning CLI entrypoints moved into binary-private `src/planning_cli.rs`, closing the recorded baseline findings `cmd_assess=18` and `cmd_planning_history=16` as superseded measurements (`18 -> 2` and `16 -> 4` own McCabe, `run`=9 qualified pure dispatch at 222 runtime lines); the historical measurements and their report provenance remain recorded below as historical. Earlier history: PR #21 merged `DW-IMPLEMENT-M0C-C0-FRESH-AUTHORITY` on 2026-10-03 at base `cc6e157393c4c6961e32af27a69f60a16e6bab42` (exact-master [CI run 37092184903](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37092184903)); `assess` on a brand-new `current_authority_confirmed=true` request passes one bounded live challenge/response on stderr/stdin scoped to that invocation's transient challenge before the unchanged Mission assessment runs, while durable duplicates and `recover=true` keep the existing read-only path with no interaction. PR #18 merged R3's behavior-preserving extraction of planning-history identity/dedup/conflict persistence into a narrow Trajectory-owned journal on 2026-10-02 ([CI run 36992649673](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/36992649673)); PR #19 merged the zero-runtime-change `DW-FIX-DELIVERY-CONTEXT-AND-BUILD-REUSE` workflow corrections, and exact-master [CI run 37022757041](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/37022757041) succeeded on that base; PR #20 merged the document-only review-policy update covering the QUALITY_BAR.md section 7 evidence-based review checklist, the 50-line function review trigger and the qualified pure-dispatch McCabe allowance. B1a, B1b, B2 and R3 are merged partial M0B; see [RUN_B1A](RUN_B1A.md), [RUN_B1B](RUN_B1B.md) and [RUN_B2](RUN_B2.md).

C0 requires a fresh bounded dialogue for new affirmative assessments; historical duplicate/recovery paths stay read-only. The M0C-C1a candidate in `D:/DuskWeave-m0c-c0`, branch `feat/m0c-c1a-withdrawal`, contains authored local withdrawal acceptance with an immutable Mission marker/publication obligation, immediate denial of new affected assessments, separate Trajectory history/completion and fresh read-only recovery. Focused C1a local tests passed on the staged candidate. The previous full instrumented run failed at the private upgrade fixture, whose missing 0005 bootstrap was a schema prerequisite failure. Revision 1 authorizes the two fixture corrections: a per-process upgrade database with collision denial, explicit confirmed ownership and success-only cleanup, plus independent registration CLI setup through 0005. The corrected upgrade target passed all four cases on Windows x64/MSVC Rust 1.94.1 and PostgreSQL 17.11, including collision denial, repeated 0005 preservation and ownership-checked success cleanup. The earlier registration CLI target failed its legacy universal error-prefix assertion. Revision 2 authorizes the withdrawal-specific amendment: exact bounded JSON rejection plus category error, empty stderr, failed exit and unchanged counts across seven tables using the restricted login. That amendment passed the focused assertion and the full 5-case CLI target. The replacement all-targets instrumented run passed all 120 tests with zero ignored/skipped on Windows x64/MSVC Rust 1.94.1 and PostgreSQL 17.11, using the restricted candidate login. Production line coverage is 96.13% (1989/2069); every executable production file exceeds 80%. Fmt, Clippy, links, structure, checker selftests, audit, whitespace and cumulative runtime budget passed. Runtime remains 573 additions + 3 deletions = 576/600; author and distinct adversarial review resolved the >400 cohesion/ownership trigger, including the fixture and receipt correction deltas. McCabe measurement is UNVERIFIED without an analyzer. The prior failed full run and stale assertion failure remain historical evidence. Final committed-head PR/CI outcomes belong to the external delivery report; this entry establishes local verification and review. This authored local candidate is not merged, accepted, demonstrated or sealed; check/review provenance belongs to the external delivery report.

### Delivered C1a correction (2026-10-03 — merged through PR #22)

The owner rejected a runtime-ceiling increase and authorized direct inspection/refactoring.
That instruction supersedes the earlier FIX packet's mandatory binary CLI-child partition;
it does not relax owner, behavior, file, complexity or cumulative-budget rules.
PR #23 has since merged at `22230eb7458e9ca708db9ffa9df0f339c9bfc038`; the preserved
local C1a correction is rebased onto that merged base and republished on the still-OPEN
PR #22 as the reconciled candidate head (the final SHA belongs to the external delivery
report, not this file). The superseded published head was `0be98a78a8f141d50cb2c9b745eb49fd24ae3726`.
The uncommitted revised worktree superseded the historical ready/UNVERIFIED claims above.
The prior 696-line correction STOP was valid; its claimed absolute 675-680 minimum was
an estimate, disproved by a formatted candidate of 594/600 under the default checker,
re-measured at the same 594 cumulative runtime lines on the rebased candidate.
No exception, policy, counting rule, schema or public contract was changed.

Binary-private withdrawal functions remain in main.rs (390 lines); Mission acceptance
and Trajectory history stay in separate owner modules (125/142 lines). The revised
producer removes the fresh flag and redundant eligibility/timestamp round trips;
explicit catalog projections preserve payload binding and fixed-header checks.
One owner-local SERIALIZABLE transaction retains duplicate/no-allocation behavior,
fresh insertion/commit and conservative unknown ACK. Recovery remains read-only.
The current CLI path still publishes or inspects history and emits the bounded receipt.

rust-code-analysis-cli 0.0.25 measures every C1a function and closure at <=7;
cmd_withdraw own=3, its attempt closure=5, parse=7, fresh_event=7,
withdraw own=4/transaction closure=7, publish=6, publish_outcome=7,
check_predecessor=7, event validation=5/binding=4. Raw JSON and method evidence:
`D:/DuskWeave-build/rust-1.94.1/reports/DW-M0C-C1A-LOC-REPAIR`.
Own complexity subtracts immediate child sums; aggregate cyclomatic.sum is not McCabe
for one function. SQL/catalog semantic checks remain review obligations, not analyzer coverage.
The previously recorded baseline cmd_assess=18 and cmd_planning_history=16 findings are
closed as superseded by merged PR #23 (2 and 4 on master); run remains <=10 as qualified
pure dispatch on this candidate. This does not claim repository-wide compliance or grant
a baseline exception.

Baseline and repaired focused C1a suites each passed 18 tests, zero ignored/skipped,
with real CLI, restricted PostgreSQL role, race and post-commit ACK-loss assertions.
Producer negative controls now also bind operation/registration IDs and coherent
wrong scope, and preserve collision-before-stale rejection. Fmt and all-targets
Clippy passed. Final all-targets instrumented suite passed 120 tests, zero ignored;
production coverage is 96.24% (1996/2074), every executable file >=80%.
The expanded producer corruption/precedence controls are included in this full run.
Author Q1-Q8 evidence is recorded with the local reports; it is not an independent
review. Links, structure, 10 structure selftests, 15 budget selftests, 7 coverage-gate
selftests, cargo audit and whitespace passed. That reviewed candidate
`640968028d7871c9e98e3227cae20cac8f75e1a2` then merged through PR #22 with
exact-master CI run 37134662240; earlier CI/coverage is historical. Container `dwpg-r3`, the restricted
C1a test database/login and ordinary build cache were reused. Original candidate,
profiles and unrelated worktrees were preserved; the removed CLI child has a backup
outside source. The subsequent PR #22 publication and merge are recorded above; no acceptance or seal is granted.

`DW-FOUNDATION-001` remains explicitly resealed against authority baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0`. PRD-000..010, ADR-001..008, and the [minimum M0 mission/planning/history contract](contracts/M0-mission-authority-history.md) are ACCEPTED under their recorded owner decisions. The accepted bounded sequencing permits local M0 delivery before full domain/evidence coverage; it grants no target acquisition/execution or full `DW-DOMAIN-001` seal.

Historical repository verification on 2026-10-01: local and remote `master` were `387910c06e7d26c6b81da9fd2a0001e974bf9aa7`; PR #5 merged M0A registration/history at `913d6f2`, PR #10 merged CodeQL maintenance at `ac40ca9`, and PR #9 merged the line-coverage gate at `387910c`. No open PR was returned by repository search. M0A runtime exists: local CLI registration, required publication, Trajectory history, inspection and reconciliation. Do not rebuild it.

Exact-master CI [run 36806405110](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/36806405110) succeeded on that SHA: links, structure/runtime budget, Rust, CodeQL and `ci-ok`. The Rust job reports 29 passing tests, 0 failures/ignored, PostgreSQL 17 integration, and production line coverage 94.43% (695/736); the fixed 90% total / 80% per-source-file gate passed. This is GitHub-hosted CI evidence, not a new local test run, proof of every supported platform, or a client campaign demonstration.

Next: deliver the issued partial FIX `DW-FIX-M0-MISSION-DECODING-COMPLEXITY` as one draft PR to master; it removes only the two scoped Mission row-decoding McCabe violations while preserving real assessment, duplicate and recovery behavior, and the Mission/planning baseline remainder plus draft-PR findings stay pending their own packets. Draft PR #25 carries C1b versioned durable refusal (F2/F3 reviewed, F1 blocked), draft PR #26 carries the registration-complexity FIX (scoped review passed, unmerged), draft PR #27 carries the Trajectory-complexity FIX (F1 CLOSED, scoped review reused) and draft PR #24 carries the CI McCabe gate; their integration and the remaining minimum-M0 work continue under separate packets. C1a returns bounded `authority_withdrawn` before allocation/insertion and does not implement that durable refusal event. Full positive admission remains unfinished. M0 is DEMO_PENDING and unsealed; the accepted register -> assess -> withdraw -> refuse -> restart -> inspect demonstration remains outstanding. No acceptance or seal is granted by this status correction. ADR-009..012 remain deferred until their named evidence/proof/sensitive/key behavior requires them; target execution, LLM integration and five-model scaffolding remain outside M0.

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

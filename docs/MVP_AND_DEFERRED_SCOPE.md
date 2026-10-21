# DuskWeave MVP Direction and Deferred Scope

| Metadata | Value |
| :--- | :--- |
| Recorded | Initial direction 2026-09-30; offensive-direction guardrails 2026-10-03; cognitive MVP scope/phase confirmation 2026-10-11 |
| Review base | `d7297dad0f5269b3d95e2bd9dc1d28919c2ecd97` |
| Status | Owner-approved direction; M1 product/behavior contract and linear delivery ACCEPTED 2026-10-06; M1 cognitive direction approved 2026-10-07 (R2); enabling architecture and implementation remain subject to accepted authority |
| Authority | PRD-000..010 and ADR-001..008; DW-FOUNDATION-001 unchanged |
| Purpose | Preserve MVP boundaries and future candidates without designing future subsystems now |

## 1. MVP direction

Preserve DuskWeave's campaign reasoning identity, Strategic/Access/Expansion/Objective loops, adaptation, five owner-specific operational models, and INV-001..007.

- Initial engagements are bounded in hours. Long dwell is a future capability, not an initial delivery requirement.
- Low-footprint, purpose-driven actions, current authority, stopping, accountable history, and sensitive-data boundaries apply from the first applicable behavior.
- First-party core is Rust. Additional languages require a concrete need and applicable accepted authority; the ADR-004 Go adapter baseline still requires a bounded amendment for affected Rust-first integration responsibilities.
- Initial deployment is single tenant. Explicit engagement/campaign isolation remains mandatory; no speculative tenant framework is required.
- Product demonstrations use authorized real-client observations/results. Deterministic isolated failure-path tests remain separate from live pilot evidence.
- Add one mission-relevant family of dependency or identity/trust relationships through eligible acquisition and owner reconciliation. Relationship discovery supplies a sourced candidate, not access or third-party execution authority.
- Owner-approved delivery rhythm: minimum accepted outcome/contracts -> code/tests -> demo -> same-head review/fixes -> exact-head verification -> explicit bounded milestone seal.
- Research is targeted at material ambiguity; no blanket operator-study requirement is introduced for every PRD/ADR.

### 1.1 Cognitive MVP completion

On 2026-10-11 the owner confirmed that operator-style hypothesis/action/evidence/adaptation
belongs in MVP: the LLM chooses and revises strategy, may challenge the supplied options,
request bounded additional owner context and identify a capability gap. Deterministic
authority validates current use and execution; it cannot secretly replace strategy.
APT41/Lazarus inform behavioral questions, not actor-persona playbooks or a claim of
equivalent operator capability. The full four-loop/access breadth remains governed by
its actual later dependencies; M1 does not acquire that breadth by analogy.

A first real-inference ContextPack/proposal/history path is a partial delivery. Cognitive
MVP completion additionally needs owner-qualified action outcomes consumed by the LLM,
belief/question revision and another justified proposal, with bounded applicable
campaign-local feedback. Synthesis/evolution remains in the approved R2 M1 direction
under fixed effect, build, qualification and admission boundaries; compile/test success
alone proves neither mission success nor an unrestricted new effect.

Each complete observable runtime outcome follows linear delivery and the canonical
engineering policy in [QUALITY_BAR.md](../QUALITY_BAR.md); no fixed PR count or dormant
framework is selected. Profile details and current amendment acceptance/readiness are recorded in
[ENGINEERING_STATE.md](ENGINEERING_STATE.md), not inferred from this scope confirmation.

### 1.2 Autonomous campaign delivery order (owner direction, 2026-10-19)

The operator supplies Mission, admitted seeds, scope/disclosure/ROE, exercise mode,
window and finite budgets once. Routine acquisition, owner intake, reasoning,
authorization, execution, feedback and history proceed without intermediate JSON
editing, copied IDs, per-record approval or `approve`/`continue` input. Operator
stop/withdrawal and scope changes remain explicit. DNS is the first admitted action
used to demonstrate this campaign loop, not a replacement product or fixed strategy.

| Outcome | Completion evidence / dependency |
| --- | --- |
| Resolve autonomous-run runtime contracts | Publish the bounded [run amendment](contracts/M1-enabling-architecture.md#7-proposed-autonomous-run-amendment-2026-10-19); section 7 and ADR-016 section 4a were accepted by the owner on 2026-10-19 at exact text `0615fb72c946fd6abf0823692be621096091377c`, for Walking Skeleton implementation. Preserve existing sections and seals; runtime/deployment remains unqualified. |
| Walking skeleton (slice 1): prove autonomous orchestration in a controlled fixture | First runtime slice: `autonomous-dns-fixture-v1`, production `m1-run`/owner ports and restricted PostgreSQL, deterministic inference and Broker effect fixtures, distinct source/mode and no external egress. Mission → proposal → authorized fixture effect → Terrain → second proposal → durable history/inspect; no intermediate input. Qualify idempotency, stop, UNKNOWN and database-only recovery here. No real LLM/live DNS or M1 seal claim. |
| Real inference (slice 2): prove cognitive feedback with the real provider | After qualifying the new inference boundary, `autonomous-dns-provider-fixture-v1` uses actual LLM calls with the same deterministic Broker effect/source and fresh provider consent. No target egress; not blocked on Go capture/nft qualification. Require the second admitted proposal to consume the fixture result; a scripted replacement does not qualify real cognition. |
| Qualify live boundaries (may proceed independently of fixture wiring) | New inference timing/cancellation and ADR-015 Linux core/Broker/capture/exporter/host STOP, ADR-016 admission/accounting, ADR-017 real producer/consumer IPC. A running Docker/WSL installation alone is insufficient. Infrastructure work supports the loop; it is not a prerequisite for the isolated fixture above. |
| Real Broker egress (slice 3): first operational autonomous live run | Mission → real LLM proposal → authorized DNS → owner-qualified Terrain → real second LLM decision → durable history/inspect, with no operator input between start and terminal receipt. Requires accepted profiles, qualified live boundaries and fresh explicit client/inference authorization; fixture or old consent cannot satisfy them. |
| Validation and adaptation | Admitted HTTPS validation in the same loop; new evidence materially changes or supports the next hypothesis/action with visible rationale. No forced DNS/HEAD/subdomain playbook. |
| Complete cognitive M1 | CT, bounded synthesis/evolution, applicable campaign-local feedback and minimum report under the M1 contract. Two LLM calls or DNS/HEAD alone do not seal M1. |
| After M1 | Bounded access/authorized credential validation through its owning accepted contracts and current permission. |

Manual observation input remains optional with its original source label. Freeze
one-use demo helper development; historical episodes and failures remain evidence,
never reusable consent. No tracking-document family, fixed PR count, daemon,
message bus, Tokio conversion or multi-crate rewrite is required.

**Owner-directed method: Walking Skeleton / Tracer Bullet First.** Target one
cohesive PR for slice 1 through the real application supervisor, restricted SQL
state transitions, Broker admission, Terrain reconciliation and durable history.
Only external inference/effect adapters are deterministic substitutes. Decision 2
must consume a newly produced pack linked to decision 1 and the fixture outcome;
printing two prewritten receipts without these owner transitions does not pass.
Tests cover duplicate/conflicting start, withdrawal-before-release, child loss,
commit/ACK uncertainty and fresh inspect/recover with zero effect replay. Keep
this executable regression while replacing adapters in slices 2 and 3. A fake
LLM proves feedback plumbing, not cognitive quality. Live qualification remains
required; do not perfect every infrastructure component before proving the loop.

Use owner-local unit tests for private logic and integration tests through the
application ports. Preserve at least one real m1-run subprocess/entrypoint test;
no new public micro-command or public visibility solely for a terminal test.
Functional cohesion and reviewable failure semantics define a slice; 450 lines
may be justified without artificial splitting. McCabe remains enforced.

A spike answers a specific unknown with synthetic safe inputs and its own bounded
configuration. Before external provider use, obtain fresh disclosure permission;
this direction runs no provider or target experiment. Measure full P/C/B plus
repair, escaped UTF-8, model/settings, actual/reserved calls, token usage where
reported, phase latency, cancellation and durable accounting separately. Record
sample size and distribution limitations; a small sample does not establish P99,
and P99 does not bound every legal serialized input. Derive fit from tested input
bounds plus complete serialization, not a percentile alone. Numerical ceilings
may be resource/authority policy choices; label them as selected until calibrated.
A spike does not silently widen an accepted profile or convert test results into
production authority. Retain concise evidence in the existing PR/qualification
record, not another report/ADR/tracking family.

### 1.3 Audit disposition attached to these deliveries

The numerical debt below is the retained baseline recorded in ENGINEERING_STATE,
not a fresh complexity measurement or proof of a runtime defect.

| Owner / trigger | Concrete work and closure evidence |
| --- | --- |
| Application/domain boundary; first cross-context runtime wiring | Add a CI-executed dependency check for the actual protected owner modules and inward adapter direction. Baseline allowed contracts pass; an injected forbidden infrastructure/sibling-implementation dependency fails, including alias/re-export and qualified-path cases. No blanket baseline allowlist. |
| Terrain persistence; automatic issued-result intake | Repay `PgTerrainStore::record_dns` (24) and `load_existing` (17) cohesively to ≤7 per resulting function; include `read_current_claim` (10) if changed. Candidate-bound measurement plus real SQL tests must preserve source binding, atomicity, conflicts, role separation and commit-unknown. Renaming or forwarding helpers do not close debt. |
| Application/Broker/transport; run supervision and inference integration | Persist safe phase/outcome evidence for admission, generation, conditional repair, termination and publication separately from acquisition/Terrain. Inject deadline/cancel/child-exit/history faults. A may-send marker never proves a send; hard parent loss cannot promise a final receipt. Recovery makes zero provider/DNS calls. |
| Terrain/CLI/history; when the named function is changed | Retained debt: `inspect_dns` (9), `verify_mission_permission` (13), `TerrainReceipt::from_event` (10), `map_policy_reason` (10), `connect_qualified` (8), `run_record` (8), `command` (8), `EvaluationOutcome::as_str` (8), `record_terrain_history` (12), `inspect_terrain_history` (13), plus the ambiguous `m1_terrain_cli::command::<anonymous>` group. Preserve the ratchet; repay changed responsibility, leave untouched debt explicitly open. |

Large files trigger ownership/cohesion review; size alone is not a God Object.
Single-crate design, `#[path]`, alleged absence of threads/timeouts and alleged
absence of schema isolation do not justify a rewrite: the latter claims were
corrected by source inspection. Preserve existing thread/watchdog/I/O bounds and
SQL owner/role isolation; qualify their actual behavior on the changed path.
Do not turn all historical debt into a prerequisite for the first run.

Cleanup decisions from current source inspection (2026-10-19):

| Item | Disposition / retirement trigger |
| --- | --- |
| Local root artifacts/cache | Remove only the named untracked root nul/evidence_authcap.out and ignored Python caches. Root-specific ignores avoid hiding unrelated .out evidence; __pycache__ is already ignored. Never use blanket git clean. |
| Granular public CLI | Freeze test-only expansion. Deprecate individually after m1-run is stable and real callers/tests have replacements. Stop/withdrawal, inspect/recover and accepted M0 behavior remain operational contracts, not automatically obsolete scaffolding. Test support consumes production use cases; it must not duplicate their logic. |
| PostgreSQL adapters | Current root inventory has 20 postgres_*.rs files, not 18. Review duplication/cohesion when touched; consolidate proven duplication within the same owner. Do not compress Mission, Broker/authority and Trajectory responsibilities or qualified roles into three broad repositories. Preserve session-lock lifetime and transaction/fence semantics before considering pooling. No rename-only sweep before the skeleton. |
| M0 planning | Production authority_confirmation, PgMissionStore, PgTrajectory and withdrawal paths still depend on planning contracts. Preserve them now. Retire only after caller/history compatibility, withdrawal/refusal and fresh recovery have tested replacements and an explicit disposition of accepted M0 behavior. An M1 seal alone is insufficient. |
| SQL and McCabe | SQL string size does not directly create Rust control-flow complexity. Terrain already uses database acceptance/conflict functions. Repay record_dns/load_existing by cohesive orchestration/decoding responsibilities and measured behavior; do not move business authority into SQL merely to lower Rust McCabe. A new view/function needs a real ownership/atomicity reason, new migration and role/integrity/commit-unknown tests. |
| Accepted PRD/ADR and evidence | Preserve authoritative semantics, seals and referenced historical evidence. Simplify current navigation and genuinely superseded duplication; do not delete authority to reduce the document count. |

The minimal-loop approach follows [Anthropic's agent engineering guidance](https://www.anthropic.com/engineering/building-effective-agents):
explicit tool/environment feedback and stopping conditions, starting with simple
composition. [Agent evaluation guidance](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents)
supports isolated reproducible environments and grading actual outcomes. Apply
these as implementation/evaluation references, not product authority: assertions
must show that decision two consumed the qualified result, duplicate start/recovery
caused zero additional effects, and fixture provenance cannot satisfy live checks.
A green scripted fixture is necessary wiring evidence, not real LLM evaluation.

### 1.4 Delivery performance and debt closure (owner direction, 2026-10-19)

These are delivery obligations, not a new milestone or authority to start a later
runtime stage. Keep the Walking Skeleton -> real-provider fixture -> qualified
live sequence in section 1.2. Record closure in the existing PR and this roadmap;
do not create another tracker or count a retained warning as repaid debt.

| Order / owner | Bounded outcome | Closure evidence |
| --- | --- | --- |
| After PR #71 disposition; CI/test tooling | First optimize documentation-only delivery: a conservative changed-path classifier avoids full Rust/coverage only for explicitly qualified documentation inputs. Source, migrations, tests, manifests, build/security configuration, CI changes, unknown paths or classification failure require full verification. Keep required check identities and an explicit fail-closed ci-ok decision; do not silently convert missing/skipped execution into PASS. | Positive documentation cases and negative runtime/configuration/unknown-path cases; no dropped applicable check; compare exact-candidate cold/warm wall time with the baseline below. |
| Same tooling owner; measured follow-up | Expose sanitized phase/target progress without raw credentials or unrestricted child output. Cache compilation with separate normal/coverage identities bound to platform, toolchain, dependency/build inputs and instrumentation. Preserve fresh coverage profiles and actual current-candidate execution; cached binaries/results are not cached PASS. | Secret-bearing negative controls, failed-child exit propagation, unchanged assertions/test inventory and coverage floors; measured cache hit/miss and build-versus-test time. No routine cargo clean. |
| Test-fixture owner; only after isolation qualification | Parallelize proven independent tests with bounded concurrency. Keep shared PostgreSQL role/permission/migration fixtures serialized; separate databases do not isolate cluster roles. Do not merely remove --test-threads=1 or introduce a new runner/framework before measuring its need. | Same test inventory/outcomes and coverage; repeated isolated runs without cross-fixture interference, role leakage or hidden retries; measured end-to-end improvement. |
| Terrain owner; first automatic issued-result intake | Close record_dns=24 and load_existing=17 when the skeleton changes that intake; include read_current_claim=10 if changed. Apply section 1.3's real SQL, source-binding, conflict, role, atomicity and commit-unknown checks. | Every resulting function <=7 under the pinned ratchet; passing baseline characterization and entrypoint/consumer regression. No renamed debt, forwarding helpers or branches shifted into SQL solely to game metrics. |
| Existing owner modules; checkpoint after the real-provider fixture demo and before optional capability expansion | Reconcile every remaining named hotspot and the ambiguous closure group in section 1.3 into bounded owner-specific repayment/removal work. Keep untouched items explicitly OPEN until verified closure; essential inference/live qualification is not replaced by a general cleanup rewrite. | Candidate-bound function identity/complexity and consumer-path evidence for each closed item; unchanged function allowances are not closure. |
| CLI/application and persistence owners; after a tested replacement exists | Retire obsolete commands/helpers individually, migrate their real tests/callers to existing application contracts, and remove unreachable production code. Consolidate proven SQL/error-mapping duplication only within the same owner. Preserve production-used M0 assessment/history/withdrawal until accepted compatibility has a tested replacement and authorized disposition. | Before/after caller/consumer inventory, tested replacement, stop/inspect/recover and M0 compatibility evidence; no orphan exports or duplicate test-only business implementation. Do not delete accepted authority, historical evidence or migrations to lower counts. |

**LOC disposition:** keep QUALITY_BAR's advisory file/diff/span references and
the enforced McCabe ratchet unchanged. Re-inventory the touched large files
listed in ENGINEERING_STATE, including the ContextPack/Terrain adapters and large
Terrain test suites. Retain a large cohesive responsibility only with a concrete
review reason; consolidate duplication or split mixed responsibilities when
proven. Closure is simpler ownership, less duplication/dead code and preserved
behavior, not a required percentage reduction or smaller files at any cost.
Do not raise thresholds, remove explanations or fragment helpers to satisfy LOC.

**Current implementation disposition (2026-10-21):** the pinned Git baseline
records PR #72 (`ca7d4ef`), roadmap PR #73 (`00e5667`), Walking Skeleton PR #74
(`94c240b`), tracking PR #106 (`2b807d6`) and provider-fixture PR #107
(`2af7d01`). The skeleton's Terrain
`record_dns` 24 → 6 and `load_existing` 17 → 2 remain its reviewed disposition.
The delivered [provider-fixture packet](workflows/DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK.md)
implements slice 2 offline; no real cognition demonstration is
claimed. It shares HTTP/TLS/response mechanics and credential validation between
native/autonomous transport, retains the existing inference engine and separates
local revocation, authority coordination and transport permits. The touched CLI
`command` and its ambiguous closure group were remeasured after separating
parsing/receipts from execution and pre-mutation signaling; the packet records
`command` at 5 against its original PR-base 8. Untouched
`connect_qualified`, `run_record` and the other named hotspots remain OPEN.
Legacy CLI/M0 consumers remain supported.

**Measured CI baseline:** PR #71 run
[39276753676](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39276753676),
source head 0615fb7 (PR merge checkout 157c155), retained qualification artifact:
instrumented test build 1m23s; 101 test binaries report 869.94 seconds total and
549 passing tests; coverage 96.74%. Slowest target totals are
m1_inference_budget_limits 105.76s, m1_session_cli 67.09s and
m1_prepared_withdrawal_commit_ack 40.98s. These are one-run measurements, not a
latency percentile or promised speedup. The bootstrap captures child output until
completion, so a silent UI interval alone does not prove a hung test. Final
job/check status must be read separately; a completed log or earlier head is not
a substitute for current ci-ok.

## 2. Deferred development candidates

Deferred means outside initial scope; evaluate means a possible direction, not a commitment to build it. Revisit using evidence and the stated trigger.

| Candidate | Disposition | Why deferred | Trigger for reconsideration |
| :--- | :--- | :--- | :--- |
| Longer campaigns, dwell, richer pause/resume and re-entry | Deferred capability expansion | Initial client delivery is bounded in hours | A client mission requires longer continuity and validated short-campaign recovery is available |
| Broader dependency, SSO/federation and delegated-integration coverage | Deferred breadth | One family/use case can establish the initial relationship reasoning path | Pilot misses mission-relevant routes because the supported relationship family is insufficient |
| More admitted capabilities and adapters | Deferred breadth | Each action family adds execution, footprint, evidence and recovery obligations | A measured mission need cannot be satisfied by current admitted capabilities |
| General chain composition and deeper recursive expansion | Deferred breadth | A bounded mission-required transition can precede a general composer | Repeated authorized campaigns require longer chains and per-link validation/reconciliation is proven |
| Richer independent Observer/Grader and defender assessment | Deferred automation | Preserve source/mode separation now; automate assessment after the action/evidence path exists | Authorized telemetry coverage and repeated evaluation needs justify integration |
| OpenCTI/MISP hosting and multi-source intelligence curation | Evaluate later | Direct bounded references may serve the first hypothesis use case | Source volume, sharing or curation becomes an observed operational problem |
| Bounded graph acceleration | Evaluate after measurement | ADR-006 already permits owner queries as the initial path | Actual eligible traversal/rebuild workload demonstrates a bottleneck |
| Concurrent customers and multi-tenancy | Deferred deployment expansion | Single-tenant operation avoids premature tenancy complexity | Multiple independent customers require concurrent isolation, administration and capacity |
| New vulnerability research, memory-corruption exploit development or adaptive payload research | Evaluate separately | Research uncertainty is incompatible with a promised hours-bounded initial delivery | Explicit product decision, bounded research mandate, necessary safety/authority contracts and evidence of mission value |

The later product is not defined as building every candidate in this table. Priorities may change with pilot evidence; no delivery dates or performance guarantees are asserted.

## 3. Architectural boundaries and next action

Existing Terrain and Pathing contracts own environmental relationships and candidate routes. No sixth model, universal manager, new primary store or generic framework is implied by this record. Third-party ownership, software dependency or shared SSO never inherits client campaign authority.

Before moving a candidate into implementation: identify the real mission need and owning context, resolve material ambiguity, amend only conflicting accepted decisions, accept the necessary architecture/contracts, and issue one bounded entrypoint-to-consumer packet with failure-path verification.

The owner approved the selected ADR-008 review refinements and the limited relationship-aware MVP direction on 2026-09-30. The roadmap direction and selected refinements are accepted; [full ADR-008](adr/ADR-008-observation-fact-separation.md) was explicitly accepted by the owner on 2026-09-30. Engineering/adversarial skills and current status now reflect targeted research, source eligibility and stage-appropriate evidence. The [minimum M0 contract and sequencing proposal](contracts/M0-mission-authority-history.md) and its bounded sequencing exception were accepted by the owner on 2026-09-30. It defers ADR-009..012 for the limited local behavior while retaining required history. M0 needs no language amendment; future integration responsibilities still require component-specific assessment.

M0 is **SEALED (`DW-M0-001`)** at `669f2f36bf2909f3c28f4018a6ae69e76364b286` by explicit product-owner action on 2026-10-05 after owner acceptance of the Work-reviewed bounded demo. [PR #34](https://github.com/carlitotate12160-tech/DuskWeave/pull/34) merged the recording at `d421c776da2d030d344338181628d5852b3bbe81`. [Engineering state](ENGINEERING_STATE.md) sections 5a and 6 preserve the sealed scope, predecessor evidence and limits; older DEMO_PENDING wording is superseded.

The owner accepted the [M1 external orientation contract](contracts/M1-external-orientation-decision-loop.md) and sequential delivery flow on 2026-10-06: DuskWeave acquires sourced external observations, reconciles one decision-relevant DNS relationship, performs an admitted bounded HTTPS validation and revises the next campaign decision. Reuse one verified source worktree, qualified local resources and unchanged M0 evidence; preserve per-candidate verification. The next preparation resolves the common enabling design and explicit sequencing under contract section 11 before IMPLEMENT. Scope acceptance grants no target/pilot execution, automatic acceptance, M1 seal or change to DW-FOUNDATION-001.

On 2026-10-07 the owner approved the R2 cognitive direction for M1: evidence-led reasoning, bounded runtime capability synthesis/evolution, campaign-local feedback learning and the minimum report are inside the M1 outcome, replacing the deterministic-only selection. Retained for later selection — not hidden M1 criteria, mandatory providers or a fixed sequence — are additional passive sources (Wayback, OTX, VT, Shodan/Censys and further CT), scheduled episodes with change-diff, bounded GET/fingerprinting/takeover heuristics, and throughput optimizations such as parallel episodes or warm workers; each activates only for a concrete mission need under its owning contract. Online model-weight training and actor-persona playbooks remain unselected; self-granted permission, mutable audit history and self-certified success stay rejected. The accepted post-M1 access/credential pull is unchanged, and no new milestone promises or provider checklist are created.


## 4. Permanent offensive-direction tripwires

Owner-directed on 2026-10-03. These constrain downstream contracts and reviews;
they authorize no new capability, target activity, M1 acceptance or seal.

| Tripwire | Required slice/contract evidence |
| --- | --- |
| Adversary movement | State which authorized adversary movement or decision the slice enables. "More complete data" alone is insufficient. A foundation/control refactor names the campaign behavior it supports without pretending to create a target capability. |
| Terrain serves decisions | Bind sourced claims/relationships to a decision premise. Accumulating claims never used by a decision is a drift finding, not an inventory-success metric. |
| Access pull after M1 | Register bounded access/authorized credential validation as the next state-relevant pull. After M1, do not deliver two consecutive product increments whose sole outcome is read-only resolve/probe/cataloguing. Required FIX/contracts/prerequisites are not artificial capability increments. |
| INV-007 and separation | Blind reasoning excludes privileged defender/Observer/Grader oracle feeds. Legitimately obtained campaign-visible effects/telemetry retain accepted provenance, epistemic and sensitive-data limits. Observer/Grader stay deferred and separately owned. |
| Rhetorical canary | Campaign design, prioritization and acceptance optimize mission progress/position. Detection coverage may describe a separately labeled defensive consumer, but cannot steer blind campaign operations. |
| Observable progress | The demo shows evidence changing a hypothesis and selecting a next step that advances mission position/objective; tidy collection or successful tool invocation alone does not pass. |

State relevance does not require target writes, and pressure to advance never grants
scope, admission or secret-custody authority. Scope gates, append-only action/history
evidence, stop conditions and sensitive-data hygiene remain a non-negotiable floor
from the first applicable action. Controls are pulled by concrete demo requirements;
this direction does not authorize a general control or recovery framework.

The accepted [M1 contract](contracts/M1-external-orientation-decision-loop.md) section 6 assigns environmental Observation/claim reconciliation to Terrain, derived candidate hypotheses to Pathing, and alternative comparison to a bounded reasoning episode. The A/B/C roadmap describes sourced observations plus one decision-relevant relationship; admitted validation with attempt/outcome/unknown; and a connected demo. It is not a guarantee of exactly three PRs. Under the approved R2 direction the M1 reasoning structure is cognitive and evidence-led; the deterministic hypothesis comparator remains a test control or explicitly labeled non-cognitive mode rather than the accepted reasoning mechanism. Isolated qualification/failure tests do not replace an explicitly authorized real-client milestone demonstration. M1 does not itself establish access, a validated foothold, all four loops or objective completion.

| Post-M1 pull | Required admission |
| --- | --- |
| One bounded access or authorized credential validation capable of informing the next campaign position | Accepted owning contracts; specific operator scope; current deterministic admission; one capability/adapter; bounded effects and stop; attempt/outcome/unknown reconciliation; applicable PRD-010 isolated custody using opaque references; no brute-force or breadth commitment. |

The accepted M1 contract binds these tripwires to its particular decisions, observations,
attempts and demo assertions. Defensive consumers assess campaign results; they do
not become the campaign's operational direction.

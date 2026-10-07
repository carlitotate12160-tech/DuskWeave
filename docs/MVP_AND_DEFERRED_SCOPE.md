# DuskWeave MVP Direction and Deferred Scope

| Metadata | Value |
| :--- | :--- |
| Recorded | Initial direction 2026-09-30; offensive-direction guardrails 2026-10-03 |
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

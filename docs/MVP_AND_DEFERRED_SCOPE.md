# DuskWeave MVP Direction and Deferred Scope

| Metadata | Value |
| :--- | :--- |
| Recorded | 2026-09-30 |
| Review base | `d7297dad0f5269b3d95e2bd9dc1d28919c2ecd97` |
| Status | Owner-approved direction; downstream architecture and implementation remain subject to accepted authority |
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

Next work is the first executable M0A registration/history packet with a resolved exact file map. Complete M0A/B/C, demonstrate and explicitly seal bounded M0 before expanding M1 design beyond its recorded roadmap. Acceptance permits local M0 coding via issued packets, not target acquisition/execution. Future candidates do not authorize runtime, tool acquisition, exploit execution, third-party testing, automatic acceptance or a change to DW-FOUNDATION-001.

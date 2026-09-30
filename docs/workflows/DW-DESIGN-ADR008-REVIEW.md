# ADR-008 Historical Research and Review Memo

| Metadata | Value |
| :--- | :--- |
| Status | HISTORICAL RESEARCH ARCHIVE; full ADR-008 ACCEPTED by the owner on 2026-09-30; no additional acceptance gate |
| Review base | `d7297dad0f5269b3d95e2bd9dc1d28919c2ecd97` |
| Branch | `design/inv007-defender-knowledge-boundary` |
| Date | 2026-09-30 |
| Authority at review base | PRD-000..010 and ADR-001..007 ACCEPTED |
| Active seal | DW-FOUNDATION-001; baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0` |
| Scope | Targeted CTI ambiguity, observation/fact outline, and proposed MVP delivery workflow |

## 1. Scope and verified starting point

The repository remains document-only. The review base adds document-link CI to the prior seal-recording HEAD; it does not change accepted product or architecture authority. The pre-existing untracked `audit-kompleksitas.html` is outside this work.

The owner requests an hours-bounded MVP, Rust first, single tenant, real-client pilot evidence, preserved DuskWeave identity, and research only for material ambiguities. This memo researches the distinction between external knowledge, test artifacts, target observations, and accepted claims. It does not claim a comprehensive operator-practice study.

The following records the reviewed proposals and their research basis. The owner approved the selected refinements and MVP direction on 2026-09-30; the [full ADR-008](../adr/ADR-008-observation-fact-separation.md) was explicitly accepted by the owner on 2026-09-30. This memo preserves source facts and pre-decision reasoning; the accepted ADR is the decision authority and this archive requires no further approval. No foundation reseal, runtime, target acquisition, CTI connector, or capability execution is authorized by this memo.

## 2. Primary-source findings and architectural inference

Source statements and DuskWeave architectural inference are separate columns. These project documents establish tool/feed behavior; they do not establish that any particular action is suitable for a client campaign.

| Source | Documented practice or behavior | Evidence boundary | Architectural inference for DuskWeave |
| :--- | :--- | :--- | :--- |
| [CISA KEV official mirror](https://github.com/cisagov/kev-data) and [CISA exploitation notice](https://www.cisa.gov/news-events/alerts/2025/09/11/cisa-adds-one-known-exploited-vulnerability-catalog) | CISA adds vulnerabilities based on exploitation evidence; its mirror publishes JSON/CSV and tracks source changes. Updates follow catalog changes, with no universal daily-update guarantee. | Exploitation evidence concerns a vulnerability in the wider world. It establishes neither present exploitation nor applicability on this client target. | Use a source-qualified reference for hypothesis prioritization. Preserve source revision, source dates, retrieval time, and applicability limits. |
| [Nuclei code protocol](https://docs.projectdiscovery.io/templates/protocols/code) and [template signing](https://docs.projectdiscovery.io/templates/reference/template-signing) | Templates can execute external code on the runner; code execution is opt-in. Signing authenticates content/author provenance and detects alteration. | YAML and a valid signature do not prove harmlessness, low footprint, correctness, or client authorization. External payload files are not universally covered by the template digest. | Treat selected templates and their dependencies as capability candidates requiring review of their actual behavior. A feed update cannot silently change an executable capability. |
| [Atomic Red Team getting started](https://github.com/redcanaryco/atomic-red-team/wiki/Getting-Started) | Operators obtain environment-owner permission, select a test, check prerequisites, execute it, observe security telemetry, and perform cleanup; some tests change the environment. | A test library and cleanup commands are not a guarantee of safe execution, complete cleanup, or undetected behavior. | Admit individual bounded emulation capabilities only when needed by the mission. Keep defender evaluation separate from blind campaign reasoning. |
| [LOLBAS](https://lolbas-project.github.io/) | The project catalogs techniques involving legitimate binaries, scripts, and libraries, with ATT&CK references and multiple behavior categories. | Documentation is not an execution contract, guaranteed availability, target access, or permission; its scope is broader than lateral movement. | Use relevant technique knowledge for hypothesis work. Build only the concrete capability required by the pilot. |
| [OpenCTI connectors](https://docs.opencti.io/latest/development/connectors/) | Connectors import external knowledge and bridge sources into the platform. | Hosting a platform does not supply every intelligence source or grant source access. | Defer platform hosting until multiple-source curation/sharing becomes a demonstrated need; a bounded direct feed reader can serve the initial use case. |
| [MISP default feeds](https://www.misp-project.org/feeds/) | MISP supports public feeds and correlation; formats and conditions differ, and some listed feeds require a license. | Open-source platform availability is not a universal free-content or uniform-reliability guarantee. Overlap is not independent corroboration. | Select sources by the concrete use case; preserve upstream provenance and conditions rather than treating platform membership as trust. |

Public feed content remains untrusted data. Its embedded instructions, links, code, or proposed actions cannot gain policy, model-write, or dispatch authority.

## 3. Comparison with accepted authority

| Authority | Obligation retained by the proposed outline |
| :--- | :--- |
| [PRD-007](../prd/PRD-007-observation-model.md) | Raw capture stays outside ordinary core surfaces until eligible interpretation; admission is bounded; a receiving owner qualifies use. No global reconciliation barrier or generic three-tier Observation lifecycle. |
| [PRD-008](../prd/PRD-008-evidence.md) | Evidence is claim-specific. Correlated copies do not become independent sources; late receipt does not refresh target reality; correction and contradictory material remain attributable. |
| [PRD-009](../prd/PRD-009-client-proof.md) | A feed entry or template result cannot become verified client impact. Client claims retain basis, vantage, time, limitations, and separate disclosure authority. |
| [PRD-010](../prd/PRD-010-sensitive-data-handling.md) | Core receives non-sensitive semantics, opaque references, and safe metadata. Proof content remains attempt-ephemeral; operational secrets require separately authorized campaign-scoped custody. No general raw quarantine. |
| [ADR-003](../adr/ADR-003-domain-events.md) | Only the accountable owner publishes its accepted transition. Delivery, correction, replay, and deduplication grant no execution authority. |
| [ADR-005](../adr/ADR-005-postgres-system-of-record.md) | Permitted owner state/history and required propagation retain PostgreSQL durability and outbox/inbox obligations. This outline selects no additional store or schema. |
| [ADR-006](../adr/ADR-006-cyber-terrain-storage-model.md) | Terrain retains provenance, epistemic limits, stakes-based tiers, and time eligibility. Owner queries are a valid starting path; graph cache is optional. |
| [ADR-007](../adr/ADR-007-foothold-and-path-separation.md) | Candidate applicability belongs to sourced hypothesis/path work; only Access validates a position, and only Objectives accepts fulfillment. No template match creates a foothold. |

## 4. Contradictions and gaps

1. The proposed CTI list conflates intelligence, executable test artifacts, technique documentation, and hosting platforms. They require distinct treatment; no accepted PRD change is needed to preserve that distinction.
2. "Nuclei templates are non-destructive" and "Atomic tests are already safe" are unsupported blanket assertions contradicted by documented execution and environment-changing behavior.
3. KEV inclusion is not proof of client vulnerability. An unavailable feed or absent entry is not evidence of safety.
4. [ADR-004](../adr/ADR-004-rust-core-language.md) already permits Rust implementations of core persistence, messaging and external-client ports. An amendment is needed only where a component responsibility departs from the Go baseline for tool adapters, collectors, network/integration workers or telemetry adapters. Rust-first is not a blanket conflict and this outstanding component-specific decision does not block ADR-008.
5. At the review base, Engineering State and Build Order prescribed broad research and serial stages. Their current status now records targeted research and owner-approved MVP direction; a bounded sequencing amendment and minimum implementation contracts remain outstanding.
6. Concrete external-knowledge input/consumer contracts and capability/execution contracts are not yet accepted. ADR-008 must not invent or authorize those integrations.

## 5. Historical ADR-008 decision outline

**Decision:** admit only eligible, bounded, non-sensitive observations at the appropriate input boundary; let each receiving model owner accept or reject claims and qualify non-authoritative use through its own contract.

1. Raw capture, admitted Observation, inference, owner-accepted claim, evidence, client proof, and execution authority remain distinct. "Accepted fact" is not a new universal entity or common promotion state.
2. Admission preserves campaign/purpose, accountable source, exercise mode, vantage/origin, bounded assertion, relevant times and uncertainty, method limits, sensitivity disposition, and safe provenance where applicable. These are semantic obligations, not a universal payload schema.
3. External authorized observations do not require a prior foothold. Transient-origin orientation retains PRD-003/007 PROVISIONAL limits. Origin validation never automatically corroborates a claim.
4. Terrain, Access, Pathing, Objectives, and Trajectory retain their own acceptance burden. After owner qualification, eligible provisional input may inform the declared hypothesis/ranking purpose; it cannot establish a consequential premise or bypass current action authority.
5. Source reliability is assessed for the bounded claim and use; author identity, signatures, feed reputation, and duplicated reports do not replace applicability or corroboration. Authorized external knowledge, where separately admitted, remains a statement about its source until target evidence supports a target-specific claim.
6. Freshness is evaluated at use against effect/observation time and explicit limits. Receipt, processing, or source refresh cannot renew an older target observation. No shared timeout or mandatory periodic status writes are selected.
7. Conflicts and unknown outcomes remain explicit; corrections append linked accountable records. Late arrivals cannot restore superseded current claims. Unresolved dependencies block their own consequential use while independent eligible work continues.
8. Protected-edge denial proves only the interaction observed from its vantage and time. It establishes neither origin state nor bypass permission. A reasoner may propose another already-authorized lower-footprint action.
9. Observation admission, an owner claim, a CTI entry, or an execution result never dispatches a capability. Current deterministic authority and the Gateway/Broker/Adapter path remain necessary.
10. Keep sensitive handling and defender-source eligibility at the input boundary. Operational secrets are available only through separately authorized custody/execution; privileged assessment feeds remain excluded from blind reasoning.

**Alternatives rejected:** direct raw-output promotion; universal Observation Manager; mandatory global approval/reconciliation barrier; generic promotion tiers shared by all owners.

**Consequences:** fast local orientation remains possible, with explicit owner decisions and provenance. Costs are claim-specific contracts, current-premise checks, and visible unresolved dependencies.

**Deferred:** schemas, enums, state machines, queues, brokers, topology, identity algorithms, retention periods, sanitization mechanisms, CTI integrations, template admission, and runtime execution.

## 6. Proposed milestone workflow

For each milestone: define one observable outcome and its minimum accepted contracts, implement with meaningful tests, demonstrate the outcome, review the same commit and evidence, then explicitly seal the bounded milestone.

CODE/TEST -> DEMO -> REVIEW -> SEAL are delivery steps, not campaign runtime gates. Routine authorized target actions do not acquire per-step human approval from this workflow.

A milestone seal names an exact commit, its bounded guarantees, test/demo evidence, limitations, and remaining work. It does not claim a successful target compromise, complete assessment, global product readiness, or replacement of DW-FOUNDATION-001.

Real client observations/results establish pilot behavior. Isolated failure-path verification establishes expiry, stop, duplicate/recovery, and sensitivity behavior without injecting hazardous failures into the client environment.

Suggested outcomes:
- M0: Rust mission/authority and accountable history exercised through a real entrypoint and consumer, including denial/expiry/stop/recovery behavior.
- M1: one approved acquisition path produces sourced client observations that reach owner reconciliation and a report draft.
- M2: eligible real feedback changes hypothesis selection and the next bounded proposal.
- Subsequent bounded milestones: access validation, mission-required limited expansion, objective proof, and final client delivery.

These milestone names and boundaries remain proposals; they are not issued implementation packets.

## 7. Minimal CTI integration proposal

If the pilot has a CVE-applicability question, start with one real KEV source and relevant vendor advisory references. CTI is optional and mission-driven. Preserve source version/provenance and time; correlate only with eligible observations of the in-scope environment. A matching reference may support a qualified hypothesis or candidate check, not a verified vulnerability.

Connect that input to an actual bounded hypothesis consumer when its contract is accepted; avoid an importer consumed only by tests. Missing, stale, malformed, or conflicting input exposes its limits rather than supplying negative evidence or automatic network retry.

Treat CTI ingestion and executable capability admission as separate changes. Selected Nuclei/Atomic artifacts require pinned reviewed content and dependencies, declared behavior/footprint/sensitive effects, current mission authority, and the existing dispatch path. Do not dynamically execute repository updates or arbitrary LLM-selected source commands.

Defer OpenCTI/MISP hosting and a general feed framework. This proposal selects no production library, schema, refresh schedule, source failover, or storage topology.

## 8. Review and acceptance boundary

Document review checks ownership, source eligibility, protected-edge scope, provisional use, freshness, correction, sensitive custody, and absence of runtime choices. Future contract tests must demonstrate those guarantees; no runtime tests have been run.

Product-owner acceptance of the full ADR-008 is recorded on 2026-09-30. This memo is retained for source traceability and imposes no additional review, approval or runtime gate. Other accepted PRDs/ADRs and DW-FOUNDATION-001 remain unchanged. No capability, payload, CTI feed connector, target action, or new runtime language is admitted here.

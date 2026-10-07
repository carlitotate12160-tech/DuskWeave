# M1 — External Orientation and a Bounded Decision Loop

| Metadata | Value |
| --- | --- |
| Document | DW-M1-EXTERNAL-ORIENTATION-CONTRACT-20261006 |
| Status | ACCEPTED — product owner, 2026-10-06; product/behavior scope and linear delivery cadence |
| Amendment | R2 — cognitive direction approved by the product owner on 2026-10-07 (evidence-led reasoning, bounded capability synthesis/evolution, feedback learning, minimum report); published as a candidate amendment, exact-text acceptance recorded separately |
| Date | 2026-10-06 |
| Repository path | `docs/contracts/M1-external-orientation-decision-loop.md` |
| Verified authoring base | `d421c776da2d030d344338181628d5852b3bbe81` |
| M0 prerequisite | `DW-M0-001` explicitly SEALED on 2026-10-05 at `669f2f36bf2909f3c28f4018a6ae69e76364b286` |
| Governing authority | Accepted PRD-000..010, ADR-001..008; current build order, MVP direction and QUALITY_BAR |
| Acceptance basis | Owner accepted the final scope proposal and then the sequential delivery flow in this session; documentation recording explicitly requested |
| Readiness | Product/behavior contract ACCEPTED; enabling architecture and sequencing remain required before IMPLEMENT |
| Execution permission | This contract records the selected capability; it grants no current target/pilot permission or milestone seal |

## 1. Product outcome

M1 proves that DuskWeave can acquire eligible external information itself, reconcile a narrow environmental relationship, select and execute a permitted validation, and use its actual outcome to revise the next campaign decision.

R2 sharpens this outcome: M1 must contain an evidence-led cognitive loop in which the reasoner itself proposes hypotheses, compares alternatives, identifies missing information, chooses the next proposed action and adapts after results — deterministic authority validates but never secretly substitutes strategy. The M1 outcome also includes bounded runtime capability synthesis and evolution, campaign-local feedback learning, and the minimum report defined below. The deterministic reasoner may remain a test control or an explicitly labeled non-cognitive operating mode; it cannot satisfy the cognitive acceptance criterion.

The concrete capability is **selecting the next authorized external web entry candidate for later access work**. Its mission question is:

> Which already-authorized, mission-relevant HTTPS candidate is the best next subject for access-validation planning within the permitted contact envelope, and which routing or response premise still needs observation, a different candidate, or deferral?

The operator declares the mission purpose, eligible candidate class or exact assets, vantage, permissions, exclusions, time window, and finite budgets. DuskWeave performs the acquisition and comparison. Operator-imported observations may supplement this flow but cannot substitute for its own acquisition in M1 qualification.

Mission relevance is a declared premise or an explicitly sourced inference, not a fact established by a suggestive hostname. A responding service is an entry candidate, not validated access, an exploitable vulnerability, or objective fulfillment.

Comparison starts with declared mission priority, then the unresolved question an available action can answer, its remaining authority/evidence dependencies and its permitted cost. For equal-priority candidates, an informative permitted check is preferred over repeating a check that cannot resolve the question. A terminal response on the declared route and a redirect to an uncontactable dependency have different planning limits; neither proves which target is exploitable.

M1 is a development milestone implementing part of external orientation. Reconnaissance remains a recurring activity throughout later campaign operation; it is not completed forever by this milestone.

## 2. Included behavior and explicit limits

| Included in M1 | Bounded meaning |
| --- | --- |
| One passive source adapter | Existing Certificate Transparency records through crt.sh; candidate hints, with source lineage and age |
| DNS acquisition | Current bounded name resolution from a declared resolver/vantage; one routing relationship family |
| HTTPS validation | One fixed TLS/HTTP metadata profile for an admitted host, port and path; no exploit or authentication |
| Terrain reconciliation | Narrow sourced claims, applicable epistemic status/tier, contradictions, freshness and linked corrections |
| Candidate reasoning | Evidence-led cognitive comparison of eligible alternatives and no-action choices; state a question, expected/disconfirming evidence and next proposal |
| Capability synthesis | A recognized capability gap may be met by a candidate implementation: immutable source bundle plus manifest, trusted isolated build, bounded qualification and campaign admission before dispatch; more than template selection or parameter filling |
| Feedback | Reconsider candidates and the next step from the validation outcome; preserve unresolved premises; reflections stay revisable source-qualified hypotheses; capability revisions create new artifact digests |
| Minimum report | One machine-readable report and one human-readable engagement summary from the same pinned snapshot, with claim tiers, UNKNOWN/failed/deferred outcomes, omissions, contradictions and limitations |
| Durable accountability | Link mission, decisions, authorization disposition, attempts, admitted observations, evidence evaluation and outcomes |
| Operator inspection | A non-sensitive account of what changed, why, what remains unknown, and the permitted next step |

M1 excludes credential use, raw leak ingestion, authentication attempts, exploitation, origin hunting to bypass protection, foothold creation, pivoting, expansion, objective execution, arbitrary shell, dynamic plugin loading, universal graph search, Observer/Grader integration, and client-proof release. Inference is a replaceable adapter under deployment-selected model/placement; external inference requires permission for the specific safe fields and recipient, and generated workers are qualified artifacts under the bounded lane — not dynamic plugin loading.

These exclusions bound M1; they do not redefine DuskWeave as a passive scanner. The registered next product pull after M1 is a bounded access or authorized credential-validation capability under PRD-003/010 and its own accepted authority. It is not another indefinite series of inventory connectors.

## 3. Passive-source policy

| Source | M1 disposition | Reason / later trigger |
| --- | --- | --- |
| crt.sh / CT | Required first passive adapter | Tests source-derived candidate acquisition with a small integration surface; records do not establish a current service |
| Operator seed | Supported starting input | Allows a bounded mission to continue when the passive provider is unavailable; no invented permission or reality |
| Wayback | Deferred source profile | Add existing index metadata when a historical route/service question materially affects a decision; no Save Page Now |
| Shodan / Censys | Deferred source profiles | Useful for already-collected exposure information beyond CT names; add for a demonstrated mission gap, not parity |
| OTX / VirusTotal | Deferred source profiles | Add when lineage-preserving enrichment changes a candidate decision; existing records only, no submission/rescan |
| Leaked data | Raw material outside M1 | No passwords, hashes, tokens, dumps or personal records; an explicitly permitted non-sensitive exposure reference may inform a later proposal |

An adapter is a DuskWeave integration responsibility, not a requirement to launch a separate third-party CLI. DNS and HTTPS may use a bounded Go collector rather than expose dnsx/httpx flags. Nmap, osquery, Amass, Subfinder and Nuclei are not required for this capability.

crt.sh discovery is deliberately incomplete: CT does not cover every IP-only service, non-certificate asset, identity, organization relationship or historical application route. M1 must describe its observed slice, not claim complete attack-surface discovery.

Provider calls are outbound actions and disclosures. The approved profile binds the provider endpoint, allowed query material, response limits and purpose. Public availability is not authorization to send confidential target identifiers to a provider.

A provider error, timeout, rate limit, oversized response or missing record produces a bounded partial/unknown outcome. It does not mean that no target exists. An eligible seed may still be examined; with no eligible candidate, the episode defers. No fallback secretly expands providers, query scope or target contacts.

Different services reporting the same CT record are one underlying source lineage, not independent corroboration. Fetching a record again does not renew the real-world condition it describes.

## 4. Discovery scope and contact authority

Passive discovery scope and permission for target contact are separate inputs. A newly discovered name becomes a hint only. Active acquisition requires a match to an already-authorized exact asset or explicitly approved host class, including exclusions, current purpose, port/path limits and vantage.

There is no mandatory human approval for every observation. A name within a pre-approved class may qualify automatically through the deterministic authority boundary. A name outside that class may be retained only as an admitted safe hint; it cannot be contacted or silently added to scope.

A DNS CNAME, shared certificate, redirect, shared IP or provider relationship does not extend permission. Domain affiliation, ownership and authority are distinct claims. M1 does not infer a trust boundary or identify an origin server from delivery metadata.

The selected host-to-address contact policy is an authorized hostname contact bound to a currently admitted DNS result, a pinned destination address, the original hostname/SNI, and the authorized port/path. Its enforcement mechanism is an enabling-architecture decision before implementation. It grants no permission to probe that address or CNAME destination as an independent asset. Private, local, link-local, reserved or otherwise excluded destinations are denied unless the particular authorized lab/engagement explicitly permits them.

Resolution is bounded, CNAME loops/depth overflow are rejected, and the connection must use the admitted address rather than perform an uncontrolled second resolution. A changed required binding invalidates the dependent proposal and requires reconsideration. A DNS result alone cannot establish asset ownership or widen authority.

Redirects are observed but not followed in M1. An outside-authority redirect may explain why the candidate remains unresolved; it does not authorize visiting the destination. A possible supporting-domain visit is a later, separately declared permission, not an implicit feature of the HTTPS adapter.

M0 eligibility/planning history remains historical. It is not a reusable execution grant. Dispatch and actual effect start require current M1 authority, required premises, unexpired budgets and no withdrawal/freeze/termination.

## 5. The connected workflow

1. **Begin an episode.** Load the current mission and execution envelope. Bind the source profile and explicit finite limits. Compare available eligible premises with the mission question.
2. **Acquire candidate hints.** DuskWeave queries the permitted CT source when useful, or uses an admitted seed. Deduplicate identities while preserving lineage, time and truncation. Scope qualification decides which hints may proceed.
3. **Orient with DNS.** Acquire a bounded current resolution and admit its safe semantics. Terrain reconciles the host-to-routing-destination relationship against its accepted history and contrary material.
4. **Form alternatives.** Pathing supplies derived candidate views. A bounded cognitive reasoning episode compares candidates and the choices to observe more, validate, retain, defer or stop. It names the question the selected validation can answer and evidence that could disconfirm the premise.
5. **Admit one proposal.** Deterministic Action Authority checks current permission and required inputs. Gateway/Broker dispatches only the fixed profile. The reasoner cannot bypass this path or mutate Terrain.
6. **Perform bounded HTTPS validation.** The worker makes the admitted request and returns a bounded result through the sensitive-data/admission boundary. Attempt identity and outcome remain explicit even if the result is incomplete.
7. **Reconcile and reconsider.** Terrain evaluates the new evidence; Pathing invalidates or updates dependent candidates; the next reasoning episode compares the remaining alternatives. Trajectory records what was known and decided at each step.
8. **Explain the next step.** The operator receives safe observations, qualified claims, limitations, candidate disposition and rationale. A recommendation for later access work is not authority to execute it.

This is not a mandatory fixed sequence for every input. Current eligible premises can avoid redundant acquisition; an unavailable provider can be bypassed with an eligible seed. An identical result may justify retaining the same choice. M1 qualification nevertheless requires at least one material revision caused by new evidence.

## 6. Owners and minimum model surface

| Responsibility | Owner / boundary | Mutation limit |
| --- | --- | --- |
| Mission purpose and constraints | Existing Mission contract/application | Does not accept target reality or issue execution authority by itself |
| Raw capture and permitted interpretation | Isolated adapter/handling boundary | No raw output to ordinary IPC, core, persistence, logs or reasoning |
| Admitted external Observation records | Terrain-owned family for this slice | Observation identity remains distinct from claim identity |
| Environmental claims and DNS relationship | CyberTerrain | Only Terrain accepts its claim/status/tier and corrections |
| Claim-relative evidence evaluation | Terrain for its declared claims | Bounded EvidenceEnvelope semantics under PRD-008; no global evidence owner |
| Candidate hypotheses and invalidation | AttackPathView / Pathing projection | Derived, rebuildable candidates; no FootholdGraph node |
| Alternative comparison and proposal | Bounded strategic reasoning episode | Uses narrow eligible views; no owner-state mutation or dispatch |
| Current action permission | Deterministic Action Authority | Does not promote observations into facts or choose sibling state |
| Dispatch and worker lifecycle | Rust Gateway/Broker; Go adapter/collector | Fixed admitted request only; no arbitrary execution escape hatch |
| Decisions, attempts and outcomes | CampaignTrajectory | Accountable history; not a mutable shared campaign context |

Under R2 the reasoner is cognitive and evidence-led: it proposes hypotheses, compares alternatives, expected/disconfirming evidence and no-action outcomes, identifies missing information, chooses the next proposed action and revises after results. Deterministic authority validates proposals but must not secretly choose all actions through a fixed ranking with rationale written afterward; a hard-coded tool chain followed by generated narration does not satisfy this contract. The earlier deterministic selector may remain a test control or an explicitly labeled non-cognitive operating mode; it cannot satisfy the cognitive acceptance criterion. Inference runs through a replaceable adapter under deployment-selected exact model/configuration and approved placement; external inference requires permission for the specific safe fields and recipient, with retention/use terms compatible with the engagement. Otherwise an eligible local deployment or deferral applies, and model switching is not an undeclared disclosure fallback.

The single relationship family is a **DNS routing binding**: the queried host, its reported resolution lineage and bounded destination binding, from a declared resolver/vantage at a stated time. It is decision-relevant because a stale, contradictory or prohibited binding changes whether and where the proposed host contact is eligible. Shared routing does not establish identical applications, shared backends or target ownership.

No full implementation of all seven Terrain layers or all five models is required. No sixth observation model, universal orchestrator, global confidence score, all-owner barrier, database graph extension or generic capability registry is introduced.

Rust retains core use cases, owner acceptance, authority and broker responsibilities; Go owns the collector/adaptation work under ADR-004. PostgreSQL remains the system of record under ADR-005. Neither language selection nor a serialized "authorized" tag establishes trust.

## 7. Observation, evidence and claim limits

Every applicable record makes campaign/engagement, purpose, source class, exercise mode, external vantage, narrow assertion, method, times, completeness and originating attempt available. Safe provenance also preserves source/derivation relationships and limitations. These are semantics, not a universal wire object.

| Material | Claim it may support after owner qualification | Claim it cannot establish |
| --- | --- | --- |
| CT record | A source reported a certificate-associated name, with the available record times | Current hosting, asset ownership, reachable service, credential exposure |
| DNS answer | This resolver/vantage returned this bounded binding at this time | Universal reachability, same backend, origin identity, contact permission |
| TLS result | The admitted handshake and configured certificate validation produced this result | Application ownership, authorization, exploitation, a foothold |
| HTTP response | The admitted endpoint returned a specific status or permitted redirect disposition from this vantage/time | Installed version, bypass, valid credentials, access, objective success |
| Timeout / partial result | This attempt has no sufficient observed result within its declared bounds | Host down, no vulnerability, no defensive detection, no effect |

A narrow Tier 1 claim may become OBSERVED from one sufficiently direct source after Terrain reconciliation. PROVISIONAL is reserved for an unvalidated originating position; external pre-access acquisition is not automatically PROVISIONAL. Stronger service/trust/scope/Key Terrain claims retain their PRD-002 burdens and cannot be smuggled into M1 routing metadata.

Observation admission alone does not alter ranking. The declared receiving owner must first qualify it for the intended use. EvidenceEnvelope semantics bind the declared claim to supporting, refuting or inconclusive eligible material, limitations and safe inspectable basis; an identifier or digest alone is not technical evidence.

Freshness is checked at use from perception/effect time and the accepted profile's validity rules. DNS TTL can bound a routing premise but cannot certify truth; a configured maximum window and current contradictions can shorten eligibility. Receipt, admission and processing cannot refresh target reality. Corrections are linked new records, not rewrites; expiry does not prove absence or refute the earlier report.

Blind reasoning receives only eligible campaign-visible sources. Privileged defender alerts, SOC verdicts, Observer/Grader outputs and their summaries cannot steer it. A visible rejection or disconnect may inform a bounded hypothesis; attribution to a named defensive control remains a separate burden. Observer silence is not evidence of stealth.

## 8. Fixed HTTPS profile and sensitive handling

The selected M1 profile is one unauthenticated HTTPS `HEAD` request to an explicitly permitted path, with normal TLS validation, the original admitted hostname/SNI, redirects disabled and no browser/subresource execution. It has no automatic GET fallback, path enumeration, port sweep, payload templates, cookie reuse or credential input. If HEAD is unsupported, the result remains limited; a different method requires an admitted profile change.

Persistable output is a purpose-bound projection: bounded host identity where its disclosure is permitted, DNS binding, times/vantage, transport/TLS outcome enums, HTTP status and safe redirect disposition. No unrestricted headers, certificate subjects/SAN lists, response bodies, full URLs/query strings, cookies, authentication challenges or tool stderr are retained.

Calling an output "metadata" does not make it safe. Unexpected content remains inside the isolated ephemeral handling boundary. Field selection, structural bounds and sensitive egress admission apply before any ordinary IPC, owner write, log, diagnostic, retry material or operator view. Hashing, redaction or a reference is not an automatic exemption under PRD-010.

The minimum accepted sensitive-boundary design must identify actual capture surfaces, crash/error behavior, permitted egress, disposal and contamination handling. It must not claim that process exit proves zero retention. If safe interpretation/egress fails, emit only a safe rejection/unknown disposition; preserve uncertainty and do not recollect automatically.

This profile has no secret custody. Later authorized credential validation must use PRD-010's isolated campaign-scoped custody and opaque core references; it cannot reuse this metadata lane for passwords or token material.

## 9. Bounds, withdrawal and recovery

Before a runtime packet or pilot, the fixed profile must bind explicit finite values for: episode lifetime; maximum candidate count; provider calls and response bytes; DNS questions, chain depth and replies; target connections and requests; response/header/IPC bytes; per-operation and total duration; concurrency; and each permitted retry. Missing, zero-as-unlimited or overflowed limits reject execution.

Count actual permitted outbound operations, including retries, secondary DNS questions and any separately admitted later request. Parser truncation, a tool's default retries, DNS address-family fallback or a redirect cannot silently escape the budget. Exceeding a bound produces a limited outcome, not an automatic scope or budget increase.

Withdrawal, freeze or termination blocks new proposals/effects without waiting for observation/history backlogs or database recovery. The accepted execution design must define the authority fence and effect-start point, worker revocation/cancellation and bounded in-flight disposition. Cancellation is not rollback, and already-started effects cannot be represented as never having occurred.

Accepted owner changes and their required publication/history use ADR-003/005 transaction/outbox/inbox semantics. Do not hold a database transaction or lock over provider or target I/O. Local publication retry may redeliver the same logical outcome; it must not rerun acquisition.

An attempt with a missing acknowledgment after a possible external effect is UNKNOWN until reconciled from permitted evidence. A fresh process must inspect durable identity/state rather than blindly repeat the network action. Any genuinely new acquisition needs a new currently admitted proposal and is not disguised as recovery.

Duplicate identical records have one logical effect. Different content under one identity is an integrity conflict; late records cannot overwrite a newer correction. Block only affected dependent use where the governing contract permits independent work to continue.

## 10. Observable M1 qualification

M1 must show a connected production entrypoint-to-consumer path, not just parser/unit tests or an imported report. Required observations and outcomes below are future implementation evidence; none were executed in this drafting session.

| Scenario | Required observable result |
| --- | --- |
| Eligible acquisition | DuskWeave obtains a candidate hint, a DNS binding and an admitted HTTPS result; owner-qualified material changes or supports a decision |
| Decision-relevant relationship | Removing, expiring, contradicting or prohibiting the routing premise changes the dependent contact proposal or candidate disposition |
| Material feedback | New permitted evidence causes a different next candidate/action, premise or defer decision; rationale predates dispatch |
| No-action path | No eligible candidate, unmet burden or exhausted authority/budget yields a safe stop/defer with no target dispatch |
| False permission | Outside-scope name/redirect, excluded address, changed binding, expired/revoked authority and forged worker input cannot create a contact |
| False certainty | CT age, copied lineage, banner/status, timeout and tool exit cannot create unsupported ownership, access or global absence |
| Sensitive/mode boundary | Synthetic sensitive sentinels and privileged defender input cannot reach ordinary IPC, database, logs, retries, summaries or blind reasoning |
| Failure integrity | Post-effect missing ack, commit/ack loss, duplicate/conflicting identity, restart, correction and consumer failure preserve state/history without blind replay |
| Capability gap synthesis | A non-sensitive held-out scenario contains a gap not solved by an installed implementation; the loop generates new executable logic, passes protected evaluation and bounded repair, is campaign-admitted, and the real reasoning loop consumes its result; renaming a tool, emitting unused code or logging a preselected answer fails this claim |
| Changed evidence | The chosen next step or an explicit uncertainty disposition responds appropriately to changed evidence rather than a preselected answer |
| Minimum report | One machine-readable report and one human-readable summary derive from the same pinned owner/history snapshot; unsupported narration is omitted or labeled as hypothesis, and a non-atomic snapshot is labeled partial |

The reference qualification scenario uses two explicitly authorized, equal-priority, mission-relevant synthetic HTTPS candidates. Candidate A has a current admitted routing binding but its bounded request reports a redirect outside the current contact envelope. DuskWeave preserves the narrow claim that A responded, does not follow the redirect, and leaves the destination-route premise unresolved. It compares a permitted check of B with repeating A and deferring for additional permission. Where B can answer a remaining mission question, DuskWeave selects and performs that admitted validation, then evaluates its outcome. Neither status nor redirect proves vulnerability or a named defense.

Changing the first result to a sufficient in-envelope response must yield a different disposition, including a legitimate retain choice. A timeout variant remains inconclusive and respects the finite budget. This demonstrates branching from evidence rather than a fixed A-then-B scanner script.

A 401/403 response, an unsupported HEAD method or an outside-envelope redirect is not a rule to abandon an asset permanently. It preserves the observed response and its exact limitation; selection follows the mission question and available permitted alternatives. Missing permission may justify deferral, not a false claim that the service is absent or the path impossible.

Deterministic tests use isolated DNS/HTTP/TLS/provider fixtures and real applicable PostgreSQL/CLI boundaries. They do not depend on the public Internet, sleeps or hazardous client fault injection. A controlled qualification fixture and a real external-provider/pilot observation are labeled separately; a fixture cannot establish provider availability or client behavior.

The milestone demonstration uses explicitly authorized real-client acquisition and outcomes under the MVP direction. It shows the connected decision loop and at least one material evidence-led revision within the actual permission envelope. Synthetic qualification proves controlled branches and failure behavior; it does not replace the real-client demonstration. A client demo need not reproduce every injected-fault scenario.

Reuse unchanged M0 review/demo evidence. Run the required checks for each actual candidate once; repeat or broaden only for changed boundaries, failed checks or unresolved findings. No new generic assurance framework, benchmark program or scanner gate is part of M1. Existing repository gates remain in force.

M1 completion requires the connected loop, source integration qualification, applicable negative/failure evidence, operator explanation, distinct Work review and explicit owner seal at a bound candidate. Merge, green CI or this contract alone does not seal M1 or the full domain.

## 11. Dependencies and the next documentation boundary

The current build order does not authorize M1 acquisition/execution: M0's exception is expressly non-acquisition/non-evidence/non-custody. ADR-009..024 are reserved/unwritten slots, and PRD-011..013 are not accepted product authority. This contract does not fill those slots by referencing them.

The remaining preparation is **one bounded DESIGN outcome, runtime diff = 0**, resolving the common enabling decisions below. This accepted contract is their product input, not evidence that they are already authored or accepted. Combine related documentation when its decisions are resolved; no separate approval layer or PR per adapter/document is required.

| Required decision | M1-specific scope / proposed disposition |
| --- | --- |
| Bounded sequencing | Explicit M1 lane for the selected Terrain/Pathing/reasoning/execution slice, before full intermediate-stage breadth; update the build-order authority, not a silent exception |
| Evidence continuity | Minimum ADR-009 coverage for accepted claim support, correction, stable identities, safe technical basis and transaction/publication behavior |
| Sensitive boundary | Minimum accepted ADR-011 behavior for this non-proof acquisition lane; the registry's dependency on ADR-010 must be explicitly resolved through an accepted sequencing amendment |
| Capability and execution | Fixed requests/results, profile/version binding, current-authority fence, Rust/Go IPC trust, worker isolation, budgets, revocation and unknown-effect reconciliation; satisfy or explicitly rebind applicable Stage 11/12 prerequisites |
| Owner/public contracts | Terrain Observation/evidence/relationship family, Pathing candidate ownership, bounded reasoner input/output, Trajectory history and narrow consumption rights |
| Deferred triggers | Client proof/fingerprint/key architecture before proof; secret custody before credential use; registry/native helpers/Observer only when their actual capability is selected. Under R2 the replaceable inference adapter is selected for bounded cognition; a different provider class, privileged feed or unbounded use still requires its owning decision |

Under the R2 amendment, the enabling architecture additionally resolves this contract's cognitive outcome: evidence-led reasoning, the bounded generated-implementation lane, feedback/reflection and the minimum report. The §11 substitution explicitly authorizes only the bounded candidate catalog responsibilities specified there — manifest, immutable artifact identity, proposed-to-admitted lifecycle, qualification, promotion and invalidation — instead of asserting full ADR-014 or PRD-011..013 completion. Full registry, polyglot admission and campaign-chain stages retain their real prerequisites.

The future bounded sequencing lane defers breadth and unwritten decisions only through owner-accepted authority changes. Acceptance of this product contract does not itself accept that sequencing amendment. It cannot weaken PRD-000..010 or accepted ADR-001..008, declare all reserved ADRs accepted, or imply a full stage/domain seal. Proof and key features remain unimplemented, rather than being fabricated to satisfy an irrelevant dependency.

Publication records this M1 scope acceptance, corrects stale M0 DEMO_PENDING wording, and retains the post-M1 access pull. Architecture/sequencing acceptance is recorded separately when actually granted. The confirmed M0 seal is not reopened.

Provider-specific additions under the unchanged accepted capability, scope, observation, sensitive and execution contracts need a bounded source profile and adapter documentation, not a new PRD/ADR by default. A new acquisition effect, trust boundary, privileged source, sensitive-data lane or owner mutation right requires the owning design decision to change. “Just an adapter” cannot conceal such a change.

After enabling authority is accepted, Work can issue a cohesive IMPLEMENT packet with a measured base/file map, real entrypoint/consumer and cumulative budget. Do not promise a fixed number of PRs or create horizontal scaffolding slices. The current 600-line runtime-diff hard cap, >400 cohesion review trigger, category caps and McCabe rules remain; leave headroom rather than filling the ceiling. Split behavior if the measured slice cannot fit safely.

Runtime packet issuance must pin its actual current base and choose numeric profile limits and the concrete isolation/wire mechanism. A client pilot additionally requires its actual targets/vantage and current execution permission. This product contract intentionally does not invent those engagement inputs.

### Linear delivery cadence

The selected source worktree is `D:/DuskWeave-ci-complexity`, reused for M1 with one active delivery branch/PR at a time. Before each new branch, reverify current master, actual worktree, tracked changes and predecessor merge; do not stack on an obsolete candidate or create a new folder per slice. Preserve unrelated local configuration and historical worktrees.

Work resolves architecture and issues one bounded, executable packet; the IDE implements its complete observable outcome. Use focused TDD iterations and an early cumulative-budget check, followed by one final required gate pass per unchanged candidate/platform/configuration. An all-targets coverage run already executes that suite. Work performs one distinct review; corrections receive delta/affected-scope review unless a changed boundary invalidates prior evidence. Required exact-head CI still applies.

Reuse qualified build cache and the owned PostgreSQL service under AGENTS.md; keep ordinary and instrumented artifacts separate, per-packet reports/coverage bound to their candidate, and runtime fixture databases/logins isolated. Reuse unchanged M0 evidence, not old PASS results for changed M1 behavior. Perform the connected milestone demo when the loop is integrated, then seek explicit bounded seal acceptance. These rules add no new manager, gate, workflow subsystem or per-slice workspace requirement.

## 12. Research rationale and source boundaries

Research checked official product documentation and primary intrusion reporting on 2026-10-06. Vendor descriptions establish advertised workflow, not comparative effectiveness, safety or competitive parity. These sources inform gaps; DuskWeave PRD/ADR remain the only project authority.

| Comparison | Relevant documented pattern | DuskWeave decision |
| --- | --- | --- |
| NodeZero [S1] | External discovery is followed by explicit asset authorization before an external pentest | Preserve discovery/contact separation without per-observation manual approval |
| Pentera [S2] | External entry validation connects to internal/cloud attack paths and remediation/retesting | M1 delivers the orientation/selection loop; access, attack-chain proof and remediation breadth remain later work |
| Hadrian [S3] | Atlas discovery/exposure context feeds Nova application testing, with visible reasoning/evidence | Retain decision-linked context and feedback, not a large inventory milestone |
| XBOW [S4] | Domain rules distinguish attackable, visit-only and blocked destinations | Supporting/third-party contact needs explicit permission; M1 records but does not follow redirects |

The joint Volt Typhoon advisory [S5] describes target organization/network/staff research and continued environment understanding. Its exposed-infrastructure search examples are attributed to industry reporting, not independently established by every advisory author. The design inference is goal-directed, revisitable recon; it does not mandate a particular provider, staff/PII collection or stealth engine.

Microsoft's Midnight Blizzard report [S6] describes entry through a legacy test identity and subsequent abuse of an application relationship. The design inference is that internet inventory alone cannot cover the later access/trust problem. M1 does not turn routing or a returned response into identity, access or a trust edge.

Mandiant's UNC5537 report [S7] describes a financially motivated credential-driven campaign, not a nation-state APT. It reinforces the later credential-validation pull and the need for its own custody contract; it does not authorize M1 leak collection. CT documentation [S8] supports using certificate records as historical source material rather than current service proof.

The sources do not establish that these actors use crt.sh, Wayback, OTX or VT in the particular described campaigns. No such claim is made. No exhaustive APT study is required to begin this bounded capability. Further research becomes necessary when a selected access hypothesis, protocol behavior or source limitation is not resolved by accepted authority and observed evidence.

### Primary sources

- [S1 — NodeZero external test documentation](https://docs.horizon3.ai/portal/test_types/external/).
- [S2 — Pentera platform](https://pentera.io/pentera-platform/). “Pantera” in the discussion is interpreted as Pentera.
- [S3 — Hadrian Atlas + Nova](https://hadrian.io/products/hadrian-platform).
- [S4 — XBOW scope configuration](https://docs.xbow.com/console/reference/scope-configuration/).
- [S5 — Joint advisory AA24-038A, February 2024, hosted by ASD/ACSC](https://www.cyber.gov.au/sites/default/files/2024-02/aa24-038a-jcsa-prc-state-sponsored-actors-compromise-us-critical-infrastructure.pdf), especially pages 6 and 9.
- [S6 — Microsoft Midnight Blizzard report, 2024-01-25](https://www.microsoft.com/en-us/security/blog/2024/01/25/midnight-blizzard-guidance-for-responders-on-nation-state-attack/).
- [S7 — Mandiant UNC5537 report, 2024-06-10](https://cloud.google.com/blog/topics/threat-intelligence/unc5537-snowflake-data-theft-extortion).
- [S8 — Certificate Transparency mechanics](https://certificate.transparency.dev/howctworks/).

### Pinned DuskWeave authority

Repository base: [DuskWeave at d421c776](https://github.com/carlitotate12160-tech/DuskWeave/tree/d421c776da2d030d344338181628d5852b3bbe81).

Relevant repository-relative paths at that base: `AGENTS.md`; `docs/ENGINEERING_STATE.md`; `docs/BUILD_ORDER.md`; `docs/build-order/00-authority-and-invariants.md`; `docs/build-order/02-reality-and-domain.md`; `docs/build-order/03-capability-and-execution.md`; `docs/MVP_AND_DEFERRED_SCOPE.md` §4; `QUALITY_BAR.md` §7; `docs/adr/README.md`; `docs/prd/PRD-000-product-thesis.md`, `PRD-002-cyber-terrain.md`, `PRD-003-access-and-footholds.md`, `PRD-006-adaptation.md`, `PRD-007-observation-model.md`, `PRD-008-evidence.md`, `PRD-010-sensitive-data-handling.md`; accepted ADR-002..008; and the accepted M0 contract. Older supplied copies were reused only for unchanged semantics; live status/build order/MVP/review policy were verified.

## 13. Document review disposition

The drafting and distinct document-review workflows used `duskweave-engineering` and `duskweave-adversarial-review`, applying canonical QUALITY_BAR §7. This is a distinct review pass by the same assistant, not a separately staffed independent review.

| Review scope | Disposition / concrete basis |
| --- | --- |
| Q1 / Q2 / Q4 ownership and dependencies | PASS for the contract specification: §6 separates model mutation, proposal and authority; §11 exposes rather than assumes missing dependencies |
| Q3 simplicity | PASS for scope: one source, one relationship family and one fixed HTTPS profile; no generic registry, graph engine, global observation gate or per-source ADR requirement |
| Q5 requirement fit | PASS for product specification: §1/5/10 require DuskWeave acquisition, decision feedback and real-client milestone evidence; §2/7 retain access, evidence, blind-mode and sensitive boundaries |
| Q6 reachability | N/A execution for this document-only artifact; §5/10 require the future actual entrypoint-to-consumer path, whose implementation remains UNVERIFIED |
| Q7 failure integrity | PASS for specified semantics in §9/10; fault execution is N/A for document-only work, and future enforcement remains UNVERIFIED |
| Q8 trust and claims | PASS for document limits; runtime enforcement UNVERIFIED: §4/7/8 prevent permission/fact/secret escalation in the specification |

Material counterexamples closed in the document: inventory without mission decisions; CT/redirect/CNAME granting contact authority; a redirect or rejection causing unjustified permanent abandonment; public metadata treated as safe by label; a fixed script presented as reasoning; a missing ack replaying contact; current-permission claims borrowed from M0 history; and reserved ADR slots treated as accepted implementation dependencies.

No remaining blocking product-scope contradiction was identified in the document pass. The product owner accepted the scope and linear cadence on 2026-10-06 and requested repository recording. The explicit enabling decisions, runtime verification and pilot inputs remain real readiness dependencies; IMPLEMENT readiness cannot be inferred from scope acceptance. This document delivery changes no runtime and performs no target acquisition, full suite, merge or seal. Exact delivery/check provenance belongs to the external delivery report, not a self-referential candidate SHA in this contract.

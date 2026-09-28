# PRD-008: Evidence Model

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-008 |
| **Title** | Evidence Model |
| **Status** | PROPOSED |
| **Stage** | Stage 4 — Reality & Evidence |
| **Direct Dependency** | [PRD-007 Observation Model](PRD-007-observation-model.md) — ACCEPTED |
| **Target Seal** | DW-DOMAIN-001 |

## 1. Purpose and product outcome

DuskWeave needs a consistent product meaning for material used to support or refute a bounded claim. An Observation reports what was perceived; Evidence records why particular eligible material matters to a declared claim, with its provenance, limitations, time, and evaluation visible.

Evidence does not become truth merely because it is collected, grouped, repeated, or delivered. The accountable operational-model owner decides whether the evaluated evidence justifies a transition in its claim or current state. The same material may be relevant to different claims, but its sufficiency and effect are evaluated separately for each claim.

An **EvidenceEnvelope** is the bounded semantic association between a declared claim and the eligible non-sensitive material assessed for or against it. It preserves support, contradiction, uncertainty, source relationships, and limitations. It is not a universal event envelope, raw artifact bucket, execution token, proof deliverable, sixth operational model, or global evidence manager.

## 2. Vocabulary and boundaries

| Term | Product meaning | What it does not establish |
| :--- | :--- | :--- |
| **Observation** | An admitted sourced report of a perceived condition, response, or effect under PRD-007. | Fact, sufficiency, access, objective success, or authorization. |
| **Evidence material** | An eligible Observation, admitted technical evidence material, accepted owner claim, or approved non-sensitive derivation considered for a declared claim. | Relevance or sufficiency until evaluated for that claim. |
| **Admitted technical evidence material** | Where applicable, a non-sensitive, synthetic, or safely transformed technical artifact—or bounded safe reference to it—produced by an authorized action and admitted for claim-specific evaluation. | Raw capture, unrestricted target output, a target-side managed campaign artifact, truth, or sufficient evidence by itself. |
| **Evidence evaluation** | A claim-specific assessment of how eligible material supports, refutes, conflicts with, or remains inconclusive for that claim. | Authority to change another owner's state or dispatch an action. |
| **EvidenceEnvelope** | A bounded, accountable record or reference set binding a declared claim to evaluated material, provenance, time, mode, limitations, and disposition. | A claim that every enclosed item is independent, current, sufficient, or true. |
| **Accepted model claim** | A claim accepted by its operational-model owner at an explicit status after the owner's reconciliation. | Universal truth or indefinite current validity. |
| **Client proof** | A scoped client-facing substantiation derived under PRD-009. | The complete evidence history, raw client content, or owner state. |

An EvidenceEnvelope identity, an Observation identity, a domain-event identity, and an owner-claim identity are distinct. Referencing one from another does not merge their ownership or semantics.

Evidence is contextual. Material that supports “a response was returned from vantage V at time T” may remain inconclusive for “the service is globally reachable,” “the installed version is X,” “access is stable,” or “the objective is satisfied.”

## 3. Ownership and accountability

CyberTerrain owns environmental claims. FootholdGraph owns validated position and health claims. AttackPathView owns candidate and derived transition views. ObjectiveState owns objective progress and satisfaction claims. CampaignTrajectory owns the chronological account of decisions, attempts, and outcomes. Evidence handling cannot accept, rewrite, or impersonate any of those owners.

- An **evidence producer** supplies eligible material and its permitted provenance; production does not grant truth authority.
- An **evidence admission boundary** confirms campaign, scope, source, exercise mode, sensitivity, and structural eligibility before material is evaluated.
- An **evidence evaluator** associates eligible material with a declared claim and records support, refutation, conflict, uncertainty, and limitations under the applicable burden.
- A **claim owner** decides whether the evaluation justifies a transition within its own model.
- A **bounded reasoner** may use only owner-qualified evidence or owner views eligible for its purpose and stakes; it cannot accept claims or dispatch execution.
- An **operator or reviewer** may inspect permitted provenance, basis, conflicts, and gaps without receiving prohibited raw sensitive material.
- The **Observer/Grader plane** remains a separately authorized assessment source and cannot supply privileged evidence to blind campaign reasoning.

A reviewer may record a disposition that accepts, challenges, rejects an evaluation for a declared consumer purpose, or requests clarification. That disposition may hold only dependent use governed by the applicable review policy. It does not mutate the evaluation or owner state, grant execution authority, or create a mandatory human gate or campaign-wide barrier; the claim owner remains accountable for reconciliation and state transition. Client-delivery sign-off remains PRD-009 scope.

A component may combine admission, evaluation, or custody responsibilities when boundaries remain enforceable. Shared deployment does not create shared ownership, and an evidence repository cannot become the system of truth for all operational models.

## 4. Required EvidenceEnvelope semantics

Every EvidenceEnvelope must make the following product meaning available where applicable:

1. **Engagement and campaign context**: the isolated context and authorized purpose for which the evidence may be evaluated.
2. **Declared claim**: the narrow proposition under assessment, its accountable owner, affected subject, and intended decision use.
3. **Exercise mode and source class**: blind campaign, separately authorized defender-informed exercise, or assessment context, with campaign-visible, operator-supplied, or privileged source distinguished.
4. **Material references**: eligible Observation, admitted technical evidence material, accepted owner-claim, or approved derivation references without embedding prohibited raw content.
5. **Stable identity and bounded semantic lineage**: stable logical identity for each retained envelope or material item within its accountable producer and engagement/campaign context; provenance family; parent/derived and correction/supersession relationships; and accountable admission, transformation/redaction, evaluation, correction/supersession, or authorized export/disclosure transformation where it changes meaning, eligibility, representation, disclosure boundary, or correction status. This does not require logging every read, view, internal handoff, or disposal action.
6. **Vantage and origin**: the external vantage, validated position, transient unvalidated origin, or other bounded source context and its limitations.
7. **Relevant times**: effect time if known, perception time, availability/admission time, evaluation time, and uncertainty or validity window. Where correlation or ordering matters, include the source clock or time basis, precision, timezone or normalization, known skew or uncertainty, whether time is observed, derived, or assigned, and resulting ordering limitations.
8. **Provenance and derivation**: enough non-sensitive continuity to assess origin, transformation, source dependence, mode eligibility, and integrity.
9. **Evaluation disposition**: the bounded respect in which material supports, refutes, conflicts with, or remains inconclusive for the declared claim.
10. **Burden and limitations**: the applicable stakes or epistemic burden, unmet requirements, contrary material, coverage gaps, and what remains unproven.
11. **Correction relationship**: a link to any superseded, corrected, or disputed evaluation without rewriting accepted history.
12. **Sensitivity disposition**: confirmation that raw sensitive content was excluded or transformed inside an authorized ephemeral boundary.
13. **Consumer purpose**: the owner reconciliation, bounded reasoning use, review, history, or proof-derivation purpose for which the envelope is eligible.

These are semantic obligations, not a universal field list, database schema, serialized type, identifier format, or wire protocol. Later contracts define applicability and representation without weakening these meanings.

## 5. Formation and claim-specific evaluation

Eligible Evidence material originates from admitted Observations, admitted technical evidence material, accepted owner claims, or approved non-sensitive/opaque derivations. Raw capture, unrestricted target output, unadmitted interpretation, a proposal, a command, event delivery, tool success, or a model-generated assertion is not Evidence merely because it is available.

Where technical basis is applicable, an admitted non-sensitive artifact may be represented by a bounded safe reference rather than embedded inline. Claim context such as identity, vantage, and timing may be supplied through the EvidenceEnvelope and permitted provenance instead of duplicated in the artifact. A fingerprint or integrity reference may support identity and continuity, but it does not replace an inspectable technical basis when the governing owner contract requires one.

An accepted owner claim is eligible only within that owner's actual semantics, source/mode bounds, freshness, and provenance. A receiving owner independently evaluates its relevance and burden; it cannot treat another owner's acceptance or domain-event delivery as universal fact, inherit that owner's sufficiency, or bypass its own reconciliation.

Formation is bounded by a declared claim. The evaluator must preserve the material's original scope and must not broaden “observed from V at T” into a global, causal, identity, access, or objective claim. Derived material retains references to eligible premises and its stated transformation limits.

An envelope may contain supporting, refuting, conflicting, and inconclusive material together. Evidence evaluation does not coerce ambiguity into binary success/failure. Negative evidence requires an accepted coverage and observation-window basis; absence, silence, timeout, or unavailable input is otherwise unknown.

A single Observation may be sufficient for a narrow, low-stakes direct claim where the owning contract permits it. Repetition from one underlying source, duplicated delivery, relabeled output, or several views of the same acquisition does not create independent corroboration. Source independence is assessed from provenance and derivation, not item count.

## 6. Sufficiency, tier, and owner use

Sufficiency is claim-specific and proportional to consequences. No EvidenceEnvelope carries a universal confidence score or becomes sufficient for every consumer.

- **Narrow low-stakes use** may rely on one sufficiently direct, owner-qualified source within its exact vantage, time, and limits.
- **Moderate-stakes use** requires the owning contract's supporting evidence and contradiction checks before a consequential transition.
- **Campaign-critical or high-stakes use** follows the governing owner contract. Multiple genuinely independent sources and full reconciliation are required only when that contract requires them, including Tier 3 Terrain and campaign-critical safety or scope claims.
- **Transient-origin material** remains origin-unvalidated and can support only the bounded PROVISIONAL orientation permitted by PRD-003 and PRD-007.
- **Access validation** must satisfy PRD-003 evidence for reliable bounded execution, output retrieval, exact identity and privilege context, relevant environment boundary, and operational stability.
- **Objective fulfillment** must satisfy the predeclared condition and least-intrusive proof rules in PRD-005; generic execution success is insufficient.

A single sufficiently direct, owner-qualified material item may satisfy the declared burden where the governing owner contract permits it and all required claim facets, provenance, integrity, freshness, and contradiction checks are satisfied.

If later consequences make a claim higher-stakes, the evidence burden rises before dependent use. Tempo, deadline pressure, tool confidence, or operator convenience cannot lower the burden. Insufficient material may still inform bounded hypothesis ranking or a separately authorized proposal to observe more, but it cannot support the pending claim or consequential action.

Current authority, safety, engagement bounds, or an authorized operational decision may decline or prohibit additional collection. The claim owner records the unmet burden and constrains dependent use; it does not dispatch or independently authorize collection.

Once the accountable claim owner qualifies an EvidenceEnvelope for a declared use, eligible bounded reasoning may use it without a campaign-wide pause. Only the dependent claim, projection, or action waits for unresolved insufficiency, conflict, staleness, or missing evidence; independent work continues.

Evidence supply requests owner reconsideration. It cannot write owner state, create a foothold, mark an objective satisfied, restore lost access, expand scope, renew authority, or dispatch a capability.

## 7. Time, freshness, history, and correction

Evidence preserves effect/perception time separately from availability, admission, evaluation, and owner-acceptance time. Processing delay remains visible and cannot make old target reality fresh. Freshness is evaluated at use against the declared claim, source behavior, validity window, current position, and consequence.

An old EvidenceEnvelope may remain an accurate historical account of what supported a decision at that time while becoming ineligible for a current claim. Staleness does not refute the earlier observation, prove current absence, or erase the historical basis. Historical evidence cannot by itself restore current access or authority.

Current-use eligibility may be narrowed, suspended, or revoked before temporal expiry when eligible new material, an accepted owner-state change, a scope or authority change, or an applicable correction invalidates a required premise. Historical meaning remains, and the evidence is not automatically refuted. In blind mode, eligibility can change only from eligible campaign-visible inputs, not privileged defender, Observer, or Grader knowledge.

Current evidence evaluation and owner state may change through accepted transitions. Once retained as an accepted historical EvidenceEnvelope or evaluation, its semantic content is append-only. A correction, reinterpretation, refutation, or narrowed scope is a new accountable linked record; it does not overwrite the original or conceal decisions made from it.

This requirement does not mandate universal event sourcing, retention of every raw capture, or one shared evidence store. ADR-009 will choose the architecture that preserves accepted evidence and correction semantics.

Each retained envelope or material item keeps a stable logical identity within its accountable producer and engagement/campaign context. Redelivery of identical semantics has no second logical effect; different semantic content under the same identity is an integrity conflict. Parent, derived, and correction references preserve the bounded semantic lineage required by Section 4 without creating a universal custody manager.

Out-of-order material may complete history but cannot overwrite a newer accepted correction. Missing predecessors, conflicting corrections, or an identity reused with different semantic content block only affected dependencies pending verifiable owner reconciliation. First/last arrival, majority count, or producer assertion cannot resolve an integrity conflict.

## 8. Mode, isolation, and sensitive-data boundary

Evidence remains isolated by engagement, campaign, exercise mode, and authorized purpose. Similar targets or a shared deployment do not authorize cross-campaign reuse. Any future sharing requires an accepted design, destination-side admission, preserved provenance, and explicit authorization; those conditions are not granted here.

In blind mode, privileged defender-internal telemetry, analyst conclusions, Observer verdicts, and Grader results are ineligible evidence for campaign reasoning. Relabeling, summarizing, referencing, or replaying them cannot launder the source. Campaign-visible effects and telemetry legitimately encountered through an authorized current position may be eligible within their narrow scope after admission and owner evaluation.

A separately authorized defender-informed exercise keeps its source and mode visible. Its evidence cannot be represented as a blind campaign result or silently combined with blind evidence to satisfy corroboration.

Raw credentials, password hashes, private keys, authentication stores, personal records, customer data, financial records, unrestricted target output, and other prohibited client material cannot enter an EvidenceEnvelope, evidence history, retry/quarantine payload, log, trace, diagnostic, report, or reasoning context.

Where an accepted access or objective flow permits minimum client content, it remains inside the isolated ephemeral sensitive boundary only long enough to derive approved opaque material. The envelope may reference that approved derivation and permitted provenance; a reference is never permission to retrieve raw content. PRD-009 and PRD-010 define proof and sensitive-handling details.

If prohibited content is discovered after admission, ordinary access and current-use eligibility for the affected material are revoked immediately. Reasoning, further derivation, ordinary review, and export are blocked; the content must not be copied, echoed, or propagated. Only safe incident metadata and a linked contamination or correction record may remain in ordinary evidence history. Known downstream consumers are identified, and dependent claims, proofs, or disclosures are reassessed.

Append-only accountability protects the safe semantic history of what occurred; it does not protect prohibited bytes from an accepted sensitive-remediation path. The incident remains unresolved until the future accepted PRD-010/ADR-011 path satisfies its irretrievability and residual-risk criteria. This PRD does not select purge, cryptographic erasure, backup remediation, or disposal mechanisms, and a campaign capability cannot perform that remediation or alter audit evidence.

Campaign evidence handling, correction, rejection, or cleanup cannot alter, delete, disable, or falsify authoritative audit evidence.

## 9. Failure and stopping semantics

- **Ineligible material**: reject without promoting it to Evidence or echoing prohibited content; retain only permitted accountability metadata.
- **Incomplete provenance**: mark the affected evaluation insufficient or unresolved; do not invent source independence.
- **Duplicate material**: redelivery of the same stable logical identity and semantics has no second logical evidential effect.
- **Identity conflict**: different semantic content under one retained logical identity is an integrity conflict and blocks affected dependencies pending reconciliation.
- **Contradictory material**: preserve both sides and block or downgrade only dependent use until the claim owner reconciles them.
- **Stale material**: preserve historical meaning while requiring refresh or downgrade for current use.
- **Unavailable referenced material**: do not treat unavailability as refutation or silently omit a required basis.
- **Late contamination**: apply Section 8 immediately, block affected use and propagation, record only safe accountability metadata, reassess downstream dependencies, and keep the incident unresolved until an accepted sensitive-remediation path satisfies its criteria.
- **Unknown external outcome**: remain inconclusive and do not authorize automatic target-side retry.
- **Authority withdrawal, safety freeze, or termination**: stop new acquisition and dependent action as required by current authority; historical evidence remains governed by permitted retention and review. Enforcement cannot wait for an evidence backlog.

Evidence recording failure blocks a decision whose accepted contract requires that evidence. It does not freeze unrelated campaign work unless a separate safety or authority condition requires that result. Retrying admission, delivery, or evaluation is bounded and cannot repeat the underlying target action.

## 10. Illustrative synthetic cases

### 10.1 Narrow service claim

An admitted Observation records that a synthetic service returned banner text from external vantage V at T. An EvidenceEnvelope supports only the declared claim that this response was perceived from V at T. It remains inconclusive for installed software version, global reachability, ownership, or access.

### 10.2 Correlated duplicates

Three records originate from one synthetic acquisition: raw tool output is summarized, forwarded, and rendered in a dashboard. They are one provenance family, not three independent sources. A Tier 2 claim still lacks independent support.

### 10.3 Transient-origin material

A transient synthetic position reports local identity information. The envelope retains the unvalidated origin and may support PROVISIONAL orientation only. Later foothold validation triggers owner reconsideration; it does not automatically corroborate the claim. Failed origin validation downgrades dependent provisional use without erasing history.

### 10.4 Access validation

An authorized attempt produces a successful exit status but no reliable output, exact identity, privilege context, or stability evidence. The envelope is inconclusive for a foothold. Only a later claim-specific set satisfying PRD-003 can support validation.

### 10.5 Protected edge and negative evidence

A synthetic protected edge returns a challenge from one vantage. The envelope supports that bounded effect, not global origin absence, a named control, or permission to circumvent it. Silence from another vantage is unknown unless an accepted coverage/window contract makes it negative evidence.

### 10.6 Objective and sensitive material

An approved synthetic marker yields opaque target-specific material satisfying one predeclared objective condition. The envelope references the opaque derivation and permitted provenance, not raw client content. It proves neither broader access nor another objective condition.

### 10.7 Late contradiction and correction

A later independent Observation conflicts with an earlier accepted evaluation. The claim owner records a linked correction or revised disposition. The original envelope remains available for historical explanation but cannot override the newer current assessment.

### 10.8 Inspectable technical basis

An authorized synthetic action yields a bounded, sanitized request/response pairing. The admitted technical evidence material is available through a safe reference, while the envelope supplies claim, vantage, timing, and provenance context. A reviewer can inspect the applicable basis without receiving raw target output; its fingerprint supports continuity but is not the basis by itself.

### 10.9 Late contamination

A previously admitted synthetic screenshot is later found to contain a token. Ordinary access, reasoning, derivation, review, and export are blocked; the token is not echoed into a correction. A safe contamination record identifies affected downstream uses for reassessment, while the future accepted sensitive-remediation path governs prohibited-byte handling.

## 11. Observable acceptance criteria

1. Reviewers can distinguish Observation, admitted technical evidence material, evidence evaluation, EvidenceEnvelope, accepted model claim, and client proof without treating them as interchangeable.
2. Each EvidenceEnvelope binds a narrow declared claim to eligible non-sensitive material, provenance, source relationships, mode, relevant times, limitations, and an explicit evaluation disposition.
3. Where the owner contract requires an inspectable technical basis, an admitted non-sensitive artifact or bounded safe reference provides it; a fingerprint or integrity reference alone does not.
4. Duplicate or correlated sources cannot masquerade as independent corroboration; stable logical identity and bounded semantic lineage preserve provenance families, derivations, transformations, evaluations, corrections, and applicable export/disclosure transformations without requiring universal custody or read/view/handoff logging.
5. Sufficiency remains claim- and stakes-specific. Multiple independent sources are required only by the governing owner contract; one sufficiently direct item may satisfy the burden only when every required facet, provenance, integrity, freshness, and contradiction check passes.
6. Another owner's accepted claim remains bounded by its own semantics and cannot transfer universal truth or inherited sufficiency to a receiving owner.
7. Evidence supply cannot mutate another owner's state, create access, complete an objective, authorize execution, expand scope, or become a sixth operational model. If collection is declined or prohibited, the claim owner records the unmet burden and constrains dependent use without dispatching collection.
8. A reviewer can accept, challenge, reject, or request clarification for a declared consumer purpose and hold only governed dependent use, without mutating owner state or creating a mandatory human or campaign-wide gate.
9. Freshness derives from target-relevant times and current premises, not processing completion; current-use eligibility may narrow, suspend, or revoke before expiry while historical meaning remains.
10. When ordering matters, source time basis, precision, normalization, skew or uncertainty, time origin, and ordering limitations remain visible.
11. Accepted safe historical semantics and linked corrections remain append-only while current evaluation and owner state may change without universal event sourcing.
12. Blind campaign evidence excludes privileged defender/Observer/Grader inputs; defender-informed evidence remains distinctly labeled and isolated, including when eligibility changes before expiry.
13. A synthetic sensitive sentinel is absent from envelopes, history, retries, quarantine, logs, traces, diagnostics, reports, and reasoning context. Late contamination revokes access and eligibility, blocks propagation, triggers downstream reassessment, and leaves only safe accountability metadata pending an accepted sensitive-remediation path.
14. Ineligible, duplicate, identity-conflicting, contradictory, stale, unavailable, corrected, contaminated, and unknown material produces bounded semantics and blocks only dependent use unless safety or authority requires broader stopping.
15. Evidence replay, redelivery, review, or correction cannot repeat a target action, renew authority, restore access, dispatch a capability, or alter audit evidence.

## 12. Explicit non-goals and deferred work

This PRD does not define:

- Rust/Go types, database tables, repository layout, serialization, wire formats, message topics, service topology, crates, or modules;
- identifier generation, hashing, signatures, proof fingerprints, cryptographic keys, key custody, or integrity algorithms;
- the architecture preserving safe accepted-evidence and correction semantics, which belongs to ADR-009 after its dependencies are accepted;
- a universal custody manager or mandatory log of every read, view, internal handoff, or disposal action;
- client-facing proof, `ProofEnvelope`, packaging, or final-delivery sign-off semantics, which belong to PRD-009 and later architecture;
- the full ephemeral sensitive-data barrier, `Sensitive<T>`, purge, cryptographic erasure, backup remediation, disposal, irretrievability, or residual-risk mechanisms, which belong to PRD-010/ADR-011 and later architecture;
- retention periods, archival tiers, compaction, deletion policy, backup policy, or cross-campaign evidence sharing;
- a mandatory per-envelope human approval step or campaign-wide review barrier;
- universal confidence scoring, automated source reputation, causal attribution, evidence-ranking algorithms, or a global evidence state machine;
- reconnaissance methods, collectors, adapters, payloads, circumvention methods, execution scheduling, or runtime implementation;
- permission to acquire new data, access privileged defender feeds, execute a capability, deploy an artifact, or begin Stage 5 implementation.

ADR-008 remains responsible for architectural separation of Observation and Fact. ADR-009 must preserve the EvidenceEnvelope, safe accepted-history, correction, ownership, isolation, and sensitive-data semantics defined here without making evidence a universal owner. PRD-009 is the next product dependency only after explicit acceptance of this PRD.

---

**Authoring boundary:** PRD-008 is `PROPOSED`. It does not accept ADR-008 or ADR-009, change the active `DW-FOUNDATION-001` seal, seal `DW-DOMAIN-001`, or authorize runtime, acquisition tooling, capability execution, or Stage 5 implementation.

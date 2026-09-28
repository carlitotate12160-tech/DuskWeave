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
| **Evidence material** | An eligible Observation, accepted owner claim, or approved non-sensitive derivation considered for a declared claim. | Relevance or sufficiency until evaluated for that claim. |
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

A component may combine admission, evaluation, or custody responsibilities when boundaries remain enforceable. Shared deployment does not create shared ownership, and an evidence repository cannot become the system of truth for all operational models.

## 4. Required EvidenceEnvelope semantics

Every EvidenceEnvelope must make the following product meaning available where applicable:

1. **Engagement and campaign context**: the isolated context and authorized purpose for which the evidence may be evaluated.
2. **Declared claim**: the narrow proposition under assessment, its accountable owner, affected subject, and intended decision use.
3. **Exercise mode and source class**: blind campaign, separately authorized defender-informed exercise, or assessment context, with campaign-visible, operator-supplied, or privileged source distinguished.
4. **Material references**: eligible Observation, accepted owner-claim, or approved derivation references without embedding prohibited raw content.
5. **Vantage and origin**: the external vantage, validated position, transient unvalidated origin, or other bounded source context and its limitations.
6. **Relevant times**: effect time if known, perception time, availability/admission time, evaluation time, and uncertainty or validity window where applicable.
7. **Provenance and derivation**: enough non-sensitive continuity to assess origin, transformation, source dependence, mode eligibility, and integrity.
8. **Evaluation disposition**: the bounded respect in which material supports, refutes, conflicts with, or remains inconclusive for the declared claim.
9. **Burden and limitations**: the applicable stakes or epistemic burden, unmet requirements, contrary material, coverage gaps, and what remains unproven.
10. **Correction relationship**: a link to any superseded, corrected, or disputed evaluation without rewriting accepted history.
11. **Sensitivity disposition**: confirmation that raw sensitive content was excluded or transformed inside an authorized ephemeral boundary.
12. **Consumer purpose**: the owner reconciliation, bounded reasoning use, review, history, or proof-derivation purpose for which the envelope is eligible.

These are semantic obligations, not a universal field list, database schema, serialized type, identifier format, or wire protocol. Later contracts define applicability and representation without weakening these meanings.

## 5. Formation and claim-specific evaluation

Eligible Evidence material originates from admitted Observations, accepted owner claims, or approved non-sensitive/opaque derivations. Raw capture, unadmitted interpretation, a proposal, a command, event delivery, tool success, or a model-generated assertion is not Evidence merely because it is available.

An accepted owner claim is eligible only within that owner's actual semantics, source/mode bounds, freshness, and provenance. A receiving owner independently evaluates its relevance and burden; it cannot treat another owner's acceptance or domain-event delivery as universal fact, inherit that owner's sufficiency, or bypass its own reconciliation.

Formation is bounded by a declared claim. The evaluator must preserve the material's original scope and must not broaden “observed from V at T” into a global, causal, identity, access, or objective claim. Derived material retains references to eligible premises and its stated transformation limits.

An envelope may contain supporting, refuting, conflicting, and inconclusive material together. Evidence evaluation does not coerce ambiguity into binary success/failure. Negative evidence requires an accepted coverage and observation-window basis; absence, silence, timeout, or unavailable input is otherwise unknown.

A single Observation may be sufficient for a narrow, low-stakes direct claim where the owning contract permits it. Repetition from one underlying source, duplicated delivery, relabeled output, or several views of the same acquisition does not create independent corroboration. Source independence is assessed from provenance and derivation, not item count.

## 6. Sufficiency, tier, and owner use

Sufficiency is claim-specific and proportional to consequences. No EvidenceEnvelope carries a universal confidence score or becomes sufficient for every consumer.

- **Narrow low-stakes use** may rely on one sufficiently direct, owner-qualified source within its exact vantage, time, and limits.
- **Moderate-stakes use** requires the owning contract's supporting evidence and contradiction checks before a consequential transition.
- **Campaign-critical or high-stakes use** requires multiple genuinely independent sources and full owner reconciliation.
- **Transient-origin material** remains origin-unvalidated and can support only the bounded PROVISIONAL orientation permitted by PRD-003 and PRD-007.
- **Access validation** must satisfy PRD-003 evidence for reliable bounded execution, output retrieval, exact identity and privilege context, relevant environment boundary, and operational stability.
- **Objective fulfillment** must satisfy the predeclared condition and least-intrusive proof rules in PRD-005; generic execution success is insufficient.

If later consequences make a claim higher-stakes, the evidence burden rises before dependent use. Tempo, deadline pressure, tool confidence, or operator convenience cannot lower the burden.

Once the accountable claim owner qualifies an EvidenceEnvelope for a declared use, eligible bounded reasoning may use it without a campaign-wide pause. Only the dependent claim, projection, or action waits for unresolved insufficiency, conflict, staleness, or missing evidence; independent work continues.

Evidence supply requests owner reconsideration. It cannot write owner state, create a foothold, mark an objective satisfied, restore lost access, expand scope, renew authority, or dispatch a capability.

## 7. Time, freshness, history, and correction

Evidence preserves effect/perception time separately from availability, admission, evaluation, and owner-acceptance time. Processing delay remains visible and cannot make old target reality fresh. Freshness is evaluated at use against the declared claim, source behavior, validity window, current position, and consequence.

An old EvidenceEnvelope may remain an accurate historical account of what supported a decision at that time while becoming ineligible for a current claim. Staleness does not refute the earlier observation, prove current absence, or erase the historical basis. Historical evidence cannot by itself restore current access or authority.

Current evidence evaluation and owner state may change through accepted transitions. Once retained as an accepted historical EvidenceEnvelope or evaluation, its semantic content is append-only. A correction, reinterpretation, refutation, or narrowed scope is a new accountable linked record; it does not overwrite the original or conceal decisions made from it.

This requirement does not mandate universal event sourcing, retention of every raw capture, or one shared evidence store. ADR-009 will choose the architecture that preserves accepted evidence and correction semantics.

Out-of-order material may complete history but cannot overwrite a newer accepted correction. Missing predecessors, conflicting corrections, or an identity reused with different semantic content block only affected dependencies pending verifiable owner reconciliation. First/last arrival, majority count, or producer assertion cannot resolve an integrity conflict.

## 8. Mode, isolation, and sensitive-data boundary

Evidence remains isolated by engagement, campaign, exercise mode, and authorized purpose. Similar targets or a shared deployment do not authorize cross-campaign reuse. Any future sharing requires an accepted design, destination-side admission, preserved provenance, and explicit authorization; those conditions are not granted here.

In blind mode, privileged defender-internal telemetry, analyst conclusions, Observer verdicts, and Grader results are ineligible evidence for campaign reasoning. Relabeling, summarizing, referencing, or replaying them cannot launder the source. Campaign-visible effects and telemetry legitimately encountered through an authorized current position may be eligible within their narrow scope after admission and owner evaluation.

A separately authorized defender-informed exercise keeps its source and mode visible. Its evidence cannot be represented as a blind campaign result or silently combined with blind evidence to satisfy corroboration.

Raw credentials, password hashes, private keys, authentication stores, personal records, customer data, financial records, unrestricted target output, and other prohibited client material cannot enter an EvidenceEnvelope, evidence history, retry/quarantine payload, log, trace, diagnostic, report, or reasoning context.

Where an accepted access or objective flow permits minimum client content, it remains inside the isolated ephemeral sensitive boundary only long enough to derive approved opaque material. The envelope may reference that approved derivation and permitted provenance; a reference is never permission to retrieve raw content. PRD-009 and PRD-010 define proof and sensitive-handling details.

Evidence handling, correction, rejection, or cleanup cannot alter, delete, disable, or falsify authoritative audit evidence.

## 9. Failure and stopping semantics

- **Ineligible material**: reject without promoting it to Evidence or echoing prohibited content; retain only permitted accountability metadata.
- **Incomplete provenance**: mark the affected evaluation insufficient or unresolved; do not invent source independence.
- **Duplicate material**: process without a second logical evidential effect where a stable identity exists.
- **Contradictory material**: preserve both sides and block or downgrade only dependent use until the claim owner reconciles them.
- **Stale material**: preserve historical meaning while requiring refresh or downgrade for current use.
- **Unavailable referenced material**: do not treat unavailability as refutation or silently omit a required basis.
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

## 11. Observable acceptance criteria

1. Reviewers can distinguish Observation, evidence material, Evidence evaluation, EvidenceEnvelope, accepted model claim, and client proof without treating them as interchangeable.
2. Each EvidenceEnvelope binds a narrow declared claim to non-sensitive material, provenance, source relationships, mode, relevant times, limitations, and an explicit evaluation disposition.
3. Duplicate or correlated sources cannot masquerade as independent corroboration; a source count alone cannot satisfy a burden.
4. Another owner's accepted claim remains bounded by its own semantics and cannot transfer universal truth or inherited sufficiency to a receiving owner.
5. Sufficiency remains claim- and stakes-specific, including the accepted Terrain tiers, Access evidence invariants, and Objective proof conditions.
6. Evidence supply cannot mutate another owner's state, create access, complete an objective, authorize execution, expand scope, or become a sixth operational model.
7. Freshness derives from target-relevant times and validity, not processing completion; old evidence may explain history without proving current reality.
8. Accepted historical envelopes and linked corrections remain append-only while current evaluation and owner state may change without universal event sourcing.
9. Blind campaign evidence excludes privileged defender/Observer/Grader inputs; defender-informed evidence remains distinctly labeled and isolated.
10. A synthetic sensitive sentinel is absent from envelopes, history, retries, quarantine, logs, traces, diagnostics, reports, and reasoning context.
11. Ineligible, duplicate, conflicting, stale, unavailable, corrected, and unknown material produces bounded semantics and blocks only dependent use unless safety or authority requires broader stopping.
12. Evidence replay, redelivery, or correction cannot repeat a target action, renew authority, restore access, or dispatch a capability.

## 12. Explicit non-goals and deferred work

This PRD does not define:

- Rust/Go types, database tables, repository layout, serialization, wire formats, message topics, service topology, crates, or modules;
- identifier generation, hashing, signatures, proof fingerprints, cryptographic keys, key custody, or integrity algorithms;
- the architecture enforcing evidence immutability, which belongs to ADR-009 after its dependencies are accepted;
- client-facing proof or `ProofEnvelope` semantics, which belong to PRD-009 and later architecture;
- the full ephemeral sensitive-data barrier or `Sensitive<T>` semantics, which belong to PRD-010 and later architecture;
- retention periods, archival tiers, compaction, deletion policy, backup policy, or cross-campaign evidence sharing;
- universal confidence scoring, automated source reputation, causal attribution, evidence-ranking algorithms, or a global evidence state machine;
- reconnaissance methods, collectors, adapters, payloads, circumvention methods, execution scheduling, or runtime implementation;
- permission to acquire new data, access privileged defender feeds, execute a capability, deploy an artifact, or begin Stage 5 implementation.

ADR-008 remains responsible for architectural separation of Observation and Fact. ADR-009 must preserve the EvidenceEnvelope, accepted-history, correction, ownership, isolation, and sensitive-data semantics defined here without making evidence a universal owner. PRD-009 is the next product dependency only after explicit acceptance of this PRD.

---

**Authoring boundary:** PRD-008 is `PROPOSED`. It does not accept ADR-008 or ADR-009, change the active `DW-FOUNDATION-001` seal, seal `DW-DOMAIN-001`, or authorize runtime, acquisition tooling, capability execution, or Stage 5 implementation.

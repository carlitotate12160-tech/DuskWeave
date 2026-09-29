# PRD-007: Observation Model

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-007 |
| **Title** | Observation Model |
| **Status** | ACCEPTED |
| **Stage** | Stage 4 — Reality & Evidence |
| **Direct Dependencies** | [PRD-000..006](README.md) and [ADR-001..007](../adr/README.md) — ACCEPTED; `DW-FOUNDATION-001` historically sealed 2026-09-28 and reopened 2026-09-29 pending explicit reseal |
| **Target Seal** | DW-DOMAIN-001 |
| **Acceptance** | Product owner, 2026-09-28 — accepted after reconciliation of observation/reconnaissance, owner-qualified use, freshness, protected-edge, and raw-input boundaries |

## 1. Purpose and product outcome

DuskWeave needs a common product meaning for what the campaign perceived without turning perception into truth, access, objective success, or execution authority. This PRD defines an Observation as a sourced, time-bounded report of a condition, response, or effect perceived from a stated vantage during an authorized engagement.

An Observation preserves what was perceived and the limits under which it was perceived. It is an input to an accountable owner's validation and reconciliation, not an accepted fact by itself. This distinction enables rapid orientation while preventing a successful command, a sensor message, an inference, or silence from silently becoming operational truth.

This PRD applies across the Strategic, Access, Expansion, and Objective loops. It defines shared semantics and admission obligations, but it does not create a sixth operational model, a universal observation manager, or a global state machine.

Observation is not synonymous with reconnaissance. Reconnaissance is an authorized campaign activity that may acquire perceptions; Observation is the bounded product representation of an eligible perception after admission. This PRD neither selects reconnaissance actions nor authorizes acquisition, execution, or dispatch.

## 2. Vocabulary and boundaries

| Term | Product meaning | What it does not establish |
| :--- | :--- | :--- |
| **Raw capture** | Untrusted acquisition material from a tool, sensor, operator, or environment, kept outside operational models, durable/cognitive surfaces, and core reasoning until permitted interpretation and sensitive-data handling have completed. | Safe retention, an Observation, truth, evidence, or permission. |
| **Observation** | An admitted, non-sensitive, sourced, time-bounded report of what was perceived from a stated vantage. | Fact, causal attribution, access, objective fulfillment, or authorization. |
| **Accepted model claim** | A claim an accountable operational-model owner has accepted at an explicit epistemic status after its own reconciliation. | Universal truth, indefinite freshness, or authority outside that model. |
| **Inference** | A reasoned conclusion derived from eligible premises, labeled with those premises and limits. | Direct perception or automatic fact promotion. |
| **Evidence** | Material assessed for its ability to support or refute a bounded claim; detailed semantics belong to PRD-008. | Automatic proof or permission. |
| **Client proof** | A client-facing substantiation of a declared result; detailed semantics belong to PRD-009. | Raw client content or a replacement for owner state. |

The foundation phrase **raw observation** names generic pre-model perception awaiting validation and reconciliation. In this PRD, raw capture is the untrusted acquisition material; pre-admission interpretation is a bounded process, not a separately owned or persisted product entity; and the capitalized Observation is the admitted report. These terms do not create another model.

Observation identity and model-claim identity remain distinct. Several observations may support one claim; one observation may be relevant to several owners, but each owner independently decides whether and how it affects its state through its own contract.

CyberTerrain owns accepted environmental claims. FootholdGraph owns validated positions and their health. AttackPathView owns candidate and derived transition views. ObjectiveState owns objective progress. CampaignTrajectory owns the chronological account of decisions, attempts, and outcomes. Observation routing cannot transfer those ownership rights.

## 3. Actors and accountability

- An **authorized producer** perceives an effect through a capability, collector, operator input, or other explicitly permitted source.
- An **observation admission boundary** validates scope, source, mode, provenance sufficiency, sensitivity, and structural limits before an Observation may reach core reasoning or an operational model.
- A **receiving model owner** reconciles eligible observations against its current state, accepted history, contrary material, freshness rules, and claim-specific burden.
- A **bounded reasoner** may use only observations or model views eligible for its campaign, exercise mode, purpose, and decision stakes. It may propose interpretations or actions but cannot accept owner state or dispatch execution.
- An **operator or reviewer** may inspect permitted provenance, limitations, conflicts, and outcomes without receiving prohibited raw sensitive material.
- The **Observer/Grader plane** may perform separately authorized assessment, but its privileged inputs remain outside blind campaign reasoning under INV-007.

No actor becomes authoritative merely by producing an Observation. A producer reports perception; the receiving owner remains accountable for any accepted state transition.

## 4. Required observation semantics

Every admitted Observation must make the following meaning available where applicable:

1. **Engagement context**: the campaign or engagement to which it belongs and the authorized purpose for which it was acquired.
2. **Source**: accountable producer/source identity and source class, including whether it is campaign-visible, operator-supplied, or privileged assessment input.
3. **Exercise mode**: blind campaign, separately authorized defender-informed exercise, or authorized assessment context.
4. **Vantage and origin**: where the perception occurred, including a validated position reference, transient unvalidated origin, external authorized vantage, or other explicitly bounded source context.
5. **Subject and bounded assertion**: the entity, attempt, response, or effect perceived and the narrow statement actually supported.
6. **Time**: when the effect occurred if known, when it was perceived, and when it became available for admission; uncertainty remains explicit.
7. **Method and limits**: the authorized capability or collection method at a product-semantic level, its relevant limitations, and whether the result is complete, partial, or unknown.
8. **Correlation**: safe references to the originating proposal, authorized attempt, action, or prior claim where such a relationship is known.
9. **Sensitivity disposition**: confirmation that prohibited raw material was rejected or transformed inside its authorized ephemeral boundary before core admission.
10. **Provenance continuity**: enough non-sensitive origin and derivation information for a receiving owner to assess independence, contradiction, freshness, and mode eligibility.

These are semantic obligations, not a database schema, wire envelope, universal payload object, or mandatory field list for every observation family. Downstream contracts define applicable fields without weakening these meanings.

An Observation carries the limits of its source. A banner is a report of returned text, not proof of installed software. A tool exit code is a tool outcome, not proof of access. A campaign-visible denial is evidence of that denial, not automatic attribution to a named defensive control. Absence of a response or alert is inconclusive unless an accepted contract establishes the relevant coverage and observation window.

## 5. Vantage and origin eligibility

Observations may originate from:

- an authorized external or pre-access vantage;
- a validated campaign position within its current identity, privilege, target, and capability bounds;
- a transient position, only for the bounded read-only provisional orientation allowed by PRD-003;
- an authorized operator-supplied or campaign-visible source whose limits remain explicit; or
- a separately authorized assessment/Observer source that remains isolated from blind campaign reasoning.

A transient-origin Observation remains unvalidated and can support only PROVISIONAL orientation. It retains its origin reference and cannot become the sole premise for scope, trust, expansion, objective work, Key Terrain, or another consequential action. If the origin becomes a validated foothold, the receiving owner reconsiders the observation under the claim's normal evidence burden; origin validation does not automatically corroborate it. If the origin fails validation or is abandoned, dependent provisional claims follow the accepted downgrade and invalidation rules.

Only a validated position may become a FootholdGraph node or serve as the consequential origin for expansion, objective work, or other state-changing follow-on action. Observation admission never upgrades a position.

A blind campaign is blind to privileged defender truth, not deprived of its authorized engagement context. Declared targets, scope, timing, exclusions, exercise mode, and operator-approved constraints are starting authority, not observations about target reality. Any additional target understanding must arise from eligible campaign acquisition and owner reconciliation.

## 6. Admission and reconciliation

Admission is a bounded eligibility decision, not a global gate sequence or a claim that interpretation is mechanically infallible. It enforces, for the applicable observation family:

1. current engagement scope, operating authority, intended use, and stopping conditions;
2. campaign isolation, exercise mode, source eligibility, and vantage/origin limits;
3. rejection of prohibited sensitive content before persistence, logs, traces, diagnostics, retry material, reports, or LLM context;
4. attributable provenance, relevant times, a narrow assertion, and adequate bounded structure;
5. explicit preservation of contrary, partial, uncertain, negative, or unknown outcomes; and
6. routing only to accountable owners or consumers with declared responsibilities.

These obligations may be evaluated concurrently or incrementally where their dependencies allow. Routine outputs from already authorized acquisition do not require per-observation human approval. Material that is ambiguous, ineligible, or unsafe is rejected, or quarantined only if a later accepted sensitive-handling contract permits that safely; unrelated eligible work continues.

Admission means that an Observation is eligible to be considered. It does not mean the reported condition is true, corroborated, current, causally attributed, or safe to act upon. Each receiving owner then performs bounded, claim-specific reconciliation under its evidence, freshness, tier, and transition rules.

After an accountable owner qualifies an admitted Observation for a declared non-authoritative purpose, it may immediately inform that purpose—for example, bounded hypothesis ranking—within that owner's limits. Consequential state, access, objective, trust, scope, or action decisions still require the owning model's accepted burden. Admission alone never changes ranking or state.

There is no campaign-wide reconciliation barrier. Independent owners and independent dependency branches may continue as soon as their own inputs are eligible; only the claim or action that depends on unresolved, stale, conflicting, or missing material is blocked or downgraded.

For CyberTerrain, PRD-002 owns epistemic status and tier. A narrow direct source may support an OBSERVED Terrain claim after Terrain validation and reconciliation; a transient origin produces PROVISIONAL orientation; consequential Tier 2 or Tier 3 claims require their accepted burden. Other operational models apply their own claim semantics and cannot borrow Terrain status as a shortcut.

An observation consumer may supply eligible material, request bounded reconsideration, maintain an owned projection, or contribute required history only through its declared contract. It cannot write another owner's state, impersonate that owner, or make event delivery equivalent to owner acceptance.

## 7. Time, freshness, and current applicability

DuskWeave distinguishes the time of a perceived real-world effect, observation time, availability/receipt time, owner acceptance time, and evaluation time where they differ. Clock time alone is not a total causal order, and a late arrival cannot overwrite a newer accepted correction.

Freshness is evaluated when an observation or derived claim is used and is anchored to the perceived effect/observation time and its stated validity window—not to the time admission or reconciliation completed. Availability, admission, and owner-processing delay remain visible; processing cannot refresh old target reality. An old Observation remains a historical report of what was perceived, but it does not establish that the condition or access still exists. Expiry does not refute the original report, create corroboration, or resolve a provisional origin.

Current owner state may change through accepted transitions. When an admitted Observation is retained as an accepted historical record, its semantic content is not rewritten; a correction or reinterpretation is linked as a new accountable record. This does not require universal event sourcing or retention of every raw capture. Retention follows the responsible owner's and later evidence/sensitive-data contracts.

Out-of-order observations, missing predecessors, incompatible versions, or unresolved causal gaps remain visible and ineligible for dependent consequential use until the responsible owner reconciles them. Historical inspection must distinguish what was known at the time from later retrospective interpretation.

## 8. Defender knowledge and source isolation

In blind mode, privileged defender-internal telemetry and verdicts—including console alerts, SIEM detections, SOC tickets, analyst conclusions, Observer verdicts, and Grader results—are ineligible for campaign reasoning even if they are structurally valid observations. Relabeling, forwarding, caching, replay, or summarization cannot launder such a source into blind context.

Telemetry legitimately encountered through an authorized current campaign position is ordinary campaign-observed material. It may be admitted only within scope and current position bounds, retains its acquisition provenance, and does not automatically prove the telemetry is correct or that a particular control caused an observed effect.

A separately authorized defender-informed exercise may use bounded defender feedback only with its distinct mode/source label and evaluation boundary. Its observations and outcomes cannot be represented as results from a blind run.

## 9. Sensitive-data and audit boundaries

Raw credentials, password hashes, private keys, authentication stores, personal records, customer data, financial records, unrestricted target output, and other prohibited client material are not Observations eligible for core admission. They must not enter durable observation history, event/retry/quarantine payloads, caches, logs, traces, crash diagnostics, reports, or reasoning context.

When an accepted access or objective proof flow expressly permits minimum client content, that content remains inside the isolated ephemeral sensitive boundary long enough only to derive approved opaque material. PRD-010 will define the detailed handling requirements. This PRD grants no new collection permission and no retention exception.

Observation, correction, cleanup, or rejection handling can never alter, delete, disable, or falsify authoritative audit evidence. A rejection may retain only safe identifiers, category, and approved non-sensitive provenance needed to account for the decision.

## 10. Conflict, correction, and failure semantics

- **Duplicate delivery**: where a later accepted contract assigns a stable observation identity, redelivery of that identity with the same semantic content is processed safely without a second logical effect.
- **Identity conflict**: where such an identity exists, reuse with different semantic content is an integrity anomaly. Affected dependent use is blocked pending verifiable owner reconciliation; first/last arrival or producer assertion cannot choose the winner.
- **Contradictory observations**: both remain sourced and visible to authorized reconciliation. Contradiction is not resolved by last-write-wins or by counting sources that are not independent.
- **Correction**: a new linked record states the corrected scope and basis. It does not erase the original perception, rewrite historical decisions, or execute compensation.
- **Partial result**: only the observed portion may be asserted; omitted or interrupted portions remain unknown.
- **Blocked or negative outcome**: a refusal, denial, challenge, or unreachable result is a bounded assertion about the tested interaction from its stated vantage and time. It may reduce or refute only dependent hypotheses whose accepted semantics cover that result; it does not establish global absence, name a control, expand scope, or authorize circumvention. A scope or authority denial stops the affected proposal.
- **Unknown external outcome**: interruption, timeout, silence, or missing acknowledgement does not imply target success or failure and does not authorize automatic repetition of the action.
- **Source unavailable or stale**: dependent claims expose the limitation and defer, refresh, or downgrade under their owner's rules; unavailable input is not negative evidence.
- **Admission failure**: prohibited or ineligible input is rejected without echoing sensitive content, contaminating a blind context, or silently dropping a required accountability signal.

Retries of collection, delivery, or reconciliation are bounded and do not repeat target-side effects merely because an observation acknowledgement is absent. Effective authorization withdrawal, safety freeze, or cancellation cannot depend on an ordinary observation backlog.

## 11. Illustrative synthetic cases

### 11.1 Direct narrow observation

From validated synthetic position B, an authorized query returns a local service banner at time T. The admitted Observation says only that this response was perceived from B at T. Terrain may accept a narrow Tier 1 OBSERVED claim after its checks; it does not establish global reachability, installed-version truth, access on another identity, or objective success.

### 11.2 Transient-origin orientation

Transient synthetic position D returns local identity and interface information through the read-only allowance in PRD-003. The observations retain D as an unvalidated origin and may support only PROVISIONAL Terrain orientation. D later fails validation, so the accepted owner downgrade/invalidation rules apply; no FootholdGraph node or expansion origin is created.

### 11.3 Protected edge and campaign-visible defensive effect

A scoped request to a synthetic protected edge receives a challenge or access-denied response while no origin response is observable. The Observation establishes only that this edge-visible effect occurred from the stated vantage and time. It does not prove that the origin is unreachable from every eligible path, identify a vendor or control, or authorize evasion, circumvention, alternate infrastructure, or a new target. Owner-qualified reasoning may revise only dependent hypotheses and may propose another already-authorized, lower-footprint action; ordinary scope and capability validation still apply.

A privileged console alert supplied separately is rejected from blind context. If comparable telemetry is legitimately encountered through a validated campaign position, it remains a sourced campaign observation whose correctness and causality still require reconciliation.

### 11.4 Ambiguous execution outcome

An authorized attempt loses connectivity after dispatch and produces no reliable completion result. The observation records interruption and unknown outcome. It cannot create a foothold, satisfy an objective, or cause automatic target-side retry.

### 11.5 Sensitive input

A synthetic tool output contains a sentinel representing a private key and customer record. Admission rejects the prohibited content before durable or cognitive surfaces. Only a safe rejection category and permitted provenance may remain; the sentinel cannot appear in history, events, diagnostics, proof, or reports.

### 11.6 Late correction and conflict

An older observation arrives after a linked correction has already changed the current owner claim. The late report may complete history but cannot restore the superseded current state. If a later accepted contract assigned a stable identity and that identity carries different content, dependent use remains blocked until an append-only, verifiable resolution is accepted.

## 12. Observable acceptance criteria

1. Reviewers can distinguish foundation-level raw observation, raw capture, admitted Observation, accepted model claim, inference, evidence, and client proof without treating any pair as interchangeable or inventing another product model.
2. Every admitted Observation exposes sufficient non-sensitive scope, source, mode, vantage/origin, time, bounded assertion, method limits, and provenance for the applicable receiving owner.
3. A tool success, source assertion, command output, silence, inferred relationship, or event delivery cannot bypass owner validation and reconciliation.
4. A transient-origin observation remains PROVISIONAL and non-consequential; only a separately validated position can become a FootholdGraph node or consequential origin.
5. Current owner claims can change while retained accepted history and linked corrections remain append-only, without requiring universal event sourcing.
6. Late, partial, duplicate, contradictory, stale, corrected, and unknown outcomes produce explicit bounded semantics rather than false certainty.
7. Blind campaign reasoning rejects privileged defender/Observer/Grader inputs while preserving eligible campaign-visible observations and distinct defender-informed mode labels.
8. A synthetic sensitive sentinel is rejected before persistence, event/retry/quarantine material, logs, traces, diagnostics, reports, or LLM context.
9. Observation replay or redelivery cannot dispatch a capability, repeat a target-side action, renew authority, restore access, or claim objective success.
10. The five operational models retain separate owners and transitions; no Observation service becomes a sixth model or global manager.
11. Owner-qualified observations may inform bounded hypothesis ranking without a campaign-wide barrier, while only dependent consequential use waits for unresolved inputs.
12. A protected-edge denial remains a narrow result and cannot itself identify a control, authorize circumvention, or establish global target absence.

## 13. Explicit non-goals and deferred work

This PRD does not define:

- evidence sufficiency, evidence immutability, or `EvidenceEnvelope` semantics, which belong to PRD-008 and later accepted architecture;
- client-facing proof, proof fingerprints, or `ProofEnvelope`, which belong to PRD-009 and later accepted architecture;
- the complete ephemeral sensitive boundary or `Sensitive<T>` semantics, which belong to PRD-010 and later accepted architecture;
- database tables, retention periods, message brokers, serialization formats, identifiers, hashes, signatures, cryptographic keys, or wire protocols;
- Rust/Go types, crates, modules, service topology, reconnaissance planners, collectors, adapters, tool commands, execution sandboxes, or runtime scheduling;
- a universal confidence score, source-ranking algorithm, tier classifier, causal-attribution engine, or global observation state machine;
- permission to collect new data, access privileged defender feeds, deploy a temporary artifact, execute a capability, or begin runtime implementation.

ADR-008 must define how architecture preserves the Observation-versus-Fact boundary without weakening this product meaning. Acceptance of this PRD permits PRD-008 product authoring; evidence architecture and runtime remain unauthorized until their own dependencies and decisions are accepted.

---

**Acceptance boundary:** PRD-007 is `ACCEPTED` by explicit product-owner decision on 2026-09-28. This accepts the Observation product semantics only; it does not accept ADR-008, change `DW-FOUNDATION-001`, seal `DW-DOMAIN-001`, or authorize runtime, acquisition tooling, or capability execution.

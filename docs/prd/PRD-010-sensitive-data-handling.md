# PRD-010: Sensitive Data Handling

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-010 |
| **Title** | Sensitive Data Handling |
| **Status** | PROPOSED |
| **Stage** | Stage 4 — Reality & Evidence |
| **Direct Dependency** | [PRD-009 Client Proof](PRD-009-client-proof.md) — ACCEPTED |
| **Target Seal** | DW-DOMAIN-001 |

## 1. Purpose and product outcome

DuskWeave must demonstrate authorized access and objective impact without becoming a repository, viewing surface, or reasoning channel for raw client-sensitive material. When an accepted proof condition cannot be satisfied with synthetic, metadata-only, or other non-sensitive evidence, expressly authorized minimum client content may exist only inside an isolated ephemeral sensitive boundary long enough to produce an approved opaque derivation.

The product outcome is bounded handling with honest disposition: ordinary campaign components receive only non-sensitive semantics; raw content is unavailable to operators, reasoners, evidence history, proof delivery, and durable infrastructure; every boundary attempt ends in a safe disposal outcome or an explicit unresolved incident. The boundary does not authorize acquisition, expand engagement scope, accept an evidence claim, or determine client risk.

Sensitive handling is a cross-cutting product constraint rather than a sixth operational model. CyberTerrain, FootholdGraph, AttackPathView, ObjectiveState, and CampaignTrajectory retain their existing owners. EvidenceEnvelope and ProofEnvelope may retain only approved non-sensitive or opaque outputs and safe accountability metadata.

## 2. Vocabulary and boundaries

| Term | Product meaning | What it does not establish |
| :--- | :--- | :--- |
| **Sensitive value** | Raw client or third-party material whose exposure, persistence, or reuse is prohibited or restricted; later contracts may represent this meaning as `Sensitive<T>`. | A runtime type, permission to acquire the value, or permission to disclose it. |
| **Prohibited raw material** | Credentials, password hashes, private keys, authentication stores, personal records, customer or financial data, unrestricted target output, and other content disallowed from ordinary DuskWeave surfaces. | That every item may enter the proof boundary; source authorization remains separate. |
| **Ephemeral sensitive boundary** | An isolated, purpose-bound processing context in which expressly permitted minimum content may exist only for the active derivation and disposal lifecycle. | A durable vault, quarantine archive, evidence store, operational model, or generic data-processing platform. |
| **Approved opaque derivation** | A bounded non-sensitive result that substantiates a declared claim without exposing the source content or enabling practical reconstruction or arbitrary guess testing beyond that claim. | Universal proof, a copy of the source, or permission to retrieve the source later. |
| **Ordinary surface** | Any campaign, evidence, reporting, reasoning, operator, telemetry, retry, diagnostic, or durable surface outside the sensitive boundary. | A place where redaction after persistence makes raw content acceptable. |
| **Safe accountability metadata** | Non-sensitive context needed to account for authorization, handling attempt, disposition, incident, and affected references without reproducing source content. | A hidden encoding, fingerprint oracle, or recoverable substitute for the raw material. |
| **Contamination incident** | Known or credible escape of prohibited raw material to an ordinary surface, unauthorized recipient, replica, or persistence mechanism. | Proof that every copy is known, contained, or remediated. |
| **Disposal assurance** | The bounded conclusion about whether the attempted disposal completed and whether recovery is infeasible under the accepted sensitivity and threat assumptions. | Absolute future-proof irretrievability or permission to hide residual uncertainty. |

A value remains sensitive through copying, parsing, transformation, or summarization until an accountable egress decision admits a specific derivation as non-sensitive for a declared purpose. A hash, fingerprint, redaction, encoding, or model summary is not automatically opaque or safe.

## 3. Actors, ownership, and separation of authority

- The **engagement authority** defines the authorized purpose, target and source bounds, permitted content category, proof condition, recipients, and stopping conditions. It cannot waive INV-005 or authorize raw content as a deliverable.
- The **operational-model owner** declares the claim and evidentiary need but cannot authorize collection or inspect prohibited raw content merely because it owns the claim.
- An **authorized capability** may introduce material to the boundary only within its separately validated action authority. Capability success does not admit the material or retain it.
- The **sensitive-boundary function** enforces purpose, isolation, bounded lifetime, output restrictions, and disposal accountability. It owns no campaign truth and cannot dispatch target actions.
- The **derivation function** attempts the predeclared transformation. Its candidate output remains sensitive until egress admission succeeds.
- The **egress authority** decides whether a candidate derivation is non-sensitive and eligible for its declared EvidenceEnvelope or ProofEnvelope purpose. It does not accept claim sufficiency, mutate owner state, or authorize proof release. Egress admission may be automated under later accepted contracts and need not create a per-value human gate.
- The **sensitive-remediation authority** handles contamination and affected replicas independently of campaign execution. It cannot falsify campaign history or grant new target authority.
- The **audit owner** preserves safe accountable history. Campaign capabilities cannot alter authoritative audit evidence.

One component or deployable may support several of these functions only when their permissions, inputs, outputs, and accountability remain separable. No universal custody manager may combine acquisition, campaign state, evidence acceptance, proof release, secret storage, incident response, and audit control.

## 4. Required handling authorization semantics

Before permitted client content enters the boundary, the following product meaning must be established where applicable:

1. **Engagement and campaign context**: the isolated context and current authorization under which handling occurs.
2. **Declared proof condition**: the narrow access or objective claim, accountable owner, and exact result the derivation is intended to establish.
3. **Lesser-proof exhaustion**: why synthetic material, metadata, canaries, or other non-sensitive evidence is insufficient for that condition.
4. **Source and scope bounds**: the authorized resource, validated position, content category, and any third-party or jurisdictional limitation.
5. **Minimum selection bound**: the smallest practical record, field, interval, or other bounded portion permitted for the derivation.
6. **Permitted transformation**: the declared derivation purpose and the properties required before its result may leave the boundary.
7. **Lifetime and termination conditions**: the period or attempt boundary and the success, failure, interruption, cancellation, expiry, or withdrawal conditions that end handling.
8. **Eligible egress purpose**: the specific EvidenceEnvelope, ProofEnvelope, review, or safe accountability use allowed to receive an admitted derivation.
9. **Disposal and assurance obligation**: the required disposal outcome, verification basis, accountable authority, and treatment of uncertainty.
10. **Incident path**: the safe escalation and remediation authority if content escapes, disposal cannot be established, or authorization changes.

Authorization may cover a bounded class of materially equivalent proof attempts when their purpose, source class, selection limit, transformation, egress, and disposal obligations are the same. This avoids a mandatory human gate for every value while preventing authorization from silently broadening through repetition.

Access to a resource, possession of a credential, or approval of an objective does not by itself authorize raw-content handling. The client-content proof exception does not authorize acquiring passwords, private keys, authentication stores, or bulk datasets. Any later capability that must use a secret for execution requires its own accepted product and runtime authority; this PRD grants none.

## 5. Least-content handling

The product first seeks proof that avoids client content. Synthetic markers, canaries, bounded metadata, safe technical observations, and already approved opaque material are preferred. Once the declared proof burden is satisfied, collection for that condition stops.

If client content is still necessary, the proposal identifies and the current action and handling authorities validate the minimum bounded portion needed for the predeclared derivation. The authorized capability may admit no more than that bound. Convenience, possible future analysis, model training, debugging, completeness, or speculative reuse cannot justify additional content. Unrestricted command output, database dumps, mailbox exports, directory trees, memory images, authentication stores, and similar bulk material are ineligible unless another accepted product decision expressly changes the governing authority.

Material that is unexpectedly sensitive, exceeds its selection bound, has an unapproved source, or cannot be classified safely does not enter an ordinary surface. It is rejected or kept only within the active boundary while immediate disposal or incident handling occurs. Ambiguity is not permission to downgrade sensitivity.

Third-party content encountered on an authorized client system receives the same protection. Client ownership of the target does not prove authority to retain or disclose data belonging to employees, customers, suppliers, tenants, or other parties.

## 6. Boundary lifecycle and propagation

A handling attempt follows one bounded semantic lifecycle:

1. validate current action authority and the handling authorization;
2. admit only the permitted minimum content directly into the sensitive boundary;
3. restrict use to the declared transformation and proof condition;
4. treat every intermediate and candidate output as sensitive;
5. admit only a specifically approved non-sensitive derivation through egress;
6. end raw access and perform the required disposal on success, failure, interruption, expiry, cancellation, or withdrawal;
7. record only safe accountability metadata and the honest disposal disposition.

Raw content and sensitive intermediates must not enter durable storage, temporary files, caches, queues, event or retry payloads, logs, traces, metrics labels, crash diagnostics, core dumps, test fixtures, reports, operator interfaces, clipboard-like surfaces, model prompts, training corpora, or authoritative audit payloads. Later architecture must choose a boundary whose normal operation does not replicate sensitive values into those surfaces.

No general raw-data quarantine is permitted. Temporary containment inside the same boundary may continue only while an active disposal or contamination response is being performed. It cannot become an investigation archive, debugging store, or delayed evidence bucket.

Process failure, host failure, lost connectivity, or campaign termination does not change sensitivity or make content eligible for persistence. If the system cannot establish that all boundary-held material followed the required disposition, the attempt remains unresolved and dependent claims cannot treat cleanup as complete.

## 7. Opaque derivation and egress admission

An opaque derivation is claim-specific and least-disclosing. It exposes only what an authorized reviewer needs to evaluate the declared result and preserves non-sensitive provenance, scope, time, and transformation limits. It must not disclose source content, permit practical reconstruction, or provide an oracle for testing arbitrary guesses about the source beyond the claim expressly being proven.

A bare hash or fingerprint is insufficient when the source has low entropy, predictable values, an enumerable population, or other properties that make guessing practical. Redaction is insufficient when surrounding structure, metadata, error messages, or small remaining fragments reveal the protected content. The architecture and proof-fingerprint decisions may choose mechanisms later; this PRD requires the outcome rather than a cryptographic construction.

Before egress, the candidate derivation is assessed against:

- the declared proof condition and eligible consumer;
- exposure or reconstruction risk for the relevant content class;
- residual sensitive fragments, metadata, or identifiers;
- source, campaign, mode, and disclosure isolation;
- the limitations needed to prevent a stronger claim than the derivation supports.

If egress admission fails or cannot be completed, the candidate remains sensitive and is disposed of with the raw material. The evidence or proof claim remains inconclusive or limited; a processing failure cannot trigger automatic recollection or repeat a target-side action.

Only the admitted opaque derivation, bounded non-sensitive provenance, safe disposition, and linked correction or incident references may persist under PRD-008 and PRD-009. Safe accountability metadata cannot include reversible content, secret-derived lookup material, unrestricted excerpts, or values that let an ordinary consumer retrieve the raw source.

## 8. Disposal, validation, and residual risk

Ending a process, dropping a reference, deleting a file, receiving a successful tool status, expiring a lease, losing target reachability, or issuing a cleanup command does not by itself prove disposal. Disposal verification establishes what operation completed; disposal validation determines whether the result is adequate for the content sensitivity, affected medium or surface, and accepted recovery threat.

The retained non-sensitive disposition distinguishes, without mandating an implementation enum:

- disposal completed and adequately validated for the declared assumptions;
- disposal attempted but validation is unavailable or inconclusive;
- sensitive residue or an affected replica is detected or credibly suspected;
- an affected surface is unavailable, externally controlled, or cannot be remediated under current authority;
- remediation is prohibited or deferred, with accountable residual risk and next action.

An unsuccessful or uncertain disposition cannot be relabeled as zero retention achieved. It blocks closure of the affected handling incident and any proof statement that depends on verified disposal, but it does not rewrite a historically valid technical result.

Later architecture must account for every surface the chosen boundary can actually affect, including memory, process artifacts, storage abstraction, replication, backup, telemetry, and crash behavior. The product does not require elaborate sampling after every successful disposal; assurance is proportional to sensitivity, medium, known anomalies, and accepted policy. Mechanisms and thresholds belong to ADR-011 and later contracts.

## 9. Late contamination and correction

When prohibited content is discovered outside the boundary:

1. stop further propagation, ordinary access, export, reasoning use, and proof release for the affected material;
2. retain only safe incident metadata under a stable logical incident reference and identify known affected surfaces, recipients, derivations, claims, proofs, and disclosures without echoing the content;
3. revoke or narrow current-use eligibility and issue linked evidence or proof corrections where required;
4. invoke the independent sensitive-remediation authority for affected copies and replicas;
5. validate the remediation outcome where possible and record unknown recipients, inaccessible replicas, and residual risk honestly.

The remediation path may remove or neutralize prohibited bytes from an affected store when separately authorized, even where ordinary accepted history is append-only. It must preserve a safe linked account that content existed, what decisions depended on it, and what remediation occurred. It cannot erase or falsify the historical action, use a campaign capability to alter authoritative audit evidence, or conceal a disclosure.

If prohibited bytes reached an authoritative audit system, backup, managed third-party service, or client recipient, DuskWeave cannot assume ordinary deletion is complete. The incident remains unresolved until the accountable remediation authority validates the applicable outcome or explicitly records the residual risk. Recipient acknowledgement or deletion request is not proof of recipient-side erasure.

Contamination blocks only affected data flow, evidence use, proof, and disclosure unless current safety, scope, or authority requires a broader freeze. Independent campaign work may continue without access to the affected material.

## 10. Authority, stopping, and completion semantics

- **Boundary unavailable or ineligible**: do not acquire client content; use lesser proof or report the claim as inconclusive.
- **Authorization absent, expired, or withdrawn**: block new admission immediately. Dispose of local boundary-held material without initiating a new target-side action; preserve uncertain external effects.
- **Selection bound exceeded**: stop intake, contain only inside the active boundary, and dispose or escalate safely.
- **Derivation failure**: dispose of sensitive inputs and intermediates; do not persist them for debugging or automatic retry.
- **Egress uncertainty**: withhold the output and narrow the dependent claim.
- **Disposal uncertainty**: open or retain the affected incident and report residual risk; do not claim verified zero retention.
- **External recipient or replica unavailable**: preserve safe accountability and the unresolved remediation requirement without inventing control over that surface.
- **Campaign termination**: finish only local sensitive disposal and safe incident handling that require no withdrawn target authority; target-side cleanup remains governed by PRD-001.
- **Safety freeze**: block affected handling and new egress immediately; resume only after current authority and boundary eligibility are re-established.

Engagement delivery authority cannot override failed sensitivity admission, unresolved contamination, or unsafe egress. Legal, privacy, records, and contractual authorities may further restrict handling; their absence or disagreement blocks the affected operation rather than being decided by DuskWeave.

## 11. Illustrative synthetic cases

### 11.1 Synthetic marker is sufficient

A predeclared synthetic canary proves read access to a synthetic record. The proof burden is satisfied without client content, so the campaign stops collection for that condition. No sensitive boundary is opened.

### 11.2 Minimum client-content derivation

Synthetic and metadata proof cannot establish a narrowly declared objective. Express authorization permits one bounded synthetic-like client test field to enter the boundary from a validated position. A target-specific opaque result is admitted, the field and intermediates are disposed of, and only the result, safe provenance, and validated disposition persist.

### 11.3 Unexpected credential in output

An authorized diagnostic unexpectedly returns a credential alongside eligible metadata. The credential is not admitted to Observation, EvidenceEnvelope, logs, operator display, or reasoning. It is disposed of inside the boundary; only its category, affected attempt reference, and safe disposition remain. Its presence does not authorize credential use.

### 11.4 Derivation crash

The derivation process terminates after receiving permitted minimum content and before egress. No candidate output is released. If the boundary can validate disposal of all held material, the claim remains inconclusive with a safe failed-attempt record. If not, a contamination incident remains open.

### 11.5 Late log contamination

A prohibited fragment is later found in an ordinary diagnostic sink. Propagation and dependent proof release stop. Safe references identify affected consumers; the independent remediation path addresses the bytes and any replicas while a linked correction preserves the historical account. A campaign capability never edits the audit record.

### 11.6 Backup or recipient uncertainty

A contaminated ordinary store may have entered a backup, or an invalid release may have reached a client recipient. A deletion request or primary-store cleanup does not prove all copies are gone. The incident records inaccessible surfaces and residual risk until accountable validation is available.

## 12. Observable acceptance criteria

1. Reviewers can distinguish a sensitive value, ephemeral boundary, approved opaque derivation, safe accountability metadata, contamination incident, and disposal assurance without creating a sixth operational model.
2. Raw sensitive material reaches no durable, cognitive, operator-facing, telemetry, retry, diagnostic, evidence, proof, or audit payload surface during successful, failed, interrupted, expired, cancelled, or withdrawn handling.
3. A proof attempt uses synthetic or non-sensitive evidence first, stops when sufficient, and admits only the minimum authorized client content when lesser proof is insufficient.
4. Access, objective approval, possession of a secret, or capability success cannot independently authorize client-content handling.
5. Every candidate output remains sensitive until purpose-bound egress admission establishes that it is non-sensitive and adequately resistant to reconstruction or arbitrary guess testing.
6. A hash, redaction, process exit, delete request, cleanup command, expiry, or tool-success result cannot by itself establish opaque proof or validated disposal.
7. No raw-data quarantine, debugging archive, training corpus, or speculative future-use store is created.
8. Boundary failure produces an inconclusive or limited claim rather than persistence, automatic recollection, or repeated target action.
9. Late contamination blocks affected propagation and disclosure, identifies downstream dependencies through safe references, and preserves linked corrections without copying prohibited content.
10. Sensitive remediation remains independent of campaign execution; when prohibited bytes contaminate an authoritative audit surface, only a separately authorized audit-preserving correction or remediation path may remove or neutralize those bytes while retaining safe linked accountability, and it cannot falsify, disable, or conceal authoritative audit history.
11. Disposal uncertainty, inaccessible replicas, backup exposure, and recipient-side uncertainty remain explicit residual risk rather than successful zero-retention claims.
12. Cross-engagement, cross-campaign, cross-mode, and third-party material remain isolated and require their own authority; similarity does not authorize reuse.
13. Runtime wiring, cryptography, storage, sanitization technique, and key management remain unselected until their ADRs and contracts are accepted.
14. PRD-007 Observation, PRD-008 EvidenceEnvelope, and PRD-009 ProofEnvelope receive only eligible non-sensitive semantics and retain their existing ownership.

## 13. Explicit non-goals and deferred work

This PRD does not define:

- database schemas, Rust/Go types, `Sensitive<T>` representation, crates, modules, APIs, serialization, service topology, or workflow engines;
- container, process, memory-locking, swap, core-dump, filesystem, queue, broker, telemetry, crash, backup, replication, or cloud-isolation mechanisms;
- encryption algorithms, proof fingerprints, engagement keys, key custody, cryptographic erasure, overwrite, purge, media destruction, or sanitization tooling;
- a universal data-classification engine, DLP platform, custody manager, privacy programme, records manager, evidence store, report manager, or incident-response suite;
- a durable raw-data quarantine, credential vault, secret-recovery service, debugging archive, client-data lake, model-training corpus, or reuse across engagements;
- permission to acquire passwords, private keys, authentication stores, bulk datasets, memory images, unrestricted target output, or any content outside an expressly authorized proof condition;
- payload behavior, collection commands, exfiltration channels, target-side staging, acquisition retry, client backup repair, recipient-side erasure, or target cleanup;
- legal interpretation, data-subject consent, breach-notification law, jurisdiction selection, contractual retention, client risk acceptance, or regulatory compliance certification;
- absolute future-proof irretrievability, guaranteed knowledge of every replica, or proof that an external recipient deleted data.

ADR-011 must choose the sensitive-data barrier architecture and remediation authority boundaries. ADR-010 and ADR-012 govern proof fingerprints and engagement proof keys only after their dependencies are accepted. ADR-009 must preserve safe accepted-history and correction semantics without retaining prohibited bytes. Domain contracts later define the applicable handling and failure contracts; runtime remains unauthorized.

---

**Authoring boundary:** PRD-010 is `PROPOSED`. It does not accept ADR-008..012, change the active `DW-FOUNDATION-001` seal, seal `DW-DOMAIN-001`, or authorize sensitive-data acquisition, runtime isolation, sanitization tooling, proof cryptography, capability execution, or Stage 5 implementation.
# PRD-009: Client Proof

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-009 |
| **Title** | Client Proof |
| **Status** | ACCEPTED |
| **Stage** | Stage 4 — Reality & Evidence |
| **Direct Dependency** | [PRD-008 Evidence Model](PRD-008-evidence.md) — ACCEPTED |
| **Target Seal** | DW-DOMAIN-001 |

## 1. Purpose and product outcome

DuskWeave must turn accepted campaign evidence into client-facing substantiation that is precise, reviewable, safe to disclose, and useful for remediation. A client should be able to understand what was demonstrated, against which in-scope subject, under what conditions, from which vantage and time, and what remains unproven.

Client proof is a bounded derivation from owner-qualified EvidenceEnvelopes. It does not replace the underlying evidence, accept an operational-model claim, recalculate campaign truth, or authorize another action. The same accepted evidence may support different client purposes only through separately bounded proof claims and disclosure decisions.

A **ProofEnvelope** binds one declared client proof claim to its eligible evidence basis, demonstrated result, limitations, disclosure context, and release disposition. It is not a raw evidence archive, universal report object, execution recipe, vulnerability scanner output, complete campaign history, sixth operational model, or guarantee that the observed condition still exists.

## 2. Vocabulary and boundaries

| Term | Product meaning | What it does not establish |
| :--- | :--- | :--- |
| **Client proof claim** | A narrow statement offered to a declared client audience about an accepted campaign result or limitation. | Broader access, impact, causality, control failure, or current validity beyond its stated bounds. |
| **Proof basis** | Owner-qualified EvidenceEnvelope references and permitted non-sensitive or opaque material selected to substantiate the client proof claim. | New evidence, inherited universal sufficiency, or permission to retrieve raw content. |
| **ProofEnvelope** | The bounded client-facing association among a proof claim, proof basis, result, context, limitations, review, and release disposition. | A report template, evidence store, owner state, or action authority. |
| **Reported finding** | A client-facing interpretation of one or more released proof claims as a security weakness, exposure, validated path, objective result, cleanup condition, or assessment limitation, kept under a stable client-facing reference with traceability to its supporting released proof revisions. | An automatically verified root cause, business impact, universal severity, or remediation priority. |
| **Proof view** | An audience-appropriate presentation derived from the same released ProofEnvelope semantics. | Permission to alter, omit, or strengthen the underlying claim. |
| **Release disposition** | The decision of the designated engagement delivery authority to release, withhold, supersede, or correct a proof for a declared audience and purpose. | Acceptance by the client, mutation of owner state, or authorization of campaign execution. |

A ProofEnvelope identity is distinct from an EvidenceEnvelope identity, owner-claim identity, finding identity, report revision, and delivery receipt. Each retained ProofEnvelope keeps a stable logical identity and linked revision lineage within its engagement and disclosure purpose; identifier and fingerprint mechanisms remain deferred. Each retained reported finding keeps a stable client-facing reference within its engagement and maps to the released ProofEnvelope revisions that currently or historically support it, including linked corrections, withdrawals, and retests. This traceability does not create a case manager, remediation tracker, or new owner of evidence or client risk. References preserve the ownership boundaries.

## 3. Actors, ownership, and release authority

- The **operational-model owner** remains authoritative for the accepted claim on which proof depends.
- The **proof assembler** selects eligible evidence and expresses a bounded client proof claim without broadening its source semantics.
- The **technical reviewer** inspects the basis, result, limitations, sensitivity, and reproducibility for the declared client purpose.
- The **designated engagement delivery authority** approves disclosure to a specific audience and channel within current engagement authority.
- The **client recipient** receives the released proof and may acknowledge, challenge, or request clarification; receipt is not agreement with the finding.
- The **campaign operator** may decide delivery priorities and timing but cannot use proof assembly or release to dispatch a target action.

A proof assembler, reviewer, or delivery authority cannot accept Terrain, Access, Path, Objective, or Trajectory state. Proof packaging cannot resolve contradictory evidence, make stale evidence current, transform a transient position into a foothold, or mark an objective complete.

Release sign-off is required only when proof crosses the client disclosure boundary. Drafting, technical review, correction, or withholding does not pause unrelated campaign work. Sign-off approves the bounded disclosure; it does not certify universal truth, erase limitations, replace the client's own risk decision, or override failed technical review, ineligible basis, unresolved contradiction, or sensitivity restrictions. One sign-off may cover a bounded set of proofs sharing the same audience, purpose, and disclosure conditions; this PRD does not require a separate gate for every envelope.

A component may support assembly, review, and rendering when their authorities remain distinct. No universal report manager may own evidence, findings, operational state, release authority, and delivery.

## 4. Required ProofEnvelope semantics

Every ProofEnvelope must make the following product meaning available where applicable:

1. **Engagement, campaign, and delivery purpose**: the isolated source context, authorized client purpose, intended audience, and disclosure boundary.
2. **Declared proof claim**: the narrow proposition being substantiated, its affected subject, and the accountable operational-model owner or owners of the accepted source claims on which it depends.
3. **Result and claim bounds**: what was demonstrated, partially demonstrated, not demonstrated, or remains inconclusive, without requiring those words to become an implementation enum.
4. **Eligible proof basis**: EvidenceEnvelope references and permitted non-sensitive, synthetic, safely transformed, or opaque material sufficient for the declared claim.
5. **Operational context**: relevant authorized action or assessment method, vantage or validated position, exercise mode, and material preconditions.
6. **Relevant times**: effect or observation time, evidence evaluation time, proof assembly/review time, and the as-of or validity limitation presented to the client.
7. **Technical reviewability**: enough safe detail for an authorized competent tester to understand and, where appropriate, validate the demonstrated exposure.
8. **Impact and interpretation boundaries**: verified technical consequence separated from inferred root cause, prospective business impact, severity assessment, and unverified extrapolation.
9. **Coverage and limitations**: tested and untested scope, unresolved contradictions, unavailable inputs, environmental constraints, and claims explicitly not made.
10. **Sensitivity and disclosure disposition**: redaction or transformation applied, any bounded safe reference, intended recipients, and restrictions on further disclosure.
11. **Review and release disposition**: accountable technical review outcome and final release decision for the declared audience and purpose.
12. **Stable proof identity, correction, and supersession**: stable logical identity for the retained proof and links among corrected, withdrawn, or superseding revisions without rewriting what was previously released.
13. **Actionable remediation and validation**: for a reported weakness or exposure, specific remediation direction proportionate to the verified result and a validation or retest condition. A concrete correction is stated only when the eligible basis establishes it; otherwise the proof states the intended control objective, the evidence or client input still needed, and a bounded validation question. This does not assert an unverified root cause or select the client's remediation priority.

These are product semantics, not a database schema, serialized type, PDF layout, portal workflow, cryptographic format, or mandatory field set for every audience. Later contracts may choose representations without weakening the meanings.

## 5. Formation and least-disclosure proof

A ProofEnvelope may be formed only from client-disclosure-eligible EvidenceEnvelopes linked to the accepted owner claim or claims needed for the declared proof. An accepted owner claim alone is not client proof. Raw observations, tool-native output, proposals, inferred attack paths, event delivery, generic execution success, and model-generated narratives cannot bypass evidence evaluation.

Proof formation is a derivation, not a new truth transition. The assembler preserves claim scope, provenance family, source dependence, time, mode, contradictions, and owner qualification. A proof may combine several eligible envelopes, but item count does not create corroboration and presentation order does not create causality.

Use the least disclosure that still makes the claim reviewable. Prefer concise non-sensitive technical material, synthetic markers, opaque target-specific derivations, and bounded safe references. Do not attach the complete evidence history merely because it exists.

Where validation by another authorized tester is appropriate, the proof provides bounded reproduction conditions and expected observable result. It need not disclose credentials, secrets, unrestricted target output, exploit payloads, persistence material, or exact commands that would exceed the client's authorized purpose. If safe detail is insufficient for review, the proof remains limited or unreleasable; a fingerprint alone is not an inspectable basis.

Technique or framework labels, including ATT&CK mappings, are descriptive context only. They cannot substitute for proof of the action, result, impact, control outcome, or objective.

## 6. Result, finding, and impact semantics

Proof reports the strongest claim actually supported, not the most severe plausible story. Successful tool execution is not proof of exploitability; exploitability is not proof of access; access is not proof of a complete path; a path is not proof of objective fulfillment; objective proof is not proof of business loss.

A reported finding keeps the following meanings distinct:

- the **verified technical result** supported by accepted evidence;
- the **assessed security consequence** within the demonstrated scope;
- any **inferred root cause**, labeled as inference unless separately established;
- any **prospective business impact**, labeled as assessment rather than observed fact;
- the **coverage and residual uncertainty** that constrain risk interpretation.

Severity or priority may be included when the engagement's accepted assessment method requires it. No universal score is mandatory, and a score cannot strengthen an unsupported claim. The client remains owner of business-risk acceptance and remediation priority.

Failed, prevented, or inconclusive actions may be reported when material to the client question. Failure does not prove that a named control caused prevention. Control-effect claims require the separately eligible evidence, coverage, observation window, and mode permitted by PRD-000 and PRD-008.

Campaign completion proof references the declared authorized scope and may summarize objectives reached, not reached, partially demonstrated, or not assessed; bounded coverage; unknown external effects; and residual uncertainty. It distinguishes subjects or objectives that were in scope but not attempted, in scope and attempted but inconclusive, and outside the authorized engagement boundary. Deadline expiry, termination, or lack of a finding is never represented as proof that no weakness exists.

## 7. Proof views, reports, and audience consistency

A technical view may emphasize basis, conditions, reproducibility, affected subject, and remediation validation. An executive view may emphasize demonstrated consequence, mission relevance, coverage, and residual risk. Both must preserve the same claim strength, result, scope, time, and limitations.

A report may organize multiple ProofEnvelopes approved for the same disclosure with narrative and prioritization, but the report is a projection rather than a new truth owner. One proof may appear in more than one approved view; tailoring language cannot remove a material limitation or convert inference into fact.

When the delivery purpose concerns an attack chain or campaign narrative, the report may compose approved ProofEnvelopes into a bounded narrative. Accepted AttackPathView and CampaignTrajectory claims may supply only ordering and context already substantiated by those proofs. The composition preserves demonstrated ordering, material temporal and positional dependencies, failed or inconclusive transitions, and limitations. It cannot admit an owner claim without eligible proof, infer a missing edge, convert a projected path into proof, or take ownership of Path or Trajectory truth.

Drafts are not released proof. A material change to claim, basis, result, audience, disclosure boundary, or limitation after sign-off creates a new accountable revision and requires release reconsideration. Formatting-only changes that preserve meaning do not create new evidence or owner state.

Delivery receipt records that an approved artifact reached its intended recipient where delivery confirmation is available. It does not prove that the client read, accepted, remediated, or agreed with the finding.

## 8. Sensitive material and late contamination

Raw credentials, password hashes, private keys, authentication stores, personal records, customer data, financial records, unrestricted target output, and other prohibited client material cannot enter a ProofEnvelope, proof view, report, export package, notification, delivery receipt, log, trace, diagnostic, or reasoning context.

A sanitized screenshot, bounded request/response pairing, synthetic marker, or opaque derivation may be released only when it was admitted under PRD-008 and remains non-sensitive for the intended audience. Redaction must not hide a material contradiction or make the basis appear stronger than it is.

If prohibited content or an invalid disclosure is discovered before release, release is blocked and only safe incident metadata is retained. If discovered after release, further delivery and ordinary access are revoked where possible, affected recipients and dependent findings are identified, a safe linked correction or withdrawal notice is prepared, and the incident remains unresolved under the future PRD-010/ADR-011 remediation path. The prohibited content is not copied into the notice.

Append-only proof accountability preserves safe semantics of assembly, review, release, correction, and withdrawal. It does not protect prohibited bytes from an accepted sensitive-remediation path. Campaign capabilities and proof packaging cannot alter authoritative audit evidence.

## 9. Time, correction, retest, and cleanup proof

Every released proof states the time or interval to which its claim applies. Historical proof may remain accurate for an earlier result while becoming unsuitable for a current-state claim. Delivery time does not refresh target reality.

A later evidence correction, owner-state change, authority change, or discovered contradiction triggers reconsideration of affected unreleased and released proof. A linked correction, withdrawal, or superseding revision preserves the earlier client-facing record without allowing it to override the newer disposition.

A retest produces a new result linked to the earlier proof and, where applicable, the stable client-facing finding reference. It does not overwrite the original, imply that remediation was continuous between tests, or turn an untested condition into fixed. Retest scope and method differences remain visible.

Cleanup proof for a temporary managed campaign artifact reports only its accepted opaque disposition and non-sensitive manifest reference under PRD-000 and PRD-001. Expiry, a cleanup command, or target unreachability is not verified removal. Residue or unverified cleanup remains explicit with bounded operator-facing remediation guidance.

## 10. Failure and stopping semantics

- **Missing or ineligible basis**: withhold or narrow the proof; do not fill gaps with narrative confidence.
- **Conflicting basis**: expose the conflict and block the affected claim or release until owner reconciliation permits a bounded result.
- **Stale basis**: present only the valid historical claim or require a separately authorized refresh; proof assembly cannot trigger acquisition.
- **Unavailable safe reference**: do not silently omit a required basis or replace it with a fingerprint.
- **Over-redaction**: mark the claim insufficient for the intended audience when reviewability is lost.
- **Release authority absent or withdrawn**: withhold new disclosure without altering accepted history or campaign state.
- **Delivery failure**: preserve the approved release and failed delivery outcome; bounded delivery retry cannot repeat a target action.
- **Late contamination or invalid disclosure**: apply Section 8, block propagation, issue only safe correction metadata, and defer prohibited-byte remediation to PRD-010/ADR-011.
- **Proof-generation failure**: does not repeat collection, exploitation, objective action, cleanup, or any other target-side effect.
- **Termination or deadline expiry**: release only the substantiated bounded outcome, coverage, cleanup disposition, and unresolved uncertainty.

A proof or report problem blocks only the affected disclosure. It does not freeze independent campaign work unless current safety or authority separately requires broader stopping.

## 11. Illustrative synthetic cases

### 11.1 Validated access proof

A validated synthetic foothold has accepted evidence for bounded execution, output retrieval, identity, privilege context, environment boundary, and stability. The ProofEnvelope releases a sanitized identity/result pairing and precise access claim. It does not include credentials or claim access to unrelated hosts.

### 11.2 Scanner result without validation

A scanner reports a likely vulnerability. No owner-qualified evidence demonstrates the claimed effect. After the signal observation itself receives owner qualification, a ProofEnvelope may report only the narrow informational or inconclusive observation that the scanner produced signal X from declared vantage V at time T, with its method limits. Exploitability remains unsubstantiated, the signal is not a confirmed finding, and any target-side validation remains a separately authorized action.

### 11.3 Proven path with bounded edges

Several accepted owner claims substantiate a path from external vantage to a synthetic objective. The proof identifies each demonstrated transition and any temporal dependency. A historical or hypothetical edge is not presented as proven, and the path does not imply broader network compromise.

### 11.4 Opaque objective proof

An authorized proof attempt derives an opaque target-specific marker from minimum client content inside the ephemeral sensitive boundary. The released proof references the approved derivation and non-sensitive provenance, not the source record, and states only the predeclared objective condition it satisfies.

### 11.5 Failed action and control claim

An action fails while a campaign-visible denial is observed. The proof reports the action and bounded denial result. It does not name a control as causal or claim successful prevention unless eligible control-effect evidence and coverage support that separate claim.

### 11.6 Retest and correction

A later retest no longer reproduces a previously released exposure. A linked retest proof records the new bounded result and method. The original remains historical; the new proof does not assert continuous remediation or absence outside the retested scope.

### 11.7 Cleanup with unresolved residue

Termination records an artifact manifest and target unreachability. The proof reports cleanup as unverified, includes bounded remediation guidance, and does not treat expiry as removal or expose target-side sensitive content.

### 11.8 Audience-specific views

A technical view contains safe validation conditions and remediation checks; an executive view summarizes the same demonstrated consequence and residual uncertainty. Neither view strengthens the claim or hides the tested-scope limitation.

## 12. Observable acceptance criteria

1. A client proof claim is distinguishable from Observation, EvidenceEnvelope, owner state, finding interpretation, report view, and delivery receipt.
2. Every ProofEnvelope binds a narrow client claim to eligible owner-qualified evidence, context, time, scope, limitations, review, and release purpose.
3. Proof formation cannot accept owner claims, resolve evidence conflicts, refresh stale reality, dispatch execution, or become a sixth operational model.
4. Safe reviewability is sufficient for an authorized competent tester to understand and, where appropriate, validate the result; a fingerprint alone cannot replace required technical basis.
5. Proof uses least disclosure and excludes raw sensitive material, unrestricted target output, credentials, exploit payloads, and persistence material.
6. Verified result, inferred root cause, prospective business impact, severity assessment, and client risk decision remain distinct; every reported weakness or exposure includes proportionate remediation direction and a validation or retest condition without overstating root cause.
7. Tool success, scanner output, ATT&CK mapping, event delivery, access, path, objective, control effect, and cleanup each retain their own proof burden.
8. Technical and executive views preserve the same claim strength, scope, time, result, and material limitations; an attack-chain or campaign narrative preserves accepted ordering, material dependencies, failed or inconclusive transitions, and limitations without owning Path or Trajectory truth.
9. Final release requires bounded engagement delivery authority, but assembly and review do not create a per-action or campaign-wide human gate, and delivery authority cannot override technical or sensitivity ineligibility.
10. Each retained proof has stable logical identity, and each retained reported finding has a stable client-facing reference mapped to its supporting released proof revisions; corrections, withdrawals, retests, and superseding revisions remain linked without rewriting prior released meaning.
11. Late contamination blocks propagation, preserves only safe accountability semantics, triggers recipient and dependent-finding assessment, and defers remediation mechanism to PRD-010/ADR-011.
12. Campaign completion references declared authorized scope and distinguishes in-scope not attempted, in-scope attempted but inconclusive, and outside the authorized engagement boundary while reporting objectives, cleanup disposition, unknown outcomes, and residual uncertainty without claiming that untested weakness is absent.
13. Delivery receipt does not imply client agreement, remediation, or acceptance of risk.
14. Proof assembly, rendering, release, correction, or delivery cannot repeat a target action or alter authoritative audit evidence.

## 13. Explicit non-goals and deferred work

This PRD does not define:

- database schemas, Rust/Go types, service topology, crates, modules, serialization, APIs, or workflow engines;
- proof identifier generation, fingerprints, hashes, signatures, engagement keys, cryptographic custody, or verification algorithms;
- PDF/HTML layout, report templates, dashboards, client portals, ticketing integrations, notification systems, or delivery transports;
- a universal report manager, evidence store, case-management system, remediation tracker, or client-risk register;
- a mandatory universal severity score, business-impact calculator, root-cause engine, or ATT&CK mapping algorithm;
- evidence acquisition, target-side reproduction, exploit generation, payload packaging, collection retry, or runtime execution;
- the sensitive-data barrier, purge, cryptographic erasure, backup remediation, recipient-side deletion, irretrievability, or residual-risk mechanism;
- client organizational approval, legal acceptance, risk acceptance, remediation ownership, or proof that a recipient acted;
- Observer/Grader control-gap reporting beyond the mode/source boundary already accepted; its full product semantics belong to later PRDs;
- permission to expose privileged defender data to blind campaign reasoning or to disclose any material beyond current engagement authority.

ADR-010 will choose proof-fingerprint architecture only after its dependencies are accepted. PRD-010 owns the remaining sensitive-data product semantics. Neither dependency is accepted or implemented by this proposal.

---

**Acceptance record:** The product owner explicitly accepted PRD-009 on 2026-09-29 after refinement of actionable remediation, stable client-facing finding traceability, bounded scanner-signal reporting, attack-narrative composition, and completion-scope distinctions. This acceptance does not accept ADR-008..010 or PRD-010, change the active `DW-FOUNDATION-001` seal, seal `DW-DOMAIN-001`, or authorize runtime, proof delivery infrastructure, acquisition tooling, capability execution, or Stage 5 implementation.

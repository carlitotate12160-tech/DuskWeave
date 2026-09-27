# PRD-005: Objective Loop

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-005 |
| **Title** | Objective Loop |
| **Status** | PROPOSED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | [PRD-004 Expansion Loop](PRD-004-expansion-loop.md) |
| **Target Seal** | DW-PRD-005 |

## 1. Purpose and actors

Define how a scoped campaign discovers concrete mission targets, validates their relevance, demonstrates authorized impact without retaining real client sensitive material, and reviews what to do next. Strategic goals are declared by the campaign operator; the concrete target may emerge only as terrain and positions change. A demonstrated action is not automatically an accomplished objective.

The operator defines success conditions, bounds, and permitted proof. A bounded objective reasoner compares target hypotheses and the least intrusive sufficient proof, including the option to gather more evidence or stop; its proposal states expected proof, missing evidence, and what would falsify the target claim. Deterministic authority checks scope and safety; an approved capability acts; evidence reconciliation determines what happened; a reviewer assesses objective claims from derived proof. The separate Observer evaluates defensive control outcomes under [PRD-000](PRD-000-product-thesis.md). In blind mode its privileged feed does not inform the active objective planner; a separately authorized defender-informed exercise follows PRD-000 INV-007.

## 2. Terms and ownership

- **Objective discovery** identifies a concrete, in-scope target relevant to a declared mission goal. A discovered asset is not proof of access.
- **Target validation** confirms that the target has the claimed role and that current healthy access and authorization support a bounded proof attempt. Relevance, reachability, and execution authority are separate questions.
- **Bounded evidence collection** first seeks sufficient metadata, synthetic content, canaries, or other non-sensitive proof. If these cannot prove the declared claim and explicit rules of engagement permit it, only the minimum necessary content from a scoped client resource may be downloaded into an isolated ephemeral sensitive boundary, solely to derive opaque proof. Raw content is discarded and never becomes a deliverable.
- **Staging** groups only approved non-sensitive or opaque proof for review; it is not a holding area for raw client material.
- **Approved proof or transfer** distinguishes an authorized transient download into the ephemeral proof boundary from outward delivery of evidence. Only synthetic material, derived opaque proof, and non-sensitive provenance may be staged, delivered, or reported; raw client content cannot be an operator-facing proof artifact.
- **Objective review** determines whether evidence satisfies the declared success condition, remains inconclusive, or establishes failure, then selects a bounded next posture.

ObjectiveState owns declared objectives, candidate targets, review status, and satisfaction claims. CyberTerrain owns accepted target-environment facts; FootholdGraph owns validated access; AttackPathView projects possible routes; CampaignTrajectory records the ordered decisions, attempts, and outcomes. None may infer ObjectiveState completion merely because an action returned success. Proof remains reviewable without giving the reasoner raw client material.

## 3. Objective progression and valid transitions

| From → to | Evidence needed | Inconclusive or adverse case |
| :--- | :--- | :--- |
| Declared goal → discovered target candidate | Reconciled, fresh terrain links a scoped asset to the mission goal | Unverified label or out-of-scope asset remains an unaccepted candidate |
| Candidate → validated target | Evidence confirms target role and permitted proof condition; access claim is separately current | Stale terrain or absent position suspends the target proof attempt |
| Validated target → attempted action | Bounded proposal passes deterministic authorization for that target and proof | Denial or safety concern prevents execution and is recorded as such |
| Attempt → execution outcome | Campaign-visible result and provenance show what ran, where, and under which bounded access | A timeout, ambiguous response, or tool return alone is inconclusive |
| Execution outcome → evidence of access | Reconciled, target-specific opaque proof confirms the limited claim; if ephemeral client content was used, authorization, scope, and proof derivation are reviewable without exposing that content | A generic success result, unbounded download, or failed proof transformation does not establish target access |
| Evidence of access → fulfilled objective | Proof satisfies the operator's declared success condition and approved demonstration boundary | Partial proof remains partial/inconclusive; no inferred fulfillment |
| Review → next posture | Current objective state, access health, terrain freshness, scope, and safety justify the decision | Unresolved proof or loss triggers reassessment rather than a false success |

Collection, staging, and proof need not be performed if a declared objective is already demonstrable with less intrusive approved evidence. Prefer synthetic or non-sensitive proof that satisfies the predeclared condition; if it does, stop further collection for that condition. A transient client-content download is a separate, expressly authorized proof option when lesser evidence is insufficient, with minimum scope and opaque proof derivation inside the sensitive boundary. If authorization is absent, the boundary cannot be assured, or proof remains insufficient, record the objective as inconclusive and escalate rather than broadening collection or declaring success. Each transition preserves the distinction between attempted action, execution success, access evidence, and objective fulfillment. A proof claim must specify what it demonstrates and what remains unproven.

## 4. Review and repeated discovery

Material changes in terrain, footholds, or scope cause objective discovery to be revisited. A newly observed target may have higher mission relevance, while a lost position can make an earlier target unreachable without negating historical proof. Mission progress is assessed against declared objectives, not the number of systems touched or footholds held. If the current position supplies sufficient proof, further expansion needs its own mission-relevant justification. Reconsidering an objective is not permission to cross new scope boundaries or to reuse stale access.

Review selects **continue** toward an authorized objective, **dwell** for deliberate tempo or an evidence window, **retask** within authorized mission bounds, **maintain** surviving validated access, or **re-enter** after loss under [PRD-003](PRD-003-access-and-footholds.md). It may also stop for safety, exhaustion, or scope restriction. Dwell does not manufacture evidence. Retasking that changes authorized goals requires fresh authorization; repeated inconclusive actions are bounded by a changed premise or escalation.

Raw credentials, customer records, financial data, and authentication stores never enter persistent proof, temporary files, caches, logs, traces, LLM context, crash diagnostics, or reports. Any authorized raw client-content download remains within the ephemeral sensitive boundary only for proof derivation and is discarded after success, failure, interruption, or withdrawn authorization. A boundary failure blocks the proof claim; it cannot be hidden as success. Authoritative audit evidence is not alterable by campaign capabilities. An Observer/Grader verdict does not establish objective success; objective proof requires the target-specific evidence above. In blind mode, privileged defender-internal feeds cannot become tactical feedback. A defender console legitimately reached through a validated, authorized campaign position yields only campaign-observed evidence, not automatic proof of the claimed objective or of what caused a control effect. Separately authorized defender-informed assessment or retest remains labeled and evaluated apart under PRD-000 INV-007.

## 5. Illustrative synthetic trace

A reconciled observation from validated synthetic Service B in [PRD-004](PRD-004-expansion-loop.md) suggests synthetic Asset C may satisfy a declared objective. A possible path toward Asset D adds no mission value, so it is deferred. C's role is validated within scope. An authorized proof attempt succeeds in executing but returns only a generic acknowledgment, so objective fulfillment remains inconclusive. A later approved synthetic marker yields target-specific opaque proof; review records the precise satisfied condition and stops further collection without retaining client content. In a separate synthetic scenario where the marker is insufficient, express authorization permits a minimal test-record transfer into the ephemeral boundary; the record is discarded after an opaque, target-specific proof is derived. If the boundary fails, the claim remains inconclusive and no raw record is retained. If the Service B position is lost before that proof, the review chooses re-entry or another scoped path instead of declaring success.

## 6. Observable acceptance criteria

1. For a synthetic target, a reviewer can separately identify discovery, validated role, approved action, execution outcome, evidence of access, and fulfillment against a predeclared condition.
2. Generic success without target-specific proof remains inconclusive; partial proof does not turn into full completion. Sufficient, least intrusive proof ends collection for that condition.
3. Material terrain or foothold change revisits target opportunities and allows continue, dwell, retask, maintain, re-enter, or stop under current authority.
4. A reviewer can distinguish synthetic proof from an expressly authorized minimal client-content download into an ephemeral boundary; on success, failure, or interruption, only opaque proof and non-sensitive provenance survive and raw content never appears on durable or operator-facing surfaces.
5. Audit integrity and PRD-000 INV-007 remain intact; blind objective proof and any separately authorized defender-informed control validation stay distinguishable.

## 7. Explicit non-goals

This PRD does not prescribe extraction methods, exfiltration channels, payloads, storage or network protocols, objective scoring algorithms, or control-gap grading. It does not authorize bulk extraction, indefinite raw-data access, delivery of raw client records, or using client data to obtain credentials. An ephemeral proof download is conditional and never a prerequisite when less intrusive evidence suffices. Cross-loop selection and escalation belong to [PRD-006](PRD-006-adaptation.md).

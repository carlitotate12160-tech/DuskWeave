# PRD-005: Objective Loop

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-005 |
| **Title** | Objective Loop |
| **Status** | ACCEPTED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | [PRD-004 Expansion Loop](PRD-004-expansion-loop.md) |
| **Target Seal** | DW-PRD-005 |

## 1. Purpose and actors

Define how a scoped campaign discovers concrete mission targets, validates their relevance, demonstrates authorized impact without retaining real client sensitive material, and reviews what to do next. Strategic goals are declared by the campaign operator; the concrete target may emerge only as terrain and positions change. A demonstrated action is not automatically an accomplished objective.

The operator defines success conditions, bounds, and permitted proof. A bounded objective reasoner compares target hypotheses and the least intrusive sufficient proof, including the option to gather more evidence or stop; its proposal states expected proof, missing evidence, and what would falsify the target claim. Deterministic authority checks scope and safety; an approved capability acts; evidence reconciliation determines what happened; a reviewer assesses objective claims from derived proof. The separate Observer evaluates defensive control outcomes under [PRD-000](PRD-000-product-thesis.md). In blind mode its privileged feed does not inform the active objective planner; a separately authorized defender-informed exercise follows PRD-000 INV-007.

## 2. Terms and ownership

- **Objective discovery** identifies a concrete, in-scope target relevant to a declared mission goal. A discovered asset is not proof of access. Discovery may originate from deliberate terrain analysis or from provisional observations obtained during exploitation-mode expansion (PRD-004 §4), but provisional terrain alone does not validate a target.
- **Target validation** confirms that the target has the claimed role and that current healthy access and authorization support a bounded proof attempt. Relevance, reachability, and execution authority are separate questions. Target role claims on Key Terrain entities require Tier 3 epistemic confidence (PRD-002 §5.3).
- **Bounded evidence collection** first seeks sufficient metadata, synthetic content, canaries, or other non-sensitive proof. If these cannot prove the declared claim and explicit rules of engagement permit it, only the minimum necessary content from a scoped client resource may enter an isolated ephemeral sensitive boundary, solely to derive opaque proof. The isolation and ephemerality are product requirements under PRD-000 INV-005; their implementation mechanism is an architecture decision. Raw content never persists and never becomes a deliverable.
- **Staging** groups only approved non-sensitive or opaque proof for review; it is not a holding area for raw client material.
- **Approved proof or transfer** distinguishes an authorized transient acquisition of client content for proof derivation from outward delivery of evidence. Only synthetic material, derived opaque proof, and non-sensitive provenance may be staged, delivered, or reported; raw client content cannot be an operator-facing proof artifact.
- **Objective review** determines whether evidence satisfies the declared success condition, remains inconclusive, or establishes failure, then selects a bounded next posture.

ObjectiveState owns declared objectives, candidate targets, review status, and satisfaction claims. CyberTerrain owns accepted target-environment facts; FootholdGraph owns validated access; AttackPathView projects possible routes; CampaignTrajectory records the ordered decisions, attempts, and outcomes. None may infer ObjectiveState completion merely because an action returned success. Proof remains reviewable without giving the reasoner raw client material.

## 3. Objective progression invariants

The following invariants govern objective claims regardless of operational tempo. Domain contracts own the specific transitions and evidence evaluation mechanisms.

**Invariant O-1: Discovery is not fulfillment.** Discovering a target asset does not establish access, and access does not establish objective completion. Each stage requires its own evidence.

**Invariant O-2: Target validation requires current access.** A proof attempt against a target requires a currently healthy, validated foothold with appropriate authorization. Stale, uncertain, or presumed-lost access suspends the objective attempt.

**Invariant O-3: Least intrusive sufficient proof.** Prefer synthetic, non-sensitive, or metadata-based evidence that satisfies the predeclared success condition. If sufficient, stop further collection for that condition. Client content acquisition is a conditional, expressly authorized option only when lesser evidence is insufficient.

**Invariant O-4: Raw client material never persists.** Any authorized transient acquisition of client content is confined to an isolated ephemeral sensitive boundary, processed only to derive opaque proof, and discarded after success, failure, interruption, or withdrawal of authorization. The implementation mechanism for that boundary is not prescribed by this PRD; the requirement is that raw content never appears on durable storage, logs, caches, traces, reasoning context, crash diagnostics, reports, or operator-facing surfaces.

**Invariant O-5: Inconclusive is not success.** A generic success result, ambiguous response, timeout, or tool return without target-specific proof does not constitute objective fulfillment. Partial proof remains partial. Repeated inconclusive attempts require a changed premise, alternative approach, or escalation.

**Invariant O-6: Proof specifies its scope.** Every proof claim must state what it demonstrates and what remains unproven. A proof of read access to a database table does not prove write access or access to other tables.

## 4. Objective work and operational tempo

Objective proof requires a validated foothold — not a transient position or provisional observation. This is a firm boundary: exploitation mode (PRD-003 §4.2) permits fast observation and parallel validation, but evidence collection for objective fulfillment occurs only from validated positions.

However, exploitation-mode observations may accelerate objective *discovery*. Provisional terrain from a fast expansion may reveal a target asset earlier than deliberate-mode observation would. The discovery is legitimate; the proof attempt waits for position validation.

When a validated position supports an objective proof attempt during exploitation mode, chain execution of the proof sequence is permitted if each action was independently authorized and the chain has a defined stopping condition. Post-chain reconciliation determines whether the proof satisfies the declared success condition.

## 5. Review and repeated discovery

Material changes in terrain, footholds, or scope cause objective discovery to be revisited. A newly observed target may have higher mission relevance, while a lost position can make an earlier target unreachable without negating historical proof. Mission progress is assessed against declared objectives, not the number of systems touched or footholds held. If the current position supplies sufficient proof, further expansion needs its own mission-relevant justification. Reconsidering an objective is not permission to cross new scope boundaries or to reuse stale access.

Review selects **continue** toward an authorized objective, **dwell** for deliberate tempo or an evidence window, **retask** within authorized mission bounds, **maintain** surviving validated access, or **re-enter** after loss under [PRD-003](PRD-003-access-and-footholds.md). It may also stop for safety, exhaustion, or scope restriction. Dwell does not manufacture evidence. Retasking that changes authorized goals requires fresh authorization; repeated inconclusive actions are bounded by a changed premise or escalation.

Raw credentials, customer records, financial data, and authentication stores never enter persistent proof, temporary files, caches, logs, traces, LLM context, crash diagnostics, or reports. An Observer/Grader verdict does not establish objective success; objective proof requires the target-specific evidence above. In blind mode, privileged defender-internal feeds cannot become tactical feedback. A defender console legitimately reached through an authorized campaign position yields only campaign-observed evidence, not automatic proof of the claimed objective or of what caused a control effect. Separately authorized defender-informed assessment or retest remains labeled and evaluated apart under PRD-000 INV-007.

## 6. Illustrative synthetic trace

### Deliberate objective work
A reconciled observation from validated synthetic Service B in [PRD-004](PRD-004-expansion-loop.md) suggests synthetic Asset C may satisfy a declared objective. A possible path toward Asset D adds no mission value, so it is deferred. C's role is validated at Tier 2 within scope. An authorized proof attempt succeeds in executing but returns only a generic acknowledgment, so objective fulfillment remains inconclusive. A later approved synthetic marker yields target-specific opaque proof; review records the precise satisfied condition and stops further collection without retaining client content.

### Exploitation-mode discovery
During exploitation-mode expansion through synthetic Service E, provisional observations reveal synthetic Asset F as potentially mission-relevant. The discovery is recorded but the proof attempt waits: F's role needs Tier 2 corroboration and the originating position needs validation. Once E's position is validated and F's role is corroborated, the operator assesses whether to attempt proof in exploitation mode (time-sensitive) or return to deliberate mode. If time-sensitive, an authorized proof chain executes from the now-validated E position.

### Sensitive material scenario
In a separate synthetic scenario where a synthetic marker is insufficient, express authorization permits a minimal test-record acquisition into an isolated ephemeral sensitive boundary; the record is discarded after opaque, target-specific proof is derived. If the boundary process fails, the claim remains inconclusive and no raw record is retained. If the Service B position is lost before proof, review chooses re-entry or another scoped path instead of declaring success.

## 7. Observable acceptance criteria

1. For a synthetic target, a reviewer can separately identify discovery, validated role, approved action, execution outcome, evidence of access, and fulfillment against a predeclared condition.
2. Generic success without target-specific proof remains inconclusive; partial proof does not turn into full completion. Sufficient, least intrusive proof ends collection for that condition.
3. Material terrain or foothold change revisits target opportunities and allows continue, dwell, retask, maintain, re-enter, or stop under current authority.
4. A reviewer can confirm that raw client content, when authorized and used, never appears on durable or operator-facing surfaces regardless of success, failure, or interruption.
5. Provisional observations may accelerate target discovery but cannot serve as objective proof; proof attempts require validated access.
6. Audit integrity and PRD-000 INV-007 remain intact; blind objective proof and any separately authorized defender-informed control validation stay distinguishable.

## 8. Explicit non-goals

This PRD does not prescribe extraction methods, exfiltration channels, payloads, storage or network protocols, objective scoring algorithms, control-gap grading, or the specific implementation of sensitive material isolation boundaries. It does not authorize bulk extraction, indefinite raw-data access, delivery of raw client records, or using client data to obtain credentials. Cross-loop selection and escalation belong to [PRD-006](PRD-006-adaptation.md).

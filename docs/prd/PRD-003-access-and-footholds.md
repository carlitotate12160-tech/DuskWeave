# PRD-003: Access & Footholds

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-003 |
| **Title** | Access & Footholds |
| **Status** | ACCEPTED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | [PRD-002 Cyber Terrain](PRD-002-cyber-terrain.md) |
| **Target Seal** | DW-PRD-003 |

## 1. Purpose and actors

Define when an authorized campaign can claim an operational position, how it detects degradation, and how it may regain access after loss. This document develops the Access Loop of [PRD-001](PRD-001-campaign-lifecycle.md) from terrain knowledge in PRD-002. Its outcome is credible access continuity, never access inferred from an optimistic tool result.

The campaign operator sets mission scope and permitted objectives; a bounded access reasoner proposes candidates; deterministic authority decides whether a proposed action is authorized; a capability performs an approved action; evidence reconciliation determines what can be claimed. A reviewer can inspect the resulting proof without receiving raw client material. No actor receives authority merely by proposing a route.

Real-world adversaries operate with variable tempo: patient and deliberate during approach, fast and decisive when an opportunity window opens. Access semantics must accommodate both deliberate validation and exploitation-tempo action without sacrificing safety invariants.

## 2. Terms and model ownership

- **Candidate access** is a plausible route to a scoped target. It belongs to AttackPathView as a hypothesis, with its terrain premises and freshness; it is not a position.
- **Transient access** is a short-lived, observed ability to perform a bounded action on a target after an authorized attempt. Its exact context, repeatability, and stability remain unverified; it establishes no foothold. In exploitation tempo, a transient position may serve as an observation platform for read-only situational awareness (provisional observation under §4), but it cannot authorize state-changing actions or expansion.
- **Validated foothold** is an addressable position with evidence of reliable bounded execution, output retrieval, exact identity and privilege context, relevant environment boundary, and operational stability. FootholdGraph owns it and the health of that position.
- **Foothold health** is an assessment derived from separate questions: is the position validated or affirmatively lost; how fresh is its proof; is that proof corroborated, inconclusive, conflicted, or refuted; and is the bounded capability usable, degraded, or unavailable? Health does not enlarge privileges. Stale proof and conflicting proof are different conditions, and both can apply at once.
- **Presumed loss** is an operational determination that a position is no longer usable based on sustained unreachability exceeding a configurable threshold, without requiring affirmative proof of revocation. It suspends dependent actions immediately but permits faster recovery than confirmed loss if contact resumes.
- **Confirmed loss** is a reconciled determination based on affirmative evidence (explicit denial, credential revoked, host decommissioned) that access is no longer available. Re-entry requires fresh validation under full access progression.
- **Re-entry eligibility** is a current, conditional assessment that an authorized route could re-establish a lost position; eligibility never asserts recovered access.
- CyberTerrain owns accepted environmental entities and relationships; it does not own campaign access. AttackPathView projects candidate routes. ObjectiveState owns objective progress. CampaignTrajectory records decisions, attempts, validations, loss, and re-entry outcomes in time order; it is not a substitute for current access.

A foothold's validated identity, privilege, target, and available capability bounds matter as much as its location. A route through the same host under a different identity is a distinct claim requiring validation. Provenance and time constrain every claim; a historical foothold does not remain healthy by default. A candidate belongs to AttackPathView, transient access is an unvalidated observation, and a lost position is a historical foothold claim: these are not interchangeable health states. A fresh but conflicted position is uncertain and requires reconciliation; an otherwise corroborated position with expired proof is stale and requires refresh. Neither condition alone establishes loss.

## 3. Access evidence invariants

The following invariants govern access claims regardless of operational tempo. They state what must be true, not the implementation mechanism for tracking or transitioning between conditions. Domain contracts own the specific states, transitions, and evidence evaluation rules that satisfy these invariants.

**Invariant A-1: No access without evidence.** A foothold cannot be claimed from a hypothesis, a candidate path, a tool's exit code, an inferred privilege, a discovered identity, or reachability alone. Validated access requires reconciled evidence of reliable bounded execution, output retrieval, exact identity and privilege context, and operational stability.

**Invariant A-2: Transient is not a foothold.** A short-lived observed effect does not become a validated position without explicit validation that satisfies A-1. A transient position may support bounded provisional observation (§4) but cannot serve as the origin for follow-on state-changing actions, expansion, or objective work. An expressly authorized initial access attempt and bounded validation may themselves involve reversible state changes under PRD-000; they do not require a foothold that does not yet exist.

**Invariant A-3: Stale is not lost; lost is not stale.** Expired freshness means proof needs refresh, not that access is gone. Confirmed loss requires affirmative evidence of unavailability. Sustained unreachability beyond a configurable operational threshold constitutes presumed loss, which suspends dependent actions without requiring affirmative revocation evidence.

**Invariant A-4: Re-entry requires fresh validation.** A lost position cannot be reclaimed by historical evidence. Re-entry eligibility is a hypothesis; only renewed validation under current bounds can establish a recovered position. Recovery from presumed loss (contact resumes) may use an abbreviated validation when the position context has not changed; recovery from confirmed loss requires full access progression.

**Invariant A-5: Identity and privilege scope bind the claim.** A validated position is bound to its specific identity, privilege, target, and capability. A different identity on the same host is a separate claim. Privilege escalation within a position is a new claim requiring its own evidence.

**Invariant A-6: Contradictory evidence requires reconciliation.** When fresh evidence conflicts with a current access claim, the claim enters an uncertain state. No dependent actions may proceed until reconciliation resolves the uncertainty toward healthy, stale, presumed lost, or confirmed lost.

**Invariant A-7: Durable proof excludes raw sensitive material.** All persistent access evidence uses opaque derived material. Raw secrets, credentials, and authentication stores never enter persistent storage, logs, reasoning context, or operator-facing surfaces. Audit integrity and the mode-specific Defender Knowledge Boundary in PRD-000 INV-007 remain intact during validation and recovery.

## 4. Dual-speed operational model

Real operators — including the three tradecraft profiles in PRD-000 §4.6 — do not apply identical deliberation to every action. They are patient during approach and reconnaissance, but move with decisive speed when an opportunity window opens. DuskWeave access semantics operate in two complementary tempo modes.

### 4.1 Deliberate mode (default)

Used during approach, planning, strategic assessment, and steady-state operations.

- Full access progression: candidate → authorized attempt → transient → validated foothold
- Validation is serial: each step completes before the next begins
- Epistemic confidence follows PRD-002 tier requirements without relaxation
- Health checks and freshness follow standard cadence
- All invariants (A-1 through A-7) apply without modification

### 4.2 Exploitation mode (opportunity window)

Used when a campaign identifies a time-sensitive opportunity: a discovered vulnerability, a valid credential about to expire, a maintenance window, or a newly exposed service. The transition to exploitation mode is a reasoned campaign decision, not an automatic escalation.

In exploitation mode:

- **Provisional observation from transient position**: Read-only situational awareness commands (native environment queries, identity checks, network enumeration) may execute from a transient position before foothold validation completes. Results enter CyberTerrain as PROVISIONAL (PRD-002 §3.1) and are subject to automatic downgrade if the position fails validation.
- **Parallel validation**: Foothold validation proceeds concurrently with provisional observation rather than blocking it. The reasoner does not wait for validation to begin understanding the environment.
- **Chain execution**: A pre-assessed, bounded chain may execute without per-step reasoning or full reconciliation pauses when each link remains subject to deterministic scope and safety authority at dispatch. Stop the chain if an unexpected result changes a link's target, identity, scope, safety premise, or validated origin; a transient new position permits only the bounded provisional observations above. Record each outcome for post-chain reconciliation.
- **Bounded transient retry**: Network-level transient failures (timeout, connection reset, brief service unavailability) may be retried within a configurable window (N attempts in T duration) without requiring a changed premise. Persistent failures (authentication denied, scope violation, capability not found) require a changed premise or renewed authorization.

**What remains blocking in exploitation mode:**

- Deterministic scope and safety checks gate every action, including provisional observations
- Follow-on state-changing actions originating from a transient position require a validated foothold; the initial authorized access attempt and bounded validation remain permitted under deterministic scope, safety, and recovery bounds
- Expansion to a new host, identity, or trust boundary requires validated access at the origin
- Objective work and evidence collection require validated access
- All invariants (A-1 through A-7) apply without exception

### 4.3 Provisional observation lifecycle

Observations obtained from a transient position carry a PROVISIONAL tag:

1. **Acceptance**: Enters CyberTerrain as a sourced PROVISIONAL observation with origin position reference, not as an established terrain fact or a premise for dependent action
2. **Promotion**: If origin position achieves validated foothold status, provisional observations are reconsidered through tier-aware reconciliation; valid narrow claims may become OBSERVED, while consequential claims still need their required independent support
3. **Downgrade**: If origin position fails validation or is abandoned, all provisional observations from that origin are downgraded to HYPOTHETICAL
4. **Scope**: Provisional observations may inform hypothesis formation and candidate selection. They cannot be the sole basis for campaign-critical actions, objective claims, or Key Terrain identification

### 4.4 Tempo mode transitions

Exploitation mode is entered by a reasoned decision that identifies the opportunity, its time-sensitivity, and its bounded scope. It is not a permanent state; the campaign returns to deliberate mode once the opportunity window closes, the immediate objective is achieved, or consolidation begins. CampaignTrajectory records each tempo transition with its justification.

## 5. Survivability, loss, and decisions

Survivability means tracking alternate authorized routes, validating current position health when needed, and preserving enough non-sensitive provenance to choose a safe recovery path after pause or loss. It does not require implants, covert persistence, or retention of raw credentials. Campaign state and authorized re-entry remain the default continuity mechanisms.

An explicitly authorized temporary managed access artifact may support a bounded position only through a curated capability with declared identity, target/scope, capability limits, lease/expiry, non-sensitive manifest, revocation, cleanup plan, and opaque cleanup evidence. Its presence does not by itself validate a foothold, widen capability, or make historical access current; Access still requires A-1 evidence and current authority. A reasoner cannot author or deploy an arbitrary binary. Expiry stops eligibility but is not proof of removal; residue or unverified cleanup remains visible for operator remediation and cannot be concealed by altering audit evidence.

A healthy alternate foothold may sustain the campaign while another becomes stale; the failed position cannot be used as an execution origin.

### 5.1 Operational loss threshold

Prolonged unreachability does not require affirmative proof of revocation to trigger operational consequences. When a position fails N consecutive health checks within window T, or sustains no successful contact within duration D (both configurable per engagement), it enters presumed loss:

- Dependent actions are immediately suspended
- The position cannot serve as an execution origin
- Re-entry eligibility is assessed against alternate routes
- If contact resumes and the position context is unchanged, abbreviated validation may restore health without full access re-progression
- If affirmative evidence of revocation arrives, presumed loss escalates to confirmed loss, requiring full re-entry validation

### 5.2 Loss and re-entry

A lost position — whether presumed or confirmed — triggers reassessment of dependent paths and objective opportunities. Re-entry may use an existing alternate position or a fresh candidate after terrain reconciliation. The operator may pause, maintain other positions, retask, or stop if scope or safety no longer permits recovery. Attempts remain bounded; repeated failures do not authorize infinite retries or expanded scope. Loss is not automatically campaign failure.

An access denial visible to the campaign may inform health reconciliation without proving which control caused it. Defender telemetry obtained through an authorized campaign position is campaign-observed evidence subject to reconciliation, scope, provenance, and sensitive-data rules; it does not by itself prove access health. In blind mode, privileged defender-internal feeds and Observer/Grader verdicts cannot steer Access; a separately authorized defender-informed exercise follows PRD-000 INV-007 and remains distinctly labeled. Campaign capabilities cannot alter authoritative audit evidence, and all durable access proof uses opaque derived material; raw secrets remain outside persistence, logs, and reasoning context.

## 6. Illustrative synthetic trace

### Deliberate mode trace
A current terrain observation suggests a scoped route to synthetic Host A; AttackPathView records a candidate. One authorized bounded action has an observed effect but does not reveal a reliable privilege context, so access remains transient. A separate bounded validation confirms context and stable output; FootholdGraph gains a validated position. After its freshness window expires, the position is stale without a claim of loss. A fresh but conflicting check makes its usability uncertain; neither condition permits dependent action. Reconciliation of affirmative evidence then confirms loss. An alternate scoped route becomes re-entry eligible, and only renewed validation can restore a position.

### Exploitation mode trace
During reconnaissance, the campaign discovers that synthetic Host B has a service with a newly exposed configuration weakness. The reasoner assesses this as time-sensitive (the maintenance window may close), enters exploitation tempo, and proposes an authorized attempt. The attempt succeeds — transient access established. Without waiting for full validation, the operator immediately executes read-only commands: identity check, network interfaces, local services. These observations enter terrain as PROVISIONAL. In parallel, foothold validation runs: the position demonstrates repeatable execution, stable context, and bounded capability. Validation succeeds; provisional observations are promoted to OBSERVED. The campaign returns to deliberate mode for consolidation. If instead the position had proved unstable, all provisional observations would have been downgraded to HYPOTHETICAL.

### Operational loss trace
Synthetic Host A has been validated but receives no successful contact for a duration exceeding the configured operational threshold. It enters presumed loss. Dependent expansion paths through A are immediately suspended. The campaign continues operating from independently healthy Host B. Two hours later, contact with A resumes; abbreviated validation confirms the same identity, context, and capability. A returns to healthy without full re-entry progression. In a separate scenario, an explicit denial from A arrives; the position enters confirmed loss, and recovery requires fresh candidate assessment and full re-entry validation.

## 7. Observable acceptance criteria

1. Reviewers can distinguish a candidate, transient access, validated foothold, stale position, presumed loss, confirmed loss, and re-entry eligibility for the same synthetic target without conflating their evidence.
2. A successful action without exact identity/privilege or stable bounded execution cannot become a foothold; a candidate path cannot authorize expansion.
3. Freshness expiry, contradictory current evidence, sustained unreachability, and affirmative access denial produce distinct assessments; dependent actions respond appropriately to each.
4. Provisional observations from transient positions carry explicit origin references and are automatically promoted or downgraded based on origin validation outcome.
5. Exploitation mode permits parallel validation and provisional observation while maintaining scope, safety, and state-change restrictions; the decision to enter and exit exploitation mode is recorded with justification.
6. An alternate position can sustain continuity and a lost position can regain access only through fresh authorization and validation appropriate to the loss type.
7. Durable proof excludes raw sensitive material; audit integrity and the mode-specific Defender Knowledge Boundary in PRD-000 INV-007 remain intact during validation and recovery.

## 8. Explicit non-goals

This PRD does not select credentials, artifact formats, implants, persistence techniques, validation commands, tool adapters, storage schemas, or runtime protocols. Temporary managed access remains an optional, explicitly authorized later capability and cannot become a hidden prerequisite for Access. It does not classify control gaps or infer access from vulnerability severity. It does not define specific state names, enumeration types, or transition implementation mechanisms; those belong to domain contracts that satisfy the invariants in §3. Expansion behavior and objective fulfillment belong to the following PRDs. Configurable thresholds (health check cadence, operational loss timeout, exploitation retry window) are engagement parameters, not PRD constants.

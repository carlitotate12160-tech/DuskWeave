# PRD-006: Adaptation

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-006 |
| **Title** | Adaptation |
| **Status** | ACCEPTED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | [PRD-005 Objective Loop](PRD-005-objective-loop.md) |
| **Target Seal** | DW-PRD-006 |
| **Amendment** | R2 — cognitive direction approved by the product owner on 2026-10-07 (evidence-led reasoning, capability feedback/evolution); published as a candidate amendment, exact-text acceptance recorded separately |

## 1. Purpose and actors

Define how a persistent authorized campaign reevaluates goals, access, expansion, and objective work when evidence or authority changes. Adaptation overlays the Strategic, Access, Expansion, and Objective loops in [PRD-001](PRD-001-campaign-lifecycle.md); it is neither a fifth sequential phase nor a permission to improvise execution. It sustains useful progress while keeping uncertainty, retries, and scope bounded.

The campaign operator owns mission priorities, authorization, and retasking that changes approved goals. Bounded reasoners propose alternative decisions for their own loop using reconciled campaign evidence and, only in a separately authorized defender-informed exercise, bounded defender feedback tagged by its source and exercise mode; deterministic authority checks any action against current scope and safety; capabilities perform only approved actions; evidence reconciliation decides what changed. A reviewer can inspect the decision trail. The separate Observer/Grader evaluates defender response; its privileged feed stays outside blind planning. Separately authorized defender-informed validation follows PRD-000 INV-007.

Adaptation is tempo-aware: the speed and rigor of reassessment match the current operational mode. In exploitation mode, adaptation decisions may execute faster with bounded validation; in deliberate mode, adaptation follows full epistemic rigor.

## 2. Decision inputs and model ownership

Adaptation considers reconciled environmental changes from CyberTerrain (at their appropriate epistemic tier per PRD-002 §3.3); health, presumed loss, and confirmed loss of validated positions from FootholdGraph; stale, blocked, or newly plausible candidates from AttackPathView; target opportunities and proof status from ObjectiveState; and the time-ordered decision and outcome record in CampaignTrajectory. These remain separate models. The trajectory documents what happened; it does not convert old evidence into a currently valid foothold or terrain fact.

A trigger may be a fresh campaign-visible control or denial, new key terrain, expired premises, contradictory observations, loss of a position (presumed or confirmed), an inconclusive objective, a change in authorized scope, or a deliberate tempo boundary. A separately authorized defender-informed exercise may additionally use bounded defender feedback as a labeled trigger; it must not be reported as blind campaign evidence. An observation is first reconciled at its appropriate epistemic tier; an inference remains a hypothesis. A scope reduction takes effect before any newly proposed action. Added scope or changed goals require explicit operator authorization and do not retroactively justify previous attempts.

### 2.1 Explainable reasoning episode

Each material decision must be explainable from campaign-visible observation and reconciled evidence, plus any explicitly authorized and labeled defender-informed feedback, through a hypothesis, plausible alternatives (including observe more, refresh, dwell, or stop), the selected proposal, expected evidence and disconfirming result, authorization, observed outcome, and subsequent revision of the affected models. The reasoner may choose no target action when evidence is insufficient. This is a bounded decision process, not a sixth operational model or an autonomous source of truth. A proposal describes intent and its evidence test; it grants no execution authority and does not prescribe a tool sequence.

The decision process is evidence-led and cognitive: the reasoner itself proposes hypotheses, compares alternatives, identifies missing information, chooses the next proposed action and adapts after outcomes. Deterministic mechanisms retain authorization, bounds, schema validation, current-use checks, execution accounting and owner-specific reconciliation; they may reject a proposal but must not silently preselect every action through a fixed ranking and then ask the reasoner to narrate the choice. A deterministic selector may remain a test control or an explicitly labeled non-cognitive operating mode; it cannot satisfy this requirement. When no admitted capability can address a recognized problem, the reasoner may propose a candidate implementation, which reaches dispatch only through the isolated build, qualification and admission lane of the owning capability and execution contracts; a new algorithm is distinct from a new permission.

In blind mode, privileged defender-internal EDR/AV/SIEM verdicts, SOC tickets, and Observer/Grader assessments remain outside decision inputs. Campaign-visible environmental friction may guide bounded adaptation without automatic attribution. Defender telemetry legitimately observed through an authorized campaign position follows observation/fact reconciliation at the appropriate epistemic tier and can inform a decision within its actual evidentiary limits. A separately authorized defender-informed exercise or retest may use bounded feedback, but is labeled and evaluated independently of blind results under [PRD-000 INV-007](PRD-000-product-thesis.md).

## 3. Cross-loop routing

| Loop revisited | Trigger and bounded response | Condition for progress |
| :--- | :--- | :--- |
| Strategic | Changed authorized mission, materially altered target value, exhausted avenues, or safety concern → reprioritize, request authorization, dwell, or stop | Current mission authority supports the revised goal and bounds |
| Access | Candidate ingress closes, health expires, position enters presumed or confirmed loss → reassess route, validate surviving position, or consider eligible re-entry under [PRD-003](PRD-003-access-and-footholds.md) | Fresh validation establishes usable access; eligibility alone is insufficient |
| Expansion | Fresh terrain changes a trust hypothesis, a path fails, or a position is lost → refresh premises, choose another scoped candidate, or return to Access under [PRD-004](PRD-004-expansion-loop.md). In exploitation mode, provisional observations may reveal new candidates faster. | Authorized attempt and validation establish any new position |
| Objective | Target location changes, proof is partial, or access to target is lost → revisit discovery, select a safer proof, dwell, retask, or re-enter under [PRD-005](PRD-005-objective-loop.md) | Target-specific accepted proof satisfies the declared condition |

A material change may cause several loops to reassess in either order dictated by dependencies. A loss can immediately suspend dependent actions while strategic priorities are reviewed; a new objective opportunity can redirect expansion. No loop may promote another model's hypothesis or attempted action into its own verified state.

### 3.1 Tempo-aware adaptation

Adaptation routing respects both the current operational tempo and the operator-authorized engagement envelope. Rapid validation prioritizes time-to-proof without weakening evidence or dispatch checks. A bounded campaign may branch, pause, re-enter, and pursue several objectives within its deadline. A persistent campaign may retain state through inactive intervals and reactivate only on an authorized schedule, operator retask, or eligible campaign-visible trigger. No profile creates a sixth model or independent authority.

When the engagement deadline or activity budget is exhausted, adaptation cannot extend itself. It stops new dependent proposals and preserves an accountable result identifying assessed scope, proven outcomes, untested or unresolved hypotheses, and residual uncertainty. Continuation requires a new or amended operator authorization.

Adaptation routing respects the current operational tempo:

**In exploitation mode**: Reassessment may execute rapidly. If a fast expansion reveals that the original opportunity is no longer viable, the campaign can quickly redirect to an alternative route without returning to deliberate mode first. The decision to pivot during exploitation mode requires a clear trigger, a viable alternative within current authorization, and a recorded justification. Every resulting action still passes deterministic authority and its available evidence must support the particular action's stakes. After the tempo window, reconciliation examines outcomes and corrects or suspends unsupported claims; it never retroactively authorizes an action.

**In deliberate mode**: Reassessment follows full epistemic rigor. Competing hypotheses are formally evaluated, evidence gaps are identified, and the decision trail is complete before action.

**Tempo transition as adaptation**: The decision to enter or exit exploitation mode is itself an adaptation decision. It may be triggered by: discovery of a time-sensitive opportunity (enter exploitation), closure of the opportunity window (exit to deliberate), need for consolidation after rapid action (exit to deliberate), or detection of increased environmental friction suggesting caution (exit to deliberate).

## 4. Decisions, uncertainty, and stopping

A decision names its trigger, relevant reconciled evidence and freshness (with epistemic tier), affected positions and objectives, competing hypotheses, why an option was selected or abandoned, expected and falsifying evidence, current authorization, and the condition that would change the decision. Valid responses include continue, alternate scoped path, observe more, refresh, dwell, maintain validated access, re-enter, seek operator decision, retask within approved bounds, or stop. Inconclusive evidence prompts bounded observation or escalation, not presumed success. An action's result must revise the relevant belief or mark it unresolved before another capability is considered; changing tools alone is not adaptation.

### 4.1 Failure classification and retry rules

Not all failures are equal. Adaptation distinguishes:

**Transient failures** — network timeout, connection reset, brief service unavailability, transient authentication token issue. These may be retried within a bounded window (N attempts in duration T, configurable per engagement) without requiring a changed premise. If all retries fail, the failure is reclassified as persistent.

**Persistent failures** — authentication explicitly denied, scope violation, capability definitively unavailable, access affirmatively revoked. These require a changed premise, justified alternative approach, or renewed authorization before another attempt. Repeating identical attempts without a new reason is prohibited.

**Inconclusive results** — ambiguous response, partial output, unclear outcome. These require additional observation or a different approach to resolve. They do not constitute success. Bounded retry at the same approach is permitted only if a transient condition plausibly explains the ambiguity.

The distinction matters because real operators naturally retry a dropped SSH connection immediately but fundamentally change approach after a credential is denied. Campaign behavior should match this operational instinct.

### 4.2 Dwell, loss, and bounds

Dwell preserves a decision to pause actions; it does not imply the foothold remains healthy indefinitely. Health checks during dwell follow PRD-003 health assessment rules, including the operational loss threshold.

Loss — whether presumed (unreachability threshold exceeded) or confirmed (affirmative evidence) — suspends dependent actions until a surviving position or renewed validation supports them. Presumed loss permits faster recovery if contact resumes (PRD-003 §5.1). Confirmed loss requires full re-entry validation.

If authority is revoked, a safety boundary is breached, or no scoped route is viable, stop or freeze according to campaign lifecycle rules.

### 4.3 Feedback, reflection and progress

After an outcome, the reasoner revises beliefs and unresolved questions before proposing the next action. A reflection records its supporting outcome, uncertainty, applicability and counterevidence; it is a revisable hypothesis, never a promoted domain fact. Reflections are stored through the existing decision/history responsibility with a bounded retrieval view, not a universal writable memory; a contradicted or superseded reflection remains traceable and is not silently retrieved as current advice, and applicability is rechecked before reuse. Capability feedback may also produce a revised candidate implementation through the same isolated build/qualification/admission lane; a revision never mutates an in-flight artifact.

Progress can be a supported observation, an eliminated hypothesis, a resolved dependency or a useful artifact revision; a tool switch alone is not progress. Repeated equivalent proposals without new evidence or a changed premise produce a visible no-progress disposition; the reasoner may choose another permitted hypothesis, request operator input, defer or stop. There is no automatic campaign abandonment because one route failed, and no endless retry in pursuit of a success label.

Adaptation retains only approved opaque proof and non-sensitive decision context, including an opaque campaign-scoped secret reference and safe eligibility metadata when relevant. It cannot request or recover a raw secret as reasoning input, treat the reference as authority, or change authoritative audit evidence. Privileged defender assessment cannot become a reward signal in blind mode; any feedback released in a separately authorized defender-informed exercise remains bounded and recorded as such under PRD-000 INV-007. CampaignTrajectory records the time-ordered, non-sensitive rationale: evidence considered (with epistemic tier), selected and rejected hypotheses, attempted action, contradictory result, tempo transitions, and why the campaign changed course. Current environmental, access, path, and objective truth stays with its respective model; historical rationale cannot itself validate a position.

## 5. Illustrative synthetic trace across the four PRDs

### Deliberate adaptation
The synthetic Host A route begins as a candidate and produces transient access; only validated bounded execution creates a PRD-003 foothold. PRD-004 uses that healthy position to reconcile a Service B relationship at Tier 2, attempts a scoped transition, validates B, and observes synthetic Asset C. PRD-005 validates C as an objective target; a generic success remains inconclusive until a permitted synthetic marker yields target-specific opaque proof.

### Exploitation-mode adaptation
If A is then lost (presumed, via operational timeout), PRD-006 suspends A-dependent paths immediately without waiting for affirmative evidence. B remains independently healthy. The reasoner assesses whether B's position can reach an alternative route. A new service is observed from validated B as a sourced OBSERVED claim; its relevance and any higher-stakes relationship still need tier-appropriate corroboration. The campaign pivots to this opportunity: enters exploitation mode, executes a pre-authorized chain from validated B position, reaches a new transient position on Service D, runs immediate read-only situational awareness. In parallel, D is validated. Provisional observations from D promote. The campaign returns to deliberate mode for consolidation.

### Loss and retry
Two hours after A's presumed loss, contact with A resumes. Since A's loss was presumed (not confirmed), abbreviated validation confirms the same identity and context; A returns to healthy without full re-entry progression. Meanwhile, an attempt to expand from B encounters a transient network timeout. The bounded retry window permits two additional attempts. The second retry succeeds; the expansion continues. In contrast, an explicit authentication denial from Service E is classified as a persistent failure; the reasoner must propose a changed premise (different credential, different route, or escalation) before another attempt.

### Blind-mode boundary
No privileged defender verdict influences any of these decisions. A defender-informed exercise would be separately authorized and labeled. Campaign-visible friction (a blocked connection, a terminated session) informs adaptation as Tier 1 observations without attributing the cause to a specific control.

## 6. Observable acceptance criteria

1. A reviewer can follow a fresh or stale trigger through competing hypotheses, a justified proposal or decision to observe more, expected and contrary evidence, authorization, outcome, and model revision in any of the four loops.
2. Loss of one position (presumed or confirmed) suspends only dependent actions; an independently healthy position can sustain authorized work, while re-entry demands validation appropriate to the loss type.
3. Inconclusive observations and objectives do not turn into facts or success; an unchanged failed hypothesis cannot cause unbounded retries or a blind tool substitution. Historical rationale remains reviewable without being treated as current truth.
4. Transient failures permit bounded retry; persistent failures require a changed premise; the classification and its outcome are recorded.
5. A changed scope or goal has an explicit operator and deterministic authority boundary; a revoked authorization blocks further action.
6. Tempo transitions (entering and exiting exploitation mode) are recorded with justification and trigger appropriate post-tempo reconciliation.
7. Rapid, bounded, and persistent engagement envelopes constrain adaptation without changing proof standards; expiry or budget exhaustion prevents self-extension and produces a bounded-completion account of assessed scope and residual uncertainty.
8. The synthetic cross-PRD trace maintains distinct terrain, path, foothold, objective, and trajectory claims without raw sensitive material, audit modification, or privileged defender-oracle feedback in blind mode; any defender-informed variant is distinctly authorized, labeled, and evaluated.

## 7. Explicit non-goals

This PRD does not specify scheduling algorithms, preset duration values, retry counts or timeout values, event schemas, service topology, tool behavior, payload implementations, adaptive evasion procedures, or exploitation-mode entry criteria. A reasoner may select or propose a declared admitted capability, or a candidate implementation for one, within current authority; it cannot execute an arbitrary binary, self-grant admission or deployment authority, or treat a temporary artifact as permanent access. It does not merge the operational models, define Observer grading, or auto-authorize broader scope. The engagement envelope and configurable parameters such as retry window, operational loss threshold, and health-check cadence are operator-authorized product inputs; their concrete presets and runtime mechanisms are later design decisions, not PRD constants.

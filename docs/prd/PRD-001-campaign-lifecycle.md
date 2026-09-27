# PRD-001: Campaign Lifecycle

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-001 |
| **Title** | Campaign Lifecycle |
| **Status** | ACCEPTED |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | PRD-000 |
| **Target Seal** | DW-PRD-001 |

---

## 1. Purpose & Core Semantic Distinctions

This document specifies the operational lifecycle, semantic states, transitions, and decision loops governing DuskWeave campaigns. A campaign in DuskWeave is an enduring, goal-driven operational undertaking that navigates complex cyber terrain through hypothesis formulation, validated execution, and continuous adaptation.

To maintain conceptual rigor and prevent conflation of concerns, the lifecycle enforces clear distinctions between core semantic concepts:

- **State**: The current verified operational condition of a campaign entity (e.g., campaign status, foothold health, terrain snapshot). State is deterministic, grounded in evidence, and immutable once recorded.
- **Event**: A discrete, point-in-time occurrence or execution outcome (e.g., network probe completed, execution succeeded, connection dropped). Events serve as the audit record and trigger transitions.
- **Observation**: An unverified signal, datum, or output perceived from the environment. Observations are not facts until corroborated and reconciled.
- **Decision**: A deliberate choice made by a reasoning worker to select one candidate proposal among evaluated alternatives.
- **Objective**: A declared mission goal representing a desired real-world or emulation milestone (e.g., demonstrate access to core transaction database).
- **Position**: The operational standing of the campaign across cyber terrain, defined by the set of active, validated footholds and their reachable horizons.

---

## 2. High-Level Campaign Progression

At the macroscopic level, a campaign progresses through an overarching operational sequence:

```text
Mission Definition
      ↓
Target & Access Research
      ↓
External Reconnaissance
      ↓
Access-Path Selection
      ↓
Initial Access Attempt
      ↓
Access Validation
      ↓
Foothold Establishment
      ↓
Access Survivability
      ↓
Situational Awareness
      ↓
Recursive Expansion & Objective Realization
```

This progression is not a rigid linear pipeline. Each stage represents a discrete operational capability that feeds evidence into the ongoing campaign model. Failure at any single transition does not abort the campaign; it activates the cross-cutting adaptation mechanism to explore alternate paths.

---

## 3. Access Loop & Foothold Semantics

The Access Loop governs the transition from external discovery to validated operational presence.

### 3.1 Transient Access vs. Validated Foothold
A critical invariant of the DuskWeave lifecycle is that **`TransientAccess` does not become a `Foothold` without explicit `AccessValidation`**:

Transient access is an observed, bounded effect whose repeatability or context is still unverified. A validated foothold requires reconciled evidence of reliable bounded execution and retrievable results, exact identity and privilege context, environmental boundary, stability, and current authorized scope. A candidate path, a tool return, or reachability alone does not establish a foothold. [PRD-003](PRD-003-access-and-footholds.md) develops access evidence and health transitions; until it is accepted, these lifecycle requirements remain governing.

### 3.2 Access Survivability
Once a foothold is established, the campaign evaluates access survivability without requiring invasive persistence:
- **Primary and Alternate Positions**: The campaign identifies or prepares alternative access routes (e.g., secondary credentials, adjacent service interfaces) to ensure operational continuity.
- **Health and Freshness Verification**: Footholds are periodically checked for validity, connectivity, and token expiration using minimal, non-disruptive signals.
- **Re-Entry Feasibility**: The campaign models whether lost access can be re-established via known terrain paths without starting external reconnaissance from scratch.
- **No Invariant Violation**: Survivability semantics model operational continuity; they do not prescribe specific malware persistence, registry modifications, or backdoor installation.

### 3.3 Access Loss and Re-Entry
A timeout, reboot, or failed contact prompts health reassessment; it does not by itself prove access loss. Expired validation is stale, conflicting evidence is uncertain, and only reconciled affirmative evidence establishes that a position is lost. Dependent actions pause while evidence is insufficient.
- Confirmed access loss does **not** signify campaign failure.
- When a position is confirmed lost, assess independently healthy alternate positions and reconsider authorized re-entry routes.
- A viable route is only a candidate; operations from another position require its own current validation, and re-entry requires fresh access validation. [PRD-003](PRD-003-access-and-footholds.md) owns the detailed transition criteria.

---

## 4. Recursive Expansion Loop

Internal traversal and network expansion revisit observation, reconciliation, authorized transition, and validation from current positions. This is a recursive lifecycle relationship, not a mandatory lateral-movement stage; [PRD-004](PRD-004-expansion-loop.md) owns candidate evidence, authorized attempts, and new-position validation when accepted.

### 4.1 Lateral Movement as Recursive Traversal
Lateral movement is not an isolated phase of a kill chain. It is the repeated, hypothesis-driven traversal of trust boundaries within the recursive expansion loop. Each traversal yields new observations, expands known terrain, and potentially yields additional footholds.

### 4.2 Multi-Dimensional Access Expansion
Expansion may change host, identity, privilege, application, service, network-reach, or trust position within authorized bounds. It cannot be measured solely by credentials acquired. [PRD-004](PRD-004-expansion-loop.md) owns the specific evidence and transition semantics for these dimensions when accepted.

---

## 5. Objective Processing Loop

The Objective Loop discovers mission-relevant targets, validates a scoped claim, gathers sufficient authorized proof, and reviews fulfillment. Collection or transfer is conditional, not a mandatory lifecycle step. [PRD-005](PRD-005-objective-loop.md) owns target-specific proof transitions and review criteria when accepted.

### 5.1 Dynamic Objective Discovery
Strategic mission goals are defined at campaign inception, but concrete targets may emerge as terrain changes. Material terrain or position changes require revisiting objective opportunity; discovery alone does not establish objective success. [PRD-005](PRD-005-objective-loop.md) owns target validation and fulfillment semantics when accepted.

### 5.2 Emulation Boundaries & Zero-Retention
In strict adherence to **INV-005 (Sensitive Data Zero-Retention)**:
- **Collection**: Use sufficient non-sensitive evidence when available. PRD-000 INV-005 governs the conditional, expressly authorized minimum client-content download into the ephemeral sensitive boundary for opaque proof; unavailable containment makes the proof attempt ineligible or inconclusive. [PRD-005](PRD-005-objective-loop.md) develops target-specific evidence and review when accepted.
- **Staging & Transfer**: Only synthetic, non-sensitive, or derived opaque proof may be staged, delivered, or reported; raw client material remains subject to PRD-000 INV-005 and is discarded after the attempt.

### 5.3 Objective Review Outcomes
Following an objective action, an explicit Objective Review determines the subsequent campaign posture:
1. **Continue**: Pursue an authorized objective or justified expansion; no fixed objective order is implied.
2. **Dwell**: Intentionally pause active operations to honor tempo constraints or allow a defined observation window; any defender feedback follows the mode-specific knowledge boundary in PRD-000 INV-007.
3. **Retask**: Re-evaluate campaign priorities based on newly discovered strategic value or altered rules of engagement.
4. **Maintain Access**: Hold established footholds in a passive monitoring state without generating new actions.
5. **Re-enter**: Rebuild an access chain after partial access loss before attempting further objective work.

---

## 6. Campaign Tempo & Deliberate Dwell

Adversaries do not generate continuous, high-volume automated traffic. DuskWeave elevates dwell time to a first-class operational state:
- **Active Dwell**: An intentional pause where no target actions are dispatched, allowing natural enterprise traffic patterns to mask prior interactions.
- **Temporal Alignment**: Operations align with target organization working hours, time zones, and scheduled maintenance windows.
- **Health Reassessment**: Dwell does not prove continued foothold readiness; bounded checks may refresh health when authorized, while expired or conflicting evidence requires reconciliation before dependent actions.

---

## 7. Cross-Cutting Adaptation Mechanism

Adaptation is an active control mechanism operating across all four loops, not a terminal error handler.

### 7.1 Adaptation Triggers
Changes in reconciled campaign evidence, access health, objective opportunity, authorized scope, or operational tempo may require reassessment. A raw signal remains an observation; timeout or denial does not automatically prove a cause or access loss.

### 7.2 Multi-Loop Adaptation Routing
Reassessment may revisit Strategic, Access, Expansion, or Objective decisions without imposing a fixed sequence. [PRD-006](PRD-006-adaptation.md) owns the detailed decision triggers, routing, retries, and stopping conditions when accepted.

### 7.3 Campaign Adaptation and Defender Assessment
Blind campaign adaptation uses authorized mission context and reconciled evidence from its position. A separately authorized defender-informed exercise may additionally use bounded, labeled defender feedback. PRD-000 INV-007 owns that distinction and the treatment of telemetry obtained through a validated, authorized campaign position. Defender assessment and blind campaign outcomes remain independently identifiable.

---

## 8. Campaign Termination & Completion Criteria

A campaign enters its terminal lifecycle state under explicitly defined conditions:

### 8.1 Successful Completion
- All declared primary objectives have been fulfilled, validated, and substantiated with opaque proof.
- Clean disengagement is completed (temporary session handles closed, staging locations verified clean).
- Final audit trajectory is sealed for observer analysis.

### 8.2 Campaign Failure & Exhaustion
A campaign terminates as failed only under strict operational conditions:
- **Scope Exhaustion**: All authorized target paths, access vectors, and expansion hypotheses have been explored without achieving objectives.
- **Unrecoverable Access Loss**: All active and alternate footholds are lost, and all re-entry pathways are blocked or exhausted within authorized scope.
- **Safety Tripwire Activation**: An unresolvable safety, scope, or authorization violation is detected by the deterministic policy kernel, initiating immediate operational freeze.

---

## 9. Explicit Non-Goals

To maintain strict PRD boundaries, this document excludes:
- Implementation state machines (e.g., Rust enum transitions, event loop architectures).
- Relational database schema definitions or event-sourcing data structures.
- Specific command execution syntax, network payloads, or tool adapter APIs.

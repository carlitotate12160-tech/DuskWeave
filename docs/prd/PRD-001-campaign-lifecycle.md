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

1. **TransientAccess**: A temporary, unverified capability to execute an action on a target (e.g., a blind command injection response, a newly acquired web session cookie, an ephemeral cloud access token).
2. **AccessValidation**: A deterministic verification phase that actively confirms:
   - Command/execution determinism and output retrieval reliability.
   - Exact identity and privilege context of the execution environment.
   - Operating system and process boundaries.
   - Operational stability (ensuring action does not crash host services or violate safety bounds).
3. **Foothold**: A confirmed, addressable operational position that satisfies all validation criteria and is available for subsequent campaign operations.

### 3.2 Access Survivability
Once a foothold is established, the campaign evaluates access survivability without requiring invasive persistence:
- **Primary and Alternate Positions**: The campaign identifies or prepares alternative access routes (e.g., secondary credentials, adjacent service interfaces) to ensure operational continuity.
- **Health and Freshness Verification**: Footholds are periodically checked for validity, connectivity, and token expiration using minimal, non-disruptive signals.
- **Re-Entry Feasibility**: The campaign models whether lost access can be re-established via known terrain paths without starting external reconnaissance from scratch.
- **No Invariant Violation**: Survivability semantics model operational continuity; they do not prescribe specific malware persistence, registry modifications, or backdoor installation.

### 3.3 Access Loss and Re-Entry
Access loss is a normal operational event resulting from network timeouts, host reboots, credential revocation, or defensive containment.
- Access loss does **not** signify campaign failure.
- When a foothold is lost, the platform transitions the foothold to an inactive/stale state, assesses remaining alternate positions, and triggers re-entry planning.
- If viable alternate paths exist, operations resume immediately from the nearest surviving foothold.

---

## 4. Recursive Expansion Loop

Internal traversal and network expansion are modeled as a recursive, feedback-driven loop rather than a linear lateral movement phase:

```text
       ┌────────────────────────────────────────────────────────┐
       ▼                                                        │
    Observe ──► Update Situational Model ──► Access Expansion   │
                                                    │           │
    Observe Again ◄── New Foothold ◄── Move/Pivot ◄─┴─ Internal Path Selection
```

### 4.1 Lateral Movement as Recursive Traversal
Lateral movement is not an isolated phase of a kill chain. It is the repeated, hypothesis-driven traversal of trust boundaries within the recursive expansion loop. Each traversal yields new observations, expands known terrain, and potentially yields additional footholds.

### 4.2 Multi-Dimensional Access Expansion
Access expansion is frequently mischaracterized solely as credential harvesting. DuskWeave models access expansion across multiple operational dimensions:
- **Identity & Principal Context**: Gaining access to new service accounts, user identities, or federated roles.
- **Privilege & Entitlements**: Elevating administrative authority or gaining specific role-based permissions.
- **Host & Application Context**: Moving between isolated execution tiers (e.g., container to container host, web tier to database tier).
- **Service Authority**: Acquiring authorization to interact with enterprise services (e.g., message queues, key vaults, orchestration APIs).
- **Network Reach & Routing**: Discovering internal routing paths across dual-homed hosts, VPN concentrators, or proxy gateways.
- **Trust Relationships**: Traversing cross-forest domain trusts, cloud provider trust policies, and mutual TLS application trust boundaries.

---

## 5. Objective Processing Loop

The Objective Loop governs the identification, validation, and fulfillment of mission requirements:

```text
Objective Discovery
      ↓
Target Validation
      ↓
Simulated Collection
      ↓
Staging
      ↓
Transfer / Exfiltration Proof
      ↓
Objective Review
```

### 5.1 Dynamic Objective Discovery
Objectives are not static checklists known in advance. While strategic mission goals are defined at campaign inception, specific objective instances (e.g., the exact internal repository containing target design specifications) are discovered dynamically as terrain expands:
- **Rule**: Whenever material terrain changes occur (e.g., discovering an unmapped subnet, an internal code repository, or an identity store), the campaign must **revisit objective discovery** to evaluate whether new objective pathways have opened.

### 5.2 Emulation Boundaries & Zero-Retention
In strict adherence to **INV-005 (Sensitive Data Zero-Retention)**:
- **Collection**: DuskWeave collects metadata, file attributes, cryptographic proofs of access, or synthetic tokens. It never reads or records raw customer records, personal data, or financial stores.
- **Staging & Transfer**: Staging aggregates opaque proof material. Exfiltration simulates exfiltration channels (e.g., measuring channel bandwidth, testing egress filtering) using synthetic, non-sensitive payloads.

### 5.3 Objective Review Outcomes
Following an objective action, an explicit Objective Review determines the subsequent campaign posture:
1. **Continue**: Proceed to the next sequential objective or deeper expansion.
2. **Dwell**: Intentionally pause active operations to honor tempo constraints or allow a defined observation window; any defender response observed by the separate assessment plane is not fed into active campaign reasoning.
3. **Retask**: Re-evaluate campaign priorities based on newly discovered strategic value or altered rules of engagement.
4. **Maintain Access**: Hold established footholds in a passive monitoring state without generating new actions.
5. **Re-enter**: Rebuild an access chain after partial access loss before attempting further objective work.

---

## 6. Campaign Tempo & Deliberate Dwell

Adversaries do not generate continuous, high-volume automated traffic. DuskWeave elevates dwell time to a first-class operational state:
- **Active Dwell**: An intentional pause where no target actions are dispatched, allowing natural enterprise traffic patterns to mask prior interactions.
- **Temporal Alignment**: Operations align with target organization working hours, time zones, and scheduled maintenance windows.
- **Health Preservation**: Dwell states preserve foothold readiness through passive, low-overhead heartbeats rather than noisy active testing.

---

## 7. Cross-Cutting Adaptation Mechanism

Adaptation is an active control mechanism operating across all four loops, not a terminal error handler.

### 7.1 Adaptation Triggers
Adaptation is triggered by environmental divergences and operational friction:
- **Frictional Triggers**: Invalidation of credentials, network timeout, endpoint reboot, or target access denial.
- **Structural Triggers**: Discovery of new subnets, air-gapped segments, unexpected architecture, or altered trust paths.
- **Opportunity Triggers**: Identification of high-value key terrain or direct routes to mission objectives that bypass intermediate steps.

### 7.2 Multi-Loop Adaptation Routing
When triggered, adaptation directs re-planning to the appropriate loop level:
- *Strategic Redirect*: Retasking overall campaign priorities if primary objectives become unviable within scope constraints.
- *Access Redirect*: Selecting an alternative external access path if an initial ingress vector closes.
- *Expansion Redirect*: Identifying alternate traversal routes across internal trust boundaries when a pivot host is isolated.
- *Objective Redirect*: Re-sequencing objective pursuit based on newly discovered target locations.

### 7.3 Campaign Adaptation and Defender Assessment
Campaign adaptation uses authorized mission context, campaign-visible environmental observations, and evidence of its own actions. It does not use the isolated Observer's live EDR, AV, SIEM, or SOC verdicts to choose the next active step. After an exercise, defenders may assess outcomes and authorize a distinct retest; the retest must remain distinguishable from the original run. PRD-000 owns the product meaning of defender control-gap assessment.

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

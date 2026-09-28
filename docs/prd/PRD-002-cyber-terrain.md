# PRD-002: Cyber Terrain

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-002 |
| **Title** | Cyber Terrain |
| **Status** | ACCEPTED |
| **Acceptance clarification** | Product owner, 2026-09-28: STALE applies to expired OBSERVED/CORROBORATED claims; PROVISIONAL remains origin-uncertain when freshness expires |
| **Stage** | Stage 2 — Campaign Semantics |
| **Direct Dependencies** | PRD-001 |
| **Target Seal** | DW-PRD-002 |

---

## 1. Purpose & Scope of Cyber Terrain

Cyber Terrain owns the campaign's environmental understanding: corroborated facts together with sourced observations and hypotheses carrying explicit epistemic limits. Inclusion in this model does not promote an observation into a fact or grant execution authority.

DuskWeave operates on the principle that an adversary cannot interact with an environment without forming, updating, and navigating a conceptual map of that environment. Cyber Terrain models this operational reality: it captures network structures, compute platforms, identity architectures, software applications, defensive controls, and operational rhythms discovered during a campaign.

### 1.1 Strict Model Separation (INV-002)
To prevent architectural monoliths and maintain semantic purity, Cyber Terrain maintains strict boundaries against the other four operational models:

- **CyberTerrain**: Environmental claims and relationships with provenance, tier, and epistemic status; only claims with sufficient evidence are treated as established facts.
- **FootholdGraph**: Where the campaign currently holds validated execution access.
- **AttackPathView**: Analytical projections of potential traversal and attack transitions.
- **ObjectiveState**: Declared mission requirements and their fulfillment progress.
- **CampaignTrajectory**: Chronological event ledger recording the campaign's historical decisions and actions.

### 1.2 Boundary Restrictions
Cyber Terrain is explicitly bounded. **Cyber Terrain cannot own**:
- Foothold validation or execution session state.
- Objective tracking or mission completion scoring.
- Tactical or strategic campaign planning.
- Capability execution or tool invocation.
- The chronological event log of the campaign.

---

## 2. Semantic Dimensions of Cyber Terrain

Cyber Terrain is multi-dimensional. It cannot be reduced to a flat IP address table or a generic network graph. It models the target environment across seven distinct dimensions:

### 2.1 Network Terrain
Captures spatial and topological connectivity:
- Subnet ranges, CIDR blocks, and routing boundaries.
- Boundary devices, internal gateways, NAT boundaries, and egress filters.
- Exposed network ports, protocols, and listening services.

### 2.2 Compute Terrain
Captures execution hosts and infrastructure nodes:
- Physical servers, virtualization hypervisors, and guest virtual machines.
- Container engines, container instances, and serverless compute contexts.
- Operating system families, kernel architectures, and host hardware platforms.

### 2.3 Identity Terrain
Captures the organizational trust and authorization fabric:
- User accounts, service principals, managed identities, and machine accounts.
- Security groups, role-based access control (RBAC) assignments, and nested privileges.
- Authentication authorities (e.g., Active Directory domains, LDAP trees, cloud identity tenants).
- Cross-domain trusts, federation agreements, and single sign-on (SSO) relationships.

### 2.4 Application Terrain
Captures software systems and business logic:
- Enterprise software platforms, microservices, and databases.
- Web applications, exposed API endpoints, and message queues.
- Inter-application dependencies and automated data pipelines.

### 2.5 Control Terrain
Captures environmental defensive architecture and security enforcement boundaries:
- Security proxies, web application firewalls (WAF), and network segmentation policies.
- Endpoint detection agents, host-based firewalls, and local security configurations.
- Authentication gating mechanisms (e.g., Multi-Factor Authentication barriers, Conditional Access policies).

A campaign-visible control or access denial may be represented with provenance and freshness without assuming which control caused it. Defender telemetry obtained through an authorized campaign position may enter as a campaign observation subject to reconciliation, scope, and sensitive-data rules; observing an alert establishes only the limited observation of that alert, not its correctness or a causal link to a specific campaign action or control effect. Privileged Observer/Grader feeds remain outside blind campaign terrain. PRD-000 INV-007 owns the knowledge boundary and defender-informed mode.

### 2.6 Objective-Relevance Terrain
Captures how environmental entities relate to mission objectives:
- Designation of high-value business assets, sensitive database repositories, or key executive workstations.
- Environmental proximity of discovered terrain nodes to declared campaign objectives.

### 2.7 Temporal Terrain
Captures time-based operational dynamics:
- Enterprise business hours, work shifts, and regional holidays.
- Scheduled batch processing jobs and backup maintenance windows.
- Ephemeral infrastructure lifecycles (e.g., auto-scaling groups, transient build workers).

---

## 3. Epistemic Lifecycle & Observation Reconciliation

Under **INV-004 (Observation != Fact)**, every raw observation receives provenance, validation, and reconciliation before entering CyberTerrain. Its epistemic status describes what is known; its tier describes the operational stakes and required evidence. An OBSERVED or PROVISIONAL entry is visible for bounded orientation but is not a corroborated environmental fact or an authorization. This keeps fast reasoning possible without making all terrain claims equally certain.

### 3.1 Epistemic Product Vocabulary
Every entity, attribute, and relationship in Cyber Terrain carries an explicit epistemic status:

- **OBSERVED**: Directly perceived by a single tool execution or sensor output (e.g., a port probe returned an open banner).
- **CORROBORATED**: Confirmed by multiple independent sources, disparate techniques, or consistent repeat observations over time (e.g., port banner match confirmed by authenticated registry query).
- **INFERRED**: Derived logically from established terrain facts and protocol specifications (e.g., inferring a domain controller exists based on DNS SRV records).
- **HYPOTHETICAL**: A plausible candidate entity or relationship postulated by cognitive planners as an operational hypothesis, awaiting empirical verification.
- **STALE**: An entity, attribute, or relationship claim previously OBSERVED or CORROBORATED whose freshness window has expired without adequate refresh. Its prior evidential status and provenance remain in history. Expiry alone does not refute the claim, add corroboration, or validate a PROVISIONAL origin. PROVISIONAL is also subject to freshness: once expired, the claim is ineligible as a current premise but remains PROVISIONAL until its origin and claim undergo a Terrain-accepted evidence-based transition. If the origin fails validation or is abandoned, the existing PROVISIONAL rule downgrades it to HYPOTHETICAL.
- **REFUTED**: An entity or relationship that was previously accepted or postulated, but has been affirmatively disproven by recent observation (e.g., host no longer responds, service uninstalled).
- **PROVISIONAL**: A sourced observation from a transient or unvalidated position that has passed initial validation and reconciliation but whose origin remains unverified. It is recorded within CyberTerrain for orientation and hypothesis formation, not as an established terrain fact or a sole premise for scope, trust, access, expansion, objective, or other consequential action. If its origin fails validation or is abandoned, it is downgraded to HYPOTHETICAL without erasing its provenance or history.

### 3.2 Provenance, Freshness, & Temporal Decay
Target environments are dynamic: IP leases expire, virtual machines terminate, services relocate, and firewall rules change.
- **Provenance**: Every terrain claim tracks its originating observation event, capability source, method of discovery, and originating position validation status.
- **First and Last Observed**: Every entity, attribute, and relationship claim maintains precise timestamps for initial discovery and most recent observation, plus the most recent corroboration when one exists.
- **Freshness Rule**: **Terrain at $T_0$ is not automatically valid at $T_1$**. Confidence in terrain validity decays over time unless refreshed by operational interaction. Planners must account for staleness before relying on historical terrain state.

### 3.3 Tiered Epistemic Confidence

Observations carry different operational stakes. A port responding on a host is a low-stakes sourced observation; the identity of a Domain Controller governing an entire trust domain is a campaign-critical determination. Forcing identical reconciliation rigor on both wastes operational tempo and diverges from how real operators assess their environment.

Cyber Terrain classifies observations into three confidence tiers:

**Tier 1 — Fast sourced observation (low operational stakes)**

Narrow outputs of direct authorized queries whose immediate use has low stakes:
- The response to a direct port probe from a specified vantage and time, not a global claim about port state.
- An interface address or hostname reported by an authorized query, not proof of asset ownership or scope.
- A version string returned by a banner or system command, not independent proof of installed version.
- Local identity or group membership returned by an authorized OS query from a validated position, limited to that context.
- File existence or directory listing observed from a validated position at a recorded time.

Tier 1 observations enter terrain as OBSERVED from one sufficiently direct source after validating the source, scope, time, claim limits, and any contrary evidence. An independent second source is not required for a narrow, low-stakes observation to inform orientation. If source or context is uncertain, leave the limited observation unpromoted or keep it HYPOTHETICAL; use PROVISIONAL specifically when its originating position is unvalidated. A Tier 1 observation cannot alone establish identity or privilege beyond its observed context, authorization scope, a trust boundary, Key Terrain, or objective fulfillment. It decays to STALE under normal freshness rules.

**Tier 2 — Light Corroboration (moderate operational stakes)**

Observations where misidentification could waste operational effort but does not directly risk campaign safety or scope:
- Service identity and version behind a port.
- Trust relationships between domains or tenants.
- Network routes and segmentation boundaries.
- Application dependencies and inter-service connections.
- Privilege or role associations discovered through enumeration.

Tier 2 observations require one primary source plus one supporting signal before reaching CORROBORATED. A single source enters as OBSERVED and may inform hypothesis formation, but dependent actions that cross trust or scope boundaries wait for corroboration.

**Tier 3 — Full Reconciliation (campaign-critical, high operational stakes)**

Observations whose misidentification could compromise campaign safety, violate scope, waste significant operational capital, or mislead strategic decisions:
- Domain Controller, Root CA, or central identity provider identification.
- Key Vault, secrets manager, or credential store location.
- Scope-boundary systems (what is in-scope vs. out-of-scope).
- Trust delegation chains that govern cross-domain authority.
- Any entity classified as Key Terrain under §5.

Tier 3 observations require multiple independent sources and full reconciliation before reaching CORROBORATED. An unverified Tier 3 claim remains OBSERVED or INFERRED and cannot be the basis for campaign-critical actions.

**Tier classification is not immutable.** An initially Tier 1 observation must be elevated if later evidence makes the claim campaign-critical (e.g., a routine host turns out to be a Domain Controller). A lower tier requires evidence that the actual stakes decreased; exploitation tempo alone never lowers the required tier or bypasses scope, trust, Key Terrain, access, or objective evidence boundaries.

### 3.4 Reconciliation Flow

The reconciliation pipeline adapts to the tier of the observation:

```text
Raw Observation (Perception)
           ↓
Tier Classification (stakes assessment)
           ↓
     ┌─────┼─────────────────────┐
     ↓     ↓                     ↓
  Tier 1   Tier 2              Tier 3
  Single   Primary +           Multiple independent
  source   supporting signal   sources + reconciliation
     ↓     ↓                     ↓
  OBSERVED CORROBORATED        CORROBORATED
     ↓     ↓                     ↓
     └─────┴─────────────────────┘
           ↓
  CyberTerrain entry at stated status
  (with tier, provenance, freshness)
```

A transient-origin observation first enters as PROVISIONAL after preliminary validation and reconciliation, then is reconsidered on origin validation; it is never silently promoted to a fact. The tiered reconciliation pipeline implements INV-004: every observation undergoes validation and reconciliation; the tier sets the corroboration burden appropriate to its operational stakes. Tier 1 acceptance as OBSERVED satisfies INV-004 only for a narrow sourced observation and does not make it CORROBORATED ground truth. PROVISIONAL remains an observation pending origin validation. Every entry retains source, time, position, epistemic status, and tier; consequential claims wait for the evidence appropriate to their stakes.

---

## 4. Terrain Deltas & Snapshots

To enable efficient reasoning and historical review, Cyber Terrain supports two representation concepts:
1. **Terrain Snapshots**: Immutable, point-in-time representations of accepted environmental state. Snapshots provide stable contexts for reasoning workers.
2. **Terrain Deltas**: Structured representations of state changes between two points in time (e.g., entities discovered, relationships refuted, attributes modified). Deltas trigger adaptation events across campaign loops.

---

## 5. Key Terrain Semantics

DuskWeave incorporates the military doctrine concept of **Key Terrain**, defined as:
> *Any locality, system, identity, or relationship whose access, loss, observation, or control materially changes the progress and outcome of the campaign.*

### 5.1 Determining Key Terrain
Key Terrain is identified through structural and functional characteristics:
- **Connectivity Centrality**: Nodes that bridge isolated network enclaves (e.g., dual-homed bastion hosts, VPN concentrators).
- **Authority Concentration**: Systems or identities that issue credentials, modify directory schemas, or administer authorization policies (e.g., Domain Controllers, Cloud Root Tenants, Key Vaults).
- **Trust Position**: Entities that enjoy implicit, uninspected trust from critical systems (e.g., deployment orchestration servers, vulnerability management scanning hosts).
- **Dependency Centrality**: Shared infrastructure upon which multiple essential services depend (e.g., core DNS servers, core identity providers).
- **Mission Proximity**: Entities that directly host, control, or route access to target objective assets.
- **Access Leverage**: Nodes that provide broad operational reach with minimal footprint or low risk of disruption.

### 5.2 The Critical Invariant: Key Terrain != Vulnerability Severity
A foundational rule of DuskWeave is that **Key Terrain must never be equated with highest vulnerability severity**:
- An unpatched edge device with a CVSS 9.8 remote code execution vulnerability is **not** Key Terrain if it resides in an isolated DMZ with zero internal routing, no privileged identities, and no path to mission objectives.
- Conversely, a fully patched Active Directory Domain Controller or an enterprise Kubernetes control plane with **zero known vulnerabilities** is supreme Key Terrain because its legitimate operational authority and trust position dictate overall network control.
- DuskWeave evaluates Key Terrain based on topological leverage, trust architecture, and mission relevance—not vulnerability scoring.

### 5.3 Key Terrain and Epistemic Tiers
All Key Terrain entities and their defining relationships require Tier 3 (Full Reconciliation) epistemic confidence before campaign-critical actions depend on them. Misidentifying Key Terrain carries outsized campaign risk. A candidate Key Terrain entity at Tier 1 or Tier 2 may inform hypothesis formation but cannot be the basis for strategic campaign decisions until fully reconciled.

---

## 6. Relationship with the Other Operational Models

To maintain **INV-002**, the exact boundaries between Cyber Terrain and adjacent models are strictly enforced:

### 6.1 CyberTerrain vs. FootholdGraph
- Cyber Terrain models what exists (e.g., Host A exists, has IP 10.0.1.5, runs SSH on port 22).
- FootholdGraph models where the campaign possesses verified execution capability (e.g., active SSH credential with execution privileges on Host A).
- Knowledge of a host in Cyber Terrain does **not** grant or imply a foothold.

### 6.2 CyberTerrain vs. AttackPathView
- Cyber Terrain is the underlying model of reality.
- AttackPathView is an analytical projection derived by evaluating Cyber Terrain alongside current footholds, available capabilities, and strategic objectives.
- AttackPathView is a dynamic analytical lens, **not** primary environmental truth.

### 6.3 CyberTerrain vs. ObjectiveState
- Cyber Terrain identifies where target data stores and business services reside in relation to the network and compute fabric.
- ObjectiveState evaluates whether the campaign has successfully fulfilled mission requirements against those targets.

### 6.4 CyberTerrain vs. CampaignTrajectory
- Cyber Terrain reflects the current accepted understanding of the environment.
- CampaignTrajectory records the immutable, time-ordered narrative of decisions made, actions executed, and outcomes achieved.

---

## 7. Explicit Non-Goals

To maintain PRD boundaries and adhere to repository governance:
- **No Database Schemas**: This document does not specify relational tables, document schemas, or graph database query languages (e.g., Cypher, SQL).
- **No Programming Language Types**: This document does not define Rust structs, traits, Go interfaces, or serialization formats.
- **No Algorithmic Implementations**: Graph traversal algorithms (e.g., Dijkstra, A*, shortest-path traversals) belong to downstream design and implementation specifications, not to this PRD.
- **No Tier Classification Algorithms**: The specific rules, thresholds, or scoring systems that classify observations into tiers belong to domain contracts, not to this PRD. This document establishes the semantic requirement for tiered confidence, not the mechanism.

# PRD-000: Product Thesis

| Metadata | Value |
| :--- | :--- |
| **Document ID** | PRD-000 |
| **Title** | Product Thesis |
| **Status** | ACCEPTED |
| **Stage** | Stage 1 — Product Thesis |
| **Direct Dependencies** | None |
| **Target Seal** | DW-PRD-000 |

---

## 1. Identity & Core Purpose

DuskWeave is a persistent campaign reasoning and adversary-emulation platform designed to evaluate enterprise cyber resilience through prolonged, multi-stage, authorized campaigns.

DuskWeave is not a vulnerability scanner, an automated exploit script runner, or a linear attack-graph replayer. It is an operational system that separates cognitive reasoning, deterministic policy authority, isolated execution, continuous observation, and objective proof into distinct, non-interfering architectural planes.

The core mission of DuskWeave is to model how sophisticated adversaries plan, execute, adapt, and sustain campaigns across complex, evolving cyber terrain—while remaining strictly non-destructive, auditable, and bounded by authorized rules of engagement.

---

## 2. Target Audience & Stakeholders

DuskWeave serves specialized security functions requiring high-fidelity adversary emulation:

1. **Authorized Red Teams & Adversary Emulation Operators**: Practitioners requiring an autonomous campaign engine capable of reasoning over complex terrain, maintaining access across operational pauses, and systematically pursuing multi-stage mission objectives.
2. **Enterprise Blue Teams & Detection Engineers**: Defenders who need realistic campaign pressure—including realistic operational tempo, living-off-the-land techniques, and multi-day dwell times—to rigorously test telemetry pipelines, correlation logic, and containment procedures.
3. **Cyber Risk & Security Leadership**: Decision-makers seeking empirical, evidence-backed evaluation of systemic control gaps, defensive visibility deficits, and blast radiuses across critical business assets, rather than point-in-time vulnerability counts.

---

## 3. Problem Statement & Motivation

### 3.1 The Limits of Vulnerability Scanning
Traditional vulnerability scanners evaluate individual software flaws in isolation. They answer the question: *"Does this known vulnerability signature exist on this accessible interface?"*

This paradigm fails to represent real-world cyber risk because:
- Vulnerabilities are assessed without operational context, trust topology, or defensive mitigations.
- Vulnerability scanners generate high-volume, indiscriminate network noise uncharacteristic of real threat actors.
- Complex adversary compromises rarely rely solely on unpatched high-severity CVEs; they string together mundane configurations, legitimate administrative capabilities, identity privileges, and native system binaries.

### 3.2 The Flaws of Linear Orchestration
Automated breach-and-attack simulation (BAS) tools and attack scripts typically execute predefined, linear directed acyclic graphs (DAGs) or single-pass kill chains (Recon → Weaponize → Deliver → Exploit → Control).

In dynamic enterprise environments, linear workflows fail:
- If a target endpoint is patched, rebooted, or network-isolated, linear sequences terminate prematurely.
- They lack the capacity to maintain state across weeks, observe environmental shifts, pivot through alternative routes, or perform deliberate operational dwell.
- They lack architectural separation between the reasoning that proposes an action, the deterministic authority that validates safety and scope, and the broker that executes it.

### 3.3 The DuskWeave Paradigm
DuskWeave solves this by modeling campaigns as persistent, goal-directed feedback loops operating over dynamic, corroborating environmental state. When an access route fails, DuskWeave does not abort; its adaptation mechanics re-evaluate terrain, reassess alternative trust relationships, and pursue secondary paths within authorized bounds.

---

## 4. Operating Model & Campaign Fidelity

DuskWeave achieves high-fidelity adversary emulation through specific operational characteristics:

### 4.1 Nested Reasoning Loops
Rather than a single linear pipeline, DuskWeave models adversary activity as four continuous, nested loops:
- **Strategic Loop**: Mission objectives, boundary constraints, priority scoring, and global campaign posture.
- **Access Loop**: Initial terrain discovery, entry path identification, access validation, and survivability.
- **Expansion Loop (Recursive)**: Situational awareness, internal traversal, trust relationship leverage, and positional advancement.
- **Objective Loop**: Target validation, bounded evidence collection when authorized, staging of derived proof, and objective proof review.

Adaptation is not a final phase; it is an active control mechanism that cross-cuts all four loops to redirect decisions whenever reality diverges from assumptions.

### 4.2 Low Unnecessary Operational Footprint
Sophisticated threat actors do not conduct sweeping port scans or high-frequency brute force attacks from internal positions. DuskWeave enforces a disciplined operational footprint:
- Actions are strictly purposeful and hypothesis-driven.
- Passive and contextual queries are prioritized over active network probes.
- High-volume requests that trigger obvious anomalous network volume are treated as operational failures during planning.

### 4.3 Campaign Tempo & Deliberate Dwell
Adversary campaigns unfold over days, weeks, or months. DuskWeave treats time and operational tempo as first-class campaign dimensions:
- Emulation plans incorporate deliberate dwell periods, respecting business hours, maintenance schedules, and natural operational rhythms.
- Dwell is recognized as an active strategic choice, not an idle error state.

### 4.4 Living-Off-The-Land & Native-Environment Awareness
Rather than deploying heavy custom payloads or unvetted foreign tooling, DuskWeave emphasizes native-environment awareness:
- Reasoners leverage established administration channels, native system binaries, cloud APIs, and directory services already present in the target environment.
- The platform evaluates how normal business workflows can be traversed without introducing foreign artifacts.

### 4.5 Campaign Continuity
Threat actors survive operator handoffs, network disruptions, and endpoint reboots. DuskWeave maintains campaign continuity by storing immutable operational models, enabling campaigns to pause, resume, re-evaluate footholds, and execute re-entry without losing accumulated knowledge.

### 4.6 Threat-Informed Design Framing
DuskWeave draws structural inspiration from observed adversary tradecraft patterns:
- **APT41**: Staged operations, prolonged access survivability, operator shifts, repeated discovery, and adaptive re-entry.
- **Lazarus**: Modular chain composition and diverse execution mechanisms tailored to target contexts.
- **Volt Typhoon**: Extreme reliance on native living-off-the-land techniques, stealth tempo, and thorough situational awareness.

*Note*: These profiles represent design inspiration for capability breadth and behavioral fidelity. They do not constitute attribution claims, threat intelligence feeds, or hardcoded actor heuristics in core architecture. MITRE ATT&CK concepts serve as semantic taxonomy metadata, not product architecture authority.

---

## 5. Defender Control-Gap Assessment

DuskWeave is built to provide empirical visibility into enterprise defensive efficacy. However, it enforces a strict boundary between campaign execution and defensive measurement:

### 5.1 Post-Hoc & Observer Assessment
Defensive efficacy is measured through distinct control-gap dimensions:
- **EDRVisibilityGap**: Gaps where endpoint telemetry fails to capture malicious process ancestry, memory modifications, or credential access attempts.
- **AVCoverageGap**: Blind spots in signature and heuristic file/behavioral scanning.
- **LoggingIntegrityGap**: Unlogged audit events, missing command-line arguments, or disabled audit channels on target hosts.
- **SIEMCorrelationGap**: Failures of central log aggregators to alert on coordinated multi-host access patterns.
- **TemporalCorrelationGap**: Defensive breakdowns caused by adversary actions dispersed across multi-day dwell intervals.

### 5.2 Defender Assessment and Knowledge Boundary (INV-007)
DuskWeave uses defender telemetry to assess how controls responded to authorized emulation. The Observer correlates action evidence with defender visibility, alerts, prevention, and response independently of campaign decisions. INV-007 below owns the knowledge boundary for blind campaigns, separately authorized defender-informed exercises, and telemetry obtained through a validated campaign position.

Assessment must distinguish: an action that failed independently of a control; an action prevented before objective achievement; an objective reached with a correlated detection; an objective reached without a correlated alert despite verified sensor coverage; and an inconclusive result caused by absent, incomplete, or delayed telemetry. Silence alone is not proof of evasion. Each reported gap must identify the action and objective evidence, relevant sensor coverage and observation window, and the observed control outcome.

A separately authorized defender-informed exercise or retest may use bounded defender feedback for control validation. Label and evaluate it separately from a blind adversary-emulation run. The Observer's privileged feed is not an input to blind campaign decisions. Evidence of an alert acquired through a validated, authorized campaign position follows INV-007 and INV-004; it does not by itself prove which control caused a campaign-visible effect.

---

## 6. Architectural Invariants

All features and requirements across DuskWeave are subordinate to seven governing invariants:

1. **INV-001 (No God Object)**: No central manager, coordinator, or global context struct may hold global references across the entire platform. Subsystems interact solely through typed events and bounded contracts.
2. **INV-002 (Separate Operational Models)**: The system maintains five strictly separated operational models:
   - `CyberTerrain`: What exists and the environmental relationships observed.
   - `FootholdGraph`: Where execution access is validated and available.
   - `AttackPathView`: Analytical projections of potential movement and transitions.
   - `ObjectiveState`: Mission targets, requirements, and completion status.
   - `CampaignTrajectory`: Chronological event ledger of past actions, decisions, and outcomes.
3. **INV-003 (Reasoning != Execution)**: Cognitive planners and LLMs only generate typed proposals. All execution must pass through deterministic policy kernels, capability gateways, and execution brokers.
4. **INV-004 (Observation != Fact)**: Raw sensory data from tools and environment must undergo validation, corroboration, and reconciliation before updating established operational models.
5. **INV-005 (Sensitive Data Zero-Retention)**: Raw client credentials, password hashes, cryptographic private keys, personal identity records, customer data, and financial records must never be persisted to storage or forwarded to LLM contexts. For a specifically authorized access or objective proof, the minimum necessary client content may be transferred into an isolated ephemeral sensitive boundary, processed there only to derive irreversible opaque proof, and discarded after the attempt. No raw client content becomes durable evidence or an output to operators.
6. **INV-006 (Audit Integrity)**: Campaign capabilities can never alter, delete, tamper with, or disable authoritative audit logs.
7. **INV-007 — Defender Knowledge Boundary**:

   DuskWeave may perform authorized, bounded operational adaptation intended to test whether enterprise controls can prevent, constrain, or expose an adversary-emulation campaign.

   Active campaign reasoning may use security effects and environmental changes that are genuinely observable from its validated campaign position, including blocked actions, denied access, terminated execution, lost reachability, changed system behavior, or other campaign-visible friction.

   In blind adversary-emulation mode, active campaign reasoning must not receive privileged defender-internal telemetry or verdicts that would not ordinarily be available from the campaign position, including EDR/AV console alerts, SIEM detections, SOC tickets, analyst conclusions, Observer verdicts, or Grader results.

   Campaign-observed evidence does not automatically establish which defensive control caused an effect. Such attribution requires evidence and remains subject to observation/fact reconciliation.

   A separately authorized defender-informed exercise or retest may expose bounded defender feedback to campaign planning for control-validation purposes. Such exercises must be explicitly labeled and evaluated separately from blind campaign results.

   Defender telemetry acquired legitimately through an authorized campaign-visible position is treated as ordinary campaign-observed evidence and remains subject to scope, sensitive-data, provenance, and evidence rules.

   The Defender Knowledge Boundary prohibits privileged defender-oracle feedback; it does not prohibit authorized adversary-emulation capabilities that adapt operational behavior in response to conditions the campaign itself can legitimately observe.

---

## 7. Explicit Non-Goals & Scope Boundaries

To maintain architectural purity and safety, the following capabilities are explicitly out of scope:

- **Destructive Actions**: DuskWeave will never execute ransomware, disk wiping, data destruction, denial-of-service, or irreversible hardware modifications.
- **Covert Implant Infrastructure**: DuskWeave does not build, host, or deploy covert botnet C2 networks, DNS fast-flux infrastructure, or unmonitored persistence implants on client machines.
- **Unbounded or Retained Raw Data Transfer**: Real client confidential content may not be exported as a deliverable, retained, or transferred outside an explicitly authorized ephemeral proof boundary. When synthetic or non-sensitive proof cannot satisfy a declared access or objective claim and the rules of engagement expressly permit it, DuskWeave may download only the minimum necessary content from a scoped client resource into that boundary to derive opaque proof. The raw content is discarded on success, failure, interruption, or withdrawal of authorization; only the derived proof and non-sensitive provenance may persist. Bulk extraction or transfer without a specific proof condition remains out of scope. This client-content proof exception does not authorize acquiring passwords, private keys, or authentication stores.
- **Autonomous Out-of-Scope Spreading**: DuskWeave will never incorporate unconstrained worm-like self-propagation. Every target traversal is verified deterministically against authorized scope bounds.
- **Implementation Specifications in PRD**: This document explicitly excludes database schemas, programming language type definitions, command-line tool syntax, and network wire protocol designs.

---

## 8. Success Criteria

DuskWeave achieves its product mission when:
1. An authorized campaign can execute autonomously over extended durations across multi-subnet enterprise networks, demonstrating valid access expansion and objective achievement without human intervention for routine steps.
2. If primary footholds or network segments are isolated, the platform autonomously re-evaluates terrain, recognizes secondary pathways, and adapts its trajectory without aborting.
3. Zero instances of raw client credentials or sensitive records appear in persistent storage, temporary files, caches, logs, traces, crash diagnostics, reports, or cognitive prompts across the campaign lifecycle; any authorized ephemeral proof transfer produces only opaque durable proof.
4. Defender assessment correlates verified campaign actions and objective outcomes with sensor coverage, detection, prevention, and response within a defined observation window. It distinguishes a substantiated control gap from missing telemetry and can verify improvement through a separately authorized retest.

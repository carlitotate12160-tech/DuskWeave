# DuskWeave Build Order

## Purpose

Dokumen ini menentukan urutan resmi pembangunan DuskWeave.

Tidak ada model, IDE, agent, atau contributor yang boleh melewati dependency stage hanya karena implementasi tahap berikutnya terlihat mudah.

Prinsip utama:

> Campaign semantics first. Reality/evidence second. Execution third. Tools last.

---

# 0. Authority hierarchy

Urutan authority:

```text
1. PRODUCT THESIS / PRD
2. ACCEPTED ADR
3. DOMAIN CONTRACT
4. QUALITY BAR
5. AGENTS.md
6. SKILL.md
7. IMPLEMENTATION
```

Aturan:

- `PRD` menentukan **WHAT / WHY**.
- `ADR` menentukan **architectural HOW**.
- `Domain contract` menentukan interface yang boleh diimplementasikan.
- `AGENTS.md` menentukan cara contributor/model bekerja.
- `SKILL.md` menentukan workflow reasoning/implementation.
- Code tidak boleh menciptakan architecture baru tanpa authority di atasnya.

`AGENTS.md` dan `SKILL.md` tidak boleh mengubah keputusan PRD/ADR.

---

# 1. Hard invariants

Semua fase tunduk pada invariant berikut.

## INV-001 — No God Object

Tidak boleh ada object/service seperti:

```text
CampaignManager
SystemManager
AgentManager
GlobalContext
WorldManager
ToolManager
```

yang mengetahui sebagian besar sistem.

---

## INV-002 — Separate operational models

Lima model harus tetap terpisah:

```text
CyberTerrain
FootholdGraph
AttackPathView
ObjectiveState
CampaignTrajectory
```

Tidak boleh digabung menjadi satu `CampaignState` raksasa.

---

## INV-003 — Reasoning != execution

```text
Reasoning Worker
     ↓
Proposal
     ↓
Deterministic Validation
     ↓
Capability Gateway
     ↓
Executor
```

LLM tidak memperoleh arbitrary execution authority.

---

## INV-004 — Observation != fact

Observation harus melalui:

```text
Observation
→ validation/reconciliation
→ state delta
```

Inference tidak boleh diam-diam berubah menjadi fact.

---

## INV-005 — Sensitive data zero-retention

Raw:

```text
credential material
customer records
financial records
authentication stores
```

tidak boleh mencapai persistence layer.

Proof harus diturunkan sebelum persistence.

---

## INV-006 — Audit integrity

Campaign capability tidak boleh:

```text
delete authoritative evidence
alter audit evidence
disable authoritative audit
falsify audit records
```

---

## INV-007 — Defender isolation

Campaign reasoning tidak memperoleh real-time:

```text
EDR verdict
AV verdict
SIEM alert
SOC response
```

untuk mengoptimalkan adaptive evasion.

Control-gap assessment dilakukan oleh Observer/Grader.

---

# 2. Stage 0 — Repository authority bootstrap

Selesaikan sebelum PRD implementation detail.

Files:

```text
AGENTS.md
SKILL.md
QUALITY_BAR.md
docs/BUILD_ORDER.md
docs/ENGINEERING_STATE.md
docs/prd/README.md
docs/adr/README.md
```

Tidak ada runtime code pada stage ini.

Seal:

```text
DW-BOOTSTRAP-001
```

Exit criteria:

```text
authority hierarchy documented
build order documented
God-object rules documented
module-size rules documented
ADR/PRD conventions documented
```

---

# 3. Stage 1 — Product thesis

Author:

```text
PRD-000-product-thesis.md
```

Harus menjawab hanya:

```text
What is DuskWeave?
Who is it for?
What problem does it solve?
What is explicitly outside scope?
What makes campaign emulation different from vulnerability scanning?
```

Jangan masukkan:

```text
database schema
Rust structs
tool CLI
network protocols
```

Target:

```text
4–6 pages maximum
```

Seal:

```text
DW-PRD-000
```

---

# 4. Stage 2 — Campaign semantics

Author secara berurutan:

```text
PRD-001-campaign-lifecycle.md
PRD-002-cyber-terrain.md
PRD-003-access-and-footholds.md
PRD-004-expansion-loop.md
PRD-005-objective-loop.md
PRD-006-adaptation.md
```

Dependency:

```text
PRD-001
   ↓
PRD-002
   ↓
PRD-003
   ↓
PRD-004
   ↓
PRD-005
   ↓
PRD-006
```

Jangan mengerjakan `PRD-006` sebelum semantics previous state jelas.

---

# 5. Stage 3 — Foundation ADRs

Baru setelah PRD-000..006 accepted.

Author:

```text
ADR-001-modular-monolith.md
ADR-002-domain-boundaries.md
ADR-003-domain-events.md
ADR-004-rust-core-language.md
ADR-005-postgres-system-of-record.md
ADR-006-cyber-terrain-storage-model.md
ADR-007-foothold-and-path-separation.md
```

Dependencies:

```text
PRD-001..006
       ↓
ADR-001
       ↓
ADR-002
       ↓
ADR-003
```

Language decision:

```text
ADR-004
```

Storage decisions:

```text
ADR-005
ADR-006
```

Graph separation:

```text
ADR-007
```

Tidak ada tool integration pada stage ini.

---

# 6. Stage 4 — Reality & evidence

PRDs:

```text
PRD-007-observation-model.md
PRD-008-evidence.md
PRD-009-client-proof.md
PRD-010-sensitive-data-handling.md
```

ADRs:

```text
ADR-008-observation-fact-separation.md
ADR-009-evidence-immutability.md
ADR-010-proof-fingerprint.md
ADR-011-sensitive-data-barrier.md
ADR-012-engagement-proof-key.md
```

Output utama:

```text
Observation
EvidenceEnvelope
ProofEnvelope
Sensitive<T>
```

Belum ada Nmap.

Belum ada Nuclei.

Belum ada shell adapter.

---

# 7. Stage 5 — Domain contracts

Sekarang baru implement domain contracts dalam Rust.

Recommended crates:

```text
dw-types
dw-campaign
dw-terrain
dw-foothold
dw-pathing
dw-objective
dw-trajectory
dw-evidence
```

Implement hanya:

```text
types
state transitions
invariants
domain events
repository ports
```

Tidak ada external-tool execution.

Seal:

```text
DW-DOMAIN-001
```

---

# 8. Stage 6 — Campaign state engine

Implement:

```text
CampaignLifecycle

Mission
CampaignPosition
CampaignTransition
CampaignEvent
```

Minimum lifecycle:

```text
MISSION
↓
TARGET_RESEARCH
↓
RECONNAISSANCE
↓
ACCESS_PLANNING
↓
FOOTHOLD
↓
EXPANSION
↓
OBJECTIVE
↓
DWELL
↓
REENTRY
```

`ADAPTATION` bukan state akhir.

Ia adalah re-planning mechanism.

---

# 9. Stage 7 — Cyber Terrain

Implement:

```text
TerrainEntity
TerrainRelationship
TerrainObservation
TerrainDelta
TerrainSnapshot
Freshness
Confidence
```

Terrain layers:

```text
network
compute
identity
application
control
objective
temporal
```

No Attack Path calculation yet.

---

# 10. Stage 8 — Foothold Graph

Implement:

```text
Foothold
AccessContext
FootholdState
FootholdTransition
```

Lifecycle:

```text
CANDIDATE
→ VALIDATED
→ ACTIVE
→ DEGRADED
→ LOST
→ REENTRY_CANDIDATE
```

A foothold requires evidence.

---

# 11. Stage 9 — Path engine

Only now implement:

```text
CandidatePath
AttemptedPath
ProvenPath
BlockedPath
StalePath
```

Attack Path is derived from:

```text
Terrain
+
Footholds
+
Evidence
+
Capabilities
+
Objective
```

Never primary truth.

---

# 12. Stage 10 — Objective & trajectory

Implement separately:

```text
dw-objective
dw-trajectory
```

Objective answers:

```text
Why are we operating?
What remains unresolved?
```

Trajectory answers:

```text
What actually happened?
```

Never merge both.

---

# 13. Stage 11 — Capability system

PRD:

```text
PRD-011-capability-system.md
```

ADRs:

```text
ADR-013-capability-contract.md
ADR-014-capability-registry.md
ADR-015-execution-boundary.md
```

Introduce:

```text
CapabilitySpec
CapabilityRequest
CapabilityResult
```

Still no arbitrary shell.

---

# 14. Stage 12 — Execution architecture

PRDs:

```text
PRD-012-runtime-isolation.md
PRD-013-native-execution.md
```

ADRs:

```text
ADR-016-execution-broker.md
ADR-017-cross-process-contract.md
ADR-018-native-helper-boundary.md
ADR-019-polyglot-admission-policy.md
```

Runtime ownership:

```text
Rust:
    core
    execution broker
    authority

Go:
    adapters
    collectors
    integrations

Zig:
    selective native helper only

C/C++:
    required FFI only

Python/Nim:
    research plane initially
```

---

# 15. Stage 13 — First safe tool adapters

Only after capability/execution contracts are stable.

First wave:

```text
Nmap
httpx
DNS
osquery
```

Second wave:

```text
Amass
Subfinder
Nuclei
identity graph ingestion
```

Every adapter:

```text
tool-specific input
       ↓
adapter
       ↓
normalized result
       ↓
Observation
```

Tool output never becomes truth directly.

---

# 16. Stage 14 — Expansion engine

PRD:

```text
PRD-014-expansion-planning.md
```

Implement:

```text
Observe
↓
ExpandAccess proposal
↓
InternalPath
↓
Authorized transition
↓
New Foothold
↓
Observe
```

No hard-coded playbook.

---

# 17. Stage 15 — Chain Composer

PRD:

```text
PRD-015-chain-composition.md
```

ADR:

```text
ADR-020-chain-validation.md
```

Architecture:

```text
Reasoning Worker
      ↓
ChainCandidate
      ↓
Rust Chain Validator
      ↓
ValidatedChain
```

Composer cannot execute.

---

# 18. Stage 16 — Adaptation engine

Input:

```text
terrain delta
foothold changes
objective changes
evidence freshness
capability availability
```

Output:

```text
ReplanProposal
```

No execution authority.

---

# 19. Stage 17 — Stealth & control-gap architecture

PRDs:

```text
PRD-016-stealth-fidelity.md
PRD-017-campaign-tempo.md
PRD-018-operational-footprint.md
PRD-019-observer-plane.md
PRD-020-control-gap-assessment.md
```

ADRs:

```text
ADR-021-campaign-observer-separation.md
ADR-022-stealth-assessment-model.md
ADR-023-defender-telemetry-isolation.md
ADR-024-audit-integrity.md
```

Measured after execution:

```text
EDRVisibilityGap
AVCoverageGap
LoggingIntegrityGap
SIEMCorrelationGap
TemporalCorrelationGap
```

These are assessment outputs.

They are not adaptive runtime rewards.

---

# 20. Stage 18 — Client proof plane

Implement:

```text
ProofCollector
ProofTransformer
ProofEnvelope
ProofSealer
SensitiveDataBarrier
```

Hard requirement:

```text
raw secret/client data
        ↓
ephemeral memory
        ↓
proof transform
        ↓
zeroize
```

Persistence receives only proof/evidence metadata.

---

# 21. Stage 19 — Telemetry plane

First integrations:

```text
Sysmon
Windows Event Log
osquery
auditd
Zeek
Suricata
```

Purpose:

```text
campaign action
↔
expected telemetry
↔
actual telemetry
```

Campaign reasoning cannot query this plane while active.

---

# 22. Stage 20 — Autonomous grader

PRD:

```text
PRD-021-autonomous-grader.md
```

Dimensions:

```text
MissionProgress
TerrainAccuracy
FootholdValidity
PathValidation
ChainCoherence
EvidenceCompleteness
ObjectiveAchievement
AdaptationQuality
OperationalFootprint
DetectionCoverage
AuditIntegrity
Safety
```

---

# 23. Stage 21 — Campaign memory

Do this late.

Not before core semantics work.

Memory classes:

```text
episodic
semantic
procedural
```

Flow:

```text
campaign trajectory
→ lesson candidate
→ grader
→ validation
→ experience store
```

LLM cannot write directly into permanent experience.

---

# 24. Stage 22 — Threat campaign scenarios

Only now create campaign models.

Order:

```text
SCENARIO-001
APT41 DUST-derived campaign semantics

SCENARIO-002
APT41 newer modular-chain semantics

SCENARIO-003
Lazarus-derived chain composition

SCENARIO-004
Volt-Typhoon-derived native-operation / long-dwell scenario
```

Scenario definitions must use existing generic domains.

If implementing a scenario requires modifying CampaignKernel heavily:

```text
STOP
```

because generic architecture is insufficient.

---

# 25. Slice rule

Every implementation packet should fit:

```text
one architectural concern
+
one meaningful vertical behavior
+
tests
```

Target:

```text
runtime diff preferably < 400 LOC
```

Do not intentionally grow a slice merely to hit a minimum.

---

# 26. File rule

Preferred:

```text
< 300 LOC/module
```

Hard review threshold:

```text
400 LOC/module
```

When approaching the limit:

```text
split by responsibility
```

not:

```text
move functions randomly to helpers.py
```

---

# 27. STOP conditions

The model MUST STOP a slice if:

```text
PRD dependency not accepted
ADR dependency missing
requested change violates invariant
new domain boundary is required
scope expands materially
module would become God Object
runtime diff becomes mini-project
tests reveal architectural mismatch
```

Return:

```text
SPLIT_REQUIRED
```

with reason.

Do not silently expand scope.

---

# 28. Standard implementation loop

Every code slice:

```text
DESIGN AUTHORITY CHECK

↓

TDD

↓

IMPLEMENT

↓

LOCAL CHECKS

↓

SELF-REVIEW DIFF

↓

ARCHITECTURE INVARIANT CHECK

↓

PR

↓

ONE ADVERSARIAL REVIEW

↓

FIX VALID FINDINGS

↓

FINAL HEAD VALIDATION

↓

MERGE
```

---

# 29. Current next action

The next authoring sequence is exactly:

```text
1. AGENTS.md
2. SKILL.md
3. QUALITY_BAR.md
4. PRD-000 Product Thesis
5. PRD-001 Campaign Lifecycle
6. PRD-002 Cyber Terrain
7. PRD-003 Access & Footholds
8. PRD-004 Expansion Loop
9. PRD-005 Objective Loop
10. PRD-006 Adaptation
```

Only after these six PRDs are coherent:

```text
11. ADR-001 Modular Monolith
12. ADR-002 Domain Boundaries
13. ADR-003 Domain Events
14. ADR-004 Rust Core
15. ADR-005 PostgreSQL SoR
16. ADR-006 Terrain Storage
17. ADR-007 Foothold/Path Separation
```

Do not reverse this order.

---

# 30. First design seal

The first meaningful seal is:

```text
DW-FOUNDATION-001
```

It consists of:

```text
AGENTS.md
SKILL.md
QUALITY_BAR.md

PRD-000
PRD-001
PRD-002
PRD-003
PRD-004
PRD-005
PRD-006

ADR-001
ADR-002
ADR-003
ADR-004
ADR-005
ADR-006
ADR-007
```

No runtime implementation should begin before this seal is coherent.

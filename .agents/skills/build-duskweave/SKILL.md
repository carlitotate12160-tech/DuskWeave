---
name: build-duskweave
description: Guidelines and mandatory reasoning protocol for designing, implementing, reviewing, or extending DuskWeave.
---

# Skill: build-duskweave

## Purpose

Use this skill when designing, implementing, reviewing, or extending DuskWeave.

DuskWeave is a persistent campaign reasoning and adversary-emulation platform.

Its defining architecture is based on four nested operational loops:

```text
STRATEGIC LOOP
    ↓
ACCESS LOOP
    ↓
EXPANSION LOOP
    ↓
OBJECTIVE LOOP

ADAPTATION overlays all four.
```

---

## First principle

Always ask:

```text
What is the current campaign state?

What is actually known?

What evidence supports it?

What terrain changed?

What access is validated?

What objective remains?

Which domain owns this decision?
```

Do not begin with:

```text
Which tool should I run?
```

---

## Mandatory reasoning order

For every feature:

### 1. Identify product requirement

Locate relevant PRD.

If none exists:

```text
STOP
```

and create/design PRD first.

### 2. Identify architectural decision

Locate relevant ADR.

If an architectural decision is missing:

```text
STOP
```

and create a focused ADR.

### 3. Identify bounded context

Examples:

```text
campaign
terrain
foothold
pathing
objective
trajectory
evidence
capability
execution
telemetry
proof
grading
```

### 4. Define contract

Before implementation define:

```text
input
output
state transition
domain event
failure semantics
evidence requirements
```

### 5. Test behavior

Write tests against the contract.

### 6. Implement the smallest complete vertical behavior.

---

## Campaign mental model

Do not model the platform as:

```text
recon
→ exploit
→ lateral
→ exfil
```

Use:

```text
Mission
↓
Target Research
↓
Reconnaissance
↓
Access Path Selection
↓
Access Validation
↓
Foothold
↓
Situational Awareness
↓
Expansion Loop
↓
Objective Discovery
↓
Objective Validation
↓
Collection Exercise
↓
Staging Exercise
↓
Transfer Exercise
↓
Objective Review
↓
Dwell / Re-entry / Retask
```

Adaptation may redirect the campaign to an earlier state.

---

## Five-state-model rule

Keep these separate:

### CyberTerrain

```text
What exists?
```

### FootholdGraph

```text
Where does the campaign have validated access?
```

### AttackPathView

```text
Where could the campaign go,
and what transitions are proven?
```

### ObjectiveState

```text
Why is the campaign operating,
and what remains?
```

### CampaignTrajectory

```text
What actually happened over time?
```

Do not create a combined universal state model.

---

## Terrain reasoning

Terrain must include at least:

```text
network
compute
identity
application
control
objective
temporal
```

Every fact should carry:

```text
source
confidence
first_seen
last_seen
evidence_refs
```

Valid confidence states:

```text
OBSERVED
CORROBORATED
INFERRED
HYPOTHETICAL
STALE
REFUTED
```

---

## Path reasoning

Attack paths are derived.

A path may be:

```text
CANDIDATE
ATTEMPTED
PROVEN
BLOCKED
STALE
```

Never mark a path `PROVEN` without corresponding evidence.

---

## Tool reasoning

Tools are replaceable implementation details.

Model:

```text
SemanticAction
↓
CapabilityResolver
↓
ToolAdapter
↓
NormalizedResult
↓
Observation
```

Never let domain code depend directly on Nmap, Nuclei, BloodHound, OS commands, or another tool's native result schema.

---

## Language choice

Prefer:

```text
Rust
```

for correctness-sensitive stateful domain logic.

Prefer:

```text
Go
```

for concurrent integrations, collectors, tool adapters, and IO workers.

Use Zig only when a concrete native-helper requirement justifies it.

C/C++ require interoperability justification.

Python/Nim remain research-plane by default.

---

## Stealth reasoning

Stealth fidelity is assessed through:

```text
OperationalFootprint
EDRVisibilityGap
AVCoverageGap
LoggingIntegrityGap
SIEMCorrelationGap
TemporalCorrelationGap
```

Do not feed defender detection results into active campaign reasoning as an adaptive evasion oracle.

---

## Sensitive proof reasoning

For sensitive client material:

```text
bounded read
↓
ephemeral sensitive buffer
↓
approved proof transform
↓
zeroize raw material
↓
persist proof only
```

Never design:

```text
store raw
↓
hash later
↓
delete
```

because persistence may leave copies in WAL, backups, replicas, snapshots, logs, or crash artifacts.

---

## Anti-God-Object test

Before approving a component ask:

```text
Does it know multiple unrelated domains?

Does it hold multiple repositories?

Does it know campaign + terrain + tools + telemetry + proof?

Does it select strategy and execute actions?

Would changing one domain frequently modify this class?
```

If yes:

```text
REJECT ARCHITECTURE
```

and split by responsibility.

---

## Slice quality check

A good slice has:

```text
one clear authority
one bounded context
one contract
one behavior
focused tests
small diff
```

A bad slice contains:

```text
new architecture
+
new database model
+
new tool adapter
+
new agent
+
new telemetry integration
```

Return:

```text
SPLIT_REQUIRED
```

instead.

---

## Review checklist

Before declaring a slice ready:

```text
PRD satisfied?
ADR satisfied?
dependency direction intact?
God Object introduced?
observation/fact boundary intact?
sensitive-data boundary intact?
execution authority respected?
evidence generated?
state transition explicit?
failure semantics tested?
module-size budget respected?
```

Only then proceed to delivery.

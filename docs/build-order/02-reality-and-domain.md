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

Primary outputs:

```text
Observation
EvidenceEnvelope
ProofEnvelope
Sensitive<T>
```

No Nmap yet.

No Nuclei yet.

No shell adapter yet.

---

# M0 lane — accepted bounded sequencing

The owner requested minimum M0 contract authoring on 2026-09-30.
[M0 mission/planning/history](../contracts/M0-mission-authority-history.md)
and its sequencing exception were ACCEPTED by the owner on 2026-09-30.
Only local Mission/planning responsibility and required Trajectory history may
be implemented before full Stage 5/10 coverage, through issued bounded packets.
M0A/B/C remain vertical deliveries; target acquisition/execution is unauthorized.

ADR-003/005 durability and append-only history remain mandatory. ADR-009..012
are deferred only for this non-evidence/non-proof/non-custody/non-acquisition
behavior; the contract identifies their later triggers. No five-model skeleton,
tool execution or full DW-DOMAIN-001 seal is implied. Full-stage requirements
below remain in force outside an accepted bounded M0 lane.

---

# M1 lane — accepted enabling amendment

The [M1 enabling architecture/owner contracts](../contracts/M1-enabling-architecture.md)
resolve the accepted product contract section 11. The owner ACCEPTED R1 and scoped
ADR-009/011/013/015/016/017 together on 2026-10-06. Publish this accepted authority
before Work measures and issues a bounded M1 runtime packet; acceptance alone
is not a ready implementation packet or current target permission.
The linked dependency table preserves full-stage prerequisites, rebinds the
ADR-011 proof prerequisite only for non-proof acquisition, and limits Terrain/
Pathing/Trajectory to the selected external-orientation lane.
M0 seal/evidence and its non-acquisition exception remain unchanged.
No full DW-DOMAIN-001 seal, Access/Objective scaffolding or target permission.

---

# 7. Stage 5 — Domain contracts

Only now implement the domain contracts in Rust.

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

Implement only:

```text
types
state transitions
invariants
domain events
repository ports
```

There is no external-tool execution.

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

`ADAPTATION` is not a terminal state.

It is a re-planning mechanism.

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
EpistemicStatus (OBSERVED, CORROBORATED, INFERRED, HYPOTHETICAL, STALE, REFUTED, PROVISIONAL)
EpistemicTier (Tier 1 fast sourced observation, Tier 2 supporting evidence, Tier 3 full reconciliation)
```

Terrain layers per PRD-002 §2:

```text
network
compute
identity
application
control
objective-relevance
temporal
```

No Attack Path calculation yet.

---

# 10. Stage 8 — Foothold Graph

Implement:

```text
Foothold
AccessContext
FootholdHealth
FootholdTransition
```

Lifecycle governed by PRD-003 invariants A-1 through A-7.
Domain contracts define specific states and transitions.

Key semantic conditions (not a rigid state machine):

```text
candidate access (AttackPathView hypothesis)
transient access (unvalidated observed effect)
validated foothold (evidence of bounded execution)
stale (freshness expired, proof needs refresh)
presumed loss (configurable unreachability threshold)
confirmed loss (affirmative revocation evidence)
re-entry eligibility (hypothesis, not recovered access)
```

A foothold requires evidence. Presumed loss and confirmed
loss are distinct conditions with different recovery paths.

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

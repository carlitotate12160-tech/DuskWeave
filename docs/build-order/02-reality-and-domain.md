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

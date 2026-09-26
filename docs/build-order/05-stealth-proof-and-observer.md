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

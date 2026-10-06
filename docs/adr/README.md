# DuskWeave Architecture Decision Records (ADR)

## Purpose & Conventions

This directory contains all architecture decision records for DuskWeave.

ADRs hold the second level of authority (**Level 2: Architectural HOW**) in the DuskWeave repository. An ADR defines the technical architecture structure derived from product requirements (PRDs).

---

## 1. ADR Format & Conventions

Every ADR is written using the following fixed structure:

1. **Title & Status**:
   - Status: `PROPOSED` | `ACCEPTED` | `REJECTED` | `DEPRECATED` | `SUPERSEDED`
2. **Context & Problem Statement**:
   - Architectural background and the needs underlying the decision.
3. **Decision Drivers**:
   - Driving factors (performance, determinism, isolation bounds, invariant enforcement).
4. **Considered Options**:
   - Solution alternatives considered, with their strengths and weaknesses.
5. **Decision Outcome**:
   - The chosen option and the reasons for choosing it.
6. **Consequences**:
   - Architectural impact (positive, negative, and risk mitigation).
7. **Invariant Compliance Matrix**:
   - Explicit compliance explanation for INV-001 through INV-007.

---

## 2. ADR Registry & Dependency Index

ADR-001..008 are authored and ACCEPTED. ADR-009/011/013/015/016/017 R1 and the
M1 enabling owner/sequencing contract were ACCEPTED by the product owner on
2026-10-06 for the selected M1 lane only; no full-stage acceptance or seal is claimed.
ADR-010/012/014/018..024 remain reserved/unwritten slots. Legacy PROPOSED labels
for those slots do not satisfy dependencies. Full-stage prerequisites remain;
the [accepted M1 sequencing amendment](../contracts/M1-enabling-architecture.md)
records the exact selected-lane substitutions; all other prerequisites remain unchanged.

| ADR ID | Decision Title | Related Stage | Direct Dependency | Status |
| :--- | :--- | :--- | :--- | :--- |
| **ADR-001** | Modular Monolith Architecture | Stage 3 | PRD-000..006 | `ACCEPTED` |
| **ADR-002** | Domain Boundaries Definition | Stage 3 | ADR-001 | `ACCEPTED` |
| **ADR-003** | Domain Events Architecture | Stage 3 | ADR-002 | `ACCEPTED` |
| **ADR-004** | Rust Core Language Selection | Stage 3 | ADR-001..003 | `ACCEPTED` |
| **ADR-005** | PostgreSQL System of Record | Stage 3 | ADR-003, ADR-004 | `ACCEPTED` |
| **ADR-006** | Cyber Terrain Storage Model | Stage 3 | ADR-005 | `ACCEPTED` |
| **ADR-007** | Foothold & Path Separation | Stage 3 | ADR-002, ADR-006 | `ACCEPTED` |
| **ADR-008** | [Observation & Fact Separation](ADR-008-observation-fact-separation.md) | Stage 4 | PRD-007, ADR-003 | `ACCEPTED` |
| **ADR-009** | [Evidence Immutability / M1 Continuity](ADR-009-evidence-immutability.md) | Stage 4 | PRD-008, ADR-008 | `ACCEPTED — M1 only` |
| **ADR-010** | Proof Fingerprint Architecture | Stage 4 | PRD-009, ADR-009 | `PROPOSED` |
| **ADR-011** | [Sensitive Data Barrier / M1 Non-Proof](ADR-011-sensitive-data-barrier.md) | Stage 4 | Full: PRD-010, ADR-010; M1 accepted: PRD-010, ADR-008/009 | `ACCEPTED — M1 only` |
| **ADR-012** | Engagement Proof Key | Stage 4 | ADR-010, ADR-011 | `PROPOSED` |
| **ADR-013** | [Fixed M1 Capability Contract](ADR-013-capability-contract.md) | Stage 11 | Full: PRD-011, Stage 10; M1 accepted: accepted PRDs/M1 scope, ADR-009/011 | `ACCEPTED — M1 only` |
| **ADR-014** | Capability Registry Model | Stage 11 | ADR-013 | `PROPOSED` |
| **ADR-015** | [M1 Execution Boundary](ADR-015-execution-boundary.md) | Stage 11 | Full: ADR-014; M1 accepted: ADR-011/013 | `ACCEPTED — M1 only` |
| **ADR-016** | [M1 Execution Broker](ADR-016-execution-broker.md) | Stage 12 | Full: PRD-012..013, ADR-015; M1 accepted: ADR-013/015, ADR-003/005 | `ACCEPTED — M1 only` |
| **ADR-017** | [M1 Cross-Process Contract](ADR-017-cross-process-contract.md) | Stage 12 | ADR-016; M1 accepted: ADR-009/011/013/015/016 | `ACCEPTED — M1 only` |
| **ADR-018** | Native Helper Boundary | Stage 12 | ADR-017 | `PROPOSED` |
| **ADR-019** | Polyglot Admission Policy | Stage 12 | ADR-018 | `PROPOSED` |
| **ADR-020** | Chain Validation Architecture | Stage 15 | PRD-015 | `PROPOSED` |
| **ADR-021** | Campaign & Observer Separation | Stage 17 | PRD-016..020 | `PROPOSED` |
| **ADR-022** | Stealth Assessment Model | Stage 17 | ADR-021 | `PROPOSED` |
| **ADR-023** | Defender Telemetry Isolation | Stage 17 | ADR-021, ADR-022 | `PROPOSED` |
| **ADR-024** | Audit Integrity Model | Stage 17 | ADR-023 | `PROPOSED` |

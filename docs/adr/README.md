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

This index lists authored decisions; exact scope/status is owned by each ADR.
The [M1 sequencing contract](../contracts/M1-enabling-architecture.md)
owns scoped dependency substitutions and amendment acceptance; no full-stage
completion, deployment qualification or seal is inferred from this index.

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
| **ADR-011** | [Sensitive Data Barrier / M1 Non-Proof](ADR-011-sensitive-data-barrier.md) | Stage 4 | Full: PRD-010, ADR-010; M1 accepted: PRD-010, ADR-008/009 | `ACCEPTED — M1 only` |
| **ADR-013** | [Fixed M1 Capability Contract](ADR-013-capability-contract.md) | Stage 11 | Full: PRD-011, Stage 10; M1 accepted: accepted PRDs/M1 scope, ADR-009/011 | `ACCEPTED — M1 only` |
| **ADR-015** | [M1 Execution Boundary](ADR-015-execution-boundary.md) | Stage 11 | Full: ADR-014; M1 accepted: ADR-011/013 | `ACCEPTED — M1 only` |
| **ADR-016** | [M1 Execution Broker](ADR-016-execution-broker.md) | Stage 12 | Full: PRD-012..013, ADR-015; M1 accepted: ADR-013/015, ADR-003/005 | `ACCEPTED — M1 only` |
| **ADR-017** | [M1 Cross-Process Contract](ADR-017-cross-process-contract.md) | Stage 12 | ADR-016; M1 accepted: ADR-009/011/013/015/016 | `ACCEPTED — M1 only` |


Reserved, unwritten IDs: `ADR-010/012/014/018..024`. IDs are not reused; titles and full-stage
dependencies remain in [build-order navigation](../BUILD_ORDER.md). These slots
are not authored proposals or accepted authority and do not satisfy dependencies.
The [M1 sequencing contract](../contracts/M1-enabling-architecture.md#2-accepted-bounded-sequencing-amendment)
owns selected-lane substitutions. Add a registry row when its document exists.

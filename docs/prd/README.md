# DuskWeave Product Requirements Documents (PRD)

## Purpose & Conventions

This directory contains all product requirements documents for DuskWeave.

PRDs hold the highest level of authority (**Level 1: WHAT / WHY**) in the DuskWeave repository. No technical architecture or code implementation is legitimate without a supporting PRD that has reached `ACCEPTED` status.

---

## 1. PRD Writing Rules

Every PRD must comply with the following rules:
1. **Focus on Semantics, Problems, and Constraints**:
   - Describe the specific problem to be solved.
   - Describe the relevant actors or roles.
   - Describe success criteria and the semantic model.
   - Describe what is explicitly out of scope.
2. **Technical Detail Prohibition**:
   - Must not contain database schemas (SQL / relational schema).
   - Must not contain programming code or structs (Rust/Go/C++).
   - Must not contain command-line tool arguments (CLI flags).
   - Must not contain low-level network protocol specifications.
3. **Size Limit**:
   - Target length of 4-6 structured pages.

---

## 2. PRD Registry & Dependency Index

| PRD ID | Document Title | Related Stage | Direct Dependency | Status |
| :--- | :--- | :--- | :--- | :--- |
| **PRD-000** | Product Thesis | Stage 1 | None | `ACCEPTED` |
| **PRD-001** | Campaign Lifecycle | Stage 2 | PRD-000 | `ACCEPTED` |
| **PRD-002** | Cyber Terrain | Stage 2 | PRD-001 | `ACCEPTED` |
| **PRD-003** | Access & Footholds | Stage 2 | PRD-002 | `ACCEPTED` |
| **PRD-004** | Expansion Loop | Stage 2 | PRD-003 | `ACCEPTED` |
| **PRD-005** | Objective Loop | Stage 2 | PRD-004 | `ACCEPTED` |
| **PRD-006** | Adaptation | Stage 2 | PRD-005 | `ACCEPTED` |
| **PRD-007** | Observation Model | Stage 4 | ADR-001..007 | `ACCEPTED` |
| **PRD-008** | Evidence Model | Stage 4 | PRD-007 | `ACCEPTED` |
| **PRD-009** | Client Proof | Stage 4 | PRD-008 | `ACCEPTED` |
| **PRD-010** | Sensitive Data Handling | Stage 4 | PRD-009 | `ACCEPTED` |


Reserved, unwritten IDs: `PRD-011..021`. IDs are not reused; titles and full-stage
dependencies remain in [build-order navigation](../BUILD_ORDER.md). These slots
are not authored proposals or accepted authority and do not satisfy dependencies.
The [M1 sequencing contract](../contracts/M1-enabling-architecture.md#2-accepted-bounded-sequencing-amendment)
owns selected-lane substitutions. Add a registry row when its document exists.

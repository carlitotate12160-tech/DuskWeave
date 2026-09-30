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
| **PRD-011** | Capability System | Stage 11 | Stage 10 | `PLANNED` |
| **PRD-012** | Runtime Isolation | Stage 12 | PRD-011 | `PLANNED` |
| **PRD-013** | Native Execution | Stage 12 | PRD-012 | `PLANNED` |
| **PRD-014** | Expansion Planning | Stage 14 | Stage 13 | `PLANNED` |
| **PRD-015** | Chain Composition | Stage 15 | PRD-014 | `PLANNED` |
| **PRD-016** | Stealth Fidelity | Stage 17 | Stage 16 | `PLANNED` |
| **PRD-017** | Campaign Tempo | Stage 17 | PRD-016 | `PLANNED` |
| **PRD-018** | Operational Footprint | Stage 17 | PRD-017 | `PLANNED` |
| **PRD-019** | Observer Plane | Stage 17 | PRD-018 | `PLANNED` |
| **PRD-020** | Control-Gap Assessment | Stage 17 | PRD-019 | `PLANNED` |
| **PRD-021** | Autonomous Grader | Stage 20 | Stage 19 | `PLANNED` |

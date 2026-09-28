# DuskWeave Architecture Decision Records (ADR)

## Purpose & Conventions

Direktori ini berisi seluruh dokumen keputusan arsitektur (*Architecture Decision Records* / ADR) untuk DuskWeave.

ADR menduduki tingkat otoritas kedua (**Level 2: Architectural HOW**) dalam repositori DuskWeave. ADR menentukan struktur arsitektur teknis turunan dari kebutuhan produk (PRD).

---

## 1. Format & Konvensi ADR

Setiap ADR disusun menggunakan struktur baku berikut:

1. **Title & Status**:
   - Status: `PROPOSED` | `ACCEPTED` | `REJECTED` | `DEPRECATED` | `SUPERSEDED`
2. **Context & Problem Statement**:
   - Latar belakang arsitektur dan kebutuhan yang mendasari keputusan.
3. **Decision Drivers**:
   - Faktor pendorong (performa, determinisme, batas isolasi, penegakan invariant).
4. **Considered Options**:
   - Alternatif solusi yang dipertimbangkan beserta kelebihan dan kelemahannya.
5. **Decision Outcome**:
   - Opsi terpilih dan alasan pemilihannya.
6. **Consequences**:
   - Dampak arsitektur (positif, negatif, dan mitigasi risiko).
7. **Invariant Compliance Matrix**:
   - Penjelasan kepatuhan eksplisit terhadap INV-001 hingga INV-007.

---

## 2. ADR Registry & Dependency Index

ADR-001 and ADR-002 are authored and ACCEPTED; ADR-003 is authored and PROPOSED. Remaining entries are reserved decision slots, not authored or accepted documents. Their legacy PROPOSED labels do not satisfy dependencies; verify the corresponding file and acceptance before proceeding.

| ADR ID | Judul Keputusan | Tahapan Terkait | Ketergantungan Langsung | Status |
| :--- | :--- | :--- | :--- | :--- |
| **ADR-001** | Modular Monolith Architecture | Stage 3 | PRD-000..006 | `ACCEPTED` |
| **ADR-002** | Domain Boundaries Definition | Stage 3 | ADR-001 | `ACCEPTED` |
| **ADR-003** | Domain Events Architecture | Stage 3 | ADR-002 | `PROPOSED` |
| **ADR-004** | Rust Core Language Selection | Stage 3 | ADR-001..003 | `PROPOSED` |
| **ADR-005** | PostgreSQL System of Record | Stage 3 | ADR-003, ADR-004 | `PROPOSED` |
| **ADR-006** | Cyber Terrain Storage Model | Stage 3 | ADR-005 | `PROPOSED` |
| **ADR-007** | Foothold & Path Separation | Stage 3 | ADR-002, ADR-006 | `PROPOSED` |
| **ADR-008** | Observation & Fact Separation | Stage 4 | PRD-007, ADR-003 | `PROPOSED` |
| **ADR-009** | Evidence Immutability | Stage 4 | PRD-008, ADR-008 | `PROPOSED` |
| **ADR-010** | Proof Fingerprint Architecture | Stage 4 | PRD-009, ADR-009 | `PROPOSED` |
| **ADR-011** | Sensitive Data Barrier | Stage 4 | PRD-010, ADR-010 | `PROPOSED` |
| **ADR-012** | Engagement Proof Key | Stage 4 | ADR-010, ADR-011 | `PROPOSED` |
| **ADR-013** | Capability Contract | Stage 11 | PRD-011, Stage 10 | `PROPOSED` |
| **ADR-014** | Capability Registry Model | Stage 11 | ADR-013 | `PROPOSED` |
| **ADR-015** | Execution Boundary | Stage 11 | ADR-014 | `PROPOSED` |
| **ADR-016** | Execution Broker | Stage 12 | PRD-012..013, ADR-015 | `PROPOSED` |
| **ADR-017** | Cross-Process Contract | Stage 12 | ADR-016 | `PROPOSED` |
| **ADR-018** | Native Helper Boundary | Stage 12 | ADR-017 | `PROPOSED` |
| **ADR-019** | Polyglot Admission Policy | Stage 12 | ADR-018 | `PROPOSED` |
| **ADR-020** | Chain Validation Architecture | Stage 15 | PRD-015 | `PROPOSED` |
| **ADR-021** | Campaign & Observer Separation | Stage 17 | PRD-016..020 | `PROPOSED` |
| **ADR-022** | Stealth Assessment Model | Stage 17 | ADR-021 | `PROPOSED` |
| **ADR-023** | Defender Telemetry Isolation | Stage 17 | ADR-021, ADR-022 | `PROPOSED` |
| **ADR-024** | Audit Integrity Model | Stage 17 | ADR-023 | `PROPOSED` |

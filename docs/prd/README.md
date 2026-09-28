# DuskWeave Product Requirements Documents (PRD)

## Purpose & Conventions

Direktori ini berisi seluruh dokumen kebutuhan produk (*Product Requirements Documents* / PRD) untuk DuskWeave.

PRD menduduki tingkat otoritas tertinggi (**Level 1: WHAT / WHY**) dalam repositori DuskWeave. Tidak ada arsitektur teknis atau implementasi kode yang sah tanpa PRD pendukung yang telah berstatus `ACCEPTED`.

---

## 1. Aturan Penulisan PRD

Setiap PRD wajib mematuhi aturan berikut:
1. **Fokus pada Semantik, Masalah, dan Batasan**:
   - Menjelaskan masalah spesifik yang ingin dipecahkan.
   - Menjelaskan siapa aktor atau peran terkait.
   - Menjelaskan kriteria keberhasilan dan model semantik.
   - Menjelaskan apa yang secara eksplisit berada di luar cakupan (*out of scope*).
2. **Larangan Detail Teknis**:
   - Dilarang memuat skema database (SQL / relational schema).
   - Dilarang memuat kode pemrograman atau struct (Rust/Go/C++).
   - Dilarang memuat argumen command-line tool (CLI flags).
   - Dilarang memuat spesifikasi protokol jaringan tingkat rendah.
3. **Batas Ukuran**:
   - Target panjang 4–6 halaman terstruktur.

---

## 2. PRD Registry & Dependency Index

| PRD ID | Judul Dokumen | Tahapan Terkait | Ketergantungan Langsung | Status |
| :--- | :--- | :--- | :--- | :--- |
| **PRD-000** | Product Thesis | Stage 1 | None | `ACCEPTED` |
| **PRD-001** | Campaign Lifecycle | Stage 2 | PRD-000 | `ACCEPTED` |
| **PRD-002** | Cyber Terrain | Stage 2 | PRD-001 | `ACCEPTED` |
| **PRD-003** | Access & Footholds | Stage 2 | PRD-002 | `ACCEPTED` |
| **PRD-004** | Expansion Loop | Stage 2 | PRD-003 | `ACCEPTED` |
| **PRD-005** | Objective Loop | Stage 2 | PRD-004 | `ACCEPTED` |
| **PRD-006** | Adaptation | Stage 2 | PRD-005 | `ACCEPTED` |
| **PRD-007** | Observation Model | Stage 4 | ADR-001..007 | `PROPOSED` |
| **PRD-008** | Evidence Model | Stage 4 | PRD-007 | `PLANNED` |
| **PRD-009** | Client Proof | Stage 4 | PRD-008 | `PLANNED` |
| **PRD-010** | Sensitive Data Handling | Stage 4 | PRD-009 | `PLANNED` |
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

# SKILL.md — DuskWeave Reasoning & Implementation Skill

## Overview

Dokumen ini adalah panduan operasional dan alur penalaran (*reasoning workflow*) untuk merancang, mengimplementasikan, dan memvalidasi setiap tahapan dalam **DuskWeave**.

Pegang teguh prinsip:
> **Campaign semantics first. Reality/evidence second. Execution third. Tools last.**

---

## 1. Navigasi Tahapan Build Order

Setiap kontributor wajib memeriksa `docs/BUILD_ORDER.md` sebelum memulai tugas apa pun. Rute ketergantungan bertingkat tidak boleh dilompati:

```text
Stage 0: Authority Bootstrap (AGENTS.md, SKILL.md, QUALITY_BAR.md, registries)
   ↓
Stage 1: Product Thesis (PRD-000)
   ↓
Stage 2: Campaign Semantics (PRD-001 → PRD-002 → PRD-003 → PRD-004 → PRD-005 → PRD-006)
   ↓
Stage 3: Foundation ADRs (ADR-001 → ADR-002 → ADR-003 → ADR-004/005/006/007)
   ↓
Stage 4: Reality & Evidence (PRD-007..010 & ADR-008..012)
   ↓
Stage 5: Domain Contracts in Rust (dw-types, dw-campaign, dw-terrain, etc.)
   ↓
... [Stages 6 through 22 sesuai BUILD_ORDER.md]
```

---

## 2. Standar Penyusunan PRD (Product Requirements Document)

Saat menulis PRD (terutama Stage 1 dan Stage 2):
1. **Fokus Murni pada WHAT dan WHY**:
   - Menjelaskan masalah yang dipecahkan.
   - Siapa penggunanya / apa perannya.
   - Definisi semantik dan batasan domain.
   - Apa yang secara eksplisit berada di luar cakupan (*out of scope*).
2. **Dilarang Keras Memasukkan**:
   - Skema database (SQL / relational tables).
   - Definisi struct Rust atau kode implementasi.
   - Command-line flags tool pihak ketiga (Nmap, Nuclei, dsb).
   - Protokol jaringan teknis tingkat rendah.
3. **Format & Batasan**:
   - Maksimum 4–6 halaman terstruktur.
   - Format penamaan: `docs/prd/PRD-xxx-<topic>.md`.

---

## 3. Standar Penyusunan ADR (Architecture Decision Record)

Saat menyusun ADR (Stage 3 dst.):
1. **Format MADR Terstruktur**:
   - **Title & Status**: PROPOSED / ACCEPTED / SUPERSEDED.
   - **Context & Problem Statement**: Mengapa keputusan ini diperlukan.
   - **Decision Drivers**: Kendala, performa, keamanan, isolasi, pemisahan domain.
   - **Considered Options**: Alternatif yang dipertimbangkan beserta trade-off.
   - **Decision Outcome**: Pilihan arsitektur yang disepakati dan alasannya.
   - **Consequences**: Dampak positif dan negatif.
   - **Invariant Compliance**: Bukti kepatuhan eksplisit terhadap INV-001 hingga INV-007.
2. **Format Penamaan**: `docs/adr/ADR-xxx-<topic>.md`.

---

## 4. Standar Domain Contract (Stage 5)

Ketika memasuki Stage 5 (Domain Contracts dalam Rust):
1. **Gunakan Rust Type System untuk Memaksakan Invariant**:
   - Gunakan *typestate pattern* untuk memodelkan transisi siklus hidup (misalnya `Foothold<Candidate>`, `Foothold<Validated>`).
   - Tipe data primitif dibungkus dalam *newtype pattern* untuk mencegah *primitive obsession* (misalnya `EntityId`, `FootholdId`).
2. **Tanpa Runtime Execution / Tool Adapters**:
   - Domain crates (`dw-types`, `dw-campaign`, `dw-terrain`, `dw-foothold`, dll.) hanya berisi:
     - Definisi tipe data (*types*).
     - State transitions & pure functions.
     - Penegakan invariant.
     - Definisi Domain Events.
     - Repository Ports (trait interface).
   - Tidak boleh ada panggilan I/O langsung, network socket, atau pemanggilan child-process CLI di layer domain.

---

## 5. Preflight Checklist untuk Setiap Perubahan

Sebelum mengusulkan atau menulis perubahan:
- [ ] Apakah tahapan sebelumnya dalam `docs/BUILD_ORDER.md` sudah berstatus `ACCEPTED` / `SEALED`?
- [ ] Apakah perubahan ini bebas dari pola God Object (INV-001)?
- [ ] Apakah 5 model operasional tetap terpisah (INV-002)?
- [ ] Apakah LLM hanya menghasilkan Proposal tanpa eksekusi langsung (INV-003)?
- [ ] Apakah observasi diverifikasi sebelum menjadi state delta (INV-004)?
- [ ] Apakah penanganan data sensitif menjamin zero-retention (INV-005)?
- [ ] Apakah integritas audit dan bukti tetap terjaga (INV-006)?
- [ ] Apakah telemetri defender terisolasi dari proses reasoning (INV-007)?
- [ ] Apakah ukuran file dan perubahan modul berada di bawah ambang batas `< 300 LOC` (maks 400 LOC)?
- [ ] Apakah pendekatan TDD diterapkan (test merah sebelum implementasi)?

Jika salah satu checklist tidak terpenuhi, hentikan pengerjaan dan kembalikan `SPLIT_REQUIRED`.

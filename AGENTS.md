# AGENTS.md — DuskWeave Contributor & Agent Protocol

## Purpose & Overview

Dokumen ini adalah protokol otoritatif bagi model AI, automated agent, IDE, dan kontributor manusia yang bekerja pada repositori **DuskWeave**.

Prinsip fundamental DuskWeave:

> **Campaign semantics first. Reality/evidence second. Execution third. Tools last.**

Setiap interaksi dan kontribusi pada repositori ini wajib tunduk pada hirarki otoritas dan invariant yang ditetapkan di bawah ini.

---

## 0. Authority Hierarchy

Urutan otoritas mutlak dalam DuskWeave:

```text
1. PRODUCT THESIS / PRD     (Menentukan WHAT / WHY)
2. ACCEPTED ADR             (Menentukan Architectural HOW)
3. DOMAIN CONTRACT          (Menentukan interface yang boleh diimplementasikan)
4. QUALITY BAR              (Menentukan standar kode, budget ukuran, dan testing)
5. AGENTS.md                (Menentukan cara model/agent/kontributor bekerja)
6. SKILL.md                 (Menentukan workflow reasoning & implementasi)
7. IMPLEMENTATION           (Kode sumber dan automated test)
```

### Aturan Hirarki:
- Code tidak boleh menciptakan arsitektur baru tanpa authority yang lebih tinggi (PRD & ADR yang sudah di-*accept*).
- `AGENTS.md` dan `SKILL.md` tidak boleh mengubah atau melemahkan keputusan yang telah disepakati dalam PRD atau ADR.
- Tidak ada kontributor atau agent yang boleh melompati *stage* atau *dependency* hanya karena tahap berikutnya tampak mudah atau menarik.

---

## 1. Hard Invariants (Wajib Dipatuhi Tanpa Pengecualian)

### INV-001 — No God Object
Dilarang keras membuat atau mengintroduksi service/object monolitik serba tahu, seperti:
```text
CampaignManager
SystemManager
AgentManager
GlobalContext
WorldManager
ToolManager
```
Setiap modul harus memiliki tanggung jawab tunggal (*single responsibility*) dengan batasan domain yang eksplisit.

### INV-002 — Separate Operational Models
Lima model operasional harus tetap terpisah secara tegas:
1. `CyberTerrain`
2. `FootholdGraph`
3. `AttackPathView`
4. `ObjectiveState`
5. `CampaignTrajectory`

Dilarang keras menggabungkan kelima model ini ke dalam satu struktur raksasa (misalnya `CampaignState`).

### INV-003 — Reasoning != Execution
LLM atau Reasoning Worker tidak memiliki hak eksekusi langsung (*zero arbitrary execution authority*). Alur wajib:
```text
Reasoning Worker
     ↓
Proposal
     ↓
Deterministic Validation
     ↓
Capability Gateway
     ↓
Executor
```

### INV-004 — Observation != Fact
Hasil observasi atau output tool bukan fakta langsung (*ground truth*). Alur wajib:
```text
Observation
     ↓
Validation / Reconciliation
     ↓
State Delta
```
Inference spekulatif dilarang diam-diam berubah menjadi fakta graf.

### INV-005 — Sensitive Data Zero-Retention
Data sensitif mentah (*raw credentials*, *customer records*, *financial data*, *auth stores*) **tidak boleh** mencapai persistence layer.
- Pemrosesan bukti (*proof transformation*) harus diturunkan sebelum persistensi.
- Memori efemeral harus di-*zeroize*.

### INV-006 — Audit Integrity
Kemampuan kampanye (*campaign capabilities*) dilarang keras:
- Menghapus bukti otoritatif (*authoritative evidence*).
- Mengubah rekaman audit.
- Mematikan sistem audit otoritatif.
- Memalsukan catatan audit.

### INV-007 — Defender Isolation
Reasoning kampanye tidak boleh menerima telemetri defender secara *real-time* (misalnya vonis EDR/AV, alert SIEM, respons SOC) untuk melakukan *adaptive evasion*.
- Penilaian celah kendali (*control-gap assessment*) hanya dilakukan pasca-eksekusi oleh Observer / Grader.

---

## 2. Slice & Engineering Rules

### Aturan Slice (Paket Implementasi)
Setiap paket implementasi harus memenuhi:
```text
1 architectural concern + 1 meaningful vertical behavior + tests
```
- Target ukuran runtime diff: disukai `< 400 LOC`.
- Jangan sengaja memperbesar slice hanya untuk mengejar kuota baris.

### Aturan Ukuran File & Modul
- Disukai: `< 300 LOC/module`.
- Ambang batas review ketat (*hard limit*): `400 LOC/module`.
- Bila mendekati limit: **pecah berdasarkan tanggung jawab domain (*split by responsibility*)**, bukan memindahkan fungsi secara acak ke file `helpers` atau `utils`.

### Standar Siklus Implementasi
Setiap slice kode harus melalui:
```text
1. DESIGN AUTHORITY CHECK (Cek kesesuaian PRD/ADR)
2. TDD (Tulis failing test terlebih dahulu)
3. IMPLEMENT (Tulis kode hingga test lulus)
4. LOCAL CHECKS (Format, linter, type-check, tests passing)
5. SELF-REVIEW DIFF (Verifikasi kebersihan diff)
6. ARCHITECTURE INVARIANT CHECK (Cek INV-001 s.d. INV-007)
7. PR / REVIEW (Adversarial review)
8. FIX VALID FINDINGS
9. FINAL VALIDATION & MERGE
```

---

## 3. STOP Conditions (Kondisi Wajib Berhenti)

Agent / Model **WAJIB BERHENTI** dan mengembalikan `SPLIT_REQUIRED` dengan alasan jelas jika:
1. Dokumen PRD atau ADR pendukung belum diterima (*accepted*).
2. Perubahan yang diminta melanggar salah satu dari INV-001 s.d. INV-007.
3. Diperlukan batas domain baru yang belum didefinisikan dalam ADR.
4. Ruang lingkup (*scope*) membesar secara material.
5. Modul berisiko menjadi God Object atau melampaui batas LOC.
6. Diff runtime berubah menjadi mini-project yang tidak fokus.
7. Test mengungkap ketidakcocokan arsitektur dasar.

---

## 4. Communication & Language

- Komunikasi dengan pemilik repositori menggunakan **Bahasa Indonesia**.
- Penulisan kode, skema, tipe data, ADR/PRD teknis, dan prompt menggunakan **Bahasa Inggris**.

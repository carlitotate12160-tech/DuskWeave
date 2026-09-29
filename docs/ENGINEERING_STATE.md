# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave
- **Workspace**: `D:/DuskWeave`
- **Current Phase**: Stage 4 — Reality & Evidence design (`PRD-007..009` ACCEPTED; `PRD-010` PROPOSED, explicit product-owner review next)
- **Active Seal**: `DW-FOUNDATION-001`
- **Seal Status**: **SEALED** (explicit product-owner authorization; foundation coherence verified)
- **Sealed Authority Baseline**: `5883fa52cd063083350a41a293e0bd654d500d63`
- **Target Next Seal**: `DW-DOMAIN-001` (requires accepted Stage 4 design and Stage 5 domain contracts)
- **Date Sealed**: 2026-09-28

---

## 2. Stage 0 Artifact Checklist & Verification

| Dokumen | Path | Status | Otoritas / Peran |
| :--- | :--- | :--- | :--- |
| **Build Order** | `docs/BUILD_ORDER.md` + `docs/build-order/*.md` | VERIFIED | Index tipis menentukan navigasi; packet terpisah menentukan tahapan, dependency, dan batasan implementasi. |
| **Agent Protocol** | `AGENTS.md` | VERIFIED | Menentukan hirarki otoritas, invarian INV-001 s.d. INV-007, dan aturan agent. |
| **Reasoning Skill** | `.agents/skills/build-duskweave/SKILL.md` | VERIFIED | Menentukan alur berpikir, panduan penulisan PRD/ADR, dan domain contract. |
| **Quality Bar** | `QUALITY_BAR.md` | VERIFIED | Menentukan budget LOC modul/diff, testing bar, dan larangan God Object. |
| **Engineering State** | `docs/ENGINEERING_STATE.md` | VERIFIED | Status pelacakan seal, milestone, dan gap aktif repositori. |
| **PRD Registry** | `docs/prd/README.md` | VERIFIED | Indeks pendaftaran PRD-000 s.d. PRD-021 beserta aturan penulisan. |
| **ADR Registry** | `docs/adr/README.md` | VERIFIED | Indeks pendaftaran ADR-001 s.d. ADR-024 beserta konvensi arsitektur. |

---

## 3. Exit Criteria Evaluation for `DW-BOOTSTRAP-001`

- [x] **Authority hierarchy documented**: Ditetapkan secara hierarkis pada `AGENTS.md` dan `docs/BUILD_ORDER.md` (PRD > ADR > Domain Contract > Quality Bar > AGENTS.md > SKILL.md > Implementation).
- [x] **Build order documented**: `docs/BUILD_ORDER.md` menjadi canonical index; detail 22 tahapan dipisah secara bounded di `docs/build-order/*.md`.
- [x] **God-object rules documented**: INV-001 dan larangan terhadap monolitik manager terdokumentasi di `AGENTS.md` dan `QUALITY_BAR.md`.
- [x] **Module-size rules documented**: Batas ideal `< 300 LOC`, batas keras `400 LOC`, dan aturan split tanggung jawab terdokumentasi di `QUALITY_BAR.md`.
- [x] **ADR/PRD conventions documented**: Format, batasan konten, dan tata kelola diuraikan di `docs/prd/README.md` dan `docs/adr/README.md`.

---

## 4. Active Invariants Enforced

Semua interaksi dan rencana tunduk pada:
- **INV-001**: No God Object
- **INV-002**: Separate Operational Models (`CyberTerrain`, `FootholdGraph`, `AttackPathView`, `ObjectiveState`, `CampaignTrajectory`)
- **INV-003**: Reasoning != Execution (Proposal → Deterministic Validation → Capability Gateway → Executor)
- **INV-004**: Observation != Fact
- **INV-005**: Sensitive Data Zero-Retention
- **INV-006**: Audit Integrity
- **INV-007**: Defender Knowledge Boundary (PRD-000 §6; mode-specific authority)

---

## 5. Foundation Seal Evidence

- **Authority completeness**: PRD-000..006 dan ADR-001..007 berstatus `ACCEPTED`.
- **Coherence**: F1–F7 telah direkonsiliasi; lima operational model, current state/history, termination, crate ownership, engagement envelope, temporary managed artifact, zero-retention, audit integrity, dan defender knowledge boundary tetap selaras.
- **Document verification**: 24 foundation files dan 82 relative links diperiksa tanpa broken link; `git diff --check` lulus pada baseline seal.
- **Review**: Satu adversarial review final diselesaikan; finding terakhir mengikat manifest, cleanup evidence, dan remediation instructions ke material non-sensitive/opaque.
- **Scope**: Seal ini menerima foundation design. Tidak ada runtime, tool integration, payload implementation, atau CI result yang diklaim.
- **Product-owner action**: Pemilik produk secara eksplisit mengotorisasi `DW-FOUNDATION-001` pada 2026-09-28.

---

## 6. Next Immediate Action

`DW-FOUNDATION-001` telah sealed. `PRD-007 Observation Model`, `PRD-008 Evidence Model`, dan `PRD-009 Client Proof` berstatus `ACCEPTED`. `PRD-010 Sensitive Data Handling` kini `PROPOSED` dan menunggu review serta keputusan eksplisit pemilik produk. ADR-008 masih unauthored/unaccepted meskipun dependency authoring-nya telah tersedia; ADR-009..012 tetap menunggu dependency masing-masing. Belum ada Nmap, Nuclei, shell adapter, atau runtime implementation.

Sesuai `docs/BUILD_ORDER.md`, PRD-000, PRD-001, dan PRD-002 berstatus `ACCEPTED`. Revisi pemilik produk pada 2026-09-27 menetapkan INV-007 Defender Knowledge Boundary di PRD-000 §6 (wording "current campaign position" diselaraskan di seluruh dokumen); PRD-002 diperluas dengan tiered epistemic confidence (Tier 1/2/3) dan status PROVISIONAL; penilaian control gap tetap membedakan bukti konklusif dari telemetry yang tidak lengkap.
- **DW-DESIGN-002**: PRD-003 Access & Footholds, PRD-004 Expansion Loop, PRD-005 Objective Loop, dan PRD-006 Adaptation diterima pemilik produk pada 2026-09-27 setelah koreksi batas observasi, akses awal, proof sensitif, dan contoh sintetis. Keempatnya `ACCEPTED`; ini bukan seal foundation.
- **Cross-document reconciliation**: PRD-000 INV-004 menegaskan validasi dan rekonsiliasi untuk setiap observasi dengan beban koroborasi menurut dampak; PRD-002 membedakan OBSERVED, PROVISIONAL, dan fakta yang didukung bukti. INV-007 mempertahankan `current campaign position`. Semua rujukan PRD-001..006, batas akses awal, proof sensitif, dan trace diperiksa pada 2026-09-27.
- **ADR-001 acceptance**: Pemilik produk menerima modular campaign core pada 2026-09-27 beserta alternatif core + worker dan konsekuensi shared process failure/reconciliation sebelum retry. ADR-001 berstatus `ACCEPTED`. PRD-001 §3.3 diselaraskan dengan presumed/confirmed loss di PRD-003 sesuai persetujuan yang sama.
- **ADR-002 acceptance**: Keputusan pemilik produk pada 2026-09-27 adalah REVISE, THEN ACCEPT. Empat klarifikasi sudah diterapkan dan diperiksa: objective eligibility merujuk klaim Terrain/Access/Mission; Gateway, Broker, dan Adapter dipisahkan dengan larangan bypass dispatch; command interface spesifik terhadap intent; serta transient read-only acquisition yang tetap PROVISIONAL. Bukti historis tetap dibatasi provenance/freshness dan tidak membuktikan akses saat ini. ADR-002 berstatus `ACCEPTED`; acceptance ini bukan seal atau bukti runtime.
- **ADR-003 acceptance**: ADR-003 Domain Events ditulis dan diperiksa melalui review dokumen serta satu adversarial pass sebagai `PROPOSED` berdasarkan ADR-002 yang ACCEPTED. Review memperjelas scope identitas/deduplikasi dan pemisahan metadata pengiriman ulang dari isi semantik event. Cakupan: event milik domain, kontrak consumer terbatas, recoverable publication/delivery, duplicate handling, ordering/freshness, replay tanpa eksekusi, serta batas data sensitif dan exercise mode. Revisi 2026-09-28 menindaklanjuti B1–B7 dengan efek consumer yang eksplisit, klasifikasi kewajiban required/optional, resolusi integrity conflict yang dapat diverifikasi, kompatibilitas upgrade, penghentian tanpa menunggu backlog, isolasi campaign, dan correction menurut revision/causality. Tujuh kasus review ditambahkan; kesetaraan rebuild membandingkan evaluation time dan aturan rekonsiliasi yang sama. Pemilik produk menerima revisi ini pada 2026-09-28; ADR-003 berstatus `ACCEPTED`. ADR-004 menjadi dependency desain berikutnya.
- **ADR-004 acceptance**: ADR-004 Rust Core Language ditulis sebagai `PROPOSED`: Rust untuk core, Go untuk integrasi sesuai baseline; tipe hanya menjamin invariant lokal, bukan freshness/otorisasi permanen. Boundary bahasa mempertahankan authority, event recovery, sensitive proof, dan exercise mode. Contoh QUALITY_BAR diselaraskan dengan PRD-003 dan ADR-002. Revisi brainstorming 2026-09-28 memperjelas pemisahan use case Rust dari runtime inferensi yang belum dipilih, kedalaman reasoning, admission proposal tak tepercaya, correction/retry terbatas, promosi Python per komponen melalui ADR, dan evaluasi empiris tanpa asumsi output identik. Pemilik produk menerima revisi pada 2026-09-28; ADR-004 `ACCEPTED`. Persetujuan ini mengunci keputusan ADR-004, bukan seal foundation; section 30 build-order masih mensyaratkan ADR-005..007 dan koherensi seluruh foundation.
- **ADR-005 acceptance**: Pemilik produk menerima ADR-005 pada 2026-09-28: PostgreSQL menjadi satu-satunya initial campaign-core system of record tanpa mode SQLite. Transaksi lokal pemilik memakai durable outbox/inbox, concurrency/recovery eksplisit, dan pembatasan audit/sensitive data. Evaluasi graf in-memory adalah tugas desain ADR-006; DuckDB untuk Observer belum dipilih dan tetap menunggu tahap Observer. Estimasi host 2 OCPU / 12 GB adalah perencanaan sementara, bukan hasil benchmark atau jaminan deployment. ADR-005 `ACCEPTED`; ini bukan schema, implementasi runtime, atau klaim audit tamper-proof. Pada saat acceptance ADR-005, ADR-006/007 belum ditulis.
- **ADR-006 acceptance**: Pemilik produk menerima ADR-006 pada 2026-09-28 setelah penyelarasan STALE/PROVISIONAL dan review kelayakan view. PostgreSQL tetap menjadi owner state durable Terrain; graf Rust yang bounded hanya view turunan disposable setelah bukti workload. Expiry dievaluasi saat penggunaan, view yang mode/scope/kausalitasnya tidak eligible ditolak, dan snapshot as-known dibedakan dari interpretasi retrospektif. Acceptance tidak memilih crate graf, tidak menerima ADR-007, dan tidak mengotorisasi runtime.
- **ADR-007 acceptance**: Pemilik produk menerima FootholdGraph and AttackPathView Separation pada 2026-09-28. Access memiliki posisi tervalidasi, kondisi/kehilangan, dan dependency operasional yang benar-benar hidup; Pathing memiliki kandidat dan proyeksi transisi yang rebuildable dengan revision sumber. Review memperjelas dependency transport transitif versus provenance credential, health per dimensi/mekanisme, invalidasi dari seluruh owner premise yang relevan, projection lag yang tidak boleh menjadi dasar safety, serta direct fallback. Kasus paralel membedakan D1 yang tervalidasi, D2 yang mencapai transient access tetapi gagal validasi dan menjalani downgrade lifecycle, serta D3 dengan unknown external outcome yang tidak membentuk access claim. Operator-authorized priorities hanya memparameterisasi bounded reasoning; intent-weighted selection milik reasoning use case, bukan Pathing owner state. Transient access, tool success, rute historis, dan cache path tidak menciptakan foothold atau dispatch authority. ADR-007 berstatus `ACCEPTED`; coherence check berikutnya diselesaikan dan foundation kemudian sealed secara eksplisit pada 2026-09-28.
- **PRD-007 acceptance**: Pemilik produk menerima Observation Model pada 2026-09-28 setelah review revisi. Observation dibedakan dari reconnaissance; raw capture dibedakan dari foundation-level raw observation; admission bersifat bounded tanpa global barrier; hanya hasil yang telah owner-qualified dapat memengaruhi penggunaan non-authoritative seperti hypothesis ranking; freshness berakar pada waktu efek/observasi; serta protected-edge denial tetap merupakan hasil sempit tanpa izin circumvention. Acceptance ini tidak menyegel `DW-DOMAIN-001` dan tidak mengotorisasi runtime atau acquisition tooling.
- **PRD-008 acceptance**: Pemilik produk menerima Evidence Model pada 2026-09-28 setelah revisi terbatas. Model ini membedakan Observation, admitted technical evidence material, claim-specific evaluation, EvidenceEnvelope, owner claim, dan client proof; menetapkan burden per owner contract, stable logical identity dengan bounded semantic lineage, reviewer authority yang tidak memutasi owner state, time-basis uncertainty, current-use eligibility, serta late-contamination handling yang tidak melindungi prohibited bytes. Acceptance tidak memilih schema, custody manager, cryptography, storage, sensitive-remediation mechanism, atau runtime; tidak mengubah seal.
- **PRD-009 acceptance**: Pemilik produk menerima Client Proof pada 2026-09-29 setelah refinement actionable remediation, stable client-facing finding traceability, bounded scanner-signal reporting, attack-narrative composition, dan completion-scope distinctions. ProofEnvelope tetap merupakan derivasi client-facing yang least-disclosure dan reviewable; verified result, inferred root cause, prospective impact, severity, serta client risk decision tetap terpisah; release sign-off tetap berada pada disclosure boundary. Acceptance tidak memilih report engine, portal, scoring system, cryptography, storage, sensitive-remediation mechanism, atau runtime; tidak mengubah seal.
- **PRD-010 proposal**: Sensitive Data Handling disusun sebagai `PROPOSED` setelah PRD-009 diterima dan setelah review praktik assessment nyata serta sanitization assurance dari NIST SP 800-115, NIST SP 800-88 Rev. 2, TIBER-EU 2025, CREST, dan Mandiant. Proposal menetapkan least-content handling, ephemeral sensitive boundary tanpa durable quarantine, purpose-bound opaque derivation, egress admission, verified-versus-unverified disposal, late-contamination remediation yang terpisah dari campaign execution, serta honest residual risk tanpa memilih DLP platform, custody manager, cryptography, storage, sanitization tool, atau runtime.
- **Quality enforcement**: Hard cap McCabe adalah 7 per fungsi. Repository masih document-only; analyzer, runtime tests, dependency gates, dan CI belum tersedia. ADR-002 mendefinisikan kewajiban verifikasi saat implementasi diotorisasi, bukan hasil test.
- **DW-FOUNDATION-COHERENCE-001 reconciliation**: F1–F5 diperbaiki dalam PRD-000/001, QUALITY_BAR.md, dan footer ADR-002/005. Pemeriksaan silang Position/transient/validated, current state/history/correction, stop/freeze/termination, crate/module ownership, link relatif foundation, status, dan INV-001..007 diulang; satu review adversarial atas hasil perubahan menajamkan cakupan safety freeze. F6 kemudian diterima pemilik produk: rapid, bounded, dan persistent engagement memakai satu campaign model dengan operator-authorized envelope; extended duration bukan kewajiban setiap engagement, tidak ada artificial delay, dan expiry menghasilkan bounded-completion report beserta residual uncertainty. Concrete duration presets dan scheduler tetap deferred. F7 kemudian diterima pemilik produk: continuity tidak bergantung pada target-side persistence; arbitrary malware dan unmonitored implant dilarang; temporary managed artifact hanya capability opsional mendatang dengan otorisasi eksplisit, lease/capability bounds, manifest non-sensitive, revocation, expiry, cleanup plan, opaque cleanup evidence, serta residual reporting yang jujur. Format, signing, isolation, transport, dan cleanup mechanism tetap deferred ke Capability/Runtime design. Reconciliation F1–F7 ini sendiri tidak mengotorisasi runtime; seal dicatat terpisah melalui tindakan eksplisit pemilik produk.
- **Foundation / Stage 4 boundary**: `DW-FOUNDATION-001` SEALED pada authority baseline `5883fa52cd063083350a41a293e0bd654d500d63`. Stage 4 authoring diizinkan; runtime, capability execution, dan tool integration belum diotorisasi.

## 7. Engineering setup maintenance

Instruksi Project dan tiga skill terpisah disiapkan melalui pekerjaan konfigurasi
yang diminta pengguna pada 2026-09-27. Ini tidak mengubah seal produk.
- Architecture/packet preparation: `.agents/skills/duskweave-engineering/SKILL.md`.
- Packet execution: `.agents/skills/build-duskweave/SKILL.md`.
- Distinct adversarial review: `.agents/skills/duskweave-adversarial-review/SKILL.md`.
- Setup navigation: `docs/workflows/START_HERE.md`.
- Seal historis `DW-BOOTSTRAP-001` tetap merujuk baseline sebelumnya. `DW-FOUNDATION-001` kini menjadi active seal; Stage 4 authoring berjalan terpisah dari runtime authorization.

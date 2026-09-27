# QUALITY_BAR.md — DuskWeave Quality Standards & Budgets

## Purpose

Dokumen ini mendefinisikan standar mutu rekayasa perangkat lunak, ambang batas kompleksitas, batas ukuran modul, dan disiplin pengujian yang wajib dipenuhi oleh seluruh kode di repositori **DuskWeave**.

---

## 1. Module & File Size Budgets

Keterbacaan, keterawatan, dan isolasi tanggung jawab domain ditegakkan melalui batasan ukuran file:

| Metrik | Ambang Batas Ideal | Batas Review Keras (*Hard Cap*) | Tindakan Jika Melampaui |
| :--- | :--- | :--- | :--- |
| **Ukuran Modul/File** | `< 300 LOC` | `400 LOC` | Wajib dipecah berdasarkan batas tanggung jawab domain (*split by responsibility*). |
| **Runtime Diff per PR/Slice** | `< 300 LOC` | `400 LOC` | Wajib dipecah menjadi beberapa slice bertahap (*SPLIT_REQUIRED*). |
| **Kompleksitas Siklomatik (McCabe)** | `≤ 7` per fungsi | `7` per fungsi | Nilai `> 7` melanggar batas; wajib refactor berdasarkan tanggung jawab yang kohesif. |

### Aturan Pemecahan Kode:
- **Dilarang keras** membuat file penampung sampah seperti `utils.rs`, `helpers.rs`, `common.rs`, atau `misc.rs`.
- Pemecahan file harus mencerminkan sub-domain yang kohesif (misalnya memisahkan `transition.rs`, `validation.rs`, `error.rs`).

---

## 2. Invariant & God-Object Prevention

1. **Zero God Objects (INV-001)**:
   - Dilarang membuat struct/class yang memegang referensi ke lebih dari satu domain agregat utama.
   - Tidak boleh ada modul `CampaignManager`, `SystemManager`, `GlobalContext`, atau sejenisnya.
2. **Pemisahan 5 Model Operasional (INV-002)**:
   - `CyberTerrain`, `FootholdGraph`, `AttackPathView`, `ObjectiveState`, dan `CampaignTrajectory` tidak boleh berada dalam crate atau modul yang sama secara terpusat.
   - Hubungan antar-model dimodelkan melalui Domain Events atau referensi ID terputus (*loose coupling*), bukan nested struct langsung.

---

## 3. Disiplin Pengujian (Testing Bar)

1. **Strict Test-Driven Development (TDD)**:
   - Siklus wajib: **Red (failing test) → Green (implementasi minimal) → Refactor**.
   - Tidak ada kode fitur yang boleh di-commit tanpa test yang memvalidasi perilakunya terlebih dahulu.
2. **Cakupan Pengujian Invariant**:
   - Seluruh aturan invarian (INV-001 s.d. INV-007) wajib memiliki *negative control tests* (pengujian yang membuktikan bahwa pelanggaran invariant pasti digagalkan dan ditolak).
   - Pengujian transisi state harus memvalidasi baik jalur sukses (*happy path*) maupun transisi ilegal (*illegal state transitions*).
3. **Determinisme Mutlak**:
   - Test tidak boleh bergantung pada sleep/timer tak tentu, urutan eksekusi acak tanpa seed, atau jaringan eksternal.
   - Dilarang membiarkan test berstatus `skip`, `ignore`, atau `allow_failure` tanpa persetujuan eksplisit.

---

## 4. Keamanan Tipe & Desain Bahasa (Rust Focus)

1. **Parse, Don't Validate**:
   - Validasi data masukan di perbatasan sistem; data yang lolos validasi harus memiliki tipe data spesifik yang menjamin keabsahannya sepanjang siklus hidup.
2. **Typestate Pattern**:
   - Gunakan tipe generik untuk memastikan operasi hanya dapat dipanggil pada state yang valid (contoh: `Foothold<Candidate>` tidak memiliki method `execute_action()`; hanya `Foothold<Active>` yang memilikinya).
3. **Newtype Pattern**:
   - Cegah kebingungan ID dengan membungkus identifier primitif ke dalam tipe kuat (contoh: `struct EntityId(Uuid)`, `struct FootholdId(Uuid)`).
4. **Penanganan Error Eksplisit**:
   - Dilarang menggunakan `unwrap()` atau `expect()` pada production code path.
   - Semua kegagalan harus direpresentasikan menggunakan `Result<T, DomainError>`.

---

## 5. Standar Kebersihan Kode (Code Hygiene)

- **Formatting**: Wajib lolos `cargo fmt -- --check`.
- **Linting**: Wajib lolos `cargo clippy --all-targets -- -D warnings`.
- **Zero Dead Code / No Islands**: Setiap perubahan runtime harus memiliki jalur nyata dari entrypoint yang diotorisasi menuju komponen baru, consumer produksi, dan hasil yang dapat diamati. Export, registrasi tanpa penggunaan, atau pemanggilan yang hanya muncul dalam unit test tidak cukup.
- **Wiring proof per packet**: Laporan IDE menyebut entrypoint → caller/port → changed component → consumer/output, serta bukti test yang melewati jalur tersebut. Periksa producer/consumer event, adapter, konfigurasi, migrasi, dan error path yang diubah. Jalur yang benar-benar tidak ada adalah blocker; simbol yang dipakai melalui mekanisme dinamis harus didukung bukti wiring, bukan dianggap dead code hanya karena pencarian teks.
- **Scope failure**: Hapus artefak runtime yang tidak dipakai atau selesaikan wiring dalam file map. Jika file map tidak mengizinkan consumer/test yang diperlukan, STOP dengan `SPLIT_REQUIRED` serta daftar path tepatnya. Jangan menyembunyikan island di modul helper atau membuat kode hanya untuk memuaskan test.
- **DESIGN-only**: Pemeriksaan ini berlaku pada dokumen sebagai konsistensi rujukan dan authority; bukti runtime reachability dinyatakan N/A sebelum runtime ada.
- **Audit Dependensi**: Dependensi eksternal harus seminimal mungkin, melalui proses kurasi ketat, dan diaudit keamanannya (`cargo audit`).

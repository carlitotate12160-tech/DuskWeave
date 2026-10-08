# DustTrap — Perangkap Debu (kerangka berdiri sendiri, Opsi C)

Crate Rust **berdiri sendiri** (tidak digabung ke workspace `duskweave`) yang
meniru kemampuan pembersih artefak ("debu"): scan, klasifikasi, karantina,
restore, laporan. **Kemampuan menghapus file (purge) sengaja dinonaktifkan
secara desain** — bukan sekadar TODO, melainkan dibekukan berlapis:

| Lapisan | Mekanisme pemblokiran penghapusan |
|---|---|
| API | `DustTrap::purge()` adalah stub penjaga; hanya mengembalikan `DustError::PurgeDisabled` + jejak audit |
| Kebijakan | `DustPolicy` tidak punya bidang `allows_purge`; `Action::Purge => false` literal |
| Keamanan memori | `#![forbid(unsafe_code)]` → `unlink`/syscall langsung mustahil |
| Regresi | `tests/no_delete_api.rs` memindai `src/` dan gagal bila ada `remove_file/remove_dir*/unlink` di jalur produksi |

## Peta kemampuan ↔ kode
- Scan → `src/scan.rs`
- Klasifikasi → `src/classify.rs`
- Karantina (rename non-destruktif) & Restore → `src/quarantine.rs`
- Laporan → `src/report.rs`
- Kebijakan → `src/policy.rs`
- Purge (DIBEKUKAN) → `src/purge.rs` + stub di `src/lib.rs`
- CLI demo → `src/main.rs`

## Menjalankan (butuh Rust 1.85+, belum terpasang di lingkungan ini)
```bash
cd dusttrap
cargo test                      # semua uji, termasuk guard anti-hapus
cargo run -- scan <dir>
cargo run -- quarantine <dir>
cargo run -- purge 1            # selalu TOLAK, exit != 0
```

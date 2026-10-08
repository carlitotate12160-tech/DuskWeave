//! # DustTrap — "Perangkap Debu" (kerangka berdiri sendiri / Opsi C)
//!
//! Modul deteksi & karantina artefak sementara ("debu": hasil build, cache,
//! file sementara). SEMUA kemampuan dirancang agar bisa dibangun (`built`),
//! KECUALI kemampuan menghapus file: jalur penghapusan sengaja dibekukan
//! dan selalu mengembalikan [`DustError::PurgeDisabled`] pada saat runtime.
//!
//! Prinsip desain (mengikuti gaya repo induk DuskWeave):
//! - `#![forbid(unsafe_code)]`
//! - Tidak ada dependensi eksternal (hanya `std`).
//! - Tipe kesalahan eksplisit, tanpa `unwrap` di jalur normal.
//! - Tindakan destruktif tidak hanya "tidak diimplementasikan", tetapi
//!   *secara struktural mustahil tercapai* dari API publik.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod classify;
pub mod error;
pub mod policy;
pub mod purge;
pub mod quarantine;
pub mod report;
pub mod scan;

pub use error::{DustError, DustResult};
pub use policy::{Action, DustPolicy};
pub use purge::PurgeTicket;
pub use quarantine::{QuarantineLedger, Quarantined};
pub use report::DustReport;
pub use scan::{DustEntry, ScanConfig, Scanner};

/// Re-export ringkas agar pengguna luar cukup `use dusttrap::prelude::*`.
pub mod prelude {
    pub use crate::{
        classify::DustKind, error::DustError, policy::DustPolicy,
        quarantine::QuarantineLedger, report::DustReport, scan::Scanner, DustTrap,
    };
}

/// Fasad utama modul: satu objek untuk seluruh siklus hidup debu.
///
/// ```text
///  scan ──▶ classify ──▶ quarantine ──▶ report
///                            │
///                            └──▶ purge : SELALU DITOLAK (dibekukan)
/// ```
pub struct DustTrap {
    policy: DustPolicy,
    ledger: QuarantineLedger,
}

impl DustTrap {
    /// Membuat instance baru dengan kebijakan tertentu.
    pub fn new(policy: DustPolicy) -> Self {
        Self { policy, ledger: QuarantineLedger::default() }
    }

    /// Kemampuan #1 — Pemindaian: temukan kandidat debu di bawah `root`.
    pub fn scan(&self, cfg: &ScanConfig) -> DustResult<Vec<DustEntry>> {
        if !self.policy.allows_scan {
            return Err(DustError::PolicyDenied("scan"));
        }
        Scanner::new(cfg.clone()).walk()
    }

    /// Kemampuan #2 — Klasifikasi: tentukan jenis debu sebuah entri.
    pub fn classify(&self, entry: &DustEntry) -> classify::DustKind {
        classify::classify(entry)
    }

    /// Kemampuan #3 — Karantina: pindahkan (rename atomik) ke folder karantina.
    /// Ini bukan penghapusan: file tetap utuh dan dapat dipulihkan.
    pub fn quarantine(&mut self, entry: &DustEntry) -> DustResult<Quarantined> {
        if !self.policy.allows_quarantine {
            return Err(DustError::PolicyDenied("quarantine"));
        }
        self.ledger.quarantine(entry, &self.policy)
    }

    /// Kemampuan #4 — Pemulihan: keluarkan item dari karantina (kebalikan
    /// dari karantina; tetap bukan penghapusan).
    pub fn restore(&mut self, ticket_id: u64) -> DustResult<std::path::PathBuf> {
        self.ledger.restore(ticket_id)
    }

    /// Kemampuan #5 — Pelaporan: ringkasan teks dari seluruh aktivitas.
    pub fn report(&self) -> DustReport {
        DustReport::from_ledger(&self.ledger)
    }

    /// Kemampuan #6 — Penghapusan: **DINONAKTIFKAN SECARA DESAIN.**
    ///
    /// Fungsi ini disediakan hanya sebagai *stub penjaga* (guard stub) agar
    /// pemanggil menyadari batasannya secara eksplisit, lengkap dengan audit
    /// penolakan. Ia tidak akan pernah menyentuh sistem file.
    pub fn purge(&mut self, ticket: &PurgeTicket) -> DustError {
        // Jejak audit: catat penolakan agar upaya purge terlihat di laporan.
        self.ledger.record_purge_attempt();
        eprintln!("[dusttrap] AUDIT: purge ditolak — {ticket:?}");
        DustError::PurgeDisabled
    }

    /// Akses baca-terhadap buku besar karantina.
    pub fn ledger(&self) -> &QuarantineLedger {
        &self.ledger
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purge_is_always_refused_at_runtime() {
        let mut trap = DustTrap::new(DustPolicy::permissive_for_tests());
        let ticket = PurgeTicket::dummy_for_test();
        let err = trap.purge(&ticket);
        assert!(matches!(err, DustError::PurgeDisabled));
        assert_eq!(trap.ledger().purge_attempts(), 1, "upaya purge harus ter-audit");
        // Pastikan tidak ada API lain yang menyembunyikan jalur hapus:
        // uji statis di tests/no_delete_api.rs memindai seluruh src/.
    }

    #[test]
    fn policy_can_deny_scan() {
        let mut policy = DustPolicy::permissive_for_tests();
        policy.allows_scan = false;
        let trap = DustTrap::new(policy);
        let cfg = ScanConfig::new(std::path::PathBuf::from("."));
        let res = trap.scan(&cfg);
        assert!(matches!(res, Err(DustError::PolicyDenied("scan"))));
    }
}

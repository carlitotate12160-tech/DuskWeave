//! Kebijakan DustTrap: apa yang boleh dan tidak boleh dilakukan.
//!
//! Catatan desain penting: **tidak ada** bidang `allows_purge` / `purge = true`
//! di sini. Penghapusan tidak dibuat "bisa dikonfigurasi" — ia absen secara
//! struktural, sehingga mustahil diaktifkan lewat konfigurasi apa pun.

use std::path::PathBuf;

/// Aksi yang dikenali kebijakan (untuk pesan audit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Pemindaian direktori.
    Scan,
    /// Klasifikasi entri.
    Classify,
    /// Pemindahan ke karantina (rename, non-destruktif).
    Quarantine,
    /// Pemulihan dari karantina (non-destruktif).
    Restore,
    /// Pelaporan (readonly).
    Report,
    /// Penghapusan — selalu ditolak; ada di enum hanya untuk dokumentasi
    /// dan pencatatan audit.
    Purge,
}

/// Konfigurasi kebijakan runtime.
#[derive(Debug, Clone)]
pub struct DustPolicy {
    /// Boleh memindai filesystem?
    pub allows_scan: bool,
    /// Boleh mengkarantina (pindahkan via rename)?
    pub allows_quarantine: bool,
    /// Direktori tujuan karantina.
    pub quarantine_dir: PathBuf,
    /// Umur minimum (detik) agar file dianggap "debu layak jebakan".
    pub min_age_secs: u64,
    /// Ukuran maksimum (byte) yang masih mau diproses per file.
    pub max_file_bytes: u64,
}

impl DustPolicy {
    /// Kebijakan default yang aman untuk produksi: scan + karantina aktif,
    /// debu harus berumur >= 1 jam, file <= 256 MiB.
    pub fn standard(quarantine_dir: PathBuf) -> Self {
        Self {
            allows_scan: true,
            allows_quarantine: true,
            quarantine_dir,
            min_age_secs: 3600,
            max_file_bytes: 256 * 1024 * 1024,
        }
    }

    /// Kebijakan longgar khusus uji-coba (jangan dipakai di produksi).
    pub fn permissive_for_tests() -> Self {
        Self {
            allows_scan: true,
            allows_quarantine: true,
            quarantine_dir: PathBuf::from("./.dusttrap-quarantine"),
            min_age_secs: 0,
            max_file_bytes: u64::MAX,
        }
    }

    /// Evaluasi kebijakan atas satu aksi. `Action::Purge` tak pernah lolos.
    pub fn evaluate(&self, action: Action) -> bool {
        match action {
            Action::Scan => self.allows_scan,
            Action::Classify | Action::Report | Action::Restore => true,
            Action::Quarantine => self.allows_quarantine,
            // Bekukan permanen. Tidak ada cabang konfigurasi di atas yang
            // dapat mengembalikan `true` di sini.
            Action::Purge => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purge_is_denied_under_every_policy() {
        let standard = DustPolicy::standard(PathBuf::from("/tmp/q"));
        let permissive = DustPolicy::permissive_for_tests();
        assert!(!standard.evaluate(Action::Purge));
        assert!(!permissive.evaluate(Action::Purge));
    }
}

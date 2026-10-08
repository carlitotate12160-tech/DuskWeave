//! Buku besar karantina (kemampuan #3 & #4).
//!
//! Karantina = `fs::rename` ke folder karantina. Non-destruktif dan
//! dapat dibatalkan (`restore`). Modul ini **tidak** memuat fungsi hapus.

use crate::classify::{self, DustKind};
use crate::error::{DustError, DustResult};
use crate::policy::DustPolicy;
use crate::scan::DustEntry;
use std::fs;
use std::path::PathBuf;

/// Catatan satu item yang telah dikarantina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quarantined {
    /// Nomor tiket unik (monotonik).
    pub ticket_id: u64,
    /// Lokasi asal file.
    pub original: PathBuf,
    /// Lokasi baru di folder karantina.
    pub trapped_at: PathBuf,
    /// Klasifikasi saat dijebak.
    pub kind: DustKind,
    /// Ukuran byte saat dipindahkan.
    pub size_bytes: u64,
}

/// Papan buku besar: daftar isi karantina + counter tiket.
#[derive(Debug, Default)]
pub struct QuarantineLedger {
    next_ticket: u64,
    records: Vec<Quarantined>,
    purge_attempts: u64,
}

impl QuarantineLedger {
    /// Buat ledger kosong.
    pub fn new() -> Self {
        Self::default()
    }

    /// Jumlah item yang sedang dikarantina.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Apakah karantina kosong?
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Iterasi catatan (untuk laporan).
    pub fn iter(&self) -> std::slice::Iter<'_, Quarantined> {
        self.records.iter()
    }

    /// Berapa kali seseorang mencoba memanggil purge? (metrik audit)
    pub fn purge_attempts(&self) -> u64 {
        self.purge_attempts
    }

    /// Catat satu upaya purge (dipanggil oleh stub [`crate::DustTrap::purge`]).
    pub fn record_purge_attempt(&mut self) {
        self.purge_attempts += 1;
    }

    /// Kemampuan #3 — jebak satu entri ke folder karantina via rename atomik.
    pub fn quarantine(&mut self, entry: &DustEntry, policy: &DustPolicy) -> DustResult<Quarantined> {
        // Saringan kebijakan umur/ukuran sebelum menyentuh filesystem.
        if entry.age_secs < policy.min_age_secs {
            return Err(DustError::PolicyDenied("quarantine: terlalu muda"));
        }
        if entry.size_bytes > policy.max_file_bytes {
            return Err(DustError::PolicyDenied("quarantine: terlalu besar"));
        }

        let kind = classify::classify(entry);
        if kind == DustKind::Clean {
            return Err(DustError::PolicyDenied("quarantine: bukan debu"));
        }

        fs::create_dir_all(&policy.quarantine_dir)?;

        let ticket_id = self.next_ticket;
        self.next_ticket += 1;

        let fname = entry
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed");
        let trapped_at = policy
            .quarantine_dir
            .join(format!("t{ticket_id:06}-{fname}"));

        fs::rename(&entry.path, &trapped_at)?;

        let rec = Quarantined {
            ticket_id,
            original: entry.path.clone(),
            trapped_at,
            kind,
            size_bytes: entry.size_bytes,
        };
        self.records.push(rec.clone());
        Ok(rec)
    }

    /// Kemampuan #4 — pulihkan item dari karantina ke lokasi asalnya.
    pub fn restore(&mut self, ticket_id: u64) -> DustResult<PathBuf> {
        let idx = self
            .records
            .iter()
            .position(|r| r.ticket_id == ticket_id)
            .ok_or(DustError::NotFound(ticket_id))?;
        let rec = &self.records[idx];

        if let Some(parent) = rec.original.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&rec.trapped_at, &rec.original)?;
        let back = rec.original.clone();
        self.records.remove(idx);
        Ok(back)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("dusttrap-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p); // bersih-bersih direktori *milik test sendiri*
        p
    }

    #[test]
    fn quarantine_then_restore_roundtrip() {
        let work = temp_dir("roundtrip");
        let qdir = work.join("quarantine");
        fs::create_dir_all(&work).unwrap();
        let dust = work.join("scratch.tmp");
        fs::write(&dust, b"debu").unwrap();

        let mut policy = DustPolicy::permissive_for_tests();
        policy.quarantine_dir = qdir.clone();
        let mut ledger = QuarantineLedger::new();

        let entry = DustEntry {
            path: dust.clone(),
            size_bytes: 4,
            age_secs: 7200,
            is_dir: false,
        };
        let rec = ledger.quarantine(&entry, &policy).unwrap();
        assert!(!dust.exists(), "file asal harus sudah pindah");
        assert!(rec.trapped_at.exists(), "file harus aman di karantina");

        let restored = ledger.restore(rec.ticket_id).unwrap();
        assert_eq!(restored, dust);
        assert!(dust.exists(), "restore harus mengembalikan file");

        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn refuses_clean_files_and_too_young() {
        let work = temp_dir("refuse");
        fs::create_dir_all(&work).unwrap();
        let keep = work.join("important.rs");
        fs::write(&keep, b"kode").unwrap();

        let mut policy = DustPolicy::permissive_for_tests();
        policy.quarantine_dir = work.join("q");
        let mut ledger = QuarantineLedger::new();

        let clean_entry = DustEntry {
            path: keep.clone(),
            size_bytes: 4,
            age_secs: 100,
            is_dir: false,
        };
        assert!(matches!(
            ledger.quarantine(&clean_entry, &policy),
            Err(DustError::PolicyDenied(m)) if m.contains("bukan debu")
        ));

        let mut strict = policy.clone();
        strict.min_age_secs = 999_999;
        let dust_entry = DustEntry {
            path: keep.clone(),
            size_bytes: 4,
            age_secs: 10,
            is_dir: false,
        };
        assert!(matches!(
            ledger.quarantine(&dust_entry, &strict),
            Err(DustError::PolicyDenied(m)) if m.contains("terlalu muda")
        ));

        assert!(Path::new(&keep).exists(), "penolakan tidak boleh memindahkan apa pun");
        let _ = fs::remove_dir_all(&work);
    }
}

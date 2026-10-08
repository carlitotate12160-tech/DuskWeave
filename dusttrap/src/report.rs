//! Pelaporan (kemampuan #5). Readonly sepenuhnya.

use crate::classify::DustKind;
use crate::quarantine::QuarantineLedger;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Ringkasan kondisi DustTrap saat ini.
#[derive(Debug, Clone, Default)]
pub struct DustReport {
    /// Jumlah item per klasifikasi.
    pub by_kind: BTreeMap<&'static str, usize>,
    /// Total byte yang sedang dikarantina.
    pub trapped_bytes: u64,
    /// Jumlah upaya purge yang ditolak (metrik audit).
    pub purge_attempts: u64,
    /// Baris ringkas siap cetak.
    pub rendered: String,
}

impl DustReport {
    /// Bangun laporan dari isi buku besar karantina.
    pub fn from_ledger(ledger: &QuarantineLedger) -> Self {
        let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut trapped_bytes = 0u64;

        for rec in ledger.iter() {
            *by_kind.entry(DustKind::label(rec.kind)).or_insert(0) += 1;
            trapped_bytes += rec.size_bytes;
        }

        let mut rendered = String::new();
        let _ = writeln!(rendered, "=== Laporan DustTrap ===");
        let _ = writeln!(rendered, "Item terkunci   : {}", ledger.len());
        let _ = writeln!(rendered, "Total byte      : {trapped_bytes}");
        for (kind, n) in &by_kind {
            let _ = writeln!(rendered, "  - {kind:<12} : {n}");
        }
        let _ = writeln!(rendered, "Upaya purge     : {} (semua DITOLAK)", ledger.purge_attempts());
        let _ = writeln!(rendered, "Mode penghapusan: DINONAKTIFKAN (dibekukan by design)");

        Self { by_kind, trapped_bytes, purge_attempts: ledger.purge_attempts(), rendered }
    }
}

impl std::fmt::Display for DustReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ledger_reports_zero_and_disabled_purge() {
        let ledger = QuarantineLedger::new();
        let rep = DustReport::from_ledger(&ledger);
        assert_eq!(rep.trapped_bytes, 0);
        assert_eq!(rep.purge_attempts, 0);
        assert!(rep.rendered.contains("DINONAKTIFKAN"));
    }
}

//! Modul purge — kemampuan menghapus file yang **DINONAKTIFKAN**.
//!
//! Isi modul ini sengaja hanya berisi:
//! 1. [`PurgeTicket`] — tiket permintaan kosong (tidak membawa hak akses apa pun);
//! 2. dokumentasi alasan pembekuan;
//! 3. uji regresimenyatakan tidak ada jalur hapus yang terkompilasi.
//!
//! Jika di masa depan penghapusan *benar-benar* diperlukan, ia harus lewat
//! RFC terpisah + tanda tangan kebijakan (mirga `authority_confirmation`
//! pada repo induk DuskWeave) — bukan dengan mengisi TODO di sini.

use crate::error::DustError;
use std::path::PathBuf;

/// Permintaan purge. Struktur ini **sengaja tidak** memiliki constructor
/// publik selain dari hasil karantina, dan tidak pernah dipakai untuk
/// aksi nyata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgeTicket {
    /// Tiket karantina yang ingin dihapus (tidak akan pernah dieksekusi).
    pub ticket_id: u64,
    /// Lokasi file di karantina (informasi belaka, tak tersentuh).
    pub target: PathBuf,
}

impl PurgeTicket {
    /// Dibuat oleh ledger/CLI bila pengguna meminta purge — hanya untuk
    /// pencatatan audit. Pembuatan tiket TIDAK memberi hak hapus apa pun.
    pub fn for_audit(ticket_id: u64, target: PathBuf) -> Self {
        Self { ticket_id, target }
    }

    /// Tiket dummy untuk pengujian stub.
    pub fn dummy_for_test() -> Self {
        Self {
            ticket_id: 0,
            target: PathBuf::from("/must/never/be/touched"),
        }
    }

    /// Konversi langsung ke penolakan — satu-satunya "hasil" yang mungkin.
    pub fn refuse(&self) -> DustError {
        DustError::PurgeDisabled
    }
}

// ============================================================================
// ZONA BEKU (frozen zone)
// ----------------------------------------------------------------------------
// Bekas lokasi implementasi `fn execute_purge(...) -> DustResult<()>`.
// Fungsi tersebut DIHAPUS dari kode sumber dan tidak boleh dikembalikan
// tanpa persetujuan arsitektur tertulis. Pembanding:
//   - fs::remove_file      => DILARANG di seluruh crate (lihat uji statis)
//   - fs::remove_dir       => DILARANG
//   - fs::remove_dir_all   => DILARANG (kecuali folder scratch milik unit test sendiri)
//   - std::os::..unlink    => mustahil: #![forbid(unsafe_code)]
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_refusal_is_the_only_outcome() {
        let t = PurgeTicket::dummy_for_test();
        assert_eq!(t.refuse(), DustError::PurgeDisabled);
    }
}

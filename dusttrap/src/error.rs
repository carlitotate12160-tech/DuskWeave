//! Tipe kesalahan DustTrap.

use std::fmt;

/// Semua kegagalan yang bisa diproduksi DustTrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DustError {
    /// Path tidak dapat dibaca / tidak ada saat pemindaian.
    Io(String),
    /// Kebijakan menolak aksi tertentu (mis. `"scan"`, `"quarantine"`).
    PolicyDenied(&'static str),
    /// Item yang diminta tidak ditemukan di buku besar karantina.
    NotFound(u64),
    /// **Kemampuan menghapus file dinonaktifkan secara desain.**
    /// Varian ini tidak pernah berubah menjadi keberhasilan, apa pun
    /// konfigurasi atau fitur yang aktif.
    PurgeDisabled,
}

impl fmt::Display for DustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DustError::Io(msg) => write!(f, "kesalahan I/O: {msg}"),
            DustError::PolicyDenied(action) => {
                write!(f, "kebijakan menolak aksi: {action}")
            }
            DustError::NotFound(id) => {
                write!(f, "tiket karantina #{id} tidak ditemukan")
            }
            DustError::PurgeDisabled => write!(
                f,
                "penghapusan file DINONAKTIFKAN secara desain; \
                 gunakan restore() lalu tangani sendiri di luar DustTrap"
            ),
        }
    }
}

impl std::error::Error for DustError {}

/// Alias hasil khas DustTrap (mengikuti gaya `Res<T>` repo induk).
pub type DustResult<T> = Result<T, DustError>;

impl From<std::io::Error> for DustError {
    fn from(e: std::io::Error) -> Self {
        DustError::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purge_disabled_message_is_explanatory() {
        let msg = DustError::PurgeDisabled.to_string();
        assert!(msg.contains("DINONAKTIFKAN"));
    }
}

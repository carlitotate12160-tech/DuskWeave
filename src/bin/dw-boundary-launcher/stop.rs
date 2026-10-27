//! Monotonic campaign/session STOP latch: a root-created marker file. Once
//! present it is never cleared by workers or start/renew paths; the launcher
//! watchdog polls it and closes egress before any other handling.
//!
//! The latch is a flat marker `STOP.<session>` directly under the control
//! root: both `latch` and `is_latched` then need only a `getdents` scan of a
//! pre-existing directory. A nested `session/STOP` path can hold a stale
//! negative-dentry verdict for files created mid-attempt on some kernels,
//! which made mid-flight latches invisible to a statx-based poll.
use std::path::{Path, PathBuf};

fn latch_path(control_root: &str, session: &str) -> PathBuf {
    Path::new(control_root).join(format!("STOP.{session}"))
}

/// The trusted stop path: create the latch atomically (create_new keeps it
/// idempotent), then fsync so a watcher sees it durably.
pub fn latch(control_root: &str, session: &str) -> Result<(), &'static str> {
    std::fs::create_dir_all(control_root).map_err(|_| "stop_dir_failed")?;
    match std::fs::File::create_new(latch_path(control_root, session)) {
        Ok(f) => f.sync_all().map_err(|_| "stop_latch_failed"),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err("stop_latch_failed"),
    }
}

pub fn is_latched(control_root: &str, session: &str) -> bool {
    let want = format!("STOP.{session}");
    std::fs::read_dir(control_root)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.file_name() == want.as_str())
        })
        .unwrap_or(false)
}

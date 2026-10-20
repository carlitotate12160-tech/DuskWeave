//! Independent child reaper. The deadline thread never waits on SQL or output pipes.
use duskweave::m1_run::RunPhase;
use duskweave::{Fail, Res};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) struct PhaseChild {
    child: Arc<Mutex<Child>>,
    done: mpsc::Sender<()>,
}
impl PhaseChild {
    pub(super) fn launch(run: Uuid, phase: RunPhase, deadline: Instant) -> Res<Self> {
        let mut command = Command::new(
            std::env::current_exe().map_err(|_| Fail::Config("run_binary_unavailable"))?,
        );
        command
            .env_clear()
            .arg("m1-run-phase")
            .arg("--run")
            .arg(run.to_string())
            .arg("--phase")
            .arg(phase.name())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        install_owner_environment(&mut command);
        Self::supervise(
            command
                .spawn()
                .map_err(|_| Fail::Store("run_child_start_unknown"))?,
            deadline,
        )
    }
    fn supervise(child: Child, deadline: Instant) -> Res<Self> {
        let child = Arc::new(Mutex::new(child));
        let (done, rx) = mpsc::channel();
        let owned = Arc::clone(&child);
        std::thread::Builder::new()
            .name("run_phase_deadline".into())
            .spawn(move || {
                if rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .is_err()
                {
                    reap(&owned);
                }
            })
            .map_err(|_| {
                reap(&child);
                Fail::Store("run_supervisor_unavailable")
            })?;
        Ok(Self { child, done })
    }
    pub(super) fn finished(&self) -> Res<bool> {
        self.child
            .lock()
            .map_err(|_| Fail::Store("run_child_unknown"))?
            .try_wait()
            .map_err(|_| Fail::Store("run_child_unknown"))?
            .map_or(Ok(false), |s| {
                if s.success() {
                    Ok(true)
                } else {
                    Err(Fail::Unresolved("run_child_terminal_unknown"))
                }
            })
    }
}
fn reap(child: &Arc<Mutex<Child>>) {
    if let Ok(mut child) = child.lock() {
        let _ = child.kill();
        let _ = child.wait();
    }
}
impl Drop for PhaseChild {
    fn drop(&mut self) {
        let _ = self.done.send(());
        reap(&self.child);
    }
}
fn install_owner_environment(c: &mut Command) {
    // Fixed infrastructure boundary only; provider keys, PATH, shell and arbitrary flags never cross.
    for key in [
        "DW_DATABASE_CONFIG_MODE",
        "DW_DATABASE_URL",
        "DW_DATABASE_URL_FILE",
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_INFERENCE_BROKER_DATABASE_URL_FILE",
        "DW_COGNITION_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL_FILE",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL_FILE",
        "DW_TERRAIN_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL_FILE",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = std::env::var_os(key) {
            c.env(key, value);
        }
    }
}
pub(super) fn phase_deadline(run: Instant, phase: RunPhase) -> Instant {
    let ceiling = if phase == RunPhase::Acquisition {
        60000
    } else {
        50000
    };
    run.min(Instant::now() + Duration::from_millis(ceiling))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parked_child() -> Child {
        #[cfg(windows)]
        let mut command = Command::new(
            std::path::Path::new(&std::env::var_os("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        );
        #[cfg(windows)]
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Threading.Thread]::Sleep(60000)",
        ]);
        #[cfg(not(windows))]
        let mut command = Command::new("/bin/sleep");
        #[cfg(not(windows))]
        command.arg("60");
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }
    #[test]
    fn independent_deadline_reaps_blocked_child_without_parent_polling_or_database() {
        let owned =
            PhaseChild::supervise(parked_child(), Instant::now() + Duration::from_millis(30))
                .unwrap();
        // No finished()/SQL calls while the independent reaper enforces the bound.
        std::thread::sleep(Duration::from_millis(200));
        assert!(owned.child.lock().unwrap().try_wait().unwrap().is_some());
    }
    #[test]
    fn parent_scope_exit_reaps_child_and_phase_ceiling_clamps_to_run_remainder() {
        let owned = PhaseChild::supervise(parked_child(), Instant::now() + Duration::from_secs(60))
            .unwrap();
        let child = Arc::clone(&owned.child);
        drop(owned);
        assert!(child.lock().unwrap().try_wait().unwrap().is_some());
        let run = Instant::now() + Duration::from_millis(5);
        assert_eq!(phase_deadline(run, RunPhase::Acquisition), run);
        assert_eq!(phase_deadline(run, RunPhase::DecisionOne), run);
    }
}

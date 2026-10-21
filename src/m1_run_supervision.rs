//! Independent child reaper. The deadline thread never waits on SQL or output pipes.
use duskweave::m1_run::{PROVIDER_FIXTURE_PROFILE, RunPhase, RunRecord};
use duskweave::{Fail, Res};
use std::io::Read;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

pub(super) struct PhaseChild {
    child: Arc<Mutex<Child>>,
    done: mpsc::Sender<()>,
}
impl PhaseChild {
    pub(super) fn launch(run: &RunRecord, phase: RunPhase, deadline: Instant) -> Res<Self> {
        let (mut command, control, readiness) = phase_command(run, phase)?;
        let prepared_by = Instant::now() + Duration::from_millis(5000);
        let mut spawned = command
            .spawn()
            .map_err(|_| Fail::Store("run_child_start_unknown"))?;
        let pipe = readiness.as_ref().and_then(|_| spawned.stdout.take());
        let owned = Self::supervise(spawned, deadline)?;
        owned.arm_guards(control, readiness.zip(pipe), prepared_by, deadline)?;
        Ok(owned)
    }
    fn arm_guards(
        &self,
        control: Option<super::m1_run_control::Binding>,
        ready: Option<(String, ChildStdout)>,
        prepared_by: Instant,
        deadline: Instant,
    ) -> Res<()> {
        if let Some(binding) = control {
            monitor_control(Arc::clone(&self.child), binding, deadline)?;
        }
        if let Some((expected, pipe)) = ready {
            monitor_readiness(
                Arc::clone(&self.child),
                pipe,
                expected,
                prepared_by,
                deadline,
            )?;
        }
        Ok(())
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
fn phase_command(
    run: &RunRecord,
    phase: RunPhase,
) -> Res<(
    Command,
    Option<super::m1_run_control::Binding>,
    Option<String>,
)> {
    run.request.validate()?;
    let provider = run.request.run_profile == PROVIDER_FIXTURE_PROFILE;
    let control = if provider {
        Some(super::m1_run_control::Binding::qualify(&run.request)?)
    } else {
        None
    };
    let mut command =
        Command::new(std::env::current_exe().map_err(|_| Fail::Config("run_binary_unavailable"))?);
    command
        .env_clear()
        .arg("m1-run-phase")
        .arg("--run")
        .arg(run.request.run_id.to_string())
        .arg("--phase")
        .arg(phase.name())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    install_owner_environment(&mut command);
    let mut readiness = None;
    if provider {
        install_provider_environment(&mut command, phase);
        if phase != RunPhase::Acquisition {
            readiness = Some(super::m1_run_control::ready_frame(
                run.request.run_id,
                phase.name(),
            ));
        }
    }
    Ok((command, control, readiness))
}
fn install_provider_environment(c: &mut Command, phase: RunPhase) {
    if let Some(root) = std::env::var_os("DW_M1_RUN_CONTROL_ROOT") {
        c.env("DW_M1_RUN_CONTROL_ROOT", root);
    }
    if phase != RunPhase::Acquisition {
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .env(super::m1_run_control::PROVIDER_HINT, "1");
        if let Some(key) = std::env::var_os("DEEPSEEK_API_KEY") {
            c.env("DEEPSEEK_API_KEY", key);
        }
    }
}

/// A dedicated reader owns the fixed child's stdout pipe so neither the
/// watchdog nor the parent's SQL loop performs a synchronous pipe read.
fn read_ready(mut pipe: impl Read, expected: &str, verdict: &mpsc::Sender<Res<()>>) {
    let _ = verdict.send(await_ready(&mut pipe, expected));
    // Keep draining so the child's terminal/error line cannot hit a closed pipe.
    let mut chunk = [0u8; 512];
    while pipe.read(&mut chunk).map(|n| n > 0).unwrap_or(false) {}
}
fn await_ready(pipe: &mut impl Read, expected: &str) -> Res<()> {
    let mut buffered: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(Fail::Config("run_readiness_refused")),
            Ok(n) => {
                buffered.extend_from_slice(&chunk[..n]);
                if let Some(result) = ready_lines(&mut buffered, expected) {
                    return result;
                }
                if buffered.len() > 16384 {
                    return Err(Fail::Config("run_readiness_refused"));
                }
            }
        }
    }
}
fn ready_lines(buffered: &mut Vec<u8>, expected: &str) -> Option<Res<()>> {
    while let Some(pos) = buffered.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = buffered.drain(..=pos).collect();
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end();
        if text == expected {
            return Some(Ok(()));
        }
        if text.starts_with(super::m1_run_control::READY_PREFIX) {
            return Some(Err(Fail::Config("run_readiness_refused")));
        }
    }
    None
}
/// Independent of the parent's SQL loop: missing, malformed, mismatched,
/// oversized, EOF or late readiness cannot extend or disarm the deadline.
fn monitor_readiness(
    child: Arc<Mutex<Child>>,
    pipe: ChildStdout,
    expected: String,
    prepared_by: Instant,
    deadline: Instant,
) -> Res<()> {
    let (verdict, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("run_readiness_reader".into())
        .spawn(move || read_ready(pipe, &expected, &verdict))
        .map_err(|_| Fail::Config("run_control_unqualified"))?;
    std::thread::Builder::new()
        .name("run_readiness_watchdog".into())
        .spawn(move || {
            if !matches!(
                rx.recv_timeout(prepared_by.saturating_duration_since(Instant::now())),
                Ok(Ok(()))
            ) {
                cooperative_stop(&child, deadline);
            }
        })
        .map_err(|_| Fail::Config("run_control_unqualified"))?;
    Ok(())
}
fn monitor_control(
    child: Arc<Mutex<Child>>,
    binding: super::m1_run_control::Binding,
    deadline: Instant,
) -> Res<()> {
    std::thread::Builder::new()
        .name("run_local_control".into())
        .spawn(move || {
            while Instant::now() < deadline {
                let running = child
                    .lock()
                    .ok()
                    .is_some_and(|mut c| matches!(c.try_wait(), Ok(None)));
                if !running {
                    return;
                }
                if binding.current().is_err() {
                    cooperative_stop(&child, deadline);
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        })
        .map_err(|_| Fail::Config("run_control_unqualified"))?;
    Ok(())
}
fn cooperative_stop(child: &Arc<Mutex<Child>>, deadline: Instant) {
    if let Ok(mut child) = child.lock() {
        // Closing our sole write handle sends EOF without a potentially blocking pipe write.
        child.stdin.take();
    }
    let end = deadline.min(Instant::now() + Duration::from_millis(2000));
    while Instant::now() < end {
        if child
            .lock()
            .ok()
            .is_some_and(|mut c| matches!(c.try_wait(), Ok(Some(_))))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    reap(child);
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
    // Base owner allowlist; selected inference key/control are added separately, never PATH/proxies/flags.
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
    #[test]
    fn readiness_frame_contract_accepts_exact_and_refuses_everything_else() {
        let run = uuid::Uuid::from_u128(0x5150);
        let expected = super::super::m1_run_control::ready_frame(run, "decision_one");
        let verdict = |bytes: &[u8]| {
            let (tx, rx) = mpsc::channel();
            read_ready(std::io::Cursor::new(bytes.to_vec()), &expected, &tx);
            rx.recv().unwrap()
        };
        // Exact frame, and unrelated lines (e.g. a bind_input receipt) skipped.
        assert!(verdict(format!("{expected}\n").as_bytes()).is_ok());
        assert!(verdict(format!("{{\"receipt\":1}}\n{expected}\nerror=x\n").as_bytes()).is_ok());
        // Missing, mismatched and prefix-shaped-but-wrong content refuse.
        assert!(verdict(b"").is_err());
        assert!(verdict(b"hello\n").is_err());
        assert!(verdict(format!("{expected} two\n").as_bytes()).is_err());
        assert!(
            verdict(format!("{} bad\n", super::super::m1_run_control::READY_PREFIX).as_bytes())
                .is_err()
        );
        // Oversized content without a complete frame refuses, never grows.
        assert!(verdict(&vec![b'x'; 20000]).is_err());
    }
    #[test]
    fn pending_readiness_expires_and_reapes_parked_child_without_parent_polling() {
        let mut command = Command::new(
            std::path::Path::new(&std::env::var_os("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        );
        #[cfg(not(windows))]
        let mut command = Command::new("/bin/sleep");
        #[cfg(windows)]
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Threading.Thread]::Sleep(60000)",
        ]);
        #[cfg(not(windows))]
        command.arg("60");
        let mut spawned = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pipe = spawned.stdout.take().unwrap();
        let child = Arc::new(Mutex::new(spawned));
        // Absolute remaining deadline: pending readiness expires on its own.
        monitor_readiness(
            Arc::clone(&child),
            pipe,
            "never sent".to_string(),
            Instant::now() + Duration::from_millis(150),
            Instant::now() + Duration::from_secs(60),
        )
        .unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert!(child.lock().unwrap().try_wait().unwrap().is_some());
    }
}

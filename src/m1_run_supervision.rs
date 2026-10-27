//! Independent child reaper. The deadline thread never waits on SQL or output pipes.
use duskweave::m1_reasoning::transport_control::ReportedGuard;
use duskweave::m1_run::{
    CauseSource, DiagnosticCause, PhaseDiagnostic, RunPhase, RunRecord, safe_child_error,
};
use duskweave::{Fail, Res};
use std::io::Read;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

pub(super) struct PhaseChild {
    child: Arc<Mutex<Child>>,
    done: mpsc::Sender<()>,
    observation: Arc<Mutex<Observation>>,
    terminal: Option<mpsc::Receiver<()>>,
    launched: Instant,
    deadline: Instant,
    readiness_required: bool,
    successor: bool,
}
#[derive(Default)]
struct Observation {
    cause: Option<DiagnosticCause>,
    ready: bool,
    error: Option<&'static str>,
    reported_guard: Option<ReportedGuard>,
}
impl Observation {
    fn from_readiness(result: &Res<()>) -> Self {
        match result {
            Ok(()) => Self {
                ready: true,
                ..Self::default()
            },
            Err(Fail::Config("run_readiness_refused")) => Self {
                cause: Some(DiagnosticCause::ReadinessRefused),
                ..Self::default()
            },
            Err(Fail::Config(code)) => Self {
                cause: Some(DiagnosticCause::ChildError),
                error: Some(code),
                ..Self::default()
            },
            _ => Self {
                cause: Some(DiagnosticCause::ReadinessRefused),
                ..Self::default()
            },
        }
    }
    fn merge(&mut self, other: Self) {
        self.error = other.error.or(self.error);
        self.reported_guard = other.reported_guard.or(self.reported_guard);
        if let Some(cause) = other.cause {
            self.cause.get_or_insert(cause);
        }
    }
}
fn observed(state: &Arc<Mutex<Observation>>, cause: DiagnosticCause) {
    if let Ok(mut state) = state.lock() {
        state.cause.get_or_insert(cause);
    }
}
impl PhaseChild {
    pub(super) fn launch(run: &RunRecord, phase: RunPhase, deadline: Instant) -> Res<Self> {
        let (mut command, control, readiness) = phase_command(run, phase)?;
        let launched = Instant::now();
        let prepared_by = Instant::now() + super::m1_run_control::preparation_bound();
        let mut spawned = command
            .spawn()
            .map_err(|_| Fail::Store("run_child_start_unknown"))?;
        let pipe = readiness.as_ref().and_then(|_| spawned.stdout.take());
        let mut owned = Self::supervise(spawned, deadline)?;
        owned.launched = launched;
        owned.successor = run.request.successor();
        owned.arm_guards(control, readiness.zip(pipe), prepared_by, deadline)?;
        Ok(owned)
    }
    fn arm_guards(
        &mut self,
        control: Option<super::m1_run_control::Binding>,
        ready: Option<(String, ChildStdout)>,
        prepared_by: Instant,
        deadline: Instant,
    ) -> Res<()> {
        if let Some(binding) = control {
            monitor_control(
                Arc::clone(&self.child),
                binding,
                deadline,
                Arc::clone(&self.observation),
            )?;
        }
        if let Some((expected, pipe)) = ready {
            self.readiness_required = true;
            self.terminal = Some(monitor_readiness(
                Arc::clone(&self.child),
                pipe,
                expected,
                prepared_by,
                deadline,
                Arc::clone(&self.observation),
            )?);
        }
        Ok(())
    }
    fn supervise(child: Child, deadline: Instant) -> Res<Self> {
        let child = Arc::new(Mutex::new(child));
        let (done, rx) = mpsc::channel();
        let owned = Arc::clone(&child);
        let observation = Arc::new(Mutex::new(Observation::default()));
        let watchdog = Arc::clone(&observation);
        std::thread::Builder::new()
            .name("run_phase_deadline".into())
            .spawn(move || {
                if rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .is_err()
                    && owned
                        .lock()
                        .is_ok_and(|mut c| matches!(c.try_wait(), Ok(None)))
                {
                    observed(&watchdog, DiagnosticCause::PhaseDeadline);
                    reap(&owned);
                }
            })
            .map_err(|_| {
                reap(&child);
                Fail::Store("run_supervisor_unavailable")
            })?;
        Ok(Self {
            child,
            done,
            observation,
            terminal: None,
            launched: Instant::now(),
            deadline,
            readiness_required: false,
            successor: false,
        })
    }
    pub(super) fn finished(&self) -> Res<Option<std::process::ExitStatus>> {
        self.child
            .lock()
            .map_err(|_| Fail::Store("run_child_unknown"))?
            .try_wait()
            .map_err(|_| Fail::Store("run_child_unknown"))
    }
    /// Stop before SQL, then drain only within the original phase deadline.
    pub(super) fn diagnostic(&mut self, run_id: uuid::Uuid, phase: RunPhase) -> PhaseDiagnostic {
        let exit_code = self.terminal_status();
        let state = self.observation.lock().ok();
        let error = state.as_ref().and_then(|s| s.error);
        let cause = state.as_ref().and_then(|s| s.cause).unwrap_or_else(|| {
            if error.is_some() {
                DiagnosticCause::ChildError
            } else {
                DiagnosticCause::ChildExit
            }
        });
        let cause_source = match cause {
            DiagnosticCause::ChildError => CauseSource::ChildReport,
            DiagnosticCause::Unavailable => CauseSource::Unavailable,
            _ => CauseSource::ParentObserved,
        };
        PhaseDiagnostic {
            version: PhaseDiagnostic::version_for(self.successor).into(),
            run_id,
            phase,
            cause,
            cause_source,
            child_error: error.map(str::to_owned),
            exit_code,
            readiness_observed: state.as_ref().is_some_and(|s| s.ready),
            elapsed_ms: Some(self.launched.elapsed().as_millis().min(150000) as u32),
            reported_guard: state.as_ref().and_then(|s| s.reported_guard),
        }
    }
    fn terminal_status(&mut self) -> Option<i32> {
        let _ = self.done.send(());
        if matches!(self.finished(), Ok(None)) {
            let cause = if Instant::now() >= self.deadline {
                DiagnosticCause::PhaseDeadline
            } else {
                DiagnosticCause::SupervisorUnavailable
            };
            observed(&self.observation, cause);
        }
        reap(&self.child);
        let exit_code = self.finished().ok().flatten().and_then(|s| s.code());
        if let Some(terminal) = self.terminal.take() {
            // Reader EOF and the deadline-qualified readiness verdict both settle
            // before snapshot/acceptance. Neither receives a new grace clock.
            for _ in 0..2 {
                if terminal
                    .recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
                    .is_err()
                {
                    observed(&self.observation, DiagnosticCause::Unavailable);
                    break;
                }
            }
        }
        exit_code
    }
    pub(super) fn protocol_valid(&self) -> bool {
        self.observation.lock().is_ok_and(|s| {
            s.cause.is_none() && s.error.is_none() && (!self.readiness_required || s.ready)
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
    let provider = run.request.supervised_inference();
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
    install_launch_environment(&mut command, run);
    let mut readiness = None;
    if provider {
        install_provider_environment(&mut command, phase, run);
        if phase != RunPhase::Acquisition {
            readiness = Some(super::m1_run_control::ready_frame(
                run.request.run_id,
                phase.name(),
            ));
        }
    }
    Ok((command, control, readiness))
}
fn install_provider_environment(c: &mut Command, phase: RunPhase, run: &RunRecord) {
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
    } else {
        for key in ["DW_BOUNDARY_LAUNCHER", "DW_BOUNDARY_IMAGE"] {
            if let Some(value) = std::env::var_os(key) {
                c.env(key, value);
            }
        }
    }
    let network = run
        .request
        .inference_profile
        .as_deref()
        .and_then(|p| duskweave::m1_inference_profile::lookup(p).ok())
        .is_some_and(|p| p.connection.credential_environment.is_some());
    if run.request.inference_profile.is_some() && !network {
        c.env_remove("DEEPSEEK_API_KEY");
    }
}

/// A dedicated reader owns the fixed child's stdout pipe so neither the
/// watchdog nor the parent's SQL loop performs a synchronous pipe read.
fn read_ready(pipe: impl Read, expected: &str, verdict: &mpsc::Sender<Res<()>>) -> Observation {
    let mut pipe = pipe.take(16384);
    let first = await_ready(&mut pipe, expected);
    let mut state = Observation::from_readiness(&first);
    let _ = verdict.send(first);
    if state.ready && read_reports(&mut pipe, &mut state).is_err() {
        state.reported_guard = None;
        state.cause = Some(DiagnosticCause::ReadinessRefused);
    }
    // Keep draining so the child's terminal/error line cannot hit a closed pipe.
    if !matches!(std::io::copy(&mut pipe, &mut std::io::sink()), Ok(0)) || pipe.limit() == 0 {
        state.cause = Some(DiagnosticCause::ReadinessRefused);
    }
    state
}
fn read_reports(pipe: &mut impl Read, state: &mut Observation) -> Res<()> {
    let mut reports = 0;
    for _ in 0..8 {
        let Some(frame) = optional_frame(pipe)? else {
            return Ok(());
        };
        if let Some(guard) = guard_frame(&frame) {
            reports += 1;
            state.reported_guard = (reports == 1).then_some(guard);
        } else {
            state.error = Some(error_frame(&frame).ok_or(Fail::Config("run_readiness_refused"))?);
            if optional_frame(pipe)?.is_some() {
                return Err(Fail::Config("run_readiness_refused"));
            }
            return Ok(());
        }
    }
    Err(Fail::Config("run_readiness_refused"))
}
fn guard_frame(frame: &[u8]) -> Option<ReportedGuard> {
    let text = std::str::from_utf8(frame)
        .ok()?
        .strip_prefix("m1_run_guard_v1 ")?
        .strip_suffix('\n')?;
    serde_json::from_value(serde_json::Value::String(text.into())).ok()
}
fn await_ready(pipe: &mut impl Read, expected: &str) -> Res<()> {
    let frame = read_frame(pipe)?;
    if frame == format!("{expected}\n").as_bytes() {
        return Ok(());
    }
    if let Some(code) = error_frame(&frame) {
        return Err(Fail::Config(code));
    }
    Err(Fail::Config("run_readiness_refused"))
}
fn error_frame(frame: &[u8]) -> Option<&'static str> {
    let text = std::str::from_utf8(frame).ok()?;
    let code = text.strip_prefix("error=")?.strip_suffix('\n')?;
    // Framing syntax is exact; unknown code content is never reflected.
    if code.is_empty() || code.bytes().any(|b| b.is_ascii_whitespace()) {
        return None;
    }
    Some(safe_child_error(code))
}
fn optional_frame(pipe: &mut impl Read) -> Res<Option<Vec<u8>>> {
    let mut first = [0u8; 1];
    if pipe
        .read(&mut first)
        .map_err(|_| Fail::Config("run_readiness_refused"))?
        == 0
    {
        return Ok(None);
    }
    let mut prefixed = std::io::Cursor::new(first).chain(pipe);
    read_frame(&mut prefixed).map(Some)
}
fn read_frame(pipe: &mut impl Read) -> Res<Vec<u8>> {
    let mut frame = Vec::with_capacity(128);
    for _ in 0..128 {
        let mut byte = [0u8; 1];
        pipe.read_exact(&mut byte)
            .map_err(|_| Fail::Config("run_readiness_refused"))?;
        frame.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(frame);
        }
    }
    Err(Fail::Config("run_readiness_refused"))
}
/// Independent of the parent's SQL loop: missing, malformed, mismatched,
/// oversized, EOF or late readiness cannot extend or disarm the deadline.
fn monitor_readiness(
    child: Arc<Mutex<Child>>,
    pipe: ChildStdout,
    expected: String,
    prepared_by: Instant,
    deadline: Instant,
    observation: Arc<Mutex<Observation>>,
) -> Res<mpsc::Receiver<()>> {
    let prepared_by = prepared_by.min(deadline);
    let (verdict, rx) = mpsc::channel();
    let (finished, terminal) = mpsc::channel();
    let reader_finished = finished.clone();
    let reader = Arc::clone(&observation);
    std::thread::Builder::new()
        .name("run_readiness_reader".into())
        .spawn(move || {
            let result = read_ready(pipe, &expected, &verdict);
            if let Ok(mut state) = reader.lock() {
                state.merge(result);
            }
            let _ = reader_finished.send(());
        })
        .map_err(|_| Fail::Config("run_control_unqualified"))?;
    std::thread::Builder::new()
        .name("run_readiness_watchdog".into())
        .spawn(move || {
            readiness_watch(&child, &rx, prepared_by, &observation);
            let _ = finished.send(());
        })
        .map_err(|_| Fail::Config("run_control_unqualified"))?;
    Ok(terminal)
}
fn readiness_watch(
    child: &Arc<Mutex<Child>>,
    rx: &mpsc::Receiver<Res<()>>,
    prepared_by: Instant,
    observation: &Arc<Mutex<Observation>>,
) {
    let wait = prepared_by
        .saturating_duration_since(Instant::now())
        .saturating_sub(Duration::from_millis(50));
    let result = match rx.recv_timeout(wait) {
        Ok(result) if Instant::now() < prepared_by => Observation::from_readiness(&result),
        Err(mpsc::RecvTimeoutError::Disconnected) => Observation {
            cause: Some(DiagnosticCause::ReadinessRefused),
            ..Observation::default()
        },
        _ => Observation {
            cause: Some(DiagnosticCause::ReadinessDeadline),
            ..Observation::default()
        },
    };
    let ready = result.ready;
    if let Ok(mut state) = observation.lock() {
        state.ready = ready;
        state.merge(result);
    }
    if !ready {
        cooperative_stop(child, prepared_by);
    }
}
#[cfg(test)]
fn ready_before(rx: &mpsc::Receiver<Res<()>>, prepared_by: Instant) -> bool {
    matches!(
        rx.recv_timeout(
            prepared_by
                .saturating_duration_since(Instant::now())
                .saturating_sub(Duration::from_millis(50))
        ),
        Ok(Ok(()))
    ) && Instant::now() < prepared_by
}
fn monitor_control(
    child: Arc<Mutex<Child>>,
    binding: super::m1_run_control::Binding,
    deadline: Instant,
    observation: Arc<Mutex<Observation>>,
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
                    observed(&observation, DiagnosticCause::LocalStop);
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
fn install_launch_environment(c: &mut Command, run: &RunRecord) {
    if run.request.successor() {
        c.args([
            "--timing-profile",
            duskweave::m1_inference_profile::TIMING_PROFILE,
        ]);
    }
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

pub(super) fn selected_phase_deadline(run: Instant, phase: RunPhase, successor: bool) -> Instant {
    if successor && phase != RunPhase::Acquisition {
        return run.min(
            Instant::now()
                + Duration::from_millis(
                    duskweave::m1_inference_profile::REASONING_BOUNDED.phase_ms,
                ),
        );
    }
    phase_deadline(run, phase)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successor_phase_selection_keeps_acquisition_and_original_run_bounds() {
        let now = Instant::now();
        let run = now + Duration::from_millis(400000);
        let successor = selected_phase_deadline(run, RunPhase::DecisionOne, true);
        assert!(successor >= now + Duration::from_millis(134000));
        assert!(successor <= Instant::now() + Duration::from_millis(134000));
        let acquisition = selected_phase_deadline(run, RunPhase::Acquisition, true);
        assert!(acquisition <= Instant::now() + Duration::from_millis(60000));
        let legacy = selected_phase_deadline(run, RunPhase::DecisionTwo, false);
        assert!(legacy <= Instant::now() + Duration::from_millis(50000));
        let remainder = Instant::now() + Duration::from_millis(10);
        assert_eq!(
            selected_phase_deadline(remainder, RunPhase::DecisionTwo, true),
            remainder
        );
    }
    fn parsed(bytes: &[u8]) -> Observation {
        let (tx, _rx) = mpsc::channel();
        read_ready(std::io::Cursor::new(bytes), "expected", &tx)
    }
    #[test]
    fn guard_reports_never_change_parent_cause_and_competing_or_malformed_reports_are_unknown() {
        let one = parsed(b"expected\nm1_run_guard_v1 transport_hard_deadline\n");
        assert_eq!(
            one.reported_guard,
            Some(ReportedGuard::TransportHardDeadline)
        );
        assert_eq!(one.cause, None);
        assert!(one.ready);
        let competing = parsed(
            b"expected\nm1_run_guard_v1 transport_hard_deadline\nm1_run_guard_v1 permit_expiry\n",
        );
        assert_eq!(competing.reported_guard, None);
        assert_eq!(competing.cause, None);
        for bad in ["timeout", "permit_expiry\r", "synthetic-sensitive-sentinel"] {
            let malformed = parsed(format!("expected\nm1_run_guard_v1 {bad}\n").as_bytes());
            assert_eq!(malformed.reported_guard, None);
            assert_eq!(malformed.cause, Some(DiagnosticCause::ReadinessRefused));
        }
        let missing = parsed(b"expected\nerror=child_failed\n");
        assert_eq!(missing.reported_guard, None);
        assert_eq!(missing.error, Some("child_failed"));
    }
    #[test]
    fn terminal_projection_is_closed_before_and_after_ready_and_rejects_extra_frames() {
        for prefix in ["", "expected\n"] {
            for code in ["missing_env", "run_child_refused", "run_inference_pending"] {
                let observed = parsed(format!("{prefix}error={code}\n").as_bytes());
                assert_eq!(observed.error, Some(code));
                assert_eq!(observed.ready, !prefix.is_empty());
            }
            let state = parsed(format!("{prefix}error=synthetic-sensitive-sentinel\n").as_bytes());
            assert_eq!(state.error, Some("child_failed"));
            for bad in [
                "error=missing_env\r\n",
                "error=missing_env",
                "error=missing_env \n",
                "error=\n",
                "{}\n",
                "error=missing_env\nextra\n",
            ] {
                let state = parsed(format!("{prefix}{bad}").as_bytes());
                assert_eq!(state.cause, Some(DiagnosticCause::ReadinessRefused));
            }
            let mut bad = format!("{prefix}error=").into_bytes();
            bad.extend_from_slice(&[0xff, b'\n']);
            assert_eq!(parsed(&bad).cause, Some(DiagnosticCause::ReadinessRefused));
            let oversize = format!("{prefix}error={}\n", "x".repeat(128));
            assert_eq!(
                parsed(oversize.as_bytes()).cause,
                Some(DiagnosticCause::ReadinessRefused)
            );
        }
        assert!(parsed(b"expected\n").ready);
        assert_eq!(parsed(b"expected\n").error, None);
    }
    fn spawn_terminal(frame: &str, exit: i32) -> Child {
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
            &format!("[Console]::Write('{frame}'); exit {exit}"),
        ]);
        #[cfg(not(windows))]
        let mut command = Command::new("/bin/sh");
        #[cfg(not(windows))]
        command.args(["-c", &format!("printf '{frame}'; exit {exit}")]);
        command.env_remove("DEEPSEEK_API_KEY");
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }
    fn terminal_child(frame: &str, exit: i32) -> PhaseChild {
        let mut spawned = spawn_terminal(frame, exit);
        let pipe = spawned.stdout.take().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut child = PhaseChild::supervise(spawned, deadline).unwrap();
        child
            .arm_guards(None, Some(("expected".into(), pipe)), deadline, deadline)
            .unwrap();
        while child.finished().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        child
    }
    #[test]
    fn immediate_exit_reader_race_retains_code_and_exit_124_does_not_identify_a_watchdog() {
        for _ in 0..12 {
            let mut child = terminal_child("error=missing_env\n", 1);
            let d = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
            assert_eq!(d.child_error.as_deref(), Some("missing_env"));
            assert_eq!(d.exit_code, Some(1));
            assert_eq!(d.cause_source, CauseSource::ChildReport);
            assert!(!child.protocol_valid());
        }
        let mut child = terminal_child("", 124);
        let d = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
        assert_eq!(d.exit_code, Some(124));
        assert_eq!(d.child_error, None);
        assert_ne!(d.cause, DiagnosticCause::ReadinessDeadline);
        assert_ne!(d.cause, DiagnosticCause::PhaseDeadline);
        #[cfg(windows)]
        {
            let mut child = terminal_child("", -1073741819);
            assert_eq!(
                child
                    .diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne)
                    .exit_code,
                Some(-1073741819)
            );
        }
    }
    #[test]
    fn reader_completion_without_a_watchdog_completion_cannot_finalize_success() {
        let mut child = terminal_child("expected\n", 0);
        // Synthetic completion ordering: reader has finished, watchdog is still pending.
        // Keep its sender alive so absence is a wait, not a disconnect.
        let (finished, rx) = mpsc::channel();
        finished.send(()).unwrap();
        child.terminal = Some(rx);
        child.deadline = Instant::now() + Duration::from_millis(30);
        let diagnostic = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
        assert_eq!(diagnostic.cause, DiagnosticCause::Unavailable);
        assert!(!child.protocol_valid());
        drop(finished);
    }
    #[test]
    fn gated_late_watchdog_rejects_exact_ready_and_successful_exit_before_snapshot() {
        let mut spawned = spawn_terminal("expected\n", 0);
        let pipe = spawned.stdout.take().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut child = PhaseChild::supervise(spawned, deadline).unwrap();
        child.readiness_required = true;
        let (verdict, rx) = mpsc::channel();
        let (finished, terminal) = mpsc::channel();
        child.terminal = Some(terminal);
        let result = read_ready(pipe, "expected", &verdict);
        assert!(result.ready);
        child.observation.lock().unwrap().merge(result);
        finished.send(()).unwrap();
        while child.finished().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(child.finished().unwrap().unwrap().success());
        assert!(
            !child.protocol_valid(),
            "reader alone cannot authorize acceptance"
        );
        let (gate, held) = mpsc::channel();
        let observed = Arc::clone(&child.observation);
        let process = Arc::clone(&child.child);
        let prepared_by = Instant::now() - Duration::from_millis(1);
        let watchdog = std::thread::spawn(move || {
            held.recv().unwrap();
            readiness_watch(&process, &rx, prepared_by, &observed);
            finished.send(()).unwrap();
        });
        // The queued exact frame is now processed after its original readiness bound.
        gate.send(()).unwrap();
        let diagnostic = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
        watchdog.join().unwrap();
        assert_eq!(diagnostic.exit_code, Some(0));
        assert_eq!(diagnostic.cause, DiagnosticCause::ReadinessDeadline);
        assert!(!diagnostic.readiness_observed);
        assert!(!child.protocol_valid());
    }
    #[test]
    fn child_terminal_error_before_ready_preserves_the_safe_code() {
        let (tx, rx) = mpsc::channel();
        read_ready(
            std::io::Cursor::new(b"error=missing_env\n"),
            "expected",
            &tx,
        );
        assert!(matches!(
            rx.recv().unwrap(),
            Err(Fail::Config("missing_env"))
        ));
    }
    #[test]
    fn queued_late_ready_eof_and_refusal_cannot_disarm_the_original_guard() {
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(())).unwrap();
        assert!(!ready_before(
            &rx,
            Instant::now() - Duration::from_millis(1)
        ));
        let end = Instant::now() + Duration::from_secs(5);
        tx.send(Err(Fail::Config("run_readiness_refused"))).unwrap();
        assert!(!ready_before(&rx, end));
        tx.send(Ok(())).unwrap();
        assert!(ready_before(&rx, end));
        drop(tx);
        assert!(!ready_before(&rx, end));
    }
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
        // Exact frame only; a prelude must never be skipped.
        assert!(verdict(format!("{expected}\n").as_bytes()).is_ok());
        assert!(verdict(format!("{{\"receipt\":1}}\n{expected}\nerror=x\n").as_bytes()).is_err());
        for suffix in [" ", "  ", "\t", "\r", &" ".repeat(200)] {
            assert!(
                verdict(format!("{expected}{suffix}\n").as_bytes()).is_err(),
                "suffix accepted"
            );
        }
        let mut invalid = expected.as_bytes().to_vec();
        invalid.extend_from_slice(&[0xff, b'\n']);
        assert!(verdict(&invalid).is_err());
        assert!(verdict(expected.as_bytes()).is_err());
        for frame in [
            super::super::m1_run_control::ready_frame(
                uuid::Uuid::from_u128(0x5151),
                "decision_one",
            ),
            super::super::m1_run_control::ready_frame(run, "decision_two"),
            expected.to_uppercase(),
        ] {
            assert!(verdict(format!("{frame}\n").as_bytes()).is_err());
        }
        for n in [128, 129] {
            let mut padded = expected.as_bytes().to_vec();
            padded.resize(n - 1, b' ');
            padded.push(b'\n');
            assert!(verdict(&padded).is_err(), "{n}-byte padding accepted");
            let mut cursor = std::io::Cursor::new(padded);
            assert!(await_ready(&mut cursor, &expected).is_err());
            assert_eq!(
                cursor.position(),
                128,
                "raw framing cannot consume a 129th byte"
            );
        }
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
        #[cfg(windows)]
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
            Arc::new(Mutex::new(Observation::default())),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(400));
        assert!(child.lock().unwrap().try_wait().unwrap().is_some());
    }
    fn credential_record(profile: Option<&str>) -> RunRecord {
        let id = uuid::Uuid::from_u128(0x77);
        serde_json::from_value(serde_json::json!({
            "request":{"version":"m1_autonomous_run_request_v2","run_id":id,
                "run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
                "inference_profile":profile,"deadline_at_ms":1700004000000i64,
                "context":{"version":"m1_context_prepare_request_v1","mission":{
                    "version":"m1_inference_reservation_request_v1","engagement_id":id,"campaign_id":id,
                    "operator_ref":id,"expected_mission_revision":1,"permission_id":id,"goal_ref":id,
                    "exercise_mode":"blind"},"candidates":[{"asset_ref":id,"priority":"high"}]},
                "seed_names":[{"asset_ref":id,"name":"seed.example.invalid"}]},
            "decision_one":id,"decision_two":id,"attempt":id,"result":id,"terrain_operation":id,
            "release_sha256":"ab","qualification_ref":id,"status":"running","deadline_at_ms":1700004000000i64
        }))
        .unwrap()
    }
    #[test]
    fn credential_forwarding_follows_the_selected_profile_credential() {
        const CASE: &str = "DW_TEST_CREDENTIAL_FORWARDING_CASE";
        if let Ok(case) = std::env::var(CASE) {
            assert_credential_forwarding(case == "present");
            return;
        }
        for case in ["absent", "present"] {
            let mut child = Command::new(std::env::current_exe().unwrap());
            child.args([
                "--exact",
                "m1_run_supervision::tests::credential_forwarding_follows_the_selected_profile_credential",
                "--test-threads=1",
            ]).env(CASE, case).env_remove("DEEPSEEK_API_KEY");
            if case == "present" {
                child.env("DEEPSEEK_API_KEY", "synthetic-forwarding-sentinel");
            }
            let output = child.output().unwrap();
            // Child output is private even on failure; never log the sentinel.
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "credential forwarding child failed: {case}"
            );
        }
    }
    fn assert_credential_forwarding(key_present: bool) {
        for (profile, forwarded) in [
            (Some("deepseek-responses-reasoning-v1"), true),
            (Some("deepseek-responses-low-v1"), true),
            (Some("fixture-responses-v1"), false),
            (Some("fixture-chat-completions-v1"), false),
            (Some("synthetic-unknown-profile"), false),
        ] {
            let run = credential_record(profile);
            for phase in [
                RunPhase::DecisionOne,
                RunPhase::DecisionTwo,
                RunPhase::Acquisition,
            ] {
                let mut c = Command::new("unused");
                c.env_clear();
                install_provider_environment(&mut c, phase, &run);
                let entry = c
                    .get_envs()
                    .find(|(k, _)| *k == "DEEPSEEK_API_KEY")
                    .map(|(_, v)| v.map(|v| v.to_os_string()));
                let carries_key = matches!(entry, Some(Some(_)));
                assert_eq!(
                    carries_key,
                    forwarded && phase != RunPhase::Acquisition && key_present,
                    "{profile:?}/{phase:?}: key must reach only a network decision child with a synthetic key"
                );
                assert!(
                    entry
                        .flatten()
                        .is_none_or(|value| value == "synthetic-forwarding-sentinel")
                );
            }
        }
    }
}

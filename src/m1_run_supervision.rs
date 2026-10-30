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
    let result = readiness_result(rx, prepared_by);
    let ready = result.ready;
    if let Ok(mut state) = observation.lock() {
        state.ready = ready;
        state.merge(result);
    }
    if !ready {
        cooperative_stop(child, prepared_by);
    }
}
/// One readiness decision shared by the real watchdog and its boundary tests.
fn readiness_result(rx: &mpsc::Receiver<Res<()>>, prepared_by: Instant) -> Observation {
    // Do not revoke a pending child before its original readiness deadline.
    // A queued frame is still rejected if processed after that deadline.
    let wait = prepared_by.saturating_duration_since(Instant::now());
    match rx.recv_timeout(wait) {
        Ok(result) if Instant::now() < prepared_by => Observation::from_readiness(&result),
        Err(mpsc::RecvTimeoutError::Disconnected) => Observation {
            cause: Some(DiagnosticCause::ReadinessRefused),
            ..Observation::default()
        },
        _ => Observation {
            cause: Some(DiagnosticCause::ReadinessDeadline),
            ..Observation::default()
        },
    }
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
#[path = "../tests/support/m1_run_supervision.rs"]
mod tests;

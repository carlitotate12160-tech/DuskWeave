//! Profile-selected child launch environment and bounded phase deadlines.
use crate::m1_run_control::{Binding, PROVIDER_HINT, ready_frame};
use duskweave::m1_inference_profile::{Timing, lookup};
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::{Fail, Res};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn ready_control(
    run: &RunRecord,
    phase: RunPhase,
    provider: bool,
) -> Res<(Option<Binding>, Option<String>)> {
    if !provider {
        return Ok((None, None));
    }
    let control = Some(Binding::qualify(&run.request)?);
    let readiness =
        (phase != RunPhase::Acquisition).then(|| ready_frame(run.request.run_id, phase.name()));
    Ok((control, readiness))
}

pub(super) fn phase_command(
    run: &RunRecord,
    phase: RunPhase,
) -> Res<(Command, Option<Binding>, Option<String>)> {
    run.request.validate()?;
    let provider = run.request.supervised_inference();
    let (control, readiness) = ready_control(run, phase, provider)?;
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
    install_launch_environment(&mut command, run)?;
    if provider {
        install_provider_environment(&mut command, phase, run);
    }
    Ok((command, control, readiness))
}

pub(super) fn install_provider_environment(c: &mut Command, phase: RunPhase, run: &RunRecord) {
    if let Some(root) = std::env::var_os("DW_M1_RUN_CONTROL_ROOT") {
        c.env("DW_M1_RUN_CONTROL_ROOT", root);
    }
    if phase != RunPhase::Acquisition {
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .env(PROVIDER_HINT, "1");
        if let Some(key) = std::env::var_os("DEEPSEEK_API_KEY") {
            c.env("DEEPSEEK_API_KEY", key);
        }
    }
    let network = run
        .request
        .inference_profile
        .as_deref()
        .and_then(|p| lookup(p).ok())
        .is_some_and(|p| p.connection.credential_environment.is_some());
    if run.request.inference_profile.is_some() && !network {
        c.env_remove("DEEPSEEK_API_KEY");
    }
}

pub(super) fn install_launch_environment(c: &mut Command, run: &RunRecord) -> Res<()> {
    if let Some(timing) = run.request.timing_profile()? {
        c.args(["--timing-profile", timing.name]);
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
    Ok(())
}

pub(super) fn phase_deadline(run: Instant, phase: RunPhase) -> Instant {
    let ceiling = if phase == RunPhase::Acquisition {
        60000
    } else {
        50000
    };
    run.min(Instant::now() + Duration::from_millis(ceiling))
}

pub(crate) fn selected_phase_deadline(
    run: Instant,
    phase: RunPhase,
    timing: Option<&Timing>,
) -> Instant {
    if let Some(t) = timing
        && phase != RunPhase::Acquisition
    {
        return run.min(Instant::now() + Duration::from_millis(t.phase_ms));
    }
    phase_deadline(run, phase)
}

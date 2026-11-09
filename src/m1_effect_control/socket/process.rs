//! Fixed launcher subprocess composition and bounded exit/reap ownership.
use super::*;

#[cfg(target_os = "linux")]
pub(super) fn spawn_launcher(
    read_ms: u64,
) -> Res<(
    HostChannel<std::os::unix::net::UnixStream>,
    std::process::Child,
)> {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;
    use std::time::Duration;
    let (parent, child) = UnixStream::pair().map_err(|_| Fail::Config("live_host_unavailable"))?;
    parent
        .set_read_timeout(Some(Duration::from_millis(read_ms)))
        .map_err(|_| Fail::Config("live_host_unavailable"))?;
    parent
        .set_write_timeout(Some(Duration::from_millis(1000)))
        .map_err(|_| Fail::Config("live_host_unavailable"))?;
    let process = launcher_command()
        .stdin(Stdio::from(OwnedFd::from(child)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Fail::Config("live_host_unavailable"))?;
    Ok((HostChannel::new(parent), process))
}

#[cfg(target_os = "linux")]
fn launcher_command() -> std::process::Command {
    #[cfg(test)]
    if let Some(mut command) = tests::peer::command() {
        command.env_clear();
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        return command;
    }
    let mut command = std::process::Command::new("/usr/bin/sudo");
    command.args(["-n", "--", LAUNCHER]).env_clear();
    command
}

#[cfg(target_os = "linux")]
pub(super) fn exited(process: &mut std::process::Child, grace_ms: u64) -> Res<bool> {
    let budget = Duration::from_millis(grace_ms);
    let deadline = Instant::now() + budget;
    let termination = deadline - (budget / 2).min(Duration::from_millis(50));
    if let Some(clean) = wait_exited(process, termination)? {
        return Ok(clean);
    }
    let _ = process.kill();
    // Termination and reap share the original owner budget.
    wait_exited(process, deadline)?;
    Ok(false)
}
#[cfg(target_os = "linux")]
fn wait_exited(process: &mut std::process::Child, deadline: Instant) -> Res<Option<bool>> {
    loop {
        if let Some(status) = process
            .try_wait()
            .map_err(|_| Fail::Config("live_host_unavailable"))?
        {
            return Ok(Some(status.success()));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

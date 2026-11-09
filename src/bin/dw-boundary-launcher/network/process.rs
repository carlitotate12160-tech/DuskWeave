//! Bounded ip/nft subprocess and pipe lifetime; caller owns policy and deadlines.
use super::{IP, NFT};
use duskweave::{Fail, Res};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn spawn(mut command: Command) -> Res<std::process::Child> {
    #[cfg(test)]
    if let Some(result) = TEST_SPAWN.with(|slot| slot.borrow_mut().as_mut().map(|f| f(&command))) {
        return result.map_err(|_| Fail::Config("live_host_unqualified"));
    }
    command
        .spawn()
        .map_err(|_| Fail::Config("live_host_unqualified"))
}

pub(super) fn finish(mut child: std::process::Child, end: Instant) -> Res<()> {
    #[cfg(test)]
    let started = Instant::now();
    let left = end.saturating_duration_since(Instant::now());
    let execution_end = end - (left / 2).min(Duration::from_millis(50));
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| Fail::Config("live_host_unqualified"))?
        {
            #[cfg(test)]
            eprintln!(
                "network_helper elapsed_ms={} succeeded={}",
                started.elapsed().as_millis(),
                status.success()
            );
            return status
                .success()
                .then_some(())
                .ok_or(Fail::Config("live_host_unqualified"));
        }
        if Instant::now() >= execution_end {
            let _ = reap_nft(&mut child, end);
            return Err(Fail::Unresolved("live_host_control_unavailable"));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn reap_nft(child: &mut std::process::Child, end: Instant) -> bool {
    let _ = child.kill();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Instant::now() < end,
            Err(_) => return false,
            Ok(None) => {}
        }
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        std::thread::sleep(left.min(Duration::from_millis(2)));
    }
}

pub(super) fn finish_output(mut child: std::process::Child, end: Instant) -> Res<String> {
    let result = collect_nft_output(&mut child, end);
    if result.is_err() && !reap_nft(&mut child, end) {
        return Err(Fail::Unresolved("live_host_control_unavailable"));
    }
    result
}

pub(super) fn nft_stdout(child: &mut std::process::Child) -> Res<std::process::ChildStdout> {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
    let stdout = child
        .stdout
        .take()
        .ok_or(Fail::Config("live_host_unqualified"))?;
    let flags = fcntl_getfl(&stdout).map_err(|_| Fail::Config("live_host_unqualified"))?;
    fcntl_setfl(&stdout, flags | OFlags::NONBLOCK)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(stdout)
}

pub(super) fn admit_nft_output(bytes: &mut Vec<u8>, chunk: &[u8]) -> Res<()> {
    if bytes.len() + chunk.len() > 65536 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}

pub(super) fn read_nft_chunk(
    source: &mut std::process::ChildStdout,
    chunk: &mut [u8],
) -> Res<Option<usize>> {
    use std::io::Read;
    match source.read(chunk) {
        Ok(count) => Ok(Some(count)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(_) => Err(Fail::Config("live_host_unqualified")),
    }
}

pub(super) fn drain_nft_output(
    stdout: &mut std::process::ChildStdout,
    bytes: &mut Vec<u8>,
) -> Res<bool> {
    let mut chunk = [0u8; 4096];
    while let Some(count) = read_nft_chunk(stdout, &mut chunk)? {
        if count == 0 {
            return Ok(true);
        }
        admit_nft_output(bytes, &chunk[..count])?;
    }
    Ok(false)
}

pub(super) fn nft_completed(child: &mut std::process::Child, eof: bool) -> Res<bool> {
    match child.try_wait() {
        Ok(Some(status)) if !status.success() => Err(Fail::Config("live_host_unqualified")),
        Ok(Some(_)) => Ok(eof),
        Ok(None) => Ok(false),
        Err(_) => Err(Fail::Config("live_host_unqualified")),
    }
}

pub(super) fn collect_nft_output(child: &mut std::process::Child, end: Instant) -> Res<String> {
    let mut stdout = nft_stdout(child)?;
    let left = end.saturating_duration_since(Instant::now());
    let execution_end = end - (left / 2).min(Duration::from_millis(50));
    let mut bytes = Vec::with_capacity(65536);
    loop {
        if Instant::now() >= execution_end {
            return Err(Fail::Unresolved("live_host_control_unavailable"));
        }
        let eof = drain_nft_output(&mut stdout, &mut bytes)?;
        if nft_completed(child, eof)? {
            return String::from_utf8(bytes).map_err(|_| Fail::Config("live_host_unqualified"));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn run(binary: &str, args: &[&str], end: Instant) -> Res<()> {
    if Instant::now() >= end {
        return Err(Fail::Unresolved("live_host_control_unavailable"));
    }
    finish(
        {
            let mut command = Command::new(binary);
            command
                .args(args)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            spawn(command)?
        },
        end,
    )
}
pub(super) fn nft_command(netns: Option<&str>) -> Command {
    match netns {
        Some(ns) => {
            let mut c = Command::new(IP);
            c.args(["netns", "exec", ns, NFT, "-f", "-"]);
            c
        }
        None => {
            let mut c = Command::new(NFT);
            c.args(["-f", "-"]);
            c
        }
    }
}
pub(super) fn write_nft_rules(
    child: &mut std::process::Child,
    rules: &str,
    end: Instant,
) -> Res<()> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or(Fail::Config("live_host_unqualified"))?;
    if stdin.write_all(rules.as_bytes()).is_err() {
        let _ = reap_nft(child, end);
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(())
}
pub(super) fn spawn_nft(
    netns: Option<&str>,
    rules: &str,
    stdout_pipe: bool,
    end: Instant,
) -> Res<std::process::Child> {
    if Instant::now() >= end {
        return Err(Fail::Unresolved("live_host_control_unavailable"));
    }
    let out = if stdout_pipe {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    let mut command = nft_command(netns);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(out)
        .stderr(Stdio::null());
    let mut child = spawn(command)?;
    write_nft_rules(&mut child, rules, end)?;
    Ok(child)
}
pub(super) fn nft(netns: Option<&str>, rules: &str, end: Instant) -> Res<()> {
    finish(spawn_nft(netns, rules, false, end)?, end)
}
pub(super) fn nft_output(netns: Option<&str>, rules: &str, end: Instant) -> Res<String> {
    finish_output(spawn_nft(netns, rules, true, end)?, end)
}

#[cfg(test)]
type Spawn = Box<dyn FnMut(&Command) -> std::io::Result<std::process::Child>>;
#[cfg(test)]
thread_local! {
    static TEST_SPAWN: std::cell::RefCell<Option<Spawn>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) struct SpawnScope(Option<Spawn>);
#[cfg(test)]
impl Drop for SpawnScope {
    fn drop(&mut self) {
        TEST_SPAWN.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
#[cfg(test)]
pub(super) fn scoped_spawn(
    f: impl FnMut(&Command) -> std::io::Result<std::process::Child> + 'static,
) -> SpawnScope {
    SpawnScope(TEST_SPAWN.with(|slot| slot.replace(Some(Box::new(f)))))
}

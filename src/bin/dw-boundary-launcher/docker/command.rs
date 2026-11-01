//! Deadline-bound Docker subprocesses. No raw stderr or caller-selected executable.
use duskweave::{Fail, Res};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub(super) fn remaining(deadline: Instant) -> Res<Duration> {
    remaining_at(deadline, Instant::now())
}
fn remaining_at(deadline: Instant, now: Instant) -> Res<Duration> {
    deadline
        .checked_duration_since(now)
        .filter(|left| !left.is_zero())
        .ok_or(Fail::Unresolved("live_host_metadata_unavailable"))
}
pub(super) fn create(
    name: &str,
    image: &str,
    attempt: uuid::Uuid,
    exporter: bool,
    deadline: Instant,
) -> Res<String> {
    let (binary, uid) = if exporter {
        ("/dw-dns-exporter", "65533:65533")
    } else {
        ("/dw-dns-capture", "65532:65532")
    };
    let attempt_label = format!("org.duskweave.attempt={attempt}");
    let mut args = vec![
        "create",
        "--name",
        name,
        "--label",
        "org.duskweave.owner=live-dns-20261030",
        "--label",
        &attempt_label,
        "--network",
        "none",
        "--ipc",
        "none",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--user",
        uid,
        "--pids-limit",
        "32",
        "--memory",
        "128m",
        "--memory-swap",
        "128m",
        "--cpus",
        "0.5",
        "--ulimit",
        "nofile=64:64",
        "--ulimit",
        "core=0:0",
        "--log-driver",
        "none",
        "--interactive",
        "--entrypoint",
        binary,
        image,
    ];
    if exporter {
        args.push("--holder");
    }
    metadata_until(&args, deadline)
}

pub(super) fn metadata(args: &[&str]) -> Res<String> {
    metadata_until(args, Instant::now() + Duration::from_millis(1000))
}
pub(super) fn metadata_until(args: &[&str], deadline: Instant) -> Res<String> {
    remaining(deadline)?;
    #[cfg(test)]
    if let Some(result) =
        TEST_RUNNER.with(|hook| hook.borrow_mut().as_mut().map(|run| run(args, deadline)))
    {
        return result;
    }
    let mut command = Command::new("/usr/bin/docker");
    command.args(args);
    #[cfg(test)]
    let started = Instant::now();
    let result = output(command, deadline);
    #[cfg(test)]
    eprintln!(
        "launcher_helper phase={} elapsed_ms={} succeeded={}",
        args[0],
        started.elapsed().as_millis(),
        result.is_ok()
    );
    result
}
pub(super) fn output(mut command: Command, deadline: Instant) -> Res<String> {
    remaining(deadline)?;
    let mut child = command
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    collect(&mut child, deadline)
}
fn collect(child: &mut Child, deadline: Instant) -> Res<String> {
    // Keep termination inside the caller's budget, including STOP's shared bound.
    let left = deadline.saturating_duration_since(Instant::now());
    let execution_end = deadline - (left / 2).min(Duration::from_millis(50));
    let result = read_output(child, execution_end);
    if result.is_err() && !reap(child, deadline) {
        return Err(Fail::Unresolved("live_host_metadata_unavailable"));
    }
    result
}
pub(super) fn reap(child: &mut Child, deadline: Instant) -> bool {
    let _ = child.kill();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Instant::now() < deadline,
            Err(_) => return false,
            Ok(None) => {}
        }
        let Ok(left) = remaining(deadline) else {
            return false;
        };
        std::thread::sleep(left.min(Duration::from_millis(2)));
    }
}
fn nonblocking_stdout(child: &mut Child) -> Res<std::process::ChildStdout> {
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
fn read_output(child: &mut Child, deadline: Instant) -> Res<String> {
    let mut stdout = nonblocking_stdout(child)?;
    let mut bytes = Vec::with_capacity(4097);
    loop {
        remaining(deadline)?;
        let eof = drain(&mut stdout, &mut bytes)?;
        if completed(child, eof, deadline)? {
            return String::from_utf8(bytes).map_err(|_| Fail::Config("live_host_unqualified"));
        }
    }
}
fn completed(child: &mut Child, eof: bool, deadline: Instant) -> Res<bool> {
    let status = child
        .try_wait()
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if let Some(status) = status {
        if !status.success() {
            return Err(Fail::Config("live_host_unqualified"));
        }
        if eof {
            return Ok(true);
        }
    }
    std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(2)));
    Ok(false)
}
fn read_chunk(source: &mut impl Read, chunk: &mut [u8]) -> Res<Option<usize>> {
    match source.read(chunk) {
        Ok(count) => Ok(Some(count)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(_) => Err(Fail::Config("live_host_unqualified")),
    }
}
fn admit_chunk(bytes: &mut Vec<u8>, chunk: &[u8]) -> Res<()> {
    if bytes.len() + chunk.len() > 4096 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}
fn drain(source: &mut impl Read, bytes: &mut Vec<u8>) -> Res<bool> {
    let mut chunk = [0; 1024];
    while let Some(count) = read_chunk(source, &mut chunk)? {
        if count == 0 {
            return Ok(true);
        }
        admit_chunk(bytes, &chunk[..count])?;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_uses_remaining_original_time_without_the_old_sub_limit() {
        let origin = Instant::now();
        let end = origin + Duration::from_millis(5000);
        assert_eq!(
            remaining_at(end, origin + Duration::from_millis(1500)),
            Ok(Duration::from_millis(3500))
        );
        assert!(remaining_at(end, origin + Duration::from_millis(5000)).is_err());
        inject(move |args, deadline| {
            assert_eq!(args[0], "create");
            assert_eq!(deadline, end);
            assert!(remaining_at(deadline, origin + Duration::from_millis(1500)).is_ok());
            Ok("created".into())
        });
        assert_eq!(
            create("exact", "image", uuid::Uuid::from_u128(1), false, end),
            Ok("created".into())
        );
    }

    #[test]
    fn expired_shared_deadline_cannot_submit_another_helper() {
        inject(|_, _| panic!("expired preparation or teardown must not spawn a helper"));
        assert!(metadata_until(&["create"], Instant::now() - Duration::from_millis(1)).is_err());
    }

    #[test]
    fn completed_daemon_create_with_lost_client_reply_is_removed_but_not_refunded() {
        use std::{cell::Cell, rc::Rc};
        let present = Rc::new(Cell::new(false));
        let removed = Rc::new(Cell::new(false));
        let running = present.clone();
        let killed = removed.clone();
        inject(move |args, _| match args[0] {
            "ps" => Ok(if running.get() { "present" } else { "" }.into()),
            "create" => {
                running.set(true);
                Err(Fail::Unresolved("live_host_metadata_unavailable"))
            }
            "inspect" => Ok(format!(
                "{} sha256:expected live-dns-20261030 {}",
                "a".repeat(64),
                uuid::Uuid::from_u128(2)
            )),
            "rm" => {
                assert_eq!(args[2], "a".repeat(64));
                running.set(false);
                killed.set(true);
                Ok(String::new())
            }
            _ => unreachable!(),
        });
        let result = super::super::create(
            "sha256:expected",
            uuid::Uuid::from_u128(2),
            false,
            Instant::now() + Duration::from_millis(5000),
        );
        assert!(matches!(
            result,
            Err(Fail::Unresolved("live_disposal_unconfirmed"))
        ));
        assert!(removed.get());
        assert!(!present.get());
        let record = super::super::record_path(uuid::Uuid::from_u128(2), false);
        assert!(
            record.exists(),
            "a timed-out daemon request must retain its uncertainty even after scoped removal"
        );
        std::fs::remove_file(record).unwrap();
    }

    #[test]
    fn expired_post_stop_budget_is_shared_by_both_workers() {
        inject(|_, _| panic!("teardown cannot refresh an exhausted shared deadline"));
        let end = Instant::now() - Duration::from_millis(1);
        for exporter in [false, true] {
            assert!(super::super::remove(uuid::Uuid::from_u128(3), exporter, end).is_err());
        }
    }

    #[test]
    fn over_limit_output_is_rejected_before_a_full_pipe_stalls_exit() {
        let mut child = Command::new("/usr/bin/head")
            .args(["-c", "131072", "/dev/zero"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let result = collect(&mut child, Instant::now() + Duration::from_millis(100));
        assert!(child.try_wait().unwrap().is_some(), "helper must be reaped");
        assert_eq!(
            result,
            Err(Fail::Config("live_host_unqualified")),
            "a full output pipe must produce a bounded-output failure, not deadlock until the helper timeout"
        );
    }

    #[test]
    fn stalled_output_times_out_and_reaps_the_helper() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "printf x; exec /bin/sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        assert_eq!(
            collect(&mut child, Instant::now() + Duration::from_millis(100)),
            Err(Fail::Unresolved("live_host_metadata_unavailable"))
        );
        assert!(
            child.try_wait().unwrap().is_some(),
            "stalled helper must be reaped"
        );
    }
}

#[cfg(test)]
type Runner = dyn FnMut(&[&str], Instant) -> Res<String>;
#[cfg(test)]
thread_local! {
    static TEST_RUNNER: std::cell::RefCell<Option<Box<Runner>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) fn inject(run: impl FnMut(&[&str], Instant) -> Res<String> + 'static) {
    TEST_RUNNER.with(|hook| *hook.borrow_mut() = Some(Box::new(run)));
}

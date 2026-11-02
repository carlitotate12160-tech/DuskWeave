//! Attempt-local veth and kernel timeout lease; preparation never grants egress.
use duskweave::{Fail, Res};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;
const IP: &str = "/usr/sbin/ip";
const NFT: &str = "/usr/sbin/nft";
mod namespace;
const ROOT: &str = "/run/duskweave/launcher";

struct Names {
    netns: String,
    host: String,
    inner: String,
    table: String,
    base: u8,
}
fn names(attempt: Uuid) -> Names {
    let tag = attempt.simple().to_string()[..12].to_string();
    Names {
        netns: format!("dw{tag}"),
        host: format!("dvh{tag}"),
        inner: format!("dvn{tag}"),
        table: format!("dwf{tag}"),
        base: attempt.as_bytes()[6] & 0xfc,
    }
}
fn slot(n: &Names) -> String {
    format!("{ROOT}/network-{}", n.base)
}
fn addresses(n: &Names) -> (String, String, String) {
    (
        format!("10.203.0.{}", n.base + 1),
        format!("10.203.0.{}", n.base + 2),
        format!("10.203.0.{}/30", n.base),
    )
}

fn finish(mut child: std::process::Child, end: Instant) -> Res<()> {
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
            let _ = super::docker::reap_child(&mut child, end);
            return Err(Fail::Unresolved("live_host_control_unavailable"));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn read_child_output(mut stdout: std::process::ChildStdout, success: bool) -> Res<String> {
    if !success {
        return Err(Fail::Config("live_host_unqualified"));
    }
    use std::io::Read;
    let mut out = String::new();
    stdout
        .take(65536)
        .read_to_string(&mut out)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(out)
}
fn finish_output(mut child: std::process::Child, end: Instant) -> Res<String> {
    let stdout = child
        .stdout
        .take()
        .ok_or(Fail::Config("live_host_unqualified"))?;
    let left = end.saturating_duration_since(Instant::now());
    let execution_end = end - (left / 2).min(Duration::from_millis(50));
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| Fail::Config("live_host_unqualified"))?
        {
            return read_child_output(stdout, status.success());
        }
        if Instant::now() >= execution_end {
            let _ = super::docker::reap_child(&mut child, end);
            return Err(Fail::Unresolved("live_host_control_unavailable"));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn run(binary: &str, args: &[&str], end: Instant) -> Res<()> {
    if Instant::now() >= end {
        return Err(Fail::Unresolved("live_host_control_unavailable"));
    }
    finish(
        Command::new(binary)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Fail::Config("live_host_unqualified"))?,
        end,
    )
}
fn spawn_nft(
    netns: Option<&str>,
    rules: &str,
    stdout_pipe: bool,
    end: Instant,
) -> Res<std::process::Child> {
    if Instant::now() >= end {
        return Err(Fail::Unresolved("live_host_control_unavailable"));
    }
    let mut command = match netns {
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
    };
    let out = if stdout_pipe {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    let mut child = command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(out)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let written = child
        .stdin
        .take()
        .ok_or(Fail::Config("live_host_unqualified"))?
        .write_all(rules.as_bytes());
    if written.is_err() {
        let _ = super::docker::reap_child(&mut child, end);
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(child)
}
fn nft(netns: Option<&str>, rules: &str, end: Instant) -> Res<()> {
    finish(spawn_nft(netns, rules, false, end)?, end)
}
fn nft_output(netns: Option<&str>, rules: &str, end: Instant) -> Res<String> {
    finish_output(spawn_nft(netns, rules, true, end)?, end)
}
fn acquire_slot(n: &Names, attempt: Uuid) -> Res<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(slot(n))
        .map_err(|_| Fail::Config("live_network_collision"))?;
    if file.write_all(attempt.to_string().as_bytes()).is_err() {
        let _ = std::fs::remove_file(slot(n));
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(())
}
fn qualify_capture(pid: u32, resolver: &str) -> Res<()> {
    resolver
        .parse::<std::net::Ipv4Addr>()
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if pid == 0
        || std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward")
            .map_err(|_| Fail::Config("live_host_unqualified"))?
            .trim()
            != "1"
    {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(())
}
pub(super) fn create(attempt: Uuid, pid: u32, resolver: &str, end: Instant) -> Res<()> {
    qualify_capture(pid, resolver)?;
    let n = names(attempt);
    if std::path::Path::new(&netns_path(attempt)).exists()
        || std::path::Path::new(&format!("/sys/class/net/{}", n.host)).exists()
    {
        return Err(Fail::Config("live_network_collision"));
    }
    acquire_slot(&n, attempt)?;
    let outcome = namespace::prepare(&n, pid, resolver, end);
    if outcome.is_err() {
        let _ = destroy(attempt, Instant::now() + Duration::from_millis(1000));
    }
    outcome
}
pub(super) fn grant(attempt: Uuid, deadline: i64) -> Res<()> {
    let n = names(attempt);
    if std::fs::read_to_string(slot(&n)).ok().as_deref() != Some(&attempt.to_string()) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let now = super::qualification::now_ms()?;
    let lease = (deadline - now).min(2000);
    if lease <= 0 {
        return Err(Fail::State("live_effect_deadline"));
    }
    let (_, inner, _) = addresses(&n);
    nft(
        None,
        &format!(
            "flush set inet {} live\nadd element inet {} live {{ {inner} timeout {lease}ms }}\n",
            n.table, n.table
        ),
        Instant::now() + Duration::from_millis(200),
    )
}
pub(super) fn revoke(attempt: Uuid) -> Res<()> {
    revoke_until(attempt, Instant::now() + Duration::from_millis(200))
}
pub(super) fn revoke_until(attempt: Uuid, end: Instant) -> Res<()> {
    nft(
        None,
        &format!("flush set inet {} live\n", names(attempt).table),
        end,
    )
}
pub(super) fn netns_path(attempt: Uuid) -> String {
    format!("/run/netns/{}", names(attempt).netns)
}
pub(super) fn destroy(attempt: Uuid, end: Instant) -> bool {
    let n = names(attempt);
    if !std::path::Path::new(&slot(&n)).exists() {
        return namespace::absent(&n);
    }
    if std::fs::read_to_string(slot(&n)).ok().as_deref() != Some(&attempt.to_string()) {
        return false;
    }
    let _ = revoke_until(
        attempt,
        end.min(Instant::now() + Duration::from_millis(200)),
    );
    let disposed = namespace::destroy(&n, end);
    if disposed {
        return std::fs::remove_file(slot(&n)).is_ok();
    }
    false
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/launcher_network.rs"]
mod tests;

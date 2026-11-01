//! Container mount policy, fixed remount commands and read-only verification.
use duskweave::{Fail, Res};
use std::process::Command;
use std::time::Instant;
/// Docker's generated system mounts are made read-only inside this container.
/// No new bind mount or writable volume is created. Unknown writable mounts refuse.
pub(super) fn seal_mounts(pid: u32, deadline: Instant) -> Res<()> {
    let source = std::fs::read_to_string(format!("/proc/{pid}/mountinfo"))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let paths = source.lines().map(writable_path).collect::<Res<Vec<_>>>()?;
    for path in paths.into_iter().flatten() {
        let mut command = Command::new("/usr/bin/nsenter");
        command.args([
            "--target",
            &pid.to_string(),
            "--mount",
            "--root=/",
            "--wd=/",
            "--",
            "/usr/bin/mount",
            "--no-canonicalize",
            "-o",
            "remount,bind,ro",
            &format!("/proc/{pid}/root{path}"),
        ]);
        super::command::output(command, deadline)?;
    }
    let after = std::fs::read_to_string(format!("/proc/{pid}/mountinfo"))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if after.lines().any(|line| {
        line.split_whitespace()
            .nth(5)
            .is_none_or(|v| v.split(',').any(|f| f == "rw"))
    }) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(())
}

fn writable_path(line: &str) -> Res<Option<&str>> {
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() < 6 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    if !fields[5].split(',').any(|value| value == "rw") {
        return Ok(None);
    }
    if !allowed_system_mount(fields[4]) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(Some(fields[4]))
}
fn allowed_system_mount(path: &str) -> bool {
    [
        "/proc",
        "/dev",
        "/dev/pts",
        "/dev/mqueue",
        "/proc/interrupts",
        "/proc/kcore",
        "/proc/keys",
        "/proc/latency_stats",
        "/proc/timer_list",
    ]
    .contains(&path)
}
#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/docker_mounts.rs"]
mod tests;

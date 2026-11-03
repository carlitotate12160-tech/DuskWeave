//! Fixed container preparation and raw-facing Docker attach framing.
use duskweave::{Fail, Res};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod attach;
mod command;
mod connection;
mod mounts;
pub(super) use attach::Attach;
pub(super) fn metadata(args: &[&str]) -> Res<String> {
    command::metadata(args)
}
use uuid::Uuid;
#[cfg(test)]
pub(super) fn reap_child(child: &mut std::process::Child, deadline: Instant) -> bool {
    command::reap(child, deadline)
}

pub(super) struct Container {
    pub id: String,
    pub pid: u32,
}

pub(super) fn names(attempt: Uuid) -> [String; 2] {
    let tag = attempt.simple();
    [format!("dwcap{tag}"), format!("dwexp{tag}")]
}

pub(super) fn qualify(image: &str) -> Res<()> {
    let actual = metadata(&["image", "inspect", "--format", "{{.Id}}", image])?;
    let debug = metadata(&["info", "--format", "{{.Debug}}"])?;
    [actual.trim() == image, debug.trim() == "false"]
        .into_iter()
        .all(|v| v)
        .then_some(())
        .ok_or(Fail::Config("live_host_unqualified"))
}

pub(super) fn create(
    image: &str,
    attempt: Uuid,
    exporter: bool,
    deadline: Instant,
) -> Res<Container> {
    let name = names(attempt)[usize::from(exporter)].clone();
    if present(&name, deadline)? {
        return Err(Fail::Config("live_container_collision"));
    }
    // Install teardown responsibility before the daemon can receive create.
    let record = record_path(attempt, exporter);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&record)
        .map_err(|_| Fail::Config("live_container_collision"))?;
    file.write_all(image.as_bytes())
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let outcome = (|| {
        let id = command::create(&name, image, attempt, exporter, deadline)?
            .trim()
            .to_string();
        std::fs::write(record.with_extension("complete"), [])
            .map_err(|_| Fail::Config("live_host_unqualified"))?;
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Fail::Config("live_host_unqualified"));
        }
        start_created(&name, id, deadline)
    })();
    if outcome.is_err()
        && remove(
            attempt,
            exporter,
            Instant::now() + Duration::from_millis(1000),
        )
        .is_err()
    {
        return Err(Fail::Unresolved("live_disposal_unconfirmed"));
    }
    outcome
}

fn start_created(name: &str, id: String, deadline: Instant) -> Res<Container> {
    command::metadata_until(&["start", name], deadline)?;
    let pid = command::metadata_until(&["inspect", "--format", "{{.State.Pid}}", name], deadline)?
        .trim()
        .parse::<u32>()
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if pid == 0 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    mounts::seal_mounts(pid, deadline)?;
    Ok(Container { id, pid })
}

fn present(name: &str, deadline: Instant) -> Res<bool> {
    Ok(!command::metadata_until(
        &[
            "ps",
            "--all",
            "--filter",
            &format!("name=^/{name}$"),
            "--format",
            "{{.ID}}",
        ],
        deadline,
    )?
    .trim()
    .is_empty())
}
fn record_path(attempt: Uuid, exporter: bool) -> PathBuf {
    #[cfg(not(test))]
    let root = PathBuf::from("/run/duskweave/launcher");
    #[cfg(test)]
    let root = tests::root();
    root.join(format!("{}.pending", names(attempt)[usize::from(exporter)]))
}
pub(super) fn remove(attempt: Uuid, exporter: bool, deadline: Instant) -> Res<()> {
    let name = names(attempt)[usize::from(exporter)].clone();
    let record = record_path(attempt, exporter);
    let completed = record.with_extension("complete").exists();
    if present(&name, deadline)? {
        let image = std::fs::read_to_string(&record)
            .map_err(|_| Fail::Config("live_container_collision"))?;
        remove_owned(&name, &image, attempt, deadline)?;
    }
    settle_record(&record, completed)
}
fn settle_record(record: &std::path::Path, completed: bool) -> Res<()> {
    // An absent lookup cannot settle an outstanding daemon-side create.
    if record.exists() && !completed {
        return Err(Fail::Unresolved("live_disposal_unconfirmed"));
    }
    if record.exists() {
        std::fs::remove_file(record).map_err(|_| Fail::Unresolved("live_disposal_unconfirmed"))?;
        std::fs::remove_file(record.with_extension("complete"))
            .map_err(|_| Fail::Unresolved("live_disposal_unconfirmed"))?;
    }
    Ok(())
}
fn remove_owned(name: &str, image: &str, attempt: Uuid, deadline: Instant) -> Res<()> {
    let identity = command::metadata_until(
        &[
            "inspect",
            "--format",
            "{{.Id}} {{.Image}} {{index .Config.Labels \"org.duskweave.owner\"}} {{index .Config.Labels \"org.duskweave.attempt\"}}",
            name,
        ],
        deadline,
    )?;
    let fields: Vec<_> = identity.split_whitespace().collect();
    if fields.len() != 4 || fields[1..] != [image, "live-dns-20261030", &attempt.to_string()] {
        return Err(Fail::Config("live_container_collision"));
    }
    command::metadata_until(&["rm", "--force", fields[0]], deadline)?;
    if present(name, deadline)? {
        return Err(Fail::Unresolved("live_disposal_unconfirmed"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/docker.rs"]
mod tests;

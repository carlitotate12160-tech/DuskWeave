//! Raw-facing TCB: Docker capture stdout reaches only the private exporter pipe.
use super::attempt::Session;
use super::docker::{Attach, Container, TimedSocket};
use duskweave::m1_dns::{
    Control, DnsRequest, DnsResult, FrameReader, FrameWriter, MAX_CONTROL, MAX_FRAME,
};
use duskweave::{Fail, Res};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn remaining(end: Instant) -> Res<Duration> {
    end.checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(Fail::State("live_effect_deadline"))
}
pub(super) struct RawPipe {
    read: UnixStream,
    write: UnixStream,
}
pub(super) fn private_raw() -> Res<RawPipe> {
    // Admission calls this before opening any other descriptor. Only the exporter
    // exec receives descriptor 3; CLOEXEC stays set for every metadata helper.
    let (read, write) = UnixStream::pair().map_err(|_| Fail::Config("live_host_unqualified"))?;
    if read.as_raw_fd() != 3 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    write
        .set_write_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(RawPipe { read, write })
}
fn socket_pair() -> Res<(UnixStream, UnixStream)> {
    UnixStream::pair().map_err(|_| Fail::Config("live_host_unqualified"))
}

fn config(dir: &Path, binary: &str) -> Res<()> {
    let uid = match binary {
        "/dw-dns-capture" => 65532,
        "/dw-dns-exporter" => 65533,
        _ => return Err(Fail::Config("live_host_unqualified")),
    };
    let process = serde_json::json!({"terminal":false,"user":{"uid":uid,"gid":uid},
        "args":[binary],"env":[],"cwd":"/",
        "capabilities":{"bounding":[],"effective":[],"inheritable":[],"permitted":[],"ambient":[]},
        "rlimits":[{"type":"RLIMIT_CORE","hard":0,"soft":0},
            {"type":"RLIMIT_NOFILE","hard":64,"soft":64},
            {"type":"RLIMIT_NPROC","hard":32,"soft":32}],"noNewPrivileges":true});
    let bytes = serde_json::to_vec(&process).map_err(|_| Fail::Config("live_host_unqualified"))?;
    std::fs::write(dir.join("process.json"), bytes)
        .map_err(|_| Fail::Config("live_host_unqualified"))
}

fn exporter(
    container: &Container,
    dir: &Path,
    input: UnixStream,
    output: UnixStream,
    raw: &RawPipe,
    end: Instant,
) -> Res<Child> {
    use rustix::io::{FdFlags, fcntl_setfd};
    fcntl_setfd(&raw.read, FdFlags::empty()).map_err(|_| Fail::Config("live_host_unqualified"))?;
    // The container was created by Docker with default seccomp, independent
    // namespaces and fixed cgroup limits. Exec retains that container policy.
    let outcome = Command::new("/usr/bin/runc")
        .args([
            "--root",
            "/run/docker/runtime-runc/moby",
            "exec",
            "--preserve-fds",
            "1",
            "--process",
        ])
        .arg(dir.join("process.json"))
        .arg(&container.id)
        .env_clear()
        .stdin(Stdio::from(OwnedFd::from(input)))
        .stdout(Stdio::from(OwnedFd::from(output)))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Fail::Config("live_host_unqualified"));
    let reset =
        fcntl_setfd(&raw.read, FdFlags::CLOEXEC).map_err(|_| Fail::Config("live_host_unqualified"));
    restored_child(outcome, reset, end)
}

fn restored_child(outcome: Res<Child>, restored: Res<()>, end: Instant) -> Res<Child> {
    let mut child = outcome?;
    if let Err(error) = restored {
        return if super::docker::reap_child(&mut child, end) {
            Err(error)
        } else {
            Err(Fail::Unresolved("live_disposal_unconfirmed"))
        };
    }
    Ok(child)
}

fn prepare_resources(
    image: &str,
    q: &DnsRequest,
    end: Instant,
) -> Res<(Attach, Container, PathBuf)> {
    let capture = super::docker::create(image, q.attempt, false, end)?;
    let exporter = super::docker::create(image, q.attempt, true, end)?;
    super::network::create(q.attempt, capture.pid, &q.resolver_ipv4, end)?;
    let dir = Path::new("/run/duskweave/launcher").join(q.attempt.simple().to_string());
    config(&dir, "/dw-dns-exporter")?;
    // Attach completes before any raw-transporter child is spawned.
    Ok((Attach::connect(&capture.id, end)?, exporter, dir))
}
fn exporter_channels(
    container: &Container,
    dir: &Path,
    raw: &RawPipe,
    attempt: uuid::Uuid,
    end: Instant,
) -> Res<(TimedSocket, TimedSocket, Child)> {
    let (input, child_in) = socket_pair()?;
    let (output, child_out) = socket_pair()?;
    super::attempt::transport_started(attempt)?;
    let process = match exporter(container, dir, child_in, child_out, raw, end) {
        Ok(process) => process,
        Err(error) => {
            // An unconfirmed reap keeps the marker honest; a spawn that never
            // produced a child, or a restore failure after a confirmed reap,
            // proves none survives, so the marker is stale and cleared.
            if !matches!(&error, Fail::Unresolved(u) if *u == "live_disposal_unconfirmed") {
                let _ = super::attempt::transport_cleared(attempt);
            }
            return Err(error);
        }
    };
    Ok((
        TimedSocket { io: input, end },
        TimedSocket { io: output, end },
        process,
    ))
}
fn ready_control(source: &mut impl Read, q: &DnsRequest) -> Res<()> {
    FrameReader::new(source).control()?.validate(q, "READY", 1)
}

pub(super) struct Pair {
    request: DnsRequest,
    capture: Attach,
    exporter_in: TimedSocket,
    exporter_out: TimedSocket,
    process: Child,
    raw: RawPipe,
    disposed: bool,
    #[cfg(test)]
    reap_confirmation_unknown: bool,
}
impl Pair {
    pub fn prepare(image: &str, session: &Session, raw: RawPipe) -> Res<Self> {
        let end = session
            .preparation_deadline
            .ok_or(Fail::Config("live_host_unqualified"))?;
        let q = &session.request;
        let (capture, exporter_container, dir) = prepare_resources(image, q, end)?;
        let (exporter_in, exporter_out, process) =
            exporter_channels(&exporter_container, &dir, &raw, q.attempt, end)?;
        let mut pair = Self {
            request: (**q).clone(),
            capture,
            exporter_in,
            exporter_out,
            process,
            raw,
            disposed: false,
            #[cfg(test)]
            reap_confirmation_unknown: false,
        };
        pair.ready(end)?;
        Ok(pair)
    }
    fn ready(&mut self, end: Instant) -> Res<()> {
        FrameWriter::new(&mut self.capture).write(&self.request, MAX_FRAME)?;
        FrameWriter::new(&mut self.exporter_in).write(&self.request, MAX_FRAME)?;
        ready_control(&mut self.capture, &self.request)?;
        ready_control(&mut self.exporter_out, &self.request)?;
        remaining(end)?;
        Ok(())
    }
    fn set_attempt_deadline(&mut self, deadline: i64) -> Res<()> {
        let left = deadline - super::qualification::now_ms()?;
        if left <= 0 {
            return Err(Fail::State("live_effect_deadline"));
        }
        self.capture.deadline(left as u64)?;
        self.exporter_in.end = Instant::now() + Duration::from_millis(left as u64);
        Ok(())
    }
    pub fn start(&mut self, deadline: i64) -> Res<()> {
        self.set_attempt_deadline(deadline)?;
        self.capture.nonblocking(true)?;
        self.exporter_in
            .io
            .set_nonblocking(true)
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        let mut start = Control::new(&self.request, "START", 2);
        start.deadline_at_ms = deadline;
        let outcome = FrameWriter::new(&mut self.exporter_in)
            .write(&start, MAX_CONTROL)
            .and_then(|_| FrameWriter::new(&mut self.capture).write(&start, MAX_CONTROL));
        let capture_reset = self.capture.nonblocking(false);
        let exporter_reset = self
            .exporter_in
            .io
            .set_nonblocking(false)
            .map_err(|_| Fail::State("dns_ipc_failure"));
        capture_reset?;
        exporter_reset?;
        outcome
    }
    pub fn result(&mut self, deadline: i64) -> Res<DnsResult> {
        let remaining = deadline.saturating_add(1000) - super::qualification::now_ms()?;
        if remaining <= 0 {
            return Err(Fail::State("live_effect_deadline"));
        }
        self.capture.deadline(remaining as u64)?;
        self.exporter_out.end = Instant::now() + Duration::from_millis(remaining as u64);
        forward_raw(&mut self.capture, &mut self.raw.write)?;
        self.raw
            .write
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        FrameReader::new(&mut self.exporter_out).result()
    }
    pub fn acknowledge(&mut self) -> Res<()> {
        self.exporter_in.end = self.exporter_out.end;
        let ack = Control::new(&self.request, "ACK", 4);
        FrameWriter::new(&mut self.exporter_in).write(&ack, MAX_CONTROL)
    }
}
impl Pair {
    pub fn dispose(&mut self) -> bool {
        let end = Instant::now() + Duration::from_millis(1000);
        let q = &self.request;
        // STOP/egress removal precede every potentially delayed child reap.
        let _ =
            super::attempt::stop_until(q.engagement_id, q.campaign_id, q.session_generation, end);
        // The exact known attempt remains accountable even if its journal is lost.
        let _ = super::attempt::dispose_until(q.attempt, end);
        let reaped = super::docker::reap_child(&mut self.process, end);
        #[cfg(test)]
        let reaped = reaped && !self.reap_confirmation_unknown;
        self.disposed = true;
        reaped
            && super::attempt::transport_reaped(q.attempt)
            && super::attempt::stopped(q)
            && super::attempt::dispose_until(q.attempt, end)
    }
}
impl Drop for Pair {
    fn drop(&mut self) {
        if !self.disposed {
            let _ = self.dispose();
        }
    }
}

/// Transport framing only: the exporter alone interprets the capture payload.
/// Extra stdout or malformed/truncated attach frames cannot become safe output.
fn raw_prefix(source: &mut impl Read) -> Res<([u8; 4], usize)> {
    let mut prefix = [0; 4];
    source
        .read_exact(&mut prefix)
        .map_err(|_| Fail::State("dns_ipc_failure"))?;
    let count = u32::from_be_bytes(prefix) as usize;
    if !(1..=MAX_FRAME).contains(&count) {
        return Err(Fail::State("dns_ipc_failure"));
    }
    Ok((prefix, count))
}
fn copy_raw(source: &mut impl Read, sink: &mut impl Write, mut remaining: usize) -> Res<()> {
    let mut chunk = [0; 1024];
    while remaining > 0 {
        let count = remaining.min(chunk.len());
        source
            .read_exact(&mut chunk[..count])
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        sink.write_all(&chunk[..count])
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        remaining -= count;
    }
    Ok(())
}
fn raw_end(source: &mut impl Read) -> Res<()> {
    let mut extra = [0; 1];
    if source
        .read(&mut extra)
        .map_err(|_| Fail::State("dns_ipc_failure"))?
        != 0
    {
        return Err(Fail::State("dns_ipc_failure"));
    }
    Ok(())
}
fn forward_raw(source: &mut impl Read, sink: &mut impl Write) -> Res<()> {
    let (prefix, count) = raw_prefix(source)?;
    sink.write_all(&prefix)
        .map_err(|_| Fail::State("dns_ipc_failure"))?;
    copy_raw(source, sink, count)?;
    raw_end(source)
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/stop.rs"]
mod tests;

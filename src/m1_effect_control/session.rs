//! Acquisition-scoped private stop ingress. No SQL or worker data enters this owner.
use super::authority::{StopDisposition, StopTarget};
use super::{CONTROL_ROOT, HostChannel};
use crate::m1_dns::DnsRequest;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Binding {
    run: Uuid,
    engagement: Uuid,
    campaign: Uuid,
    generation: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StopRequest {
    binding: Binding,
    target: StopTarget,
}
impl Binding {
    fn admits(&self, target: StopTarget) -> bool {
        match target {
            StopTarget::Run { run_id } => self.run == run_id,
            StopTarget::Campaign {
                engagement_id,
                campaign_id,
            } => (self.engagement, self.campaign) == (engagement_id, campaign_id),
        }
    }
    fn path(&self, extension: &str) -> PathBuf {
        Path::new(CONTROL_ROOT).join(format!("{}.{}", self.run.simple(), extension))
    }
}
fn root() -> Res<()> {
    let m = std::fs::symlink_metadata(CONTROL_ROOT).map_err(|_| unavailable())?;
    [
        m.is_dir(),
        m.uid() == rustix::process::geteuid().as_raw(),
        m.mode() & 0o777 == 0o700,
    ]
    .into_iter()
    .all(|v| v)
    .then_some(())
    .ok_or(unavailable())
}
fn unavailable() -> Fail {
    Fail::State("live_stop_ingress_unavailable")
}
fn peer(socket: &UnixStream) -> Res<()> {
    let credential = rustix::net::sockopt::socket_peercred(socket).map_err(|_| unavailable())?;
    if credential.uid != rustix::process::geteuid() {
        return Err(unavailable());
    }
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| unavailable())?;
    socket
        .set_write_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| unavailable())
}
pub(super) struct Ingress {
    done: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Ingress {
    pub(super) fn start(run: Uuid, q: &DnsRequest) -> Res<Self> {
        root()?;
        let binding = Binding {
            run,
            engagement: q.engagement_id,
            campaign: q.campaign_id,
            generation: q.session_generation,
        };
        let listener = bind(&binding)?;
        let done = Arc::new(AtomicBool::new(false));
        let running = Arc::clone(&done);
        let worker = std::thread::spawn(move || {
            serve(&listener.listener, &listener.binding, &running);
        });
        Ok(Self {
            done,
            worker: Some(worker),
        })
    }
}
struct Endpoint {
    listener: UnixListener,
    binding: Binding,
    journal_owned: bool,
}
impl Endpoint {
    fn configure(&self) -> Res<()> {
        std::fs::set_permissions(
            self.binding.path("sock"),
            std::fs::Permissions::from_mode(0o600),
        )
        .map_err(|_| unavailable())?;
        self.listener
            .set_nonblocking(true)
            .map_err(|_| unavailable())
    }
    fn publish(&mut self) -> Res<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.binding.path("json"))
            .map_err(|_| unavailable())?;
        self.journal_owned = true;
        file.write_all(&serde_json::to_vec(&self.binding).map_err(|_| unavailable())?)
            .map_err(|_| unavailable())
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.binding.path("sock"));
        if self.journal_owned {
            let _ = std::fs::remove_file(self.binding.path("json"));
        }
    }
}
fn bind(binding: &Binding) -> Res<Endpoint> {
    let listener = UnixListener::bind(binding.path("sock")).map_err(|_| unavailable())?;
    let mut endpoint = Endpoint {
        listener,
        binding: binding.clone(),
        journal_owned: false,
    };
    endpoint.configure()?;
    endpoint.publish()?;
    Ok(endpoint)
}
fn serve(listener: &UnixListener, binding: &Binding, done: &AtomicBool) {
    while !done.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((socket, _)) => {
                let _ = stop_peer(socket, binding);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(_) => return,
        }
    }
}
fn stop_peer(socket: UnixStream, binding: &Binding) -> Res<()> {
    peer(&socket)?;
    let mut channel = HostChannel::new(socket);
    let request: StopRequest = channel.receive()?;
    if request.binding != *binding || !binding.admits(request.target) {
        return Err(unavailable());
    }
    let stopped = super::socket::stop(request.target)?;
    channel.send(&stopped)
}
impl Drop for Ingress {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        let end = Instant::now() + Duration::from_millis(1300);
        if let Some(worker) = self.worker.take() {
            while !worker.is_finished() && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(2));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}
fn binding(path: &Path) -> Res<Binding> {
    use std::io::Read;
    let m = std::fs::symlink_metadata(path).map_err(|_| unavailable())?;
    if ![
        m.is_file(),
        m.uid() == rustix::process::geteuid().as_raw(),
        m.nlink() == 1,
        m.mode() & 0o777 == 0o600,
        m.len() <= 1024,
    ]
    .into_iter()
    .all(|v| v)
    {
        return Err(unavailable());
    }
    let file = std::fs::File::open(path).map_err(|_| unavailable())?;
    serde_json::from_reader(file.take(1025)).map_err(|_| unavailable())
}
pub(super) fn signal(target: StopTarget) -> Res<Option<StopDisposition>> {
    root()?;
    for entry in std::fs::read_dir(CONTROL_ROOT).map_err(|_| unavailable())? {
        let entry = entry.map_err(|_| unavailable())?;
        if let Some(outcome) = signal_entry(&entry.path(), target)? {
            return Ok(Some(outcome));
        }
    }
    Ok(None)
}
fn signal_entry(path: &Path, target: StopTarget) -> Res<Option<StopDisposition>> {
    if path.extension().is_none_or(|x| x != "json") {
        return Ok(None);
    }
    let b = binding(path)?;
    if !b.admits(target) {
        return Ok(None);
    }
    send(b, target).map(Some)
}
fn send(binding: Binding, target: StopTarget) -> Res<StopDisposition> {
    let socket = connect(&binding.path("sock"))?;
    peer(&socket)?;
    socket
        .set_read_timeout(Some(Duration::from_millis(1100)))
        .map_err(|_| unavailable())?;
    let mut channel = HostChannel::new(socket);
    channel.send(&StopRequest { binding, target })?;
    channel.receive()
}

// Private fixed-ingress acquisition; the path is always derived from a bound run.
fn connect(path: &Path) -> Res<UnixStream> {
    use rustix::net::{
        AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
    };
    let address = SocketAddrUnix::new(path).map_err(|_| unavailable())?;
    let fd = socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
        None,
    )
    .map_err(|_| unavailable())?;
    // One attempt. A full/pending backlog drops this OwnedFd for the fixed host fallback.
    connect(&fd, &address).map_err(|_| unavailable())?;
    let socket = UnixStream::from(fd);
    socket.set_nonblocking(false).map_err(|_| unavailable())?;
    Ok(socket)
}
#[cfg(test)]
#[path = "../../tests/m1_live_dns/units/effect_session.rs"]
mod tests;


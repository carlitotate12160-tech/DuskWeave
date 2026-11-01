//! Fixed Docker AF_UNIX acquisition; private paths/clocks exist only for owner tests.
use duskweave::{Fail, Res};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

pub(super) fn acquire(deadline: Instant) -> Res<UnixStream> {
    acquire_at(Path::new("/run/docker.sock"), deadline, Instant::now)
}

fn acquire_at(path: &Path, deadline: Instant, mut now: impl FnMut() -> Instant) -> Res<UnixStream> {
    use rustix::net::{
        AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
    };
    if now() >= deadline {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let address = SocketAddrUnix::new(path).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let fd = socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
        None,
    )
    .map_err(|_| Fail::Config("live_host_unqualified"))?;
    // One attempt only. Pending, interrupted or refused connect closes this OwnedFd.
    connect(&fd, &address).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let io = authenticate(fd)?;
    if now() >= deadline {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(io)
}
fn authenticate(fd: std::os::fd::OwnedFd) -> Res<UnixStream> {
    let peer = rustix::net::sockopt::socket_peercred(&fd)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if peer.uid.as_raw() != 0 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let io = UnixStream::from(fd);
    io.set_nonblocking(false)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(io)
}

#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/docker_connection.rs"]
mod tests;


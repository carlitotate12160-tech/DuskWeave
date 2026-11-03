//! Authenticate the actual core process before accepting even qualification requests.
use duskweave::{Fail, Res};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub(super) fn serve() -> Res<()> {
    harden()?;
    let (socket, peer) = parent_channel()?;
    serve_connection(socket, peer)
}

fn serve_connection(socket: UnixStream, peer: rustix::net::UCred) -> Res<()> {
    if peer.uid.as_raw() == 0 {
        super::qualification::watchdog_peer(peer)?;
        return super::control::watchdog(socket);
    }
    let qualified = super::qualification::identity(peer)?;
    super::control::dispatch(socket, &qualified)
}

fn harden() -> Res<()> {
    use rustix::process::{DumpableBehavior, Resource, Rlimit};
    if std::env::args_os().len() != 1 || rustix::process::geteuid().as_raw() != 0 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    rustix::process::setrlimit(
        Resource::Core,
        Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|_| Fail::Config("live_host_unqualified"))?;
    rustix::process::set_dumpable_behavior(DumpableBehavior::NotDumpable)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(())
}

fn parent_channel() -> Res<(UnixStream, rustix::net::UCred)> {
    let fd =
        rustix::io::dup(std::io::stdin()).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let socket = UnixStream::from(fd);
    socket
        .set_read_timeout(Some(Duration::from_millis(1000)))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    socket
        .set_write_timeout(Some(Duration::from_millis(1000)))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let peer = rustix::net::sockopt::socket_peercred(&socket)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok((socket, peer))
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/launcher_admission.rs"]
mod tests;

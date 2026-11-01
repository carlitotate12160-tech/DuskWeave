//! Independent host lease watchdog and its owned child lifetime.
use duskweave::m1_dns::DnsRequest;
use duskweave::m1_effect_control::{HostChannel, HostCommand, HostReply, LAUNCHER};
use duskweave::{Fail, Res};
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
pub(super) struct Watchdog {
    process: Child,
    channel: UnixStream,
}
impl Watchdog {
    pub(super) fn start(q: &DnsRequest) -> Res<Self> {
        let (parent, child) =
            UnixStream::pair().map_err(|_| Fail::Config("live_host_unqualified"))?;
        parent
            .set_read_timeout(Some(Duration::from_millis(1000)))
            .and_then(|_| parent.set_write_timeout(Some(Duration::from_millis(100))))
            .map_err(|_| Fail::Config("live_host_unqualified"))?;
        let process = Command::new(LAUNCHER)
            .env_clear()
            .stdin(Stdio::from(OwnedFd::from(child)))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Fail::Config("live_host_unqualified"))?;
        let mut owner = Self {
            process,
            channel: parent,
        };
        owner.handshake(q)?;
        Ok(owner)
    }
    fn handshake(&mut self, q: &DnsRequest) -> Res<()> {
        let mut channel = HostChannel::new(&mut self.channel);
        channel.send_command(&HostCommand::Ack {
            attempt: q.attempt,
            generation: q.session_generation,
        })?;
        match channel.reply()? {
            HostReply::Ready {
                attempt,
                generation,
            } if (attempt, generation) == (q.attempt, q.session_generation) => Ok(()),
            _ => Err(Fail::Config("live_host_unqualified")),
        }
    }
    pub(super) fn current(&mut self) -> Res<()> {
        if self
            .process
            .try_wait()
            .map_err(|_| Fail::Config("live_host_unqualified"))?
            .is_some()
        {
            return Err(Fail::State("live_watchdog_unavailable"));
        }
        Ok(())
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        let _ = self.channel.shutdown(std::net::Shutdown::Both);
        let end = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < end {
            if self.process.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let _ = self.process.kill();
        let end = Instant::now() + Duration::from_millis(100);
        while Instant::now() < end {
            if self.process.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
fn initialize(socket: &mut UnixStream) -> Res<super::super::attempt::Session> {
    let HostCommand::Ack {
        attempt,
        generation,
    } = HostChannel::new(&mut *socket).command()?
    else {
        return Err(Fail::Config("live_host_unqualified"));
    };
    let original = super::super::attempt::load(attempt, generation)?;
    HostChannel::new(&mut *socket).send_reply(&HostReply::Ready {
        attempt,
        generation,
    })?;
    socket
        .set_read_timeout(Some(Duration::from_millis(20)))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(original)
}
pub(crate) fn serve(mut socket: UnixStream) -> Res<()> {
    let original = initialize(&mut socket)?;
    let attempt = original.request.attempt;
    let generation = original.request.session_generation;
    let origin = Instant::now();
    loop {
        if watchdog_stop(&mut socket, attempt, generation)?
            || origin.elapsed() >= Duration::from_millis(9000)
        {
            let q = &original.request;
            let _ =
                super::super::attempt::stop(q.engagement_id, q.campaign_id, q.session_generation);
            return Ok(());
        }
    }
}
fn watchdog_stop(socket: &mut UnixStream, attempt: uuid::Uuid, generation: u64) -> Res<bool> {
    let Ok(session) = super::super::attempt::load(attempt, generation) else {
        return Ok(true);
    };
    if super::super::qualification::now_ms()? >= session.handling_deadline_at_ms {
        return Ok(true);
    }
    let mut byte = [0; 1];
    match socket.read(&mut byte) {
        Err(error)
            if [std::io::ErrorKind::WouldBlock, std::io::ErrorKind::TimedOut]
                .contains(&error.kind()) =>
        {
            Ok(false)
        }
        _ => Ok(true),
    }
}

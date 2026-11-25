//! Independent host lease watchdog and its owned child lifetime.
use duskweave::m1_dns::DnsRequest;
use duskweave::m1_effect_control::{HostChannel, HostCommand, HostReply, LAUNCHER};
use duskweave::{Fail, Res};
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
fn child_command() -> Command {
    #[cfg(test)]
    if let Some(mut command) = tests::child_command() {
        command.env_clear();
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        return command;
    }
    let mut command = Command::new(LAUNCHER);
    command.env_clear();
    command
}
pub(super) struct Watchdog {
    process: Child,
    channel: UnixStream,
}
impl Watchdog {
    pub(super) fn start(q: &DnsRequest) -> Res<Self> {
        Self::start_ids(q.attempt, q.session_generation)
    }
    pub(super) fn start_ids(attempt: uuid::Uuid, generation: u64) -> Res<Self> {
        let (parent, child) =
            UnixStream::pair().map_err(|_| Fail::Config("live_host_unqualified"))?;
        parent
            .set_read_timeout(Some(Duration::from_millis(1000)))
            .and_then(|_| parent.set_write_timeout(Some(Duration::from_millis(100))))
            .map_err(|_| Fail::Config("live_host_unqualified"))?;
        let process = child_command()
            .stdin(Stdio::from(OwnedFd::from(child)))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Fail::Config("live_host_unqualified"))?;
        let mut owner = Self {
            process,
            channel: parent,
        };
        owner.handshake(attempt, generation)?;
        Ok(owner)
    }
    fn handshake(&mut self, attempt: uuid::Uuid, generation: u64) -> Res<()> {
        let mut channel = HostChannel::new(&mut self.channel);
        channel.send_command(&HostCommand::Ack {
            attempt,
            generation,
        })?;
        match channel.reply()? {
            HostReply::Ready {
                attempt: got_attempt,
                generation: got_generation,
            } if (got_attempt, got_generation) == (attempt, generation) => Ok(()),
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
struct WatchdogTarget {
    attempt: uuid::Uuid,
    engagement_id: uuid::Uuid,
    campaign_id: uuid::Uuid,
    generation: u64,
    handling_deadline_at_ms: i64,
    max_wall_ms: u64,
}
fn load_watchdog(attempt: uuid::Uuid, generation: u64) -> Res<WatchdogTarget> {
    if let Ok(s) = super::super::attempt::load(attempt, generation) {
        return Ok(WatchdogTarget {
            attempt,
            engagement_id: s.request.engagement_id,
            campaign_id: s.request.campaign_id,
            generation: s.request.session_generation,
            handling_deadline_at_ms: s.handling_deadline_at_ms,
            max_wall_ms: 9000,
        });
    }
    let s = super::super::attempt::https::load(attempt, generation)?;
    Ok(WatchdogTarget {
        attempt,
        engagement_id: s.request.engagement_id,
        campaign_id: s.request.campaign_id,
        generation: s.request.session_generation,
        handling_deadline_at_ms: s.handling_deadline_at_ms,
        max_wall_ms: s.max_watchdog_ms(),
    })
}
fn initialize(socket: &mut UnixStream) -> Res<WatchdogTarget> {
    let HostCommand::Ack {
        attempt,
        generation,
    } = HostChannel::new(&mut *socket).command()?
    else {
        return Err(Fail::Config("live_host_unqualified"));
    };
    let original = load_watchdog(attempt, generation)?;
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
    let attempt = original.attempt;
    let generation = original.generation;
    let origin = Instant::now();
    loop {
        if watchdog_stop(&mut socket, attempt, generation)?
            || origin.elapsed() >= Duration::from_millis(original.max_wall_ms)
        {
            let _ = super::super::attempt::stop(
                original.engagement_id,
                original.campaign_id,
                original.generation,
            );
            return Ok(());
        }
    }
}
fn watchdog_stop(socket: &mut UnixStream, attempt: uuid::Uuid, generation: u64) -> Res<bool> {
    let Ok(session) = load_watchdog(attempt, generation) else {
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

#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/launcher_watchdog.rs"]
pub(crate) mod tests;

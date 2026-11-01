//! Fixed launcher composition. No shell, inherited credentials or path override.
use super::*;

#[cfg(not(target_os = "linux"))]
pub(super) fn qualify(_: &LiveBinding, _: &str) -> Res<()> {
    Err(Fail::Config("live_host_platform_unqualified"))
}

#[cfg(target_os = "linux")]
fn spawn_launcher(
    read_ms: u64,
) -> Res<(
    HostChannel<std::os::unix::net::UnixStream>,
    std::process::Child,
)> {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};
    use std::time::Duration;
    let (parent, child) = UnixStream::pair().map_err(|_| Fail::Config("live_host_unavailable"))?;
    parent
        .set_read_timeout(Some(Duration::from_millis(read_ms)))
        .map_err(|_| Fail::Config("live_host_unavailable"))?;
    parent
        .set_write_timeout(Some(Duration::from_millis(1000)))
        .map_err(|_| Fail::Config("live_host_unavailable"))?;
    let process = Command::new("/usr/bin/sudo")
        .args(["-n", "--", LAUNCHER])
        .env_clear()
        .stdin(Stdio::from(OwnedFd::from(child)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Fail::Config("live_host_unavailable"))?;
    Ok((HostChannel::new(parent), process))
}

#[cfg(target_os = "linux")]
fn exited(process: &mut std::process::Child, grace_ms: u64) -> Res<bool> {
    let budget = Duration::from_millis(grace_ms);
    let deadline = Instant::now() + budget;
    let termination = deadline - (budget / 2).min(Duration::from_millis(50));
    if let Some(clean) = wait_exited(process, termination)? {
        return Ok(clean);
    }
    let _ = process.kill();
    // Termination and reap share the original owner budget.
    wait_exited(process, deadline)?;
    Ok(false)
}
#[cfg(target_os = "linux")]
fn wait_exited(process: &mut std::process::Child, deadline: Instant) -> Res<Option<bool>> {
    loop {
        if let Some(status) = process
            .try_wait()
            .map_err(|_| Fail::Config("live_host_unavailable"))?
        {
            return Ok(Some(status.success()));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(target_os = "linux")]
pub(super) fn qualify(binding: &LiveBinding, run_profile: &str) -> Res<()> {
    let (mut channel, mut process) = spawn_launcher(1000)?;
    let checked = (|| {
        channel.send_command(&HostCommand::Qualify {
            binding: binding.clone(),
            run_profile: run_profile.into(),
        })?;
        channel
            .reply()?
            .qualified_for(binding)
            .then_some(())
            .ok_or(Fail::Config("live_host_unqualified"))
    })();
    // A launcher that has not terminated after its reply is not a qualified peer.
    drop(channel);
    let clean = exited(&mut process, 100)?;
    checked?;
    clean
        .then_some(())
        .ok_or(Fail::Config("live_host_unqualified"))
}

/// One bounded session command exchange. The launcher never outlives its reply.
#[cfg(target_os = "linux")]
pub(super) fn session(command: &HostCommand, deadline_ms: u32) -> Res<HostReply> {
    let (mut channel, mut process) = spawn_launcher(u64::from(deadline_ms))?;
    let outcome = channel.send_command(command).and_then(|_| channel.reply());
    drop(channel);
    let clean = exited(&mut process, 100)?;
    let reply = outcome?;
    if !clean || matches!(reply, HostReply::Refused) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(reply)
}

#[cfg(target_os = "linux")]
struct Peer {
    channel: HostChannel<std::os::unix::net::UnixStream>,
    process: std::process::Child,
    result: std::sync::mpsc::Receiver<Res<HostReply>>,
}

#[cfg(target_os = "linux")]
pub struct LiveBoundary {
    run: Uuid,
    request: Option<DnsRequest>,
    peer: Option<Peer>,
    closed: bool,
    ingress: Option<super::session::Ingress>,
}

#[cfg(target_os = "linux")]
impl LiveBoundary {
    pub fn new(run: Uuid) -> Self {
        Self {
            run,
            request: None,
            peer: None,
            closed: false,
            ingress: None,
        }
    }
    fn peer(&mut self) -> Res<&mut Peer> {
        self.peer
            .as_mut()
            .ok_or(Fail::State("live_host_unavailable"))
    }
    fn command(&mut self, command: &HostCommand) -> Res<()> {
        self.peer()?.channel.send_command(command)
    }
    fn ready_peer(&mut self, q: &DnsRequest) -> Res<()> {
        let (mut channel, mut process) = spawn_launcher(5000)?;
        let result = prepare_peer(&mut channel, self.run, q);
        match result {
            Ok(result) => {
                self.peer = Some(Peer {
                    channel,
                    process,
                    result,
                });
                Ok(())
            }
            Err(error) => {
                drop(channel);
                let _ = exited(&mut process, 1000);
                Err(error)
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn prepare_peer(
    channel: &mut HostChannel<std::os::unix::net::UnixStream>,
    run: Uuid,
    q: &DnsRequest,
) -> Res<std::sync::mpsc::Receiver<Res<HostReply>>> {
    open_peer(channel, run, q)?;
    let reader = channel
        .io
        .try_clone()
        .map_err(|_| Fail::State("live_host_unavailable"))?;
    reader
        // READY-to-START, the original attempt, and terminal handling are separate bounds.
        .set_read_timeout(Some(Duration::from_millis(1000 + 2000 + 1000)))
        .map_err(|_| Fail::State("live_host_unavailable"))?;
    let (send, result) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = send.send(HostChannel::new(reader).reply());
    });
    Ok(result)
}

#[cfg(target_os = "linux")]
fn open_peer(
    channel: &mut HostChannel<std::os::unix::net::UnixStream>,
    run: Uuid,
    q: &DnsRequest,
) -> Res<()> {
    channel.send_command(&HostCommand::Open {
        run_id: run,
        request: Box::new(q.clone()),
    })?;
    match channel.reply()? {
        HostReply::Ready {
            attempt,
            generation,
        } if (attempt, generation) == (q.attempt, q.session_generation) => {}
        _ => return Err(Fail::State("live_host_protocol_refused")),
    }
    Ok(())
}

#[cfg(target_os = "linux")]
impl DnsBoundary for LiveBoundary {
    fn ready(&mut self, q: &DnsRequest) -> Res<()> {
        if self.request.is_some() || self.closed {
            return Err(Fail::State("live_effect_already_charged"));
        }
        self.request = Some(q.clone());
        self.ingress = Some(super::session::Ingress::start(self.run, q)?);
        self.ready_peer(q)
    }
    fn start(&mut self, q: &DnsRequest, deadline_at_ms: i64) -> Res<()> {
        self.command(&HostCommand::Start {
            attempt: q.attempt,
            generation: q.session_generation,
            deadline_at_ms,
        })
    }
    fn result(&mut self) -> Res<Option<DnsResult>> {
        use std::sync::mpsc::TryRecvError;
        match self.peer()?.result.try_recv() {
            Ok(Ok(HostReply::Result { result })) => Ok(Some(result)),
            Ok(Err(error)) => Err(error),
            Err(TryRecvError::Empty) => Ok(None),
            _ => Err(Fail::State("live_host_protocol_refused")),
        }
    }
    fn renew(&mut self, q: &DnsRequest, deadline_at_ms: i64) -> Res<()> {
        self.command(&HostCommand::Renew {
            attempt: q.attempt,
            generation: q.session_generation,
            deadline_at_ms,
        })
    }
    fn acknowledge(&mut self, q: &DnsRequest) -> Res<bool> {
        self.command(&HostCommand::Ack {
            attempt: q.attempt,
            generation: q.session_generation,
        })?;
        let HostReply::Stopped { disposal_confirmed } = self.peer()?.channel.reply()? else {
            return Err(Fail::State("live_host_protocol_refused"));
        };
        let mut peer = self
            .peer
            .take()
            .ok_or(Fail::State("live_host_unavailable"))?;
        drop(peer.channel);
        let clean = exited(&mut peer.process, 1000)?;
        self.closed = disposal_confirmed && clean;
        Ok(self.closed)
    }
    fn stop(&mut self) -> Res<bool> {
        let Some(q) = &self.request else {
            return Ok(true);
        };
        let reply = session(
            &HostCommand::Stop {
                engagement_id: q.engagement_id,
                campaign_id: q.campaign_id,
                generation: q.session_generation,
            },
            1000,
        );
        if let Some(mut peer) = self.peer.take() {
            drop(peer.channel);
            let _ = exited(&mut peer.process, 1000);
        }
        let disposed = matches!(
            reply?,
            HostReply::Stopped {
                disposal_confirmed: true
            }
        );
        self.closed = disposed;
        Ok(disposed)
    }
}

#[cfg(target_os = "linux")]
impl Drop for LiveBoundary {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.stop();
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub struct LiveBoundary;
#[cfg(not(target_os = "linux"))]
impl LiveBoundary {
    pub fn new(_: Uuid) -> Self {
        Self
    }
}
#[cfg(not(target_os = "linux"))]
impl DnsBoundary for LiveBoundary {
    fn ready(&mut self, _: &DnsRequest) -> Res<()> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn start(&mut self, _: &DnsRequest, _: i64) -> Res<()> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn result(&mut self) -> Res<Option<DnsResult>> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn renew(&mut self, _: &DnsRequest, _: i64) -> Res<()> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn acknowledge(&mut self, _: &DnsRequest) -> Res<bool> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn stop(&mut self) -> Res<bool> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn stop(_: super::authority::StopTarget) -> Res<super::authority::StopDisposition> {
    Err(Fail::Config("live_host_platform_unqualified"))
}
#[cfg(target_os = "linux")]
pub(super) fn stop(target: super::authority::StopTarget) -> Res<super::authority::StopDisposition> {
    use super::authority::{StopDisposition, StopTarget};
    let command = stop_command(target);
    let reply = session(&command, 1000)?;
    match (target, reply) {
        (
            StopTarget::Run { run_id: a },
            HostReply::RunStopped {
                run_id: b,
                stop_latched,
                disposal_confirmed,
            },
        ) if a == b => Ok(StopDisposition {
            stop_latched,
            disposal_confirmed,
        }),
        (
            StopTarget::Campaign {
                engagement_id: a,
                campaign_id: b,
            },
            HostReply::CampaignStopped {
                engagement_id: c,
                campaign_id: d,
                stop_latched,
                disposal_confirmed,
            },
        ) if (a, b) == (c, d) => Ok(StopDisposition {
            stop_latched,
            disposal_confirmed,
        }),
        _ => Err(Fail::State("live_host_protocol_refused")),
    }
}

#[cfg(target_os = "linux")]
fn stop_command(target: super::authority::StopTarget) -> HostCommand {
    use super::authority::StopTarget;
    match target {
        StopTarget::Run { run_id } => HostCommand::StopRun { run_id },
        StopTarget::Campaign {
            engagement_id,
            campaign_id,
        } => HostCommand::StopCampaign {
            engagement_id,
            campaign_id,
        },
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
#[test]
fn killed_launcher_child_is_reaped_within_its_original_owner_budget() {
    let mut process = std::process::Command::new("/bin/sleep")
        .arg("10")
        .env_clear()
        .spawn()
        .unwrap();
    let path = std::path::PathBuf::from(format!("/proc/{}", process.id()));
    let clean = exited(&mut process, 100).unwrap();
    let end = Instant::now() + Duration::from_millis(50);
    while path.exists() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(1));
    }
    let reaped = !path.exists();
    let _ = process.wait(); // Always reap the RED witness before asserting.
    assert!(!clean);
    assert!(
        reaped,
        "returning after kill must not leave an unreaped owned child"
    );
}
#[cfg(test)]
#[path = "../../tests/m1_live_dns/units/effect_socket.rs"]
mod tests;

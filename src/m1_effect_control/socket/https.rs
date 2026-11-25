//! Launcher subprocess socket adapter for phased HTTPS `connect` -> `tls` -> `head`.
use super::super::https::{HttpsBoundary, HttpsEvent};
#[cfg(target_os = "linux")]
use super::super::{HostCommand, HostReply};
#[cfg(target_os = "linux")]
use super::process::{exited, spawn_launcher};
use crate::m1_https::ipc::{HttpsIpcRequest, HttpsPhase};
use crate::{Fail, Res};
use uuid::Uuid;

#[cfg(target_os = "linux")]
struct HttpsPeer {
    channel: super::super::HostChannel<std::os::unix::net::UnixStream>,
    process: std::process::Child,
    events: std::sync::mpsc::Receiver<Res<HostReply>>,
    reader: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
pub struct LiveHttpsBoundary {
    run: Uuid,
    request: Option<HttpsIpcRequest>,
    peer: Option<HttpsPeer>,
    closed: bool,
    ingress: Option<super::super::session::Ingress>,
}

#[cfg(target_os = "linux")]
impl LiveHttpsBoundary {
    pub fn new(run: Uuid) -> Self {
        Self {
            run,
            request: None,
            peer: None,
            closed: false,
            ingress: None,
        }
    }

    fn peer(&mut self) -> Res<&mut HttpsPeer> {
        self.peer
            .as_mut()
            .ok_or(Fail::State("live_host_unavailable"))
    }

    fn command(&mut self, command: &HostCommand) -> Res<()> {
        self.peer()?.channel.send_command(command)
    }

    fn ready_peer(&mut self, q: &HttpsIpcRequest) -> Res<()> {
        self.open_with(q, spawn_launcher)
    }

    fn open_with<F>(&mut self, q: &HttpsIpcRequest, spawn: F) -> Res<()>
    where
        F: FnOnce(
            u64,
        ) -> Res<(
            super::super::HostChannel<std::os::unix::net::UnixStream>,
            std::process::Child,
        )>,
    {
        let spawn_read = 2000 + u64::from(q.max_duration_ms) + 2000;
        let (mut channel, mut process) = spawn(spawn_read)?;
        match prepare_peer(&mut channel, self.run, q) {
            Ok((events, reader)) => {
                self.peer = Some(HttpsPeer {
                    channel,
                    process,
                    events,
                    reader: Some(reader),
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
    channel: &mut super::super::HostChannel<std::os::unix::net::UnixStream>,
    run: Uuid,
    q: &HttpsIpcRequest,
) -> Res<(
    std::sync::mpsc::Receiver<Res<HostReply>>,
    std::thread::JoinHandle<()>,
)> {
    channel
        .io
        .set_read_timeout(Some(std::time::Duration::from_millis(5000)))
        .map_err(|_| Fail::State("live_host_unavailable"))?;
    open_peer(channel, run, q)?;
    let reader_io = channel
        .io
        .try_clone()
        .map_err(|_| Fail::State("live_host_unavailable"))?;
    let wait_ms = 1000 + u64::from(q.max_duration_ms) + 1000;
    reader_io
        .set_read_timeout(Some(std::time::Duration::from_millis(wait_ms)))
        .map_err(|_| Fail::State("live_host_unavailable"))?;
    let (send, recv) = std::sync::mpsc::sync_channel(8);
    let expected = (q.attempt, q.session_generation);
    let reader = std::thread::spawn(move || read_replies(reader_io, expected, send));
    Ok((recv, reader))
}

#[cfg(target_os = "linux")]
fn valid_phase_reply(
    attempt: Uuid,
    generation: u64,
    phase: HttpsPhase,
    expected: (Uuid, u64),
) -> bool {
    [
        (attempt, generation) == expected,
        matches!(phase, HttpsPhase::Tls | HttpsPhase::Head),
    ]
    .into_iter()
    .all(std::convert::identity)
}

#[cfg(target_os = "linux")]
fn step_reply(
    chan: &mut super::super::HostChannel<std::os::unix::net::UnixStream>,
    expected: (Uuid, u64),
    send: &std::sync::mpsc::SyncSender<Res<HostReply>>,
) -> bool {
    match chan.reply() {
        Ok(
            reply @ HostReply::ReadyPhase {
                attempt,
                generation,
                phase,
            },
        ) => {
            if !valid_phase_reply(attempt, generation, phase, expected) {
                let _ = send.send(Err(Fail::State("live_host_protocol_refused")));
                return false;
            }
            send.send(Ok(reply)).is_ok()
        }
        Ok(reply @ HostReply::ResultHttps { .. }) => {
            let _ = send.send(Ok(reply));
            false
        }
        Ok(_) => {
            let _ = send.send(Err(Fail::State("live_host_protocol_refused")));
            false
        }
        Err(error) => {
            let _ = send.send(Err(error));
            false
        }
    }
}

#[cfg(target_os = "linux")]
fn read_replies(
    reader_io: std::os::unix::net::UnixStream,
    expected: (Uuid, u64),
    send: std::sync::mpsc::SyncSender<Res<HostReply>>,
) {
    let mut chan = super::super::HostChannel::new(reader_io);
    while step_reply(&mut chan, expected, &send) {}
}

#[cfg(target_os = "linux")]
fn open_peer(
    channel: &mut super::super::HostChannel<std::os::unix::net::UnixStream>,
    run: Uuid,
    q: &HttpsIpcRequest,
) -> Res<()> {
    channel.send_command(&HostCommand::OpenHttps {
        run_id: run,
        request: Box::new(q.clone()),
    })?;
    match channel.reply()? {
        HostReply::ReadyPhase {
            attempt,
            generation,
            phase: HttpsPhase::Connect,
        } if (attempt, generation) == (q.attempt, q.session_generation) => Ok(()),
        _ => Err(Fail::State("live_host_protocol_refused")),
    }
}

#[cfg(target_os = "linux")]
fn decode_polled_reply(reply: HostReply, expected: (Uuid, u64)) -> Res<Option<HttpsEvent>> {
    match reply {
        HostReply::ReadyPhase {
            attempt,
            generation,
            phase,
        } if (attempt, generation) == expected => Ok(Some(HttpsEvent::Ready(phase))),
        HostReply::ResultHttps { result } => Ok(Some(HttpsEvent::Result(result))),
        _ => Err(Fail::State("live_host_protocol_refused")),
    }
}

#[cfg(target_os = "linux")]
fn exchange_ack(
    channel: &mut super::super::HostChannel<std::os::unix::net::UnixStream>,
    q: &HttpsIpcRequest,
) -> Res<bool> {
    channel
        .io
        .set_read_timeout(Some(std::time::Duration::from_millis(2000)))
        .map_err(|_| Fail::State("live_host_unavailable"))?;
    channel.send_command(&HostCommand::AckHttps {
        attempt: q.attempt,
        generation: q.session_generation,
    })?;
    let HostReply::Stopped { disposal_confirmed } = channel.reply()? else {
        return Err(Fail::State("live_host_protocol_refused"));
    };
    Ok(disposal_confirmed)
}

#[cfg(target_os = "linux")]
impl HttpsBoundary for LiveHttpsBoundary {
    fn ready_connect(&mut self, q: &HttpsIpcRequest) -> Res<()> {
        if self.request.is_some() || self.closed {
            return Err(Fail::State("live_effect_already_charged"));
        }
        self.request = Some(q.clone());
        self.ingress = Some(super::super::session::Ingress::start_binding(
            self.run,
            q.engagement_id,
            q.campaign_id,
            q.session_generation,
        )?);
        self.ready_peer(q)
    }

    fn start_phase(
        &mut self,
        q: &HttpsIpcRequest,
        phase: HttpsPhase,
        deadline_at_ms: i64,
    ) -> Res<()> {
        self.command(&HostCommand::StartPhase {
            attempt: q.attempt,
            generation: q.session_generation,
            phase,
            deadline_at_ms,
        })
    }

    fn poll(&mut self) -> Res<Option<HttpsEvent>> {
        use std::sync::mpsc::TryRecvError;
        let expected = self
            .request
            .as_ref()
            .map(|r| (r.attempt, r.session_generation))
            .ok_or(Fail::State("live_host_unavailable"))?;
        match self.peer()?.events.try_recv() {
            Ok(Ok(reply)) => decode_polled_reply(reply, expected),
            Ok(Err(error)) => Err(error),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(Fail::State("live_host_protocol_refused")),
        }
    }

    fn renew(&mut self, q: &HttpsIpcRequest, deadline_at_ms: i64) -> Res<()> {
        self.command(&HostCommand::RenewHttps {
            attempt: q.attempt,
            generation: q.session_generation,
            deadline_at_ms,
        })
    }

    fn acknowledge(&mut self, q: &HttpsIpcRequest) -> Res<bool> {
        let mut peer = self
            .peer
            .take()
            .ok_or(Fail::State("live_host_unavailable"))?;
        if let Some(reader) = peer.reader.take() {
            let _ = reader.join();
        }
        let disposal_confirmed = exchange_ack(&mut peer.channel, q)?;
        drop(peer.channel);
        let clean = exited(&mut peer.process, 1000)?;
        self.ingress = None;
        self.closed = disposal_confirmed && clean;
        Ok(self.closed)
    }

    fn stop(&mut self) -> Res<bool> {
        let Some(q) = &self.request else {
            self.ingress = None;
            return Ok(true);
        };
        let reply = super::session(
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
            if let Some(reader) = peer.reader.take() {
                let _ = reader.join();
            }
        }
        self.ingress = None;
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
impl Drop for LiveHttpsBoundary {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.stop();
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub struct LiveHttpsBoundary;

#[cfg(not(target_os = "linux"))]
impl LiveHttpsBoundary {
    pub fn new(_: Uuid) -> Self {
        Self
    }
}

#[cfg(not(target_os = "linux"))]
impl HttpsBoundary for LiveHttpsBoundary {
    fn ready_connect(&mut self, _: &HttpsIpcRequest) -> Res<()> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn start_phase(&mut self, _: &HttpsIpcRequest, _: HttpsPhase, _: i64) -> Res<()> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn poll(&mut self) -> Res<Option<HttpsEvent>> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn renew(&mut self, _: &HttpsIpcRequest, _: i64) -> Res<()> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn acknowledge(&mut self, _: &HttpsIpcRequest) -> Res<bool> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
    fn stop(&mut self) -> Res<bool> {
        Err(Fail::Config("live_host_platform_unqualified"))
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
#[path = "../../../tests/m1_live_dns/units/effect_https_socket.rs"]
mod tests;

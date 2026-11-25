//! Session protocol progression; raw parsing remains owned by the relay.
use super::super::relay::{
    Pair,
    https::{HttpsPair, PairCmd, PairEvt},
};
use super::watchdog::Watchdog;
use duskweave::m1_dns::DnsRequest;
use duskweave::m1_effect_control::{HostChannel, HostCommand, HostReply};
use duskweave::m1_https::ipc::{HttpsIpcRequest, HttpsPhase, HttpsSafeResult};
use duskweave::{Fail, Res};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::time::Duration;
use uuid::Uuid;
pub(super) fn receive_start(channel: &mut HostChannel<UnixStream>, q: &DnsRequest) -> Res<i64> {
    match channel.command()? {
        HostCommand::Start {
            attempt,
            generation,
            deadline_at_ms,
        } if (attempt, generation) == (q.attempt, q.session_generation) => Ok(deadline_at_ms),
        _ => Err(Fail::State("live_effect_start_refused")),
    }
}
pub(super) fn wait_ack(
    channel: &mut HostChannel<UnixStream>,
    q: &DnsRequest,
    pair: &mut Pair,
    watchdog: &mut Watchdog,
) -> Res<()> {
    loop {
        if acknowledged(channel, q)? {
            pair.acknowledge()?;
            let disposed = pair.dispose();
            return channel.send_reply(&HostReply::Stopped {
                disposal_confirmed: disposed,
            });
        }
        // Result is exported. Queued renewals cannot reopen egress.
        watchdog.current()?;
    }
}
fn acknowledged(channel: &mut HostChannel<UnixStream>, q: &DnsRequest) -> Res<bool> {
    match channel.command()? {
        HostCommand::Renew {
            attempt,
            generation,
            ..
        } if (attempt, generation) == (q.attempt, q.session_generation) => Ok(false),
        HostCommand::Ack {
            attempt,
            generation,
        } if (attempt, generation) == (q.attempt, q.session_generation) => Ok(true),
        _ => Err(Fail::State("live_host_protocol_refused")),
    }
}
/// Wait on the existing raw-facing worker while servicing authenticated renewal.
/// The scope cannot finish an error path before host STOP has been applied.
pub(super) fn await_result(
    channel: &mut HostChannel<UnixStream>,
    peer: &UnixStream,
    q: &DnsRequest,
    deadline: i64,
    pair: &mut Pair,
    watchdog: &mut Watchdog,
) -> Res<duskweave::m1_dns::DnsResult> {
    std::thread::scope(|scope| {
        let (send, result) = std::sync::mpsc::sync_channel(1);
        scope.spawn(move || {
            let _ = send.send(pair.result(deadline));
        });
        let outcome = poll_result(channel, peer, q, deadline, &result, watchdog);
        if outcome.is_err() {
            let _ =
                super::super::attempt::stop(q.engagement_id, q.campaign_id, q.session_generation);
        }
        outcome
    })
}
fn poll_result(
    channel: &mut HostChannel<UnixStream>,
    peer: &UnixStream,
    q: &DnsRequest,
    deadline: i64,
    result: &std::sync::mpsc::Receiver<Res<duskweave::m1_dns::DnsResult>>,
    watchdog: &mut Watchdog,
) -> Res<duskweave::m1_dns::DnsResult> {
    loop {
        if let Some(value) = completed(result)? {
            return Ok(value);
        }
        poll_control(channel, peer, q, watchdog, deadline)?;
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn poll_control(
    channel: &mut HostChannel<UnixStream>,
    peer: &UnixStream,
    q: &DnsRequest,
    watchdog: &mut Watchdog,
    deadline: i64,
) -> Res<()> {
    watchdog.current()?;
    if control_readable(peer)? {
        renew(channel, q)?;
    }
    if super::super::qualification::now_ms()? >= deadline.saturating_add(1000) {
        return Err(Fail::State("live_effect_deadline"));
    }
    Ok(())
}
fn control_readable(peer: &UnixStream) -> Res<bool> {
    let mut byte = [0; 1];
    match rustix::net::recv(
        peer,
        &mut byte[..],
        rustix::net::RecvFlags::PEEK | rustix::net::RecvFlags::DONTWAIT,
    ) {
        Ok((_, 0)) => Err(Fail::State("live_host_unavailable")),
        Ok(_) => Ok(true),
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(false),
        Err(_) => Err(Fail::State("live_host_unavailable")),
    }
}

fn completed(
    result: &std::sync::mpsc::Receiver<Res<duskweave::m1_dns::DnsResult>>,
) -> Res<Option<duskweave::m1_dns::DnsResult>> {
    match result.try_recv() {
        Ok(value) => value.map(Some),
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(Fail::State("dns_ipc_failure")),
        Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
    }
}
fn renew(channel: &mut HostChannel<UnixStream>, q: &DnsRequest) -> Res<()> {
    match channel.command()? {
        HostCommand::Renew {
            attempt,
            generation,
            deadline_at_ms,
        } if (attempt, generation) == (q.attempt, q.session_generation) => {
            super::super::attempt::renew(attempt, generation, deadline_at_ms)
        }
        _ => Err(Fail::State("live_host_protocol_refused")),
    }
}

pub(super) fn receive_https_start(
    channel: &mut HostChannel<UnixStream>,
    q: &HttpsIpcRequest,
    expected: HttpsPhase,
) -> Res<i64> {
    match channel.command()? {
        HostCommand::StartPhase {
            attempt,
            generation,
            phase,
            deadline_at_ms,
        } if (attempt, generation, phase) == (q.attempt, q.session_generation, expected) => {
            Ok(deadline_at_ms)
        }
        _ => Err(Fail::State("live_effect_start_refused")),
    }
}

fn https_acknowledged(channel: &mut HostChannel<UnixStream>, q: &HttpsIpcRequest) -> Res<bool> {
    match channel.command()? {
        HostCommand::RenewHttps {
            attempt,
            generation,
            ..
        } if (attempt, generation) == (q.attempt, q.session_generation) => Ok(false),
        HostCommand::AckHttps {
            attempt,
            generation,
        } if (attempt, generation) == (q.attempt, q.session_generation) => Ok(true),
        _ => Err(Fail::State("live_host_protocol_refused")),
    }
}

pub(super) fn wait_https_ack(
    channel: &mut HostChannel<UnixStream>,
    q: &HttpsIpcRequest,
    pair: &mut HttpsPair,
    watchdog: &mut Watchdog,
) -> Res<()> {
    loop {
        if https_acknowledged(channel, q)? {
            pair.acknowledge()?;
            return channel.send_reply(&HostReply::Stopped {
                disposal_confirmed: pair.dispose(),
            });
        }
        watchdog.current()?;
    }
}

pub(super) fn await_https_result(
    channel: &mut HostChannel<UnixStream>,
    peer: &UnixStream,
    q: &HttpsIpcRequest,
    deadline: i64,
    pair: &mut HttpsPair,
    watchdog: &mut Watchdog,
) -> Res<HttpsSafeResult> {
    std::thread::scope(|scope| {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel(1);
        let (evt_tx, evt_rx) = std::sync::mpsc::sync_channel(2);
        scope.spawn(move || {
            let outcome = pair.drive(deadline, &evt_tx, &cmd_rx, &|attempt| {
                let _ = super::super::network::revoke(attempt);
            });
            let _ = evt_tx.send(PairEvt::Done(outcome));
        });
        let outcome = HttpsWait {
            channel,
            q,
            deadline,
            waiting_phase: None,
            io_revoked: false,
        }
        .run(peer, &evt_rx, &cmd_tx, watchdog);
        if outcome.is_err() {
            let _ = super::super::network::revoke(q.attempt);
            let _ =
                super::super::attempt::stop(q.engagement_id, q.campaign_id, q.session_generation);
            drop(cmd_tx);
        }
        outcome
    })
}

struct HttpsWait<'a> {
    channel: &'a mut HostChannel<UnixStream>,
    q: &'a HttpsIpcRequest,
    deadline: i64,
    waiting_phase: Option<HttpsPhase>,
    io_revoked: bool,
}

impl HttpsWait<'_> {
    fn run(
        mut self,
        peer: &UnixStream,
        evt_rx: &Receiver<PairEvt>,
        cmd_tx: &SyncSender<PairCmd>,
        watchdog: &mut Watchdog,
    ) -> Res<HttpsSafeResult> {
        loop {
            if let Some(done) = self.drain(evt_rx)? {
                return Ok(done);
            }
            self.tick(peer, cmd_tx, watchdog)?;
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn revoke_once(&mut self) {
        if !self.io_revoked {
            let _ = super::super::network::revoke(self.q.attempt);
            self.io_revoked = true;
        }
    }

    fn on_ready(&mut self, phase: HttpsPhase) -> Res<Option<HttpsSafeResult>> {
        let expired = super::super::qualification::now_ms()? >= self.deadline;
        if [self.waiting_phase.is_some(), self.io_revoked, expired]
            .into_iter()
            .any(std::convert::identity)
        {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        self.channel.send_reply(&HostReply::ReadyPhase {
            attempt: self.q.attempt,
            generation: self.q.session_generation,
            phase,
        })?;
        self.waiting_phase = Some(phase);
        Ok(None)
    }

    fn drain(&mut self, evt_rx: &Receiver<PairEvt>) -> Res<Option<HttpsSafeResult>> {
        match evt_rx.try_recv() {
            Ok(PairEvt::Ready(phase)) => self.on_ready(phase),
            Ok(PairEvt::Done(res)) => {
                self.revoke_once();
                res.map(Some)
            }
            Err(TryRecvError::Disconnected) => Err(Fail::State("https_ipc_failure")),
            Err(TryRecvError::Empty) => Ok(None),
        }
    }

    fn tick(
        &mut self,
        peer: &UnixStream,
        cmd_tx: &SyncSender<PairCmd>,
        watchdog: &mut Watchdog,
    ) -> Res<()> {
        watchdog.current()?;
        let now = super::super::qualification::now_ms()?;
        if now >= self.deadline {
            self.revoke_once();
        }
        if now >= self.deadline.saturating_add(1000) {
            return Err(Fail::State("live_effect_deadline"));
        }
        if !control_readable(peer)? {
            return Ok(());
        }
        self.command(cmd_tx)
    }

    fn on_renew(&self, attempt: Uuid, generation: u64, dl: i64) -> Res<()> {
        if (attempt, generation) != (self.q.attempt, self.q.session_generation)
            || dl > self.deadline
        {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        if !self.io_revoked {
            super::super::attempt::https::renew(attempt, generation, dl)?;
        }
        Ok(())
    }

    fn on_start(
        &mut self,
        cmd_tx: &SyncSender<PairCmd>,
        attempt: Uuid,
        generation: u64,
        phase: HttpsPhase,
        dl: i64,
    ) -> Res<()> {
        let valid = [
            (attempt, generation) == (self.q.attempt, self.q.session_generation),
            self.waiting_phase == Some(phase),
            !self.io_revoked,
        ]
        .into_iter()
        .all(std::convert::identity);
        if !valid {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        super::super::attempt::https::advance_phase(attempt, generation, phase, dl, || {
            cmd_tx
                .send(PairCmd::StartPhase(phase, dl))
                .map_err(|_| Fail::State("live_effect_stopped"))
        })?;
        self.waiting_phase = None;
        Ok(())
    }

    fn command(&mut self, cmd_tx: &SyncSender<PairCmd>) -> Res<()> {
        match self.channel.command()? {
            HostCommand::RenewHttps {
                attempt,
                generation,
                deadline_at_ms,
            } => self.on_renew(attempt, generation, deadline_at_ms),
            HostCommand::StartPhase {
                attempt,
                generation,
                phase,
                deadline_at_ms,
            } => self.on_start(cmd_tx, attempt, generation, phase, deadline_at_ms),
            _ => Err(Fail::State("live_host_protocol_refused")),
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/launcher_exchange.rs"]
mod tests;

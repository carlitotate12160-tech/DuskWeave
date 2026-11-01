//! Session protocol progression; raw parsing remains owned by the relay.
use super::super::relay::Pair;
use super::watchdog::Watchdog;
use duskweave::m1_dns::DnsRequest;
use duskweave::m1_effect_control::{HostChannel, HostCommand, HostReply};
use duskweave::{Fail, Res};
use std::os::unix::net::UnixStream;
use std::time::Duration;
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

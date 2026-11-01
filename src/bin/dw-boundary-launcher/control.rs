//! One authenticated session owns preparation, one START, safe result, then ACK.
mod exchange;
mod watchdog;
use super::qualification::{Manifest, Qualified};
use super::relay::{Pair, RawPipe};
use duskweave::m1_dns::DnsRequest;
use duskweave::m1_effect_control::{HostChannel, HostCommand, HostReply};
use duskweave::m1_inference_permission::LiveBinding;
use duskweave::{Fail, Res};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
use watchdog::Watchdog;
pub(super) use watchdog::serve as watchdog;
fn binding_of(q: &DnsRequest) -> LiveBinding {
    LiveBinding {
        qualification_ref: q.qualification_ref,
        release_sha256: q.release_sha256.clone(),
        resolver_ipv4: q.resolver_ipv4.clone(),
    }
}
fn open(
    channel: &mut HostChannel<UnixStream>,
    manifest: &Manifest,
    run: uuid::Uuid,
    q: &DnsRequest,
    raw: RawPipe,
    peer: &UnixStream,
) -> Res<()> {
    if !manifest.matches(&binding_of(q), &q.run_profile) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let session = super::attempt::open(run, q)?;
    let outcome = exchange(channel, manifest, &session, raw, peer);
    if outcome.is_err() {
        let _ = super::attempt::stop(q.engagement_id, q.campaign_id, q.session_generation);
        let _ = super::attempt::dispose(q.attempt);
    }
    outcome
}

fn exchange(
    channel: &mut HostChannel<UnixStream>,
    manifest: &Manifest,
    session: &super::attempt::Session,
    raw: RawPipe,
    peer: &UnixStream,
) -> Res<()> {
    let q = &session.request;
    let (mut pair, mut watchdog) = prepare(channel, manifest, session, raw)?;
    let deadline = release(channel, q, &mut pair, &mut watchdog)?;
    let result = exchange::await_result(channel, peer, q, deadline, &mut pair, &mut watchdog)?;
    channel.send_reply(&HostReply::Result { result })?;
    exchange::wait_ack(channel, q, &mut pair, &mut watchdog)
}
fn prepare(
    channel: &mut HostChannel<UnixStream>,
    manifest: &Manifest,
    session: &super::attempt::Session,
    raw: RawPipe,
) -> Res<(Pair, Watchdog)> {
    let q = &session.request;
    let mut watchdog = Watchdog::start(q)?;
    let pair = Pair::prepare(&manifest.image_digest, session, raw)?;
    watchdog.current()?;
    super::attempt::ready(q.attempt, q.session_generation)?;
    channel.send_reply(&HostReply::Ready {
        attempt: q.attempt,
        generation: q.session_generation,
    })?;
    Ok((pair, watchdog))
}
fn release(
    channel: &mut HostChannel<UnixStream>,
    q: &DnsRequest,
    pair: &mut Pair,
    watchdog: &mut Watchdog,
) -> Res<i64> {
    let ready = Instant::now();
    let deadline = exchange::receive_start(channel, q)?;
    if ready.elapsed() >= Duration::from_millis(1000) {
        return Err(Fail::State("live_effect_start_refused"));
    }
    watchdog.current()?;
    super::attempt::start(q.attempt, q.session_generation, deadline, || {
        pair.start(deadline)
    })?;
    Ok(deadline)
}
pub(super) fn dispatch(socket: UnixStream, qualified: &Qualified, raw: RawPipe) -> Res<()> {
    let mut channel = HostChannel::new(socket);
    let command = channel.command()?;
    match command {
        HostCommand::Stop {
            engagement_id,
            campaign_id,
            generation,
        } => channel.send_reply(&HostReply::Stopped {
            disposal_confirmed: super::attempt::stop(engagement_id, campaign_id, generation),
        }),
        HostCommand::StopRun { run_id } => {
            let (stop_latched, disposal_confirmed) = super::attempt::stop_run(run_id);
            channel.send_reply(&HostReply::RunStopped {
                run_id,
                stop_latched,
                disposal_confirmed,
            })
        }
        HostCommand::StopCampaign {
            engagement_id,
            campaign_id,
        } => {
            let (stop_latched, disposal_confirmed) =
                super::attempt::stop_campaign(engagement_id, campaign_id);
            channel.send_reply(&HostReply::CampaignStopped {
                engagement_id,
                campaign_id,
                stop_latched,
                disposal_confirmed,
            })
        }
        HostCommand::Qualify { .. } | HostCommand::Open { .. } => {
            acquisition(&mut channel, qualified, command, raw)
        }
        _ => channel.send_reply(&HostReply::Refused),
    }
}
fn acquisition(
    channel: &mut HostChannel<UnixStream>,
    qualified: &Qualified,
    command: HostCommand,
    raw: RawPipe,
) -> Res<()> {
    let peer = channel.peer()?;
    super::qualification::acquisition(qualified)?;
    let manifest = &qualified.manifest;
    let reply = match command {
        HostCommand::Qualify {
            binding,
            run_profile,
        } if manifest.matches(&binding, &run_profile) => HostReply::Qualified {
            qualification_ref: manifest.qualification_ref,
            manifest_sha256: qualified.manifest_sha256.clone(),
            boot_id: manifest.boot_id,
        },
        HostCommand::Open { run_id, request } => {
            return open(channel, manifest, run_id, &request, raw, &peer);
        }
        _ => HostReply::Refused,
    };
    channel.send_reply(&reply)
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/launcher_control.rs"]
mod tests;

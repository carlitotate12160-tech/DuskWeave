//! Ordinary Broker control. This channel carries only parent control and safe projections.
use crate::m1_dns::{DnsRequest, DnsResult};
use crate::m1_inference_permission::LiveBinding;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use uuid::Uuid;
pub mod authority;
#[cfg(target_os = "linux")]
mod session;
mod socket;
use crate::m1_dns::DnsBoundary;
use crate::m1_effect_session::{EffectSession, EffectStore};
pub use socket::LiveBoundary;
use std::time::{Duration, Instant};

pub const LAUNCHER: &str = "/usr/local/libexec/duskweave/dw-boundary-launcher";
pub const CONTROL_ROOT: &str = "/run/duskweave/m1";

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum HostCommand {
    Qualify {
        binding: LiveBinding,
        run_profile: String,
    },
    Open {
        run_id: Uuid,
        #[serde(deserialize_with = "request_object")]
        request: Box<DnsRequest>,
    },
    Start {
        attempt: Uuid,
        generation: u64,
        deadline_at_ms: i64,
    },
    Renew {
        attempt: Uuid,
        generation: u64,
        deadline_at_ms: i64,
    },
    Ack {
        attempt: Uuid,
        generation: u64,
    },
    StopRun {
        run_id: Uuid,
    },
    StopCampaign {
        engagement_id: Uuid,
        campaign_id: Uuid,
    },
    Stop {
        engagement_id: Uuid,
        campaign_id: Uuid,
        generation: u64,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum HostReply {
    Qualified {
        qualification_ref: Uuid,
        manifest_sha256: String,
        boot_id: Uuid,
    },
    Ready {
        attempt: Uuid,
        generation: u64,
    },
    Result {
        result: DnsResult,
    },
    Stopped {
        disposal_confirmed: bool,
    },
    RunStopped {
        run_id: Uuid,
        stop_latched: bool,
        disposal_confirmed: bool,
    },
    CampaignStopped {
        engagement_id: Uuid,
        campaign_id: Uuid,
        stop_latched: bool,
        disposal_confirmed: bool,
    },
    Refused,
}
#[cfg(target_os = "linux")]
impl HostReply {
    fn qualified_for(&self, binding: &LiveBinding) -> bool {
        let Self::Qualified {
            qualification_ref,
            manifest_sha256,
            boot_id,
        } = self
        else {
            return false;
        };
        [
            *qualification_ref == binding.qualification_ref,
            !boot_id.is_nil(),
            manifest_sha256.len() == 64,
            manifest_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        ]
        .into_iter()
        .all(|v| v)
    }
}

/// Finite typed host framing is separate from capture raw framing.
pub struct HostChannel<T> {
    io: T,
    read_frames: u8,
    written_frames: u8,
}
impl<T: Read + Write> HostChannel<T> {
    pub fn new(io: T) -> Self {
        Self {
            io,
            read_frames: 0,
            written_frames: 0,
        }
    }
    fn receive<V: serde::de::DeserializeOwned>(&mut self) -> Res<V> {
        if self.read_frames >= 16 {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        let mut header = [0; 4];
        self.io
            .read_exact(&mut header)
            .map_err(|_| Fail::State("live_host_unavailable"))?;
        let size = u32::from_be_bytes(header) as usize;
        if !(1..=16384).contains(&size) {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        let mut bytes = vec![0; size];
        self.io
            .read_exact(&mut bytes)
            .map_err(|_| Fail::State("live_host_unavailable"))?;
        if bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        let value = serde_json::from_slice(&bytes)
            .map_err(|_| Fail::State("live_host_protocol_refused"))?;
        self.read_frames += 1;
        Ok(value)
    }
    fn send<V: Serialize>(&mut self, value: &V) -> Res<()> {
        if self.written_frames >= 16 {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        let bytes =
            serde_json::to_vec(value).map_err(|_| Fail::State("live_host_protocol_refused"))?;
        if !(1..=16384).contains(&bytes.len()) {
            return Err(Fail::State("live_host_protocol_refused"));
        }
        self.io
            .write_all(&(bytes.len() as u32).to_be_bytes())
            .and_then(|_| self.io.write_all(&bytes))
            .and_then(|_| self.io.flush())
            .map_err(|_| Fail::State("live_host_unavailable"))?;
        self.written_frames += 1;
        Ok(())
    }
    pub fn command(&mut self) -> Res<HostCommand> {
        self.receive()
    }
    pub fn reply(&mut self) -> Res<HostReply> {
        self.receive()
    }
    pub fn send_command(&mut self, value: &HostCommand) -> Res<()> {
        self.send(value)
    }
    pub fn send_reply(&mut self, value: &HostReply) -> Res<()> {
        self.send(value)
    }
}

pub fn qualify(binding: &LiveBinding, run_profile: &str) -> Res<()> {
    binding.validate_for(run_profile)?;
    socket::qualify(binding, run_profile)
}

/// Coordinate one already-selected effect. A failed or uncertain attempt is never retried.
pub fn acquire(store: &mut dyn EffectStore, boundary: &mut dyn DnsBoundary, run: Uuid) -> Res<()> {
    let session = store.reserve(run)?;
    if !session.launchable() {
        return Err(Fail::State("live_effect_already_charged"));
    }
    let outcome = complete(store, boundary, &session);
    if let Err(error) = outcome {
        // Local STOP precedes every database operation, including failed bookkeeping.
        let disposed = boundary.stop().unwrap_or(false);
        let recorded = if disposed {
            store.close_disposed(&session)
        } else {
            store.stop(&session)
        };
        return recorded
            .map_err(|_| Fail::Unresolved("live_effect_stop_pending"))
            .and(Err(error));
    }
    Ok(())
}

fn complete(
    store: &mut dyn EffectStore,
    boundary: &mut dyn DnsBoundary,
    session: &EffectSession,
) -> Res<()> {
    let (charged, marker_origin) = release(store, boundary, session)?;
    let result = receive_result(store, boundary, &charged, marker_origin)?;
    store.record_result(&charged, &result)?;
    if !boundary.acknowledge(&session.request)? {
        return Err(Fail::Unresolved("live_disposal_unconfirmed"));
    }
    store.close_disposed(&charged)
}

fn release(
    store: &mut dyn EffectStore,
    boundary: &mut dyn DnsBoundary,
    session: &EffectSession,
) -> Res<(EffectSession, Instant)> {
    let q = &session.request;
    boundary.ready(q)?;
    let marker_origin = Instant::now();
    store.may_start(session)?;
    let charged = store.inspect(session.run_id)?;
    // The marker query has ended. A completed current-authority check precedes START.
    let deadline = store.current_deadline(&charged)?;
    boundary.start(q, deadline)?;
    Ok((charged, marker_origin))
}

fn receive_result(
    store: &mut dyn EffectStore,
    boundary: &mut dyn DnsBoundary,
    session: &EffectSession,
    marker_origin: Instant,
) -> Res<DnsResult> {
    let mut renewal = marker_origin + Duration::from_millis(500);
    let attempt_end = marker_origin + Duration::from_millis(2000);
    // The wire attempt ends first; its terminal result has the existing handling window.
    let end = attempt_end + Duration::from_millis(1000);
    loop {
        if let Some(result) = boundary.result()? {
            return validate_result(result, session);
        }
        if Instant::now() >= end {
            return Err(Fail::State("live_effect_deadline"));
        }
        renew(store, boundary, session, &mut renewal, attempt_end)?;
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn request_object<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Box<DnsRequest>, D::Error> {
    struct Object;
    impl<'de> serde::de::Visitor<'de> for Object {
        type Value = Box<DnsRequest>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("DNS request object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
            DnsRequest::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(Box::new)
        }
    }
    d.deserialize_map(Object)
}

fn validate_result(result: DnsResult, session: &EffectSession) -> Res<DnsResult> {
    result.validate(&session.request, session.attempt_deadline_at_ms)?;
    Ok(result)
}
fn renew(
    store: &mut dyn EffectStore,
    boundary: &mut dyn DnsBoundary,
    session: &EffectSession,
    renewal: &mut Instant,
    attempt_end: Instant,
) -> Res<()> {
    let now = Instant::now();
    if now >= attempt_end || now < *renewal {
        return Ok(());
    }
    let deadline = store.current_deadline(session)?;
    boundary.renew(&session.request, deadline)?;
    *renewal += Duration::from_millis(500);
    Ok(())
}

#[cfg(target_os = "linux")]
impl HostChannel<std::os::unix::net::UnixStream> {
    pub fn peer(&self) -> Res<std::os::unix::net::UnixStream> {
        self.io
            .try_clone()
            .map_err(|_| Fail::State("live_host_unavailable"))
    }
}

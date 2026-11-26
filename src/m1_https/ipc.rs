//! Closed Rust-Go HTTPS IPC wire contracts matching `boundary/internal/frame/https.go`.
use super::{HTTPS_METHOD_HEAD, HTTPS_PORT, VERSION_HTTPS_IPC, valid_local_id};
use crate::m1_inference_profile::{
    HTTPS_HEAD_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER,
    HTTPS_LAB_RUN_PROFILE, HTTPS_PUBLIC_RUN_PROFILE, is_global_ipv4,
};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use uuid::Uuid;

pub const HTTPS_IPC_MAX_FRAME: usize = 16384;
pub const HTTPS_IPC_MAX_CONTROL: usize = 1024;
pub const HTTPS_IPC_MAX_FRAMES: u8 = 16;
pub const HTTPS_IPC_MAX_NETWORK_BYTES: u32 = 65536;
pub const HTTPS_IPC_MAX_HEADER_BYTES: u32 = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpsPhase {
    Connect,
    Tls,
    Head,
}

impl HttpsPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Tls => "tls",
            Self::Head => "head",
        }
    }
    pub fn start_seq(self) -> u32 {
        match self {
            Self::Connect => 2,
            Self::Tls => 4,
            Self::Head => 6,
        }
    }
    pub fn ready_seq(self) -> u32 {
        match self {
            Self::Connect => 1,
            Self::Tls => 3,
            Self::Head => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpsOutcome {
    HeadResponse,
    ConnectionFailed,
    TlsFailed,
    TimedOut,
    ExecutionRefused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsLimits {
    pub network_inbound_bytes: u32,
    pub header_bytes: u32,
    pub frame_payload_bytes: u32,
    pub control_payload_bytes: u32,
    pub max_worker_frames: u32,
    pub min_tls_version: String,
    pub http_version: String,
    pub max_connects: u32,
    pub max_requests: u32,
}

impl HttpsLimits {
    pub fn fixed() -> Self {
        Self {
            network_inbound_bytes: HTTPS_IPC_MAX_NETWORK_BYTES,
            header_bytes: HTTPS_IPC_MAX_HEADER_BYTES,
            frame_payload_bytes: HTTPS_IPC_MAX_FRAME as u32,
            control_payload_bytes: HTTPS_IPC_MAX_CONTROL as u32,
            max_worker_frames: u32::from(HTTPS_IPC_MAX_FRAMES),
            min_tls_version: "1.2".into(),
            http_version: "HTTP/1.1".into(),
            max_connects: 1,
            max_requests: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsIpcRequest {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
    pub engagement_id: Uuid,
    pub campaign_id: Uuid,
    pub decision_id: Uuid,
    pub run_id: Uuid,
    pub step_index: u32,
    pub attempt_kind: String,
    pub result_id: Uuid,
    pub profile: String,
    pub execution_mode: String,
    pub run_profile: String,
    pub release_sha256: String,
    pub qualification_ref: Uuid,
    pub asset_ref: Uuid,
    pub endpoint_ref: String,
    pub hostname: String,
    pub ipv4: String,
    pub port: u16,
    pub approved_path: String,
    pub method: String,
    pub parent_claim_id: Uuid,
    pub parent_claim_revision: u32,
    pub resolver_ipv4: String,
    pub vantage_ref: Uuid,
    pub permission_id: Uuid,
    pub expected_mission_revision: u64,
    pub authority_evaluated_at_ms: i64,
    pub claim_valid_until_s: i64,
    pub episode_deadline_at_ms: i64,
    pub deadline_at_ms: i64,
    pub max_duration_ms: u32,
    pub limits: HttpsLimits,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsPhaseReady {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
    pub phase: HttpsPhase,
}

impl HttpsPhaseReady {
    pub fn new(q: &HttpsIpcRequest, phase: HttpsPhase) -> Self {
        Self {
            schema: VERSION_HTTPS_IPC.into(),
            kind: "READY".into(),
            attempt: q.attempt,
            session_generation: q.session_generation,
            sequence: phase.ready_seq(),
            phase,
        }
    }
    pub fn matches(&self, q: &HttpsIpcRequest, phase: HttpsPhase) -> bool {
        self.schema == VERSION_HTTPS_IPC
            && self.kind == "READY"
            && self.attempt == q.attempt
            && self.session_generation == q.session_generation
            && self.sequence == phase.ready_seq()
            && self.phase == phase
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsPhaseStart {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
    pub phase: HttpsPhase,
    pub deadline_at_ms: i64,
}

impl HttpsPhaseStart {
    pub fn new(q: &HttpsIpcRequest, phase: HttpsPhase, deadline_at_ms: i64) -> Self {
        Self {
            schema: VERSION_HTTPS_IPC.into(),
            kind: "START".into(),
            attempt: q.attempt,
            session_generation: q.session_generation,
            sequence: phase.start_seq(),
            phase,
            deadline_at_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsAck {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
}

impl HttpsAck {
    pub fn new(q: &HttpsIpcRequest) -> Self {
        Self {
            schema: VERSION_HTTPS_IPC.into(),
            kind: "ACK".into(),
            attempt: q.attempt,
            session_generation: q.session_generation,
            sequence: 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsSafeResult {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
    pub outcome: HttpsOutcome,
    pub observed_at_ms: i64,
    pub network_bytes: u32,
    pub header_bytes: u32,
    pub status_code: u16,
    pub tls_category: String,
    pub redirect: super::RedirectDisposition,
    #[serde(deserialize_with = "bounded_limitations")]
    pub limitations: Vec<String>,
}

fn bounded_limitations<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    struct Items;
    impl<'de> serde::de::Visitor<'de> for Items {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("bounded limitations array")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while let Some(v) = seq.next_element::<String>()? {
                if out.len() == 4 {
                    return Err(serde::de::Error::custom("limitations_limit"));
                }
                out.push(v);
            }
            Ok(out)
        }
    }
    d.deserialize_seq(Items)
}

impl HttpsIpcRequest {
    fn valid_lab_scope(&self) -> bool {
        [
            self.hostname == HTTPS_LAB_HOSTNAME,
            self.ipv4 == HTTPS_LAB_IPV4,
            self.resolver_ipv4 == HTTPS_LAB_RESOLVER,
        ]
        .into_iter()
        .all(|v| v)
    }

    fn valid_public_scope(&self) -> bool {
        [
            valid_public_hostname(&self.hostname),
            canonical_global_ipv4(&self.ipv4),
            canonical_global_ipv4(&self.resolver_ipv4),
        ]
        .into_iter()
        .all(|v| v)
    }

    fn valid_scope_binding(&self) -> bool {
        match self.run_profile.as_str() {
            HTTPS_LAB_RUN_PROFILE => self.valid_lab_scope(),
            HTTPS_PUBLIC_RUN_PROFILE => self.valid_public_scope(),
            _ => false,
        }
    }

    pub fn validate_shape(&self) -> Res<()> {
        let ids_ok = ![
            self.engagement_id,
            self.campaign_id,
            self.decision_id,
            self.run_id,
            self.attempt,
            self.result_id,
            self.qualification_ref,
            self.asset_ref,
            self.parent_claim_id,
            self.vantage_ref,
            self.permission_id,
        ]
        .into_iter()
        .any(|id| id.is_nil());
        let valid = [
            ids_ok,
            self.schema == VERSION_HTTPS_IPC,
            self.kind == "REQUEST",
            self.sequence == 0,
            self.session_generation > 0,
            (1..=4).contains(&self.step_index),
            self.attempt_kind == "head",
            self.profile == HTTPS_HEAD_PROFILE,
            self.execution_mode == "live",
            self.valid_scope_binding(),
            self.release_sha256.len() == 64
                && self
                    .release_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            valid_local_id(&self.endpoint_ref, "endpoint_") && self.endpoint_ref.len() <= 16,
            self.port == HTTPS_PORT,
            self.approved_path == "/",
            self.method == HTTPS_METHOD_HEAD,
            self.parent_claim_revision >= 1,
            self.expected_mission_revision >= 1,
            self.authority_evaluated_at_ms > 0,
            self.claim_valid_until_s > 0,
            self.episode_deadline_at_ms >= self.deadline_at_ms,
            (1..=5000).contains(&self.max_duration_ms),
            self.deadline_at_ms > self.authority_evaluated_at_ms,
            self.limits == HttpsLimits::fixed(),
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("https_request_refused"))
    }

    pub fn validate(&self, now_ms: i64) -> Res<()> {
        self.validate_shape()?;
        let Some(max_deadline) = now_ms.checked_add(i64::from(self.max_duration_ms)) else {
            return Err(Fail::Input("https_request_refused"));
        };
        let live_ok = [
            self.authority_evaluated_at_ms <= now_ms,
            self.claim_valid_until_s > now_ms / 1000,
            self.deadline_at_ms > now_ms && self.deadline_at_ms <= max_deadline,
        ];
        live_ok
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("https_request_refused"))
    }
}

fn valid_label_byte(b: &u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-'
}

fn valid_public_label(label: &str) -> bool {
    let b = label.as_bytes();
    [
        !b.is_empty(),
        b.len() <= 63,
        b.first() != Some(&b'-'),
        b.last() != Some(&b'-'),
        b.iter().all(valid_label_byte),
    ]
    .into_iter()
    .all(|v| v)
}

fn valid_public_hostname(host: &str) -> bool {
    [
        !host.is_empty(),
        host.len() <= 253,
        host.contains('.'),
        host.parse::<IpAddr>().is_err(),
        host.split('.').all(valid_public_label),
    ]
    .into_iter()
    .all(|v| v)
}

fn canonical_global_ipv4(raw: &str) -> bool {
    raw.parse::<Ipv4Addr>()
        .is_ok_and(|ip| ip.to_string() == raw && is_global_ipv4(ip))
}

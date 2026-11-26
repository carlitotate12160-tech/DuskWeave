//! Bounded length-prefixed framing and closed safe-result semantic validation for `m1_https_ipc_v1`.
use super::ipc::{
    HTTPS_IPC_MAX_CONTROL, HTTPS_IPC_MAX_FRAME, HTTPS_IPC_MAX_FRAMES, HttpsIpcRequest,
    HttpsOutcome, HttpsPhase, HttpsPhaseReady, HttpsPhaseStart, HttpsSafeResult,
};
use super::result::valid_issued_limitations_for_profile;
use super::{
    HttpsHeadResult, HttpsOutcomeKind, HttpsSourceKind, RedirectDisposition, VERSION_HEAD_RESULT,
    VERSION_HTTPS_IPC,
};
use crate::mission::{AssetRef, CampaignId, EngagementId, OperationId};
use crate::{Fail, Res};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};

impl From<HttpsOutcome> for HttpsOutcomeKind {
    fn from(v: HttpsOutcome) -> Self {
        match v {
            HttpsOutcome::HeadResponse => Self::HeadResponse,
            HttpsOutcome::ConnectionFailed => Self::ConnectionFailed,
            HttpsOutcome::TlsFailed => Self::TlsFailed,
            HttpsOutcome::TimedOut => Self::TimedOut,
            HttpsOutcome::ExecutionRefused => Self::ExecutionRefused,
        }
    }
}

pub fn encode_frame<T: Serialize>(value: &T, limit: usize) -> Res<Vec<u8>> {
    let data = serde_json::to_vec(value).map_err(|_| Fail::State("https_ipc_failure"))?;
    if data.is_empty() || data.len() > limit.min(HTTPS_IPC_MAX_FRAME) {
        return Err(Fail::State("https_ipc_failure"));
    }
    let mut framed = Vec::with_capacity(4 + data.len());
    framed.extend_from_slice(&(data.len() as u32).to_be_bytes());
    framed.extend_from_slice(&data);
    Ok(framed)
}

pub fn decode_payload<T: DeserializeOwned>(data: &[u8], limit: usize) -> Res<T> {
    if data.is_empty()
        || data.len() > limit.min(HTTPS_IPC_MAX_FRAME)
        || data.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{')
    {
        return Err(Fail::State("https_ipc_failure"));
    }
    serde_json::from_slice(data).map_err(|_| Fail::State("https_ipc_failure"))
}

pub struct HttpsFrameReader<R> {
    source: R,
    frames: u8,
}

pub struct HttpsFrameWriter<W> {
    sink: W,
    frames: u8,
}

impl<R: Read> HttpsFrameReader<R> {
    pub fn new(source: R) -> Self {
        Self { source, frames: 0 }
    }
    pub fn count(&self) -> u8 {
        self.frames
    }
    fn read<T: DeserializeOwned>(&mut self, limit: usize) -> Res<T> {
        if self.frames >= HTTPS_IPC_MAX_FRAMES {
            return Err(Fail::State("https_ipc_failure"));
        }
        let mut header = [0u8; 4];
        self.source
            .read_exact(&mut header)
            .map_err(|_| Fail::State("https_ipc_failure"))?;
        let len = u32::from_be_bytes(header) as usize;
        if !(1..=limit.min(HTTPS_IPC_MAX_FRAME)).contains(&len) {
            return Err(Fail::State("https_ipc_failure"));
        }
        let mut data = vec![0u8; len];
        self.source
            .read_exact(&mut data)
            .map_err(|_| Fail::State("https_ipc_failure"))?;
        let parsed = decode_payload(&data, limit)?;
        self.frames += 1;
        Ok(parsed)
    }
    pub fn ready(&mut self) -> Res<HttpsPhaseReady> {
        self.read(HTTPS_IPC_MAX_CONTROL)
    }
    pub fn result(&mut self) -> Res<HttpsSafeResult> {
        self.read(HTTPS_IPC_MAX_FRAME)
    }
}

impl<W: Write> HttpsFrameWriter<W> {
    pub fn new(sink: W) -> Self {
        Self { sink, frames: 0 }
    }
    pub fn count(&self) -> u8 {
        self.frames
    }
    pub fn write<T: Serialize>(&mut self, value: &T, limit: usize) -> Res<()> {
        if self.frames >= HTTPS_IPC_MAX_FRAMES {
            return Err(Fail::State("https_ipc_failure"));
        }
        let framed = encode_frame(value, limit)?;
        self.sink
            .write_all(&framed)
            .and_then(|_| self.sink.flush())
            .map_err(|_| Fail::State("https_ipc_failure"))?;
        self.frames += 1;
        Ok(())
    }
    pub fn request(&mut self, value: &HttpsIpcRequest) -> Res<()> {
        self.write(value, HTTPS_IPC_MAX_FRAME)
    }
    pub fn start(&mut self, value: &HttpsPhaseStart) -> Res<()> {
        self.write(value, HTTPS_IPC_MAX_CONTROL)
    }
}

impl HttpsSafeResult {
    fn validate_common(&self, q: &HttpsIpcRequest, granted: HttpsPhase) -> Res<i64> {
        let Some(earliest) = q.deadline_at_ms.checked_sub(i64::from(q.max_duration_ms)) else {
            return Err(Fail::Input("https_result_refused"));
        };
        let Some(handle_end) = q.deadline_at_ms.checked_add(1000) else {
            return Err(Fail::Input("https_result_refused"));
        };
        let time_cap = if self.outcome == HttpsOutcome::TimedOut {
            handle_end
        } else {
            q.deadline_at_ms
        };
        let base_ok = [
            self.schema == VERSION_HTTPS_IPC,
            self.kind == "RESULT",
            self.attempt == q.attempt,
            self.session_generation == q.session_generation,
            self.sequence == 7,
            self.observed_at_ms >= earliest,
            self.observed_at_ms <= time_cap,
            self.network_bytes <= q.limits.network_inbound_bytes,
            self.header_bytes <= q.limits.header_bytes,
            self.valid_phase_and_fields(granted),
            valid_issued_limitations_for_profile(
                &q.run_profile,
                self.outcome.into(),
                &self.limitations,
            ),
        ];
        base_ok
            .into_iter()
            .all(|v| v)
            .then_some(handle_end)
            .ok_or(Fail::Input("https_result_refused"))
    }

    pub fn validate_historical_for(&self, q: &HttpsIpcRequest, granted: HttpsPhase) -> Res<()> {
        self.validate_common(q, granted).map(|_| ())
    }

    pub fn validate_for(&self, q: &HttpsIpcRequest, granted: HttpsPhase, now_ms: i64) -> Res<()> {
        let handle_end = self.validate_common(q, granted)?;
        (now_ms <= handle_end && self.observed_at_ms <= now_ms)
            .then_some(())
            .ok_or(Fail::Input("https_result_refused"))
    }

    fn valid_non_tls_failure(&self, granted: HttpsPhase) -> bool {
        self.empty_failure_fields()
            && match self.outcome {
                HttpsOutcome::ConnectionFailed => {
                    matches!(granted, HttpsPhase::Connect | HttpsPhase::Head)
                }
                HttpsOutcome::TimedOut => true,
                HttpsOutcome::ExecutionRefused => {
                    valid_refused_subcode(granted, self.limitations.get(2).map(String::as_str))
                }
                _ => false,
            }
    }

    fn valid_phase_and_fields(&self, granted: HttpsPhase) -> bool {
        match self.outcome {
            HttpsOutcome::HeadResponse => self.valid_head_response(granted),
            HttpsOutcome::TlsFailed => {
                granted == HttpsPhase::Tls && self.valid_tls_failure_fields()
            }
            HttpsOutcome::ConnectionFailed
            | HttpsOutcome::TimedOut
            | HttpsOutcome::ExecutionRefused => self.valid_non_tls_failure(granted),
        }
    }

    fn empty_failure_fields(&self) -> bool {
        self.header_bytes == 0
            && self.status_code == 0
            && self.tls_category == "none"
            && self.redirect == RedirectDisposition::None
    }

    fn valid_head_response(&self, granted: HttpsPhase) -> bool {
        let expected_redirect = if (300..=399).contains(&self.status_code) {
            RedirectDisposition::RefusedRedirect
        } else {
            RedirectDisposition::None
        };
        granted == HttpsPhase::Head
            && self.header_bytes > 0
            && (200..=599).contains(&self.status_code)
            && self.tls_category == "none"
            && self.redirect == expected_redirect
    }

    fn expected_tls_limitation(cat: &str) -> Option<&'static str> {
        match cat {
            "unknown_root" => Some("tls_unknown_root"),
            "hostname_mismatch" => Some("tls_hostname_mismatch"),
            "expired_or_not_yet_valid" => Some("tls_expired_or_not_yet_valid"),
            "protocol" => Some("tls_protocol"),
            "other_validation" => Some("tls_other_validation"),
            _ => None,
        }
    }

    fn valid_tls_failure_fields(&self) -> bool {
        [
            self.header_bytes == 0,
            self.status_code == 0,
            self.redirect == RedirectDisposition::None,
            Self::expected_tls_limitation(&self.tls_category)
                .is_some_and(|exp| self.limitations.get(2).map(String::as_str) == Some(exp)),
        ]
        .into_iter()
        .all(std::convert::identity)
    }

    pub fn into_head_result(&self, q: &HttpsIpcRequest) -> Res<HttpsHeadResult> {
        if !valid_issued_limitations_for_profile(
            &q.run_profile,
            self.outcome.into(),
            &self.limitations,
        ) {
            return Err(Fail::Input("invalid_https_head_result"));
        }
        let perceived_at = self.observed_at_ms.div_euclid(1000);
        let result = HttpsHeadResult {
            version: VERSION_HEAD_RESULT.into(),
            engagement_id: EngagementId(q.engagement_id),
            campaign_id: CampaignId(q.campaign_id),
            run_id: q.run_id,
            decision_id: OperationId(q.decision_id),
            attempt_id: q.attempt,
            result_id: q.result_id,
            asset_ref: AssetRef(q.asset_ref),
            endpoint_ref: q.endpoint_ref.clone(),
            hostname: q.hostname.clone(),
            ipv4: q.ipv4.clone(),
            port: q.port,
            approved_path: q.approved_path.clone(),
            method: q.method.clone(),
            parent_claim_id: q.parent_claim_id,
            parent_claim_revision: q.parent_claim_revision,
            perceived_at,
            uncertainty_seconds: 1,
            outcome: self.outcome.into(),
            status_code: (self.outcome == HttpsOutcome::HeadResponse).then_some(self.status_code),
            redirect: self.redirect,
            source_kind: HttpsSourceKind::IssuedHttpsAcquisition,
            limitations: self.limitations.clone(),
        };
        result.validate()?;
        Ok(result)
    }
}

fn valid_refused_subcode(granted: HttpsPhase, subcode: Option<&str>) -> bool {
    matches!(
        (granted, subcode),
        (_, None)
            | (
                HttpsPhase::Tls,
                Some("network_byte_limit_exceeded" | "unsupported_http_response"),
            )
            | (
                HttpsPhase::Head,
                Some(
                    "http_header_limit_exceeded"
                        | "network_byte_limit_exceeded"
                        | "unsupported_http_response"
                        | "malformed_http_response",
                ),
            )
    )
}

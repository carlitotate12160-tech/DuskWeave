//! Typed HTTPS acquisition request/result outcome contracts.
use super::{
    HTTPS_METHOD_HEAD, HTTPS_PORT, VERSION_HEAD_RESULT, valid_approved_path, valid_hostname,
    valid_local_id,
};
use crate::m1_inference_profile::{HTTPS_LAB_RUN_PROFILE, HTTPS_PUBLIC_RUN_PROFILE};
use crate::mission::{AssetRef, CampaignId, EngagementId, OperationId};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

pub const LAB_SCOPE_LIMITATION: &str = "synthetic_https_lab_acquisition_not_public_live";
pub const PUBLIC_SCOPE_LIMITATION: &str =
    "public_https_single_endpoint_not_whole_host_or_objective";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpsOutcomeKind {
    HeadResponse,
    ConnectionFailed,
    TlsFailed,
    TimedOut,
    ExecutionRefused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedirectDisposition {
    None,
    RefusedRedirect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpsSourceKind {
    FixtureHttpsAcquisition,
    IssuedHttpsAcquisition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsHeadResult {
    pub approved_path: String,
    pub asset_ref: AssetRef,
    pub attempt_id: Uuid,
    pub campaign_id: CampaignId,
    pub decision_id: OperationId,
    pub endpoint_ref: String,
    pub engagement_id: EngagementId,
    pub hostname: String,
    pub ipv4: String,
    pub limitations: Vec<String>,
    pub method: String,
    pub outcome: HttpsOutcomeKind,
    pub parent_claim_id: Uuid,
    pub parent_claim_revision: u32,
    pub perceived_at: i64,
    pub port: u16,
    pub redirect: RedirectDisposition,
    pub result_id: Uuid,
    pub run_id: Uuid,
    pub source_kind: HttpsSourceKind,
    pub status_code: Option<u16>,
    pub uncertainty_seconds: u32,
    pub version: String,
}

impl HttpsHeadResult {
    pub fn validate(&self) -> Res<()> {
        let base_limitations_ok = !self.limitations.is_empty()
            && self
                .limitations
                .iter()
                .all(|s| !s.is_empty() && s.len() <= 96 && !s.contains("://") && !s.contains('\n'));
        let status_ok = self.valid_status_and_limitations(base_limitations_ok);
        let valid = [
            self.version == VERSION_HEAD_RESULT,
            !self.engagement_id.0.is_nil(),
            !self.campaign_id.0.is_nil(),
            !self.run_id.is_nil(),
            !self.decision_id.0.is_nil(),
            !self.attempt_id.is_nil(),
            !self.result_id.is_nil(),
            !self.asset_ref.0.is_nil(),
            valid_local_id(&self.endpoint_ref, "endpoint_"),
            valid_hostname(&self.hostname),
            self.ipv4
                .parse::<Ipv4Addr>()
                .is_ok_and(|ip| !ip.is_unspecified()),
            self.port == HTTPS_PORT,
            valid_approved_path(&self.approved_path),
            self.method == HTTPS_METHOD_HEAD,
            !self.parent_claim_id.is_nil(),
            self.parent_claim_revision >= 1,
            self.perceived_at > 0,
            status_ok,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("invalid_https_head_result"))
    }

    fn valid_fixture_status(&self) -> bool {
        match self.outcome {
            HttpsOutcomeKind::HeadResponse => {
                self.status_code.is_some_and(|c| (100..=599).contains(&c))
            }
            _ => self.status_code.is_none(),
        }
    }

    fn valid_issued_status(&self) -> bool {
        match self.outcome {
            HttpsOutcomeKind::HeadResponse => self
                .status_code
                .is_some_and(|c| (200..=599).contains(&c) && self.redirect == expected_redirect(c)),
            _ => self.status_code.is_none() && self.redirect == RedirectDisposition::None,
        }
    }

    fn valid_status_and_limitations(&self, base_limitations_ok: bool) -> bool {
        if !base_limitations_ok {
            return false;
        }
        if self.source_kind == HttpsSourceKind::FixtureHttpsAcquisition {
            return self.valid_fixture_status();
        }
        self.valid_issued_status()
            && valid_issued_limitations_shape(self.outcome, &self.limitations)
    }
}

fn expected_redirect(code: u16) -> RedirectDisposition {
    if (300..=399).contains(&code) {
        RedirectDisposition::RefusedRedirect
    } else {
        RedirectDisposition::None
    }
}

fn expected_outcome_limitation(outcome: HttpsOutcomeKind) -> &'static str {
    match outcome {
        HttpsOutcomeKind::HeadResponse => {
            "head_response_does_not_prove_access_or_objective_completion"
        }
        HttpsOutcomeKind::ConnectionFailed => "single_endpoint_connection_failed_not_whole_host",
        HttpsOutcomeKind::TlsFailed => "single_endpoint_tls_failed_not_whole_host",
        HttpsOutcomeKind::TimedOut => "single_endpoint_timeout_not_whole_host",
        HttpsOutcomeKind::ExecutionRefused => "single_endpoint_execution_refused_not_whole_host",
    }
}

fn valid_tls_subcode(c: &str) -> bool {
    matches!(
        c,
        "tls_unknown_root"
            | "tls_hostname_mismatch"
            | "tls_expired_or_not_yet_valid"
            | "tls_protocol"
            | "tls_other_validation"
    )
}

fn valid_exec_refused_subcode(c: &str) -> bool {
    matches!(
        c,
        "http_header_limit_exceeded"
            | "network_byte_limit_exceeded"
            | "unsupported_http_response"
            | "malformed_http_response"
    )
}

fn valid_subcode_limitation(outcome: HttpsOutcomeKind, subcode: Option<&str>) -> bool {
    match outcome {
        HttpsOutcomeKind::TlsFailed => subcode.is_some_and(valid_tls_subcode),
        HttpsOutcomeKind::ExecutionRefused => subcode.is_none_or(valid_exec_refused_subcode),
        _ => subcode.is_none(),
    }
}

pub(super) fn expected_scope_limitation(run_profile: &str) -> Option<&'static str> {
    match run_profile {
        HTTPS_LAB_RUN_PROFILE => Some(LAB_SCOPE_LIMITATION),
        HTTPS_PUBLIC_RUN_PROFILE => Some(PUBLIC_SCOPE_LIMITATION),
        _ => None,
    }
}

fn valid_issued_limitations_for_scope(
    scope: &str,
    outcome: HttpsOutcomeKind,
    limitations: &[String],
) -> bool {
    [
        (2..=3).contains(&limitations.len()),
        limitations.first().map(String::as_str) == Some(scope),
        limitations.get(1).map(String::as_str) == Some(expected_outcome_limitation(outcome)),
        valid_subcode_limitation(outcome, limitations.get(2).map(String::as_str)),
    ]
    .into_iter()
    .all(std::convert::identity)
}

pub(super) fn valid_issued_limitations_for_profile(
    run_profile: &str,
    outcome: HttpsOutcomeKind,
    limitations: &[String],
) -> bool {
    expected_scope_limitation(run_profile)
        .is_some_and(|scope| valid_issued_limitations_for_scope(scope, outcome, limitations))
}

fn valid_issued_limitations_shape(outcome: HttpsOutcomeKind, limitations: &[String]) -> bool {
    [LAB_SCOPE_LIMITATION, PUBLIC_SCOPE_LIMITATION]
        .into_iter()
        .any(|scope| valid_issued_limitations_for_scope(scope, outcome, limitations))
}

pub fn valid_issued_limitations(outcome: HttpsOutcomeKind, limitations: &[String]) -> bool {
    valid_issued_limitations_for_scope(LAB_SCOPE_LIMITATION, outcome, limitations)
}

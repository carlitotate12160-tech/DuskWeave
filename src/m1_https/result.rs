//! Typed HTTPS acquisition request/result outcome contracts.
use super::{
    HTTPS_METHOD_HEAD, HTTPS_PORT, VERSION_HEAD_RESULT, valid_approved_path, valid_hostname,
    valid_local_id,
};
use crate::mission::{AssetRef, CampaignId, EngagementId, OperationId};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

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
        let status_ok = match self.outcome {
            HttpsOutcomeKind::HeadResponse => {
                self.status_code.is_some_and(|c| (100..=599).contains(&c))
            }
            _ => self.status_code.is_none(),
        };
        let limitations_ok = !self.limitations.is_empty()
            && self
                .limitations
                .iter()
                .all(|s| !s.is_empty() && s.len() <= 96 && !s.contains("://") && !s.contains('\n'));
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
            limitations_ok,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("invalid_https_head_result"))
    }
}

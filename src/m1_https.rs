//! Typed HTTPS request/result and Broker-private endpoint binding contracts. No HTTP client.
pub mod codec;
pub mod ipc;
pub mod result;
use crate::mission::{AssetRef, CampaignId, EngagementId, OperationId};
use crate::{Fail, Res};
pub use ipc::{
    HTTPS_IPC_MAX_CONTROL, HTTPS_IPC_MAX_FRAME, HTTPS_IPC_MAX_FRAMES, HttpsAck, HttpsIpcRequest,
    HttpsLimits, HttpsOutcome, HttpsPhase, HttpsPhaseReady, HttpsPhaseStart, HttpsSafeResult,
};
pub use result::{HttpsHeadResult, HttpsOutcomeKind, HttpsSourceKind, RedirectDisposition};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::Ipv4Addr;
use uuid::Uuid;

pub const VERSION_HTTPS_IPC: &str = "m1_https_ipc_v1";
pub const VERSION_HTTPS_LOOP: &str = "m1_https_loop_v1";
pub const VERSION_ENDPOINT_MAP: &str = "m1_https_endpoint_map_v1";
pub const VERSION_HEAD_REQUEST: &str = "m1_https_head_request_v1";
pub const VERSION_HEAD_RESULT: &str = "m1_https_head_result_v1";
pub const INTENT_HTTPS_HEAD_V1: &str = "https-head-v1";
pub const INTENT_DNS_UDP: &str = "dns_udp";
pub const HTTPS_METHOD_HEAD: &str = "HEAD";
pub const HTTPS_PORT: u16 = 443;
pub const REFRESH_TTL_CAP_SECONDS: u32 = 60;
pub const MAX_ENDPOINTS_PER_HOST: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyRefresh {
    None,
    DnsOnceIfStale,
}

impl DependencyRefresh {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DnsOnceIfStale => "dns_once_if_stale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssuedAssetBinding {
    pub local_id: String,
    pub asset_ref: AssetRef,
}

/// Broker-private resolution of an opaque `endpoint_N` reference for one ContextPack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointBinding {
    pub local_id: String,
    pub asset_ref: AssetRef,
    pub hostname: String,
    pub ipv4: String,
    pub port: u16,
    pub approved_path: String,
    pub method: String,
    pub parent_claim_id: Uuid,
    pub parent_claim_revision: u32,
    pub resolver_ipv4: String,
    pub vantage_ref: Uuid,
    pub perceived_at: i64,
    pub lowest_ttl_seconds: Option<u32>,
    pub valid_until: i64,
}

impl EndpointBinding {
    pub fn validate(&self) -> Res<()> {
        let ipv4_ok = self
            .ipv4
            .parse::<Ipv4Addr>()
            .is_ok_and(|ip| !ip.is_unspecified());
        let resolver_ok = self
            .resolver_ipv4
            .parse::<Ipv4Addr>()
            .is_ok_and(|ip| !ip.is_unspecified());
        let valid = [
            valid_local_id(&self.local_id, "endpoint_"),
            !self.asset_ref.0.is_nil(),
            valid_hostname(&self.hostname),
            ipv4_ok,
            self.port == HTTPS_PORT,
            valid_approved_path(&self.approved_path),
            self.method == HTTPS_METHOD_HEAD,
            !self.parent_claim_id.is_nil(),
            self.parent_claim_revision >= 1,
            resolver_ok,
            !self.vantage_ref.is_nil(),
            self.valid_until >= self.perceived_at,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("invalid_endpoint_binding"))
    }

    pub fn effective_valid_until(&self) -> Option<i64> {
        ttl_capped_valid_until(
            self.perceived_at,
            self.lowest_ttl_seconds,
            Some(self.valid_until),
        )
    }

    pub fn is_fresh_at(&self, evaluated_at: i64) -> bool {
        self.effective_valid_until()
            .is_some_and(|until| evaluated_at >= self.perceived_at && evaluated_at < until)
    }
}

/// Write-once private endpoint map bound to one ContextPack and original decision/request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionEndpointMap {
    pub version: String,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub decision_id: OperationId,
    pub request_id: Uuid,
    pub pack_digest: String,
    pub pack_revision: u32,
    pub assets: Vec<IssuedAssetBinding>,
    pub bindings: Vec<EndpointBinding>,
}

impl DecisionEndpointMap {
    pub fn validate(&self) -> Res<()> {
        let base_ok = [
            self.version == VERSION_ENDPOINT_MAP,
            !self.engagement_id.0.is_nil(),
            !self.campaign_id.0.is_nil(),
            !self.decision_id.0.is_nil(),
            !self.request_id.is_nil(),
            is_sha256_hex(&self.pack_digest),
            self.pack_revision >= 1,
            !self.assets.is_empty(),
            self.bindings.len() <= MAX_ENDPOINTS_PER_HOST,
        ];
        if !base_ok.into_iter().all(|v| v) {
            return Err(Fail::Input("invalid_endpoint_map"));
        }
        self.validate_bindings()
    }

    fn validate_bindings(&self) -> Res<()> {
        let assets_ok = self
            .assets
            .iter()
            .enumerate()
            .all(|(i, a)| a.local_id == format!("asset_{i}") && !a.asset_ref.0.is_nil());
        if !assets_ok {
            return Err(Fail::Input("invalid_endpoint_map"));
        }
        let host = self.bindings.first().map(|b| b.hostname.as_str());
        for (i, b) in self.bindings.iter().enumerate() {
            b.validate()?;
            let dup = self.bindings[..i].iter().any(|prev| prev.ipv4 == b.ipv4);
            let ok = [
                b.local_id == format!("endpoint_{i}"),
                self.assets.iter().any(|a| a.asset_ref == b.asset_ref),
                host == Some(b.hostname.as_str()),
                !dup,
            ];
            if !ok.into_iter().all(|v| v) {
                return Err(Fail::Input("invalid_endpoint_map"));
            }
        }
        Ok(())
    }

    pub fn resolve_head<'a>(
        &'a self,
        refs: &[String],
    ) -> Res<(&'a EndpointBinding, &'a IssuedAssetBinding)> {
        self.validate()?;
        if refs
            .iter()
            .any(|r| r.parse::<Ipv4Addr>().is_ok() || r.contains("://"))
        {
            return Err(Fail::Input("https_reference_literal_rejected"));
        }
        let asset = find_single_ref(
            refs,
            "asset_",
            &self.assets,
            |a, r| a.local_id == r,
            (
                "https_reference_multiple_assets",
                "https_reference_missing_asset",
                "https_reference_unissued_asset",
            ),
        )?;
        let ep = find_single_ref(
            refs,
            "endpoint_",
            &self.bindings,
            |b, r| b.local_id == r,
            (
                "https_reference_multiple_endpoints",
                "https_reference_missing_endpoint",
                "https_reference_unissued_endpoint",
            ),
        )?;
        if ep.asset_ref != asset.asset_ref {
            return Err(Fail::Input("https_reference_asset_mismatch"));
        }
        Ok((ep, asset))
    }
}

fn find_single_ref<'a, T>(
    refs: &[String],
    prefix: &str,
    items: &'a [T],
    matches_id: impl Fn(&T, &str) -> bool,
    errs: (&'static str, &'static str, &'static str),
) -> Res<&'a T> {
    let matched: Vec<&str> = refs
        .iter()
        .filter(|r| r.starts_with(prefix))
        .map(String::as_str)
        .collect();
    match matched.as_slice() {
        [] => Err(Fail::Input(errs.1)),
        [id] => items
            .iter()
            .find(|item| matches_id(item, id))
            .ok_or(Fail::Input(errs.2)),
        _ => Err(Fail::Input(errs.0)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedHttpsHeadRequest {
    pub version: String,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub run_id: Uuid,
    pub decision_id: OperationId,
    pub attempt_id: Uuid,
    pub asset_ref: AssetRef,
    pub endpoint_ref: String,
    pub parent_claim_id: Uuid,
    pub parent_claim_revision: u32,
    pub hostname: String,
    pub ipv4: String,
    pub port: u16,
    pub approved_path: String,
    pub method: String,
    pub resolver_ipv4: String,
    pub vantage_ref: Uuid,
    pub dependency_refresh: DependencyRefresh,
    pub deadline_at_ms: i64,
}

impl AdmittedHttpsHeadRequest {
    pub fn validate(&self) -> Res<()> {
        let valid = [
            self.version == VERSION_HEAD_REQUEST,
            !self.engagement_id.0.is_nil(),
            !self.campaign_id.0.is_nil(),
            !self.run_id.is_nil(),
            !self.decision_id.0.is_nil(),
            !self.attempt_id.is_nil(),
            !self.asset_ref.0.is_nil(),
            valid_local_id(&self.endpoint_ref, "endpoint_"),
            !self.parent_claim_id.is_nil(),
            self.parent_claim_revision >= 1,
            valid_hostname(&self.hostname),
            self.ipv4
                .parse::<Ipv4Addr>()
                .is_ok_and(|ip| !ip.is_unspecified()),
            self.port == HTTPS_PORT,
            valid_approved_path(&self.approved_path),
            self.method == HTTPS_METHOD_HEAD,
            self.resolver_ipv4
                .parse::<Ipv4Addr>()
                .is_ok_and(|ip| !ip.is_unspecified()),
            !self.vantage_ref.is_nil(),
            self.deadline_at_ms > 0,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("invalid_https_head_request"))
    }
}

pub fn ttl_capped_valid_until(
    perceived_at: i64,
    lowest_ttl_seconds: Option<u32>,
    claim_valid_until: Option<i64>,
) -> Option<i64> {
    let ttl = lowest_ttl_seconds?;
    let capped = ttl.min(REFRESH_TTL_CAP_SECONDS);
    let ttl_until = perceived_at.checked_add(i64::from(capped))?;
    match claim_valid_until {
        Some(until) => Some(ttl_until.min(until)),
        None => Some(ttl_until),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push(hex_nibble(b >> 4));
        out.push(hex_nibble(b & 0x0f));
    }
    out
}

fn hex_nibble(n: u8) -> char {
    char::from(if n < 10 { b'0' + n } else { b'a' + (n - 10) })
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn valid_local_id(s: &str, prefix: &str) -> bool {
    s.strip_prefix(prefix)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

pub(crate) fn valid_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && !s.contains("://")
        && !s.contains('/')
        && s.parse::<Ipv4Addr>().is_err()
        && s.split('.')
            .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}
pub(crate) fn valid_approved_path(s: &str) -> bool {
    s.starts_with('/')
        && s.len() <= 128
        && !s.contains("..")
        && !s.contains('\r')
        && !s.contains('\n')
        && !s.contains("://")
}

#[cfg(test)]
#[path = "../tests/support/m1_https_unit.rs"]
mod tests;

//! Strict bounded intake parser and observation/material types for M1 Terrain DNS.
use crate::m1_permission::validate_query_name;
use crate::mission::{
    AssetRef, CampaignId, EngagementId, EventId, ExerciseMode, GoalRef, OperationId, OperatorRef,
};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::Ipv4Addr;
use uuid::Uuid;

pub const MAX_INPUT_BYTES: usize = 16_384;
pub const MAX_JSON_DEPTH: usize = 8;
pub const MAX_DNS_ANSWERS: usize = 4;
pub const MAX_LIMITATIONS: usize = 4;
pub const MAX_TTL_SECONDS: u32 = 86_400;
pub const MAX_UNCERTAINTY_SECONDS: u32 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainRequestVersion {
    #[serde(rename = "m1_terrain_dns_observation_v1")]
    M1TerrainDnsObservationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainSourceKind {
    #[serde(rename = "operator_reviewed_dns")]
    OperatorReviewedDns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsTransport {
    Udp,
    Tcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsQuestionType {
    #[serde(rename = "A")]
    A,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DnsQuestionClass {
    #[default]
    #[serde(rename = "IN")]
    In,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsRcode {
    Noerror,
    Nxdomain,
    Servfail,
    Refused,
    Formerr,
    Notimp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsCompleteness {
    Complete,
    Incomplete,
    Truncated,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsLimitation {
    CacheRemainingTtl,
    SingleVantageOnly,
    PartialAliasChain,
    ResolverRecursionVariance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationOutcome {
    QualifiedDnsClaim,
    IncompleteMaterial,
    TruncatedMaterial,
    NegativeOrErrorRcode,
    InsufficientEmptyAnswers,
    TtlZeroNotUsable,
    ApplicabilityConflict,
}

impl EvaluationOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QualifiedDnsClaim => "qualified_dns_claim",
            Self::IncompleteMaterial => "incomplete_material",
            Self::TruncatedMaterial => "truncated_material",
            Self::NegativeOrErrorRcode => "negative_or_error_rcode",
            Self::InsufficientEmptyAnswers => "insufficient_empty_answers",
            Self::TtlZeroNotUsable => "ttl_zero_not_usable",
            Self::ApplicabilityConflict => "applicability_conflict",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustTier {
    #[serde(rename = "tier_1")]
    Tier1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainClaimStatus {
    Observed,
    Withheld,
    Conflicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimFreshness {
    Fresh,
    Expired,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsAnswer {
    pub ipv4: String,
    pub ttl_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainObservation {
    pub observation_id: Uuid,
    pub operation_id: OperationId,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub correlation_id: Uuid,
    pub source_kind: TerrainSourceKind,
    pub vantage_ref: Uuid,
    pub resolver_ipv4: String,
    pub subject_name: String,
    pub asset_ref: Option<AssetRef>,
    pub perceived_at: i64,
    #[serde(default)]
    pub uncertainty_seconds: u32,
    pub recorded_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainDnsMaterial {
    pub material_id: Uuid,
    pub observation_id: Uuid,
    pub transport: DnsTransport,
    pub qtype: DnsQuestionType,
    #[serde(default)]
    pub qclass: DnsQuestionClass,
    pub rcode: DnsRcode,
    pub truncated: bool,
    pub completeness: DnsCompleteness,
    #[serde(default)]
    pub limitations: Vec<DnsLimitation>,
    pub answers: Vec<DnsAnswer>,
}

impl TerrainObservation {
    pub(crate) fn validate_basis(&self) -> Res<()> {
        validate_dns_hostname(&self.subject_name)?;
        canonical_ipv4(&self.resolver_ipv4)?;
        let ok = [
            self.perceived_at > 0,
            self.recorded_at >= self.perceived_at,
            self.uncertainty_seconds <= MAX_UNCERTAINTY_SECONDS,
        ];
        ok.into_iter()
            .all(|b| b)
            .then_some(())
            .ok_or(Fail::Input("invalid_bounds"))
    }
}

impl TerrainDnsMaterial {
    pub(crate) fn validate_basis(&self) -> Res<()> {
        validate_answers(&self.answers)?;
        validate_limitations(&self.limitations)
    }

    pub(crate) fn matches_request(&self, req: &TerrainDnsRequest) -> bool {
        [
            self.transport == req.transport,
            self.qtype == req.qtype,
            self.qclass == req.qclass,
            self.rcode == req.rcode,
            self.truncated == req.truncated,
            self.completeness == req.completeness,
            self.limitations.len() == req.limitations.len(),
            self.limitations.iter().all(|v| req.limitations.contains(v)),
            self.answers.len() == req.answers.len(),
            self.answers.iter().all(|v| req.answers.contains(v)),
        ]
        .into_iter()
        .all(|b| b)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainDnsRequest {
    pub version: TerrainRequestVersion,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operator_ref: OperatorRef,
    pub expected_mission_revision: i64,
    pub goal_ref: GoalRef,
    pub exercise_mode: ExerciseMode,
    pub correlation_id: Uuid,
    pub source_kind: TerrainSourceKind,
    pub vantage_ref: Uuid,
    pub resolver_ipv4: String,
    pub subject_name: String,
    #[serde(default)]
    pub asset_ref: Option<AssetRef>,
    #[serde(default)]
    pub expected_claim_revision: u32,
    pub perceived_at: i64,
    #[serde(default)]
    pub uncertainty_seconds: u32,
    pub transport: DnsTransport,
    pub qtype: DnsQuestionType,
    #[serde(default)]
    pub qclass: DnsQuestionClass,
    pub rcode: DnsRcode,
    pub truncated: bool,
    pub completeness: DnsCompleteness,
    #[serde(default)]
    pub limitations: Vec<DnsLimitation>,
    pub answers: Vec<DnsAnswer>,
}

pub fn canonical_ipv4(raw: &str) -> Res<Ipv4Addr> {
    let addr: Ipv4Addr = raw.parse().map_err(|_| Fail::Input("invalid_ipv4"))?;
    (addr.to_string() == raw)
        .then_some(addr)
        .ok_or(Fail::Input("invalid_ipv4"))
}

pub fn validate_dns_hostname(name: &str) -> Res<()> {
    validate_query_name(name)?;
    let ok = [
        name.contains('.'),
        !name.split('.').any(|l| l.starts_with("xn--")),
    ];
    ok.into_iter()
        .all(|b| b)
        .then_some(())
        .ok_or(Fail::Input("invalid_name"))
}

fn validate_answers(answers: &[DnsAnswer]) -> Res<()> {
    if answers.len() > MAX_DNS_ANSWERS {
        return Err(Fail::Input("answer_limit"));
    }
    for (idx, ans) in answers.iter().enumerate() {
        canonical_ipv4(&ans.ipv4)?;
        let bad = [
            ans.ttl_seconds > MAX_TTL_SECONDS,
            answers[..idx].iter().any(|p| p.ipv4 == ans.ipv4),
        ];
        if bad.into_iter().any(|b| b) {
            return Err(Fail::Input("invalid_answer"));
        }
    }
    Ok(())
}

fn validate_limitations(limits: &[DnsLimitation]) -> Res<()> {
    if limits.len() > MAX_LIMITATIONS {
        return Err(Fail::Input("limitation_limit"));
    }
    for (idx, lim) in limits.iter().enumerate() {
        if limits[..idx].contains(lim) {
            return Err(Fail::Input("duplicate_limitation"));
        }
    }
    Ok(())
}

fn step_str(in_str: bool, esc: bool, b: u8) -> (bool, bool) {
    match (in_str, esc, b) {
        (true, true, _) => (true, false),
        (true, false, b'\\') => (true, true),
        (true, false, b'"') => (false, false),
        (false, _, b'"') => (true, false),
        _ => (in_str, false),
    }
}

fn check_depth(raw: &[u8]) -> Res<()> {
    let (mut d, mut max, mut s, mut esc) = (0usize, 0usize, false, false);
    for &b in raw {
        if !s {
            d = match b {
                b'{' | b'[' => d + 1,
                b'}' | b']' => d.saturating_sub(1),
                _ => d,
            };
            max = max.max(d);
        }
        (s, esc) = step_str(s, esc, b);
    }
    (max <= MAX_JSON_DEPTH)
        .then_some(())
        .ok_or(Fail::Input("depth_limit"))
}

impl TerrainDnsRequest {
    pub fn validate(&self) -> Res<()> {
        let nil = [
            self.engagement_id.0.is_nil(),
            self.campaign_id.0.is_nil(),
            self.operator_ref.0.is_nil(),
            self.goal_ref.0.is_nil(),
            self.correlation_id.is_nil(),
            self.vantage_ref.is_nil(),
            self.asset_ref.is_some_and(|a| a.0.is_nil()),
        ];
        if nil.into_iter().any(|b| b) {
            return Err(Fail::Input("nil_reference"));
        }
        let ok = [
            self.expected_mission_revision > 0,
            self.perceived_at > 0,
            self.uncertainty_seconds <= MAX_UNCERTAINTY_SECONDS,
            self.expected_claim_revision <= 10_000,
        ];
        if !ok.into_iter().all(|b| b) {
            return Err(Fail::Input("invalid_bounds"));
        }
        validate_dns_hostname(&self.subject_name)?;
        canonical_ipv4(&self.resolver_ipv4)?;
        validate_limitations(&self.limitations)?;
        validate_answers(&self.answers)
    }

    pub fn request_hash(&self) -> Res<String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Fail::Input("invalid_json"))?;
        Ok(Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect())
    }
}

pub fn parse_terrain_dns_request(raw: &[u8]) -> Res<TerrainDnsRequest> {
    let bad = [
        raw.is_empty(),
        raw.len() > MAX_INPUT_BYTES,
        raw.iter().all(u8::is_ascii_whitespace),
    ];
    if bad.into_iter().any(|b| b) {
        return Err(Fail::Input("size_limit"));
    }
    check_depth(raw)?;
    let req: TerrainDnsRequest =
        serde_json::from_slice(raw).map_err(|_| Fail::Input("invalid_json"))?;
    req.validate()?;
    Ok(req)
}

pub fn read_terrain_dns_file(path: &std::path::Path) -> Res<TerrainDnsRequest> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| Fail::Input("unreadable_input"))?;
    let mut raw = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    parse_terrain_dns_request(&raw)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainIds {
    pub operation_id: OperationId,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub evaluation_id: Uuid,
    pub claim_id: Uuid,
    pub event_id: EventId,
}

fn scoped_uuid(e: EngagementId, c: CampaignId, op: OperationId, tag: &str) -> Uuid {
    let mut h = Sha256::new();
    for part in [
        b"dw-terrain-id-v1\0".as_slice(),
        e.0.as_bytes(),
        c.0.as_bytes(),
        op.0.as_bytes(),
        tag.as_bytes(),
    ] {
        h.update(part);
    }
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    Uuid::from_bytes(b)
}

impl TerrainIds {
    pub fn derive(e: EngagementId, c: CampaignId, operation_id: OperationId) -> Res<Self> {
        let ids = Self {
            operation_id,
            observation_id: scoped_uuid(e, c, operation_id, "observation"),
            material_id: scoped_uuid(e, c, operation_id, "material"),
            evaluation_id: scoped_uuid(e, c, operation_id, "evaluation"),
            claim_id: scoped_uuid(e, c, operation_id, "claim"),
            event_id: EventId(scoped_uuid(e, c, operation_id, "event")),
        };
        ids.validate()?;
        Ok(ids)
    }

    pub fn validate(&self) -> Res<()> {
        let all = [
            self.operation_id.0,
            self.observation_id,
            self.material_id,
            self.evaluation_id,
            self.claim_id,
            self.event_id.0,
        ];
        for (i, id) in all.iter().enumerate() {
            let bad = [id.is_nil(), all[..i].contains(id)];
            if bad.into_iter().any(|b| b) {
                return Err(Fail::State("terrain_identity_invalid"));
            }
        }
        Ok(())
    }
}

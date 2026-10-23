//! Bounded application lifecycle contracts; no operational aggregate or effect implementation.
use crate::m1_context_input::ContextRequest;
use crate::m1_reasoning::proposal::{Category, Proposal};
use crate::mission::AssetRef;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const FIXTURE_PROFILE: &str = "autonomous-dns-fixture-v1";
pub const PROVIDER_FIXTURE_PROFILE: &str = "autonomous-dns-provider-fixture-v1";
pub const FIXTURE_MODE: &str = "controlled_fixture";

pub fn controlled_fixture_profile(profile: &str) -> bool {
    [FIXTURE_PROFILE, PROVIDER_FIXTURE_PROFILE].contains(&profile)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunSeed {
    pub asset_ref: AssetRef,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub version: String,
    pub run_id: Uuid,
    pub run_profile: String,
    pub execution_mode: String,
    pub deadline_at_ms: i64,
    pub context: ContextRequest,
    pub seed_names: Vec<RunSeed>,
}
impl RunRequest {
    pub fn parse(raw: &[u8]) -> Res<Self> {
        if raw.len() > 16384 {
            return Err(Fail::Input("size_limit"));
        }
        let q: Self =
            serde_json::from_slice(raw).map_err(|_| Fail::Input("invalid_run_request"))?;
        q.validate()?;
        Ok(q)
    }

    pub fn validate(&self) -> Res<()> {
        self.context.validate_ingress()?;
        let valid = [
            self.version == "m1_autonomous_run_request_v1",
            !self.run_id.is_nil(),
            controlled_fixture_profile(&self.run_profile),
            self.execution_mode == FIXTURE_MODE,
            self.deadline_at_ms > 0,
            (1..=8).contains(&self.seed_names.len()),
            self.seed_names.len() == self.context.candidates.len(),
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("invalid_run_request"));
        }
        self.validate_seeds()
    }
    fn validate_seeds(&self) -> Res<()> {
        for (seed, candidate) in self.seed_names.iter().zip(&self.context.candidates) {
            crate::m1_terrain_input::validate_dns_hostname(&seed.name)?;
            if seed.asset_ref != candidate.asset_ref {
                return Err(Fail::Input("invalid_run_seeds"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    DecisionOne,
    Acquisition,
    DecisionTwo,
}
impl RunPhase {
    pub fn parse(raw: &str) -> Res<Self> {
        match raw {
            "decision_one" => Ok(Self::DecisionOne),
            "acquisition" => Ok(Self::Acquisition),
            "decision_two" => Ok(Self::DecisionTwo),
            _ => Err(Fail::Input("invalid_args")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::DecisionOne => "decision_one",
            Self::Acquisition => "acquisition",
            Self::DecisionTwo => "decision_two",
        }
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub request: RunRequest,
    pub decision_one: Uuid,
    pub decision_two: Uuid,
    pub attempt: Uuid,
    pub result: Uuid,
    pub terrain_operation: Uuid,
    pub release_sha256: String,
    pub qualification_ref: Uuid,
    pub status: String,
    pub deadline_at_ms: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NextStep {
    Dns { seed_index: usize },
    Terminal(&'static str),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCause {
    ChildError,
    ChildExit,
    ReadinessRefused,
    ReadinessDeadline,
    PhaseDeadline,
    LocalStop,
    SupervisorUnavailable,
    Unavailable,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CauseSource {
    ChildReport,
    ParentObserved,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseDiagnostic {
    pub version: String,
    pub run_id: Uuid,
    pub phase: RunPhase,
    pub cause: DiagnosticCause,
    pub cause_source: CauseSource,
    pub child_error: Option<String>,
    pub exit_code: Option<i32>,
    pub readiness_observed: bool,
    pub elapsed_ms: Option<u32>,
}
impl PhaseDiagnostic {
    pub fn validate(&self) -> Res<()> {
        let expected_source = match self.cause {
            DiagnosticCause::ChildError => CauseSource::ChildReport,
            DiagnosticCause::Unavailable => CauseSource::Unavailable,
            _ => CauseSource::ParentObserved,
        };
        let valid = [
            self.version == "m1_run_phase_diagnostic_v1",
            !self.run_id.is_nil(),
            self.cause_source == expected_source,
            self.child_error
                .as_ref()
                .is_none_or(|c| safe_child_error(c) == c),
            self.cause != DiagnosticCause::ChildError || self.child_error.is_some(),
            self.elapsed_ms.is_none_or(|ms| ms <= 150000),
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("invalid_run_diagnostic"))
    }
}

/// Closed projection at the child pipe boundary. Unknown text is discarded.
pub fn safe_child_error(raw: &str) -> &'static str {
    const CODES: &[&str] = &[
        "missing_env",
        "invalid_database_config_mode",
        "database_config_conflict",
        "missing_database_url_file",
        "database_file_platform_unqualified",
        "invalid_dsn",
        "non_loopback_host",
        "missing_credentials",
        "connect_failed",
        "unqualified_database_waits",
        "run_control_unqualified",
        "run_control_stopped",
        "run_child_refused",
        "inference_authority_unavailable",
        "run_inference_pending",
        "child_failed",
    ];
    CODES
        .iter()
        .copied()
        .find(|code| *code == raw)
        .unwrap_or("child_failed")
}
/// Only typed seed IDs and the qualified manifest can select an effect; prose supplies no parameters.
pub fn next_step(p: &Proposal, candidate_count: usize) -> Res<NextStep> {
    match p.next_intent.category {
        Category::ObserveMore => dns_selection(p, candidate_count),
        Category::Retain => Ok(NextStep::Terminal("retained")),
        Category::Defer => Ok(NextStep::Terminal("deferred")),
        Category::Stop => Ok(NextStep::Terminal("stopped")),
        _ => Ok(NextStep::Terminal("capability_gap")),
    }
}
fn dns_selection(p: &Proposal, candidate_count: usize) -> Res<NextStep> {
    if p.next_intent.view_request.is_some() {
        if p.next_intent.manifest_reference.is_some() {
            return Err(Fail::State("ambiguous_effect_intent"));
        }
        return Ok(NextStep::Terminal("owner_view_requested"));
    }
    if p.next_intent.manifest_reference.as_deref() != Some("dns_udp")
        || p.next_intent.references.len() != 1
    {
        return Ok(NextStep::Terminal("capability_gap"));
    }
    let index = p.next_intent.references[0]
        .strip_prefix("asset_")
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|i| *i < candidate_count)
        .ok_or(Fail::State("effect_seed_ineligible"))?;
    if format!("asset_{index}") != p.next_intent.references[0] {
        return Err(Fail::State("effect_seed_ineligible"));
    }
    Ok(NextStep::Dns { seed_index: index })
}

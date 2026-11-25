//! Bounded application lifecycle contracts; no operational aggregate or effect implementation.
use crate::m1_reasoning::proposal::{Category, Proposal};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub mod https_loop;
mod live;
mod request;
pub use request::{RunRequest, RunSeed};

pub const FIXTURE_PROFILE: &str = "autonomous-dns-fixture-v1";
pub const PROVIDER_FIXTURE_PROFILE: &str = "autonomous-dns-provider-fixture-v1";
pub const FIXTURE_MODE: &str = "controlled_fixture";

pub fn controlled_fixture_profile(profile: &str) -> bool {
    [
        FIXTURE_PROFILE,
        PROVIDER_FIXTURE_PROFILE,
        crate::m1_inference_profile::RUN_PROFILE,
        crate::m1_https::VERSION_HTTPS_LOOP,
    ]
    .contains(&profile)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    DecisionOne,
    Acquisition,
    DecisionTwo,
    AcquisitionOne,
    AcquisitionTwo,
    DecisionThree,
    AcquisitionThree,
    DecisionFour,
}
const PHASE_NAMES: &[(&str, RunPhase)] = &[
    ("decision_one", RunPhase::DecisionOne),
    ("acquisition", RunPhase::Acquisition),
    ("decision_two", RunPhase::DecisionTwo),
    ("acquisition_one", RunPhase::AcquisitionOne),
    ("acquisition_two", RunPhase::AcquisitionTwo),
    ("decision_three", RunPhase::DecisionThree),
    ("acquisition_three", RunPhase::AcquisitionThree),
    ("decision_four", RunPhase::DecisionFour),
];
impl RunPhase {
    pub fn parse(raw: &str) -> Res<Self> {
        PHASE_NAMES
            .iter()
            .find_map(|(n, p)| (*n == raw).then_some(*p))
            .ok_or(Fail::Input("invalid_args"))
    }
    pub fn name(self) -> &'static str {
        PHASE_NAMES
            .iter()
            .find_map(|(n, p)| (*p == self).then_some(*n))
            .unwrap_or("decision_one")
    }
    pub fn is_acquisition(self) -> bool {
        matches!(
            self,
            Self::Acquisition
                | Self::AcquisitionOne
                | Self::AcquisitionTwo
                | Self::AcquisitionThree
        )
    }
    pub fn is_decision(self) -> bool {
        !self.is_acquisition()
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_guard"
    )]
    pub reported_guard: Option<crate::m1_reasoning::transport_control::ReportedGuard>,
}
fn present_guard<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<crate::m1_reasoning::transport_control::ReportedGuard>, D::Error> {
    crate::m1_reasoning::transport_control::ReportedGuard::deserialize(d).map(Some)
}
impl PhaseDiagnostic {
    pub fn version_for(successor: bool) -> &'static str {
        if successor {
            "m1_run_phase_diagnostic_v2"
        } else {
            "m1_run_phase_diagnostic_v1"
        }
    }
    pub fn checked_elapsed_ms(
        elapsed: std::time::Duration,
        timing: Option<&crate::m1_inference_profile::Timing>,
    ) -> Option<u32> {
        let max = timing.map_or(150000, |t| t.diagnostic_elapsed_max_ms());
        u32::try_from(elapsed.as_millis())
            .ok()
            .filter(|ms| u64::from(*ms) <= max)
    }
    fn version_elapsed_max(&self) -> Option<u32> {
        match self.version.as_str() {
            "m1_run_phase_diagnostic_v2" => Some(650000),
            "m1_run_phase_diagnostic_v1" if self.reported_guard.is_none() => Some(150000),
            _ => None,
        }
    }
    pub fn validate(&self) -> Res<()> {
        let Some(max_elapsed_ms) = self.version_elapsed_max() else {
            return Err(Fail::Input("invalid_run_diagnostic"));
        };
        let expected_source = match self.cause {
            DiagnosticCause::ChildError => CauseSource::ChildReport,
            DiagnosticCause::Unavailable => CauseSource::Unavailable,
            _ => CauseSource::ParentObserved,
        };
        let valid = [
            !self.run_id.is_nil(),
            self.cause_source == expected_source,
            self.child_error
                .as_ref()
                .is_none_or(|c| safe_child_error(c) == c),
            self.cause != DiagnosticCause::ChildError || self.child_error.is_some(),
            self.elapsed_ms.is_none_or(|ms| ms <= max_elapsed_ms),
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
    if p.next_intent.manifest_reference.as_deref() != Some("dns_udp") {
        return Ok(NextStep::Terminal("capability_gap"));
    }
    match selected_dns_seeds(&p.next_intent.references, candidate_count)?.as_slice() {
        [seed_index] => Ok(NextStep::Dns {
            seed_index: *seed_index,
        }),
        _ => Ok(NextStep::Terminal("capability_gap")),
    }
}
fn selected_dns_seeds(refs: &[String], candidate_count: usize) -> Res<Vec<usize>> {
    let mut seeds = Vec::new();
    for r in refs {
        if let Some(index) = reference_target(r, candidate_count)?
            && !seeds.contains(&index)
        {
            seeds.push(index);
        }
    }
    Ok(seeds)
}
fn is_context_only_ref(r: &str) -> bool {
    matches!(r, "mission" | "previous_decision")
        || https_loop::canonical_index(r, "terrain_").is_some()
}
fn reference_target(r: &str, candidate_count: usize) -> Res<Option<usize>> {
    if is_context_only_ref(r) {
        return Ok(None);
    }
    https_loop::canonical_index(r, "asset_")
        .filter(|i| *i < candidate_count)
        .map(Some)
        .ok_or(Fail::State("effect_seed_ineligible"))
}

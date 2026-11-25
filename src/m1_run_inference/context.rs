//! Extracted Cognition run-basis and decision context helpers for `m1_run_inference`.
use crate::{m1_reasoning_cli, m1_run_control};
use duskweave::m1_reasoning::{self as reasoning, proposal::ProposalBasis};
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::{Fail, Res};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) struct RunBasis {
    pub terrain: reasoning::postgres_terrain::TerrainBasis,
    pub control: PgRunStore,
    pub run: Uuid,
    pub local: Option<m1_run_control::Binding>,
    pub successor: bool,
    pub expected_version: &'static str,
}

impl RunBasis {
    fn current_run(&mut self) -> Result<(), &'static str> {
        if let Some(local) = &self.local {
            local
                .current()
                .map_err(|_| "current_authority_unavailable")?;
        }
        self.control
            .check(self.run)
            .map_err(|_| "current_authority_unavailable")
    }
}

impl ProposalBasis for RunBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.terrain.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
            || (self.expected_version == duskweave::m1_https::VERSION_HTTPS_LOOP
                && r == duskweave::m1_https::INTENT_HTTPS_HEAD_V1)
    }
    fn safe_explanation(&self, s: &str) -> bool {
        self.terrain.safe_explanation(s)
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.current_run()?;
        self.terrain.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.current_run()?;
        if self.expected_version == duskweave::m1_https::VERSION_HTTPS_LOOP {
            return self.terrain.recording_integrity().map(|_| ());
        }
        self.terrain.before_provider_release()
    }
    fn current_authority_for_references(&mut self, refs: &[String]) -> Result<(), &'static str> {
        self.current_run()?;
        if self.successor {
            return self.terrain.recording_references(refs);
        }
        self.terrain.current_authority_for_references(refs)
    }
    fn recording_premises(
        &mut self,
    ) -> Result<reasoning::proposal::RecordingPremises, &'static str> {
        self.current_run()?;
        self.terrain.recording_integrity()
    }
    fn expected_proposal_version(&self) -> &'static str {
        self.expected_version
    }
}

pub(super) fn expected_proposal_version(run: &RunRecord) -> &'static str {
    if run.request.https_loop() {
        duskweave::m1_https::VERSION_HTTPS_LOOP
    } else {
        "m1_inference_proposal_v1"
    }
}

/// Original active deadline plus the bounded cancellation/accounting window,
/// clamped to the run deadline; never restarted when a coordinator or repair
/// is constructed.
pub(super) fn envelope_end(active: Instant, run: &RunRecord) -> Res<Instant> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Fail::State("clock_unavailable"))?
        .as_millis();
    let remain = run.deadline_at_ms - i64::try_from(now).unwrap_or(i64::MAX);
    if remain <= 0 {
        return Err(Fail::State("run_deadline"));
    }
    Ok((active + Duration::from_millis(5000))
        .min(Instant::now() + Duration::from_millis(remain as u64)))
}

fn legacy_decision_and_predecessor(
    control: &mut PgRunStore,
    run: &RunRecord,
    phase: RunPhase,
) -> Res<(Uuid, Option<serde_json::Value>)> {
    if phase == RunPhase::DecisionOne {
        return Ok((run.decision_one, None));
    }
    Ok((
        run.decision_two,
        Some(control.predecessor(run.request.run_id)?),
    ))
}

fn https_decision_and_predecessor(
    control: &mut PgRunStore,
    run: &RunRecord,
    phase: RunPhase,
) -> Res<(Uuid, Option<serde_json::Value>)> {
    let step = control.https_step_for_phase(run.request.run_id, phase)?;
    let pred = control.https_predecessor(run.request.run_id, step.step_index as usize)?;
    Ok((step.decision_id, pred))
}

pub(super) fn decision_context(
    run: &RunRecord,
    phase: RunPhase,
) -> Res<(
    duskweave::m1_autonomous_context::AutonomousContext,
    PgRunStore,
    Uuid,
)> {
    let mut control = PgRunStore::new(m1_reasoning_cli::broker_connection()?);
    let (decision, predecessor) = if run.request.https_loop() {
        https_decision_and_predecessor(&mut control, run, phase)?
    } else {
        legacy_decision_and_predecessor(&mut control, run, phase)?
    };
    let binding = duskweave::m1_autonomous_context::RunContextBinding {
        engagement_id: run.request.context.mission.engagement_id,
        campaign_id: run.request.context.mission.campaign_id,
        run_profile: run.request.run_profile.clone(),
        execution_mode: run.request.execution_mode.clone(),
        inference_profile: run.request.inference_profile.clone(),
        release_sha256: run.release_sha256.clone(),
        qualification_ref: run.qualification_ref,
    };
    Ok((
        duskweave::m1_autonomous_context::AutonomousContext {
            binding,
            predecessor,
        },
        control,
        decision,
    ))
}

pub(super) fn recovery_decision_ids(run: &RunRecord) -> Res<Vec<Uuid>> {
    if !run.request.https_loop() {
        return Ok(vec![run.decision_one, run.decision_two]);
    }
    let mut control = PgRunStore::new(m1_reasoning_cli::broker_connection()?);
    let steps = control.https_steps(run.request.run_id)?;
    Ok(steps
        .into_iter()
        .filter(|s| s.status != "pending")
        .map(|s| s.decision_id)
        .collect())
}

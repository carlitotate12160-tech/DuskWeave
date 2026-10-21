//! Run-scoped Cognition composition. It invokes the same reservation/publication use case as native inference.
use duskweave::m1_reasoning::{self as reasoning, proposal::ProposalBasis};
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::{Fail, Res};
use std::time::{Duration, Instant};
use uuid::Uuid;

struct RunBasis {
    terrain: reasoning::postgres_terrain::TerrainBasis,
    control: PgRunStore,
    run: Uuid,
    local: Option<super::m1_run_control::Binding>,
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
        self.terrain.before_provider_release()
    }
    fn current_authority_for_references(&mut self, refs: &[String]) -> Result<(), &'static str> {
        self.current_run()?;
        self.terrain.current_authority_for_references(refs)
    }
}

pub(super) fn decide(
    run: &RunRecord,
    phase: RunPhase,
    preparation: Option<super::m1_run_control::Preparation>,
) -> Res<()> {
    let local = if run.request.run_profile == duskweave::m1_run::PROVIDER_FIXTURE_PROFILE {
        Some(super::m1_run_control::Binding::qualify(&run.request)?)
    } else {
        None
    };
    let (a, control, decision) = decision_context(run, phase)?;
    let basis_binding = local.clone();
    let basis = |terrain| RunBasis {
        terrain,
        control,
        run: run.request.run_id,
        local: basis_binding,
    };
    if run.request.run_profile == duskweave::m1_run::FIXTURE_PROFILE {
        return super::m1_reasoning_cli::autonomous_inference(
            &run.request.context,
            decision,
            &a,
            basis,
            |_, _, _| {
                drop(preparation);
                Ok((reasoning::fixture::DeterministicProvider, ()))
            },
        );
    }
    if run.request.run_profile != duskweave::m1_run::PROVIDER_FIXTURE_PROFILE {
        return Err(Fail::State("invalid_run_request"));
    }
    let binding = local.ok_or(Fail::Config("run_control_unqualified"))?;
    super::m1_reasoning_cli::autonomous_inference(
        &run.request.context,
        decision,
        &a,
        basis,
        |s, p, deadline| {
            let coordinator = super::m1_inference_control::Coordinator::start(
                super::m1_reasoning_cli::prepared_basis(s, p, deadline)?,
                binding.clone(),
                run.request.run_id,
                deadline,
                envelope_end(deadline, run)?,
            )?;
            let provider = reasoning::transport::AutonomousProvider::new(
                coordinator.permit.clone(),
                move || {
                    binding
                        .current()
                        .map_err(|_| "current_authority_unavailable")
                },
            );
            // Armed transport and the whole-decision envelope now overlap:
            // report readiness before the preparation cover ends so no
            // interval remains without a watch.
            println!(
                "{}",
                super::m1_run_control::ready_frame(run.request.run_id, phase.name())
            );
            drop(preparation);
            Ok((provider, coordinator))
        },
    )
}

/// Original active deadline plus the bounded cancellation/accounting window,
/// clamped to the run deadline; never restarted when a coordinator or repair
/// is constructed.
fn envelope_end(active: Instant, run: &RunRecord) -> Res<Instant> {
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

fn decision_context(
    run: &RunRecord,
    phase: RunPhase,
) -> Res<(
    duskweave::m1_autonomous_context::AutonomousContext,
    PgRunStore,
    Uuid,
)> {
    let decision = if phase == RunPhase::DecisionOne {
        run.decision_one
    } else {
        run.decision_two
    };
    let mut control = PgRunStore::new(super::m1_reasoning_cli::broker_connection()?);
    let predecessor = if phase == RunPhase::DecisionTwo {
        Some(control.predecessor(run.request.run_id)?)
    } else {
        None
    };
    let binding = duskweave::m1_autonomous_context::RunContextBinding {
        engagement_id: run.request.context.mission.engagement_id,
        campaign_id: run.request.context.mission.campaign_id,
        run_profile: run.request.run_profile.clone(),
        execution_mode: run.request.execution_mode.clone(),
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

pub(super) fn recover(run: &RunRecord) -> Res<()> {
    for decision in [run.decision_one, run.decision_two] {
        super::m1_reasoning_cli::autonomous_recover(&run.request.context, decision)?;
    }
    Ok(())
}

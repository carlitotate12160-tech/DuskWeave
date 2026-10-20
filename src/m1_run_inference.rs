//! Run-scoped Cognition composition. It invokes the same reservation/publication use case as native inference.
use duskweave::Res;
use duskweave::m1_reasoning::{self as reasoning, proposal::ProposalBasis};
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::postgres_m1_run::PgRunStore;
use uuid::Uuid;

struct FixtureBasis {
    terrain: reasoning::postgres_terrain::TerrainBasis,
    control: PgRunStore,
    run: Uuid,
}
impl ProposalBasis for FixtureBasis {
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
        self.control
            .check(self.run)
            .map_err(|_| "current_authority_unavailable")?;
        self.terrain.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.control
            .check(self.run)
            .map_err(|_| "current_authority_unavailable")?;
        self.terrain.before_provider_release()
    }
    fn current_authority_for_references(&mut self, refs: &[String]) -> Result<(), &'static str> {
        self.control
            .check(self.run)
            .map_err(|_| "current_authority_unavailable")?;
        self.terrain.current_authority_for_references(refs)
    }
}

pub(super) fn decide(run: &RunRecord, phase: RunPhase) -> Res<()> {
    let decision = if phase == RunPhase::DecisionOne {
        run.decision_one
    } else {
        run.decision_two
    };
    let mut control = PgRunStore::new(super::m1_reasoning_cli::broker_connection()?);
    let feedback = if phase == RunPhase::DecisionTwo {
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
    let a = duskweave::m1_autonomous_context::AutonomousContext {
        binding,
        predecessor: feedback,
    };
    super::m1_reasoning_cli::autonomous_inference(&run.request.context, decision, &a, |terrain| {
        FixtureBasis {
            terrain,
            control,
            run: run.request.run_id,
        }
    })
}

pub(super) fn recover(run: &RunRecord) -> Res<()> {
    for decision in [run.decision_one, run.decision_two] {
        super::m1_reasoning_cli::autonomous_recover(&run.request.context, decision)?;
    }
    Ok(())
}

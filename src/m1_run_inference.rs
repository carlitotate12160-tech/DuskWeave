//! Run-scoped Cognition composition. It invokes the same reservation/publication use case as native inference.
mod context;
use context::{
    RunBasis, decision_context, envelope_end, expected_proposal_version, recovery_decision_ids,
};
use duskweave::m1_reasoning::{self as reasoning, proposal::ProposalBasis};
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::{Fail, Res};
use std::time::Instant;
use uuid::Uuid;

pub(super) fn decide(
    run: &RunRecord,
    phase: RunPhase,
    preparation: Option<super::m1_run_control::Preparation>,
) -> Res<()> {
    let local = run
        .request
        .supervised_inference()
        .then(|| super::m1_run_control::Binding::qualify(&run.request))
        .transpose()?;
    let (a, control, decision) = decision_context(run, phase)?;
    let basis_binding = local.clone();
    let expected_version = expected_proposal_version(run);
    let basis = |terrain| RunBasis {
        terrain,
        control,
        run: run.request.run_id,
        local: basis_binding,
        successor: run.request.successor(),
        expected_version,
    };
    if run.request.successor() {
        return successor_decision(run, phase, preparation, local, a, decision, basis);
    }
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
    // The fixed child claimed a validated RunRequest. The remaining legacy
    // profile requires the qualified provider control binding below.
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

#[expect(
    clippy::large_enum_variant,
    reason = "The generic Network size is unknown to this lint; production retains the concrete AutonomousProvider payload."
)]
enum SelectedProvider<N = reasoning::transport::AutonomousProvider> {
    Local(reasoning::fixture::ProtocolFixture),
    Network(N),
    Sdk(reasoning::sdk_transport::SdkProvider),
}
impl<N: reasoning::inference::Provider> reasoning::inference::Provider for SelectedProvider<N> {
    fn terminal_report(
        &self,
        binding_digest: &str,
    ) -> Option<&reasoning::inference::ProviderReport> {
        match self {
            Self::Local(p) => p.terminal_report(binding_digest),
            Self::Network(p) => p.terminal_report(binding_digest),
            Self::Sdk(p) => p.terminal_report(binding_digest),
        }
    }
    fn prepare(
        &mut self,
        b: &reasoning::request::BoundCall,
        end: Instant,
    ) -> Result<(), &'static str> {
        match self {
            Self::Local(p) => p.prepare(b, end),
            Self::Network(p) => p.prepare(b, end),
            Self::Sdk(p) => p.prepare(b, end),
        }
    }
    fn call(
        &mut self,
        b: &reasoning::request::BoundCall,
        end: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<reasoning::inference::Received, &'static str> {
        match self {
            Self::Local(p) => p.call(b, end, basis, release),
            Self::Network(p) => p.call(b, end, basis, release),
            Self::Sdk(p) => p.call(b, end, basis, release),
        }
    }
}

fn successor_decision<B: ProposalBasis>(
    run: &RunRecord,
    phase: RunPhase,
    preparation: Option<super::m1_run_control::Preparation>,
    local: Option<super::m1_run_control::Binding>,
    a: duskweave::m1_autonomous_context::AutonomousContext,
    decision: Uuid,
    basis: impl FnOnce(reasoning::postgres_terrain::TerrainBasis) -> B,
) -> Res<()> {
    let profile = duskweave::m1_inference_profile::lookup(
        run.request
            .inference_profile
            .as_deref()
            .ok_or(Fail::Input("invalid_run_request"))?,
    )?;
    super::m1_run_control::install_reporter()?;
    let binding = local.ok_or(Fail::Config("run_control_unqualified"))?;
    let build_provider = |_s: &reasoning::inference::Scope,
                          _p: &duskweave::m1_context::PreparedContext,
                          end: Instant| {
        let coordinator = super::m1_inference_control::Coordinator::start(
            super::m1_reasoning_cli::prepared_basis(_s, _p, end)?,
            binding.clone(),
            run.request.run_id,
            end,
            envelope_end(end, run)?,
        )?;
        let provider = if profile.backend == duskweave::m1_inference_profile::Backend::Genai {
            SelectedProvider::Sdk(
                reasoning::sdk_transport::SdkProvider::new(coordinator.permit.clone(), move || {
                    binding
                        .current()
                        .map_err(|_| "current_authority_unavailable")
                })
                .with_reporter(super::m1_run_control::reporter()),
            )
        } else if profile.connection.credential_environment.is_some() {
            SelectedProvider::Network(
                reasoning::transport::AutonomousProvider::new(
                    coordinator.permit.clone(),
                    move || {
                        binding
                            .current()
                            .map_err(|_| "current_authority_unavailable")
                    },
                )
                .with_reporter(super::m1_run_control::reporter()),
            )
        } else {
            local_provider(
                profile,
                coordinator.permit.clone(),
                super::m1_run_control::reporter(),
            )?
        };
        println!(
            "{}",
            super::m1_run_control::ready_frame(run.request.run_id, phase.name())
        );
        drop(preparation);
        Ok((provider, coordinator))
    };
    if run.request.https_loop() {
        return super::m1_reasoning_cli::https_loop::infer(
            &run.request.context,
            decision,
            run.request.run_id,
            &a,
            basis,
            build_provider,
        );
    }
    super::m1_reasoning_cli::autonomous_inference(
        &run.request.context,
        decision,
        &a,
        basis,
        build_provider,
    )
}

fn local_provider(
    profile: duskweave::m1_inference_profile::Profile,
    permit: reasoning::transport_control::Permit,
    reporter: Option<std::sync::mpsc::SyncSender<reasoning::transport_control::ReportedGuard>>,
) -> Res<SelectedProvider> {
    Ok(SelectedProvider::Local(
        reasoning::fixture::ProtocolFixture::controlled(profile, permit, reporter)
            .map_err(Fail::Config)?,
    ))
}

pub(super) fn recover(run: &RunRecord) -> Res<()> {
    let ids = recovery_decision_ids(run)?;
    for decision in ids {
        super::m1_reasoning_cli::autonomous_recover(&run.request.context, decision)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/m1_run_inference.rs"]
mod tests;

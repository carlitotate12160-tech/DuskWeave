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
    successor: bool,
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
}

pub(super) fn decide(
    run: &RunRecord,
    phase: RunPhase,
    preparation: Option<super::m1_run_control::Preparation>,
) -> Res<()> {
    let local = if run.request.supervised_inference() {
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
        successor: run.request.successor(),
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
}
impl<N: reasoning::inference::Provider> reasoning::inference::Provider for SelectedProvider<N> {
    fn terminal_report(
        &self,
        binding_digest: &str,
    ) -> Option<&reasoning::inference::ProviderReport> {
        match self {
            Self::Local(p) => p.terminal_report(binding_digest),
            Self::Network(p) => p.terminal_report(binding_digest),
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
    super::m1_reasoning_cli::autonomous_inference(
        &run.request.context,
        decision,
        &a,
        basis,
        |s, p, end| {
            let coordinator = super::m1_inference_control::Coordinator::start(
                super::m1_reasoning_cli::prepared_basis(s, p, end)?,
                binding.clone(),
                run.request.run_id,
                end,
                envelope_end(end, run)?,
            )?;
            let provider = if profile.connection.credential_environment.is_some() {
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
        },
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

pub(super) fn recover(run: &RunRecord) -> Res<()> {
    for decision in [run.decision_one, run.decision_two] {
        super::m1_reasoning_cli::autonomous_recover(&run.request.context, decision)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reasoning::inference::Provider;
    struct ReportedProvider {
        digest: &'static str,
        report: reasoning::inference::ProviderReport,
    }
    impl Provider for ReportedProvider {
        fn terminal_report(&self, digest: &str) -> Option<&reasoning::inference::ProviderReport> {
            (digest == self.digest).then_some(&self.report)
        }
        fn prepare(
            &mut self,
            _: &reasoning::request::BoundCall,
            _: Instant,
        ) -> Result<(), &'static str> {
            panic!("report access must not prepare a call")
        }
        fn call(
            &mut self,
            _: &reasoning::request::BoundCall,
            _: Instant,
            _: &mut dyn ProposalBasis,
            _: &mut dyn FnMut() -> Result<(), &'static str>,
        ) -> Result<reasoning::inference::Received, &'static str> {
            panic!("report access must not send a call")
        }
    }
    #[test]
    fn selected_network_provider_forwards_exact_bound_report() {
        println!(
            "selected provider layout: local={} network={} enum={}",
            std::mem::size_of::<reasoning::fixture::ProtocolFixture>(),
            std::mem::size_of::<reasoning::transport::AutonomousProvider>(),
            std::mem::size_of::<SelectedProvider>()
        );
        use reasoning::inference::{
            IncompleteReason, ProviderReport, ResponseMetadata, ResponseStatus, Usage,
            UsageAvailability,
        };
        let provider = SelectedProvider::Network(ReportedProvider {
            digest: "generation-binding",
            report: ProviderReport {
                usage: Usage {
                    input_tokens: Some(12),
                    output_tokens: Some(23),
                    reasoning_tokens: None,
                    response_bytes: 128,
                    wall_time_ms: 3,
                    requested_model: "deepseek-flash",
                    reported_model: None,
                },
                provider_response: ResponseMetadata {
                    status: ResponseStatus::Incomplete,
                    incomplete_reason: Some(IncompleteReason::MaxOutputTokens),
                    usage_availability: UsageAvailability::Partial,
                },
            },
        });
        let report = provider
            .terminal_report("generation-binding")
            .expect("SelectedProvider must forward the current network report");
        let SelectedProvider::Network(inner) = &provider else {
            panic!("network fixture")
        };
        assert!(
            std::ptr::eq(report, &inner.report),
            "forward the original borrowed report"
        );
        assert_eq!(report.usage.input_tokens, Some(12));
        assert!(provider.terminal_report("repair-binding").is_none());
    }
    #[test]
    fn selected_local_profiles_cannot_prepare_after_control_revocation() {
        for name in ["fixture-responses-v1", "fixture-chat-completions-v1"] {
            let end = Instant::now() + Duration::from_millis(124000);
            let permit = reasoning::transport_control::Permit::new(end);
            assert!(permit.publish(Instant::now(), end));
            permit.stop();
            let pack=serde_json::json!({"version":"m1_context_pack_autonomous_v2","run_profile":"autonomous-dns-fixture-v2",
                "execution_mode":"controlled_fixture","inference_profile":name,"references":[{"id":"asset_0"}],"manifests":[{"reference":"dns_udp"}]}).to_string();
            let bound = reasoning::request::bind(pack.as_bytes(), None).unwrap();
            let mut provider = local_provider(
                duskweave::m1_inference_profile::lookup(name).unwrap(),
                permit,
                None,
            )
            .unwrap();
            assert!(
                provider.prepare(&bound, end).is_err(),
                "local protocol ignored the production control permit"
            );
        }
    }
}

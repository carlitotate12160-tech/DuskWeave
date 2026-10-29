//! Owner-local regression tests.
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

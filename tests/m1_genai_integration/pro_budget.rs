//! Closed Pro-v2 resource policy and companion mirror consent/request/diagnostic contracts.
use super::consent;
use super::fixture;
use duskweave::m1_inference_profile::{
    self as profiles, Backend, FIXTURE_PRO_V2_PROFILE, LAB_RUN_PROFILE, LIVE_RUN_PROFILE,
    PRO_V2_PROFILE, PRO_V2_TIMING_PROFILE, Protocol,
};
use duskweave::m1_run::{CauseSource, DiagnosticCause, PhaseDiagnostic, RunRequest};
use serde_json::{Value, json};
use uuid::Uuid;

fn set_pro_v2_limits(p: &mut Value) {
    let op = p["operation_limits"].as_object_mut().unwrap();
    op.insert("output_tokens_per_call".into(), json!(8192));
    op.insert("request_bytes".into(), json!(16384));
    op.insert("response_bytes".into(), json!(65536));
    op.insert("soft_completion_ms".into(), json!(120000));
    op.insert("per_call_wall_time_ms".into(), json!(122000));
    op.insert("shared_wall_time_ms".into(), json!(244000));
    op.insert("decision_wall_time_ms".into(), json!(249000));
    op.insert("phase_wall_time_ms".into(), json!(254000));
    let cl = p["campaign_limits"].as_object_mut().unwrap();
    cl.insert("calls".into(), json!(4));
    cl.insert("output_token_capacity".into(), json!(32768));
    cl.insert("wall_time_ms".into(), json!(498000));
    cl.insert("request_bytes".into(), json!(65536));
    cl.insert("response_bytes".into(), json!(262144));
}

#[test]
fn pro_v2_and_mirror_consent_request_and_diagnostic_contracts() {
    for name in [PRO_V2_PROFILE, FIXTURE_PRO_V2_PROFILE] {
        let profile = profiles::lookup(name).unwrap();
        assert_eq!(profile.backend, Backend::Genai);
        assert_eq!(profile.protocol, Protocol::Responses);
        assert_eq!(profile.settings.model, "deepseek-v4-pro");
        assert_eq!(profile.settings.reasoning_effort, Some("low"));
        assert_eq!(profile.settings.output_tokens, 8192);
        assert_eq!(profile.timing.name, PRO_V2_TIMING_PROFILE);
        assert_eq!(profile.timing.parent_watchdog_ms, 650000);
    }
    assert!(profiles::live_binding(
        LIVE_RUN_PROFILE,
        "live",
        PRO_V2_PROFILE
    ));
    assert!(!profiles::live_binding(
        LAB_RUN_PROFILE,
        "live",
        PRO_V2_PROFILE
    ));
    assert!(!profiles::live_binding(
        LIVE_RUN_PROFILE,
        "live",
        FIXTURE_PRO_V2_PROFILE
    ));
    assert!(!profiles::live_binding(
        LAB_RUN_PROFILE,
        "live",
        FIXTURE_PRO_V2_PROFILE
    ));

    let mut reg = consent::registration(
        duskweave::mission::EngagementId(Uuid::from_u128(0x41)),
        duskweave::mission::CampaignId(Uuid::from_u128(0x42)),
    );
    fixture::configure_permission(&mut reg);
    reg["m1_inference_permission"]["profile"] = json!(FIXTURE_PRO_V2_PROFILE);
    reg["m1_inference_permission"]["recipient"] = json!("http://127.0.0.1:18083/responses");
    set_pro_v2_limits(&mut reg["m1_inference_permission"]);
    assert!(duskweave::input::parse_register(&serde_json::to_vec(&reg).unwrap()).is_ok());

    let mut run = fixture::run_request();
    run["inference_profile"] = json!(FIXTURE_PRO_V2_PROFILE);
    let parsed = RunRequest::parse(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert_eq!(
        parsed.timing_profile().unwrap().map(|t| t.name),
        Some(PRO_V2_TIMING_PROFILE)
    );

    let ok_diag = PhaseDiagnostic {
        version: "m1_run_phase_diagnostic_v2".into(),
        run_id: Uuid::from_u128(0x43),
        phase: duskweave::m1_run::RunPhase::DecisionOne,
        cause: DiagnosticCause::PhaseDeadline,
        cause_source: CauseSource::ParentObserved,
        child_error: None,
        exit_code: Some(124),
        readiness_observed: false,
        elapsed_ms: Some(650000),
        reported_guard: Some(
            duskweave::m1_reasoning::transport_control::ReportedGuard::CommandWatchdog,
        ),
    };
    assert!(ok_diag.validate().is_ok());
    let mut over = ok_diag.clone();
    over.elapsed_ms = Some(650001);
    assert!(over.validate().is_err());
    assert_eq!(
        PhaseDiagnostic::checked_elapsed_ms(
            std::time::Duration::from_millis(650000),
            Some(&profiles::REASONING_BOUNDED_PRO_V2)
        ),
        Some(650000)
    );
    assert_eq!(
        PhaseDiagnostic::checked_elapsed_ms(
            std::time::Duration::from_millis(650001),
            Some(&profiles::REASONING_BOUNDED_PRO_V2)
        ),
        None
    );
    assert_eq!(
        PhaseDiagnostic::checked_elapsed_ms(
            std::time::Duration::from_millis(150001),
            Some(&profiles::REASONING_BOUNDED)
        ),
        None
    );
}

#[test]
fn pro_v2_diagnostic_version_elapsed_bounds() {
    use duskweave::m1_reasoning::transport_control::ReportedGuard;
    use duskweave::m1_run::RunPhase;
    let diag =
        |version: &str, elapsed_ms: u32, reported_guard: Option<ReportedGuard>| PhaseDiagnostic {
            version: version.into(),
            run_id: Uuid::new_v4(),
            phase: RunPhase::DecisionOne,
            cause: DiagnosticCause::ChildExit,
            cause_source: CauseSource::ParentObserved,
            child_error: None,
            exit_code: Some(124),
            readiness_observed: false,
            elapsed_ms: Some(elapsed_ms),
            reported_guard,
        };
    assert!(
        diag("m1_run_phase_diagnostic_v1", 150000, None)
            .validate()
            .is_ok()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v1", 150001, None)
            .validate()
            .is_err()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v1", 200000, None)
            .validate()
            .is_err()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v1", 650000, None)
            .validate()
            .is_err()
    );
    assert!(
        diag(
            "m1_run_phase_diagnostic_v1",
            1000,
            Some(ReportedGuard::TransportHardDeadline),
        )
        .validate()
        .is_err()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v2", 150000, None)
            .validate()
            .is_ok()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v2", 200000, None)
            .validate()
            .is_ok()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v2", 650000, None)
            .validate()
            .is_ok()
    );
    assert!(
        diag(
            "m1_run_phase_diagnostic_v2",
            200000,
            Some(ReportedGuard::TransportHardDeadline),
        )
        .validate()
        .is_ok()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v2", 650001, None)
            .validate()
            .is_err()
    );
    assert!(
        diag("m1_run_phase_diagnostic_v3", 1000, None)
            .validate()
            .is_err()
    );
    assert_eq!(
        PhaseDiagnostic::checked_elapsed_ms(std::time::Duration::from_millis(150000), None),
        Some(150000)
    );
    assert_eq!(
        PhaseDiagnostic::checked_elapsed_ms(std::time::Duration::from_millis(150001), None),
        None
    );
    assert_eq!(
        PhaseDiagnostic::checked_elapsed_ms(
            std::time::Duration::from_millis(650000),
            Some(&profiles::REASONING_BOUNDED_PRO_V2)
        ),
        Some(650000)
    );
    assert_eq!(
        PhaseDiagnostic::checked_elapsed_ms(
            std::time::Duration::from_millis(650001),
            Some(&profiles::REASONING_BOUNDED_PRO_V2)
        ),
        None
    );
}

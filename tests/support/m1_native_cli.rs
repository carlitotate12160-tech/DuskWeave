//! Input rejection is exercised through the production command composition.
use super::*;

#[test]
fn invalid_input_never_constructs_provider() {
    for command in ["m1-infer", "m1-infer-inspect", "m1-infer-recover"] {
        let result = command_with::<reasoning::transport::DeepSeekProvider>(command, &[], || {
            panic!("provider factory must not be reached")
        });
        assert!(result.is_err());
    }
}

struct MockSuccessProvider;
impl reasoning::inference::Provider for MockSuccessProvider {
    fn prepare(
        &mut self,
        _: &reasoning::request::BoundCall,
        _: std::time::Instant,
    ) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &reasoning::request::BoundCall,
        _: std::time::Instant,
        _: &mut dyn reasoning::proposal::ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<reasoning::inference::Received, &'static str> {
        release()?;
        let p = reasoning::proposal::Proposal {
            version: "m1_inference_proposal_v1".into(),
            hypothesis: "mock hyp".into(),
            considered_alternative: "mock alt".into(),
            references: vec!["ref-1".into()],
            expected_observation: "mock exp".into(),
            falsifying_observation: "mock fals".into(),
            uncertainty: "unknown".into(),
            rationale: "mock safe rationale".into(),
            next_intent: reasoning::proposal::Intent {
                category: reasoning::proposal::Category::ObserveMore,
                references: vec!["ref-1".into()],
                manifest_reference: None,
                view_request: None,
            },
        };
        Ok(reasoning::inference::Received {
            accounting_confirmed: true,
            usage: reasoning::inference::Usage {
                input_tokens: Some(10),
                output_tokens: Some(20),
                reasoning_tokens: None,
                response_bytes: 100,
                wall_time_ms: 50,
                requested_model: "deepseek-flash",
                reported_model: Some("deepseek-flash".into()),
            },
            admission: reasoning::proposal::Admission::Accepted(Box::new(p)),
        })
    }
}

#[test]
fn injected_provider_lifecycle() {
    let p_json = serde_json::json!({
        "version": "m1_context_prepare_request_v1",
        "mission": {
            "version": "m1_inference_reservation_request_v1",
            "engagement_id": uuid::Uuid::from_u128(1),
            "campaign_id": uuid::Uuid::from_u128(2),
            "operator_ref": "op",
            "expected_mission_revision": 1,
            "permission_id": uuid::Uuid::from_u128(3),
            "goal_ref": "goal",
            "exercise_mode": "blind"
        },
        "candidates": [{"id": "ref-1", "kind": "dns_binding", "target": "example.com"}]
    });
    let temp = std::env::temp_dir().join("test-native-cli-injected.json");
    std::fs::write(&temp, serde_json::to_vec(&p_json).unwrap()).unwrap();
    let args = [
        "--input".to_string(),
        temp.to_str().unwrap().to_string(),
        "--operation".to_string(),
        "new".to_string(),
    ];
    let _ = command_with("m1-infer", &args, || MockSuccessProvider);
    let _ = std::fs::remove_file(temp);
}

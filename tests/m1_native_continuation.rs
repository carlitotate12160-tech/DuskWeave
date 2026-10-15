use duskweave::m1_reasoning::{
    inference::{
        Broker, Decision, DecisionStore, Provider, Received, ReleaseFailure, Reservation, Scope,
        Usage,
    },
    postgres::{Basis, PgBroker, PgDecisionStore, decode},
    proposal::{Admission, Proposal, ProposalBasis},
    request,
};
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_native_inference.rs"]
mod native;
#[path = "support/m1_inference_permission.rs"]
mod permission;

#[test]
fn postgres_basis_eligibility_and_safe_explanation() {
    let _guard = db_support::db();
    native::setup();
    let (e, c) = db_support::scope(0x9730);
    let d = Uuid::from_u128(0x9730_000d);

    let client = native::admin();
    let basis = Basis {
        client,
        scope: Scope {
            engagement: e.0,
            campaign: c.0,
            decision: d,
            mission_revision: 1,
            permission: Uuid::from_u128(0x9730_000e),
        },
        references: vec!["ref-1".into()],
        deadline: Instant::now() + Duration::from_secs(60),
    };

    assert!(basis.reference_eligible("ref-1"));
    assert!(!basis.reference_eligible("ref-2"));
    assert!(!basis.manifest_eligible("any"));

    assert!(basis.safe_explanation("Normal plain text explanation"));
    for bad in [
        "Contains sentinel",
        "Authorization: Bearer secret",
        "Has password in text",
        "Has api_key in text",
        "Has private key in text",
        "http://insecure.link",
        "https://secure.link",
        "user@example.com",
        "Contains \x00 null byte",
    ] {
        assert!(!basis.safe_explanation(bad));
    }
}

#[test]
fn postgres_decode_validation_rules() {
    let valid_decision = json!({
        "kind": "reasoning_decision",
        "occurred_at_ms": 1000,
        "recorded_at_ms": 1001,
        "source_mode": "blind",
        "provenance": {},
        "event": Uuid::from_u128(1),
        "engagement": Uuid::from_u128(2),
        "campaign": Uuid::from_u128(3),
        "decision": Uuid::from_u128(4),
        "producer": "cognition",
        "version": 1,
        "mission_revision": 1,
        "permission": Uuid::from_u128(5),
        "call_ids": [Uuid::from_u128(6)],
        "bindings": ["b1"],
        "proposal": null,
        "disposition": "validated_proposal",
        "publication_obligation": "trajectory.reasoning_history.v1"
    });

    assert!(decode(valid_decision.clone()).is_ok());

    for (k, v) in [
        ("kind", json!("wrong")),
        ("producer", json!("wrong")),
        ("version", json!(2)),
        ("source_mode", json!("invalid")),
        ("disposition", json!("nonexistent_disposition")),
        ("publication_obligation", json!("wrong")),
    ] {
        let mut bad = valid_decision.clone();
        bad[k] = v;
        assert_eq!(decode(bad).err(), Some("inference_decision_integrity"));
    }
}

#[test]
fn proposal_structural_and_semantic_rejections() {
    use duskweave::m1_reasoning::proposal;

    struct TestBasis {
        auth_err: bool,
        manifest_err: bool,
        selector_err: bool,
    }
    impl ProposalBasis for TestBasis {
        fn reference_eligible(&self, r: &str) -> bool {
            if self.selector_err && r == "bad-sel" {
                return false;
            }
            r == "ref-1"
        }
        fn manifest_eligible(&self, _: &str) -> bool {
            !self.manifest_err
        }
        fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
            if self.auth_err {
                Err("authority_revoked")
            } else {
                Ok(())
            }
        }
        fn safe_explanation(&self, text: &str) -> bool {
            !text.contains("SENTINEL")
        }
    }

    fn tb(auth: bool, mani: bool, sel: bool) -> TestBasis {
        TestBasis {
            auth_err: auth,
            manifest_err: mani,
            selector_err: sel,
        }
    }
    let mut basis = tb(false, false, false);

    let p_valid = json!({
        "version": "m1_inference_proposal_v1",
        "hypothesis": "valid hypothesis",
        "considered_alternative": "alternative",
        "references": ["ref-1"],
        "expected_observation": "expected",
        "falsifying_observation": "falsifying",
        "uncertainty": "unknown",
        "rationale": "safe explanation",
        "next_intent": {
            "category": "observe_more",
            "references": ["ref-1"],
            "manifest_reference": null,
            "view_request": null
        }
    });

    // Unexpected view request when category != observe_more
    let mut p = p_valid.clone();
    p["next_intent"]["category"] = json!("defer");
    p["next_intent"]["view_request"] = json!({"view_id":"v1","selectors":["ref-1"]});
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut basis),
        Admission::Repairable(codes) if codes.contains(&"unexpected_view_request")
    ));

    // Wrong version
    let mut p = p_valid.clone();
    p["version"] = json!("m1_wrong");
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut basis),
        Admission::Repairable(codes) if codes.contains(&"proposal_version")
    ));

    // Empty text
    let mut p = p_valid.clone();
    p["hypothesis"] = json!("   ");
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut basis),
        Admission::Repairable(codes) if codes.contains(&"proposal_text")
    ));

    // Unsupported premise (empty references when category is observe_more)
    let mut p = p_valid.clone();
    p["references"] = json!([]);
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut basis),
        Admission::Rejected("unsupported_premise")
    ));

    // Manifest ineligible
    let mut p = p_valid.clone();
    p["next_intent"]["manifest_reference"] = json!("mani-1");
    let mut bad_mani = tb(false, true, false);
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut bad_mani),
        Admission::Rejected("manifest_ineligible")
    ));

    // View selector ineligible
    let mut p = p_valid.clone();
    p["next_intent"]["view_request"] = json!({"owner":"cyber_terrain","selectors":["bad-sel"]});
    let mut bad_sel = tb(false, false, true);
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut bad_sel),
        Admission::Rejected("view_selector_ineligible")
    ));

    // Unsafe explanation
    let mut p = p_valid.clone();
    p["rationale"] = json!("SENTINEL leak");
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut basis),
        Admission::Rejected("unsafe_explanation")
    ));

    // Authority error
    let p = p_valid;
    let mut bad_auth = tb(true, false, false);
    assert!(matches!(
        proposal::admit(&serde_json::to_vec(&p).unwrap(), &mut bad_auth),
        Admission::Rejected("authority_revoked")
    ));
}

struct MockSuccessProvider;
impl Provider for MockSuccessProvider {
    fn prepare(&mut self, _: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        basis.current_authority_and_premises()?;
        Ok(Received {
            accounting_confirmed: true,
            usage: Usage {
                input_tokens: Some(10),
                output_tokens: Some(20),
                reasoning_tokens: None,
                response_bytes: 100,
                wall_time_ms: 50,
                requested_model: "deepseek-flash",
                reported_model: Some("deepseek-flash".into()),
            },
            admission: Admission::Accepted(Box::new(Proposal {
                version: "m1_inference_proposal_v1".into(),
                hypothesis: "mock hyp".into(),
                considered_alternative: "mock alt".into(),
                references: vec!["ref-1".into()],
                expected_observation: "mock exp".into(),
                falsifying_observation: "mock fals".into(),
                uncertainty: "unknown".into(),
                rationale: "mock safe rationale".into(),
                next_intent: duskweave::m1_reasoning::proposal::Intent {
                    category: duskweave::m1_reasoning::proposal::Category::ObserveMore,
                    references: vec!["ref-1".into()],
                    manifest_reference: None,
                    view_request: None,
                },
            })),
        })
    }
}

struct MockFailProvider;
impl Provider for MockFailProvider {
    fn prepare(&mut self, _: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &request::BoundCall,
        _: Instant,
        _: &mut dyn ProposalBasis,
        _: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        Err("provider_protocol")
    }
}

#[test]
fn postgres_broker_and_store_direct_lifecycle() {
    let _guard = db_support::db();
    native::setup();
    let (e, c) = db_support::scope(0x9735);
    let mut raw = permission::registration(e, c);
    let now: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    raw["starts_at"] = json!(now - 60);
    raw["ends_at"] = json!(now + 3600);
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v2");
    p["profile"] = json!("deepseek-responses-envelope32-v1");
    p["starts_at"] = json!(now - 30);
    p["ends_at"] = json!(now + 1800);
    p["operation_limits"]["json_depth"] = json!(32);
    p["field_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("admitted_repair_fields_codes"));
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut allocator, mut mission, mut history) = db_support::ports();
    let op = duskweave::registration::prepare_operation(&mut allocator).unwrap();
    duskweave::registration::register(&mut allocator, &mut mission, &mut history, op, &input)
        .unwrap();

    let decision = duskweave::registration::prepare_operation(&mut allocator)
        .unwrap()
        .0;
    let s = Scope {
        engagement: e.0,
        campaign: c.0,
        decision,
        mission_revision: 1,
        permission: Uuid::from_u128(0x31),
    };

    let req: duskweave::m1_inference_budget::ReservationRequest = serde_json::from_value(json!({
        "version": "m1_inference_reservation_request_v1",
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": raw["operator_ref"],
        "expected_mission_revision": 1,
        "permission_id": Uuid::from_u128(0x31),
        "goal_ref": raw["goal_ref"],
        "exercise_mode": "blind",
    }))
    .unwrap();

    let mut broker = PgBroker {
        client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
        mission: duskweave::postgres_mission::PgMissionStore::new(native::client(
            "DW_TEST_DATABASE_URL",
        )),
        reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
            native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
        ),
        request: req,
        deadline: Instant::now() + Duration::from_secs(60),
    };

    let pack = br#"{"references":[{"id":"ref-1"}]}"#;
    let reservation = broker.reserve(&s, pack).unwrap();
    assert_eq!(reservation.generation, 1);

    let bound = request::bind(pack, None).unwrap();
    broker
        .may_send(&s, reservation.generation_call, &bound)
        .unwrap();

    let expired_res = Reservation {
        generation: 1,
        generation_call: reservation.generation_call,
        repair_call: reservation.repair_call,
        shared_deadline: Instant::now() - Duration::from_secs(1),
    };
    let mut basis = Basis {
        client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
        scope: s.clone(),
        references: vec!["ref-1".into()],
        deadline: Instant::now() + Duration::from_secs(60),
    };
    assert!(matches!(
        broker.release_into(
            &s,
            &expired_res,
            reservation.generation_call,
            &bound,
            &mut MockSuccessProvider,
            &mut basis,
        ),
        Err(ReleaseFailure::NeverReleased("inference_deadline"))
    ));

    assert!(matches!(
        broker.release_into(
            &s,
            &reservation,
            reservation.generation_call,
            &bound,
            &mut MockFailProvider,
            &mut basis,
        ),
        Err(ReleaseFailure::MayHaveSent("provider_protocol"))
    ));

    let received = match broker.release_into(
        &s,
        &reservation,
        reservation.generation_call,
        &bound,
        &mut MockSuccessProvider,
        &mut basis,
    ) {
        Ok(r) => r,
        Err(_) => panic!("release_into failed"),
    };
    assert_eq!(received.usage.output_tokens, Some(20));

    broker
        .completed(&s, reservation.generation_call, &received.usage)
        .unwrap();

    let bound_repair = request::bind(pack, Some(&["json_structure"])).unwrap();
    broker
        .may_send(&s, reservation.repair_call, &bound_repair)
        .unwrap();
    broker
        .never_released(&s, reservation.repair_call, "never_released")
        .unwrap();
    let _ = broker.uncertain(&s, reservation.repair_call, "uncertain");
    broker.close_unused_repair(&s, &reservation).unwrap();

    let prov = broker.provenance(&s).unwrap();
    assert!(prov.is_array());

    let insp = broker.inspect(&s).unwrap();
    assert!(!insp.is_empty());

    assert_eq!(
        broker.finish(&s).err(),
        Some("inference_completion_pending")
    );

    let mut expired_basis = Basis {
        client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
        scope: s.clone(),
        references: vec!["ref-1".into()],
        deadline: Instant::now() - Duration::from_secs(1),
    };
    assert_eq!(
        expired_basis.current_authority_and_premises().err(),
        Some("inference_deadline")
    );

    let mut store = PgDecisionStore {
        client: native::client("DW_COGNITION_DATABASE_URL"),
        input_hash: "direct_lifecycle_hash".into(),
    };
    assert!(store.inspect(&s).unwrap().is_none());

    let proposal = match received.admission {
        Admission::Accepted(p) => Some(*p),
        _ => None,
    };
    let dec = Decision {
        kind: "reasoning_decision",
        occurred_at_ms: 1000,
        recorded_at_ms: 1001,
        source_mode: "blind".into(),
        provenance: json!([]),
        event: duskweave::registration::prepare_operation(&mut allocator)
            .unwrap()
            .0,
        engagement: e.0,
        campaign: c.0,
        decision,
        producer: "cognition",
        version: 1,
        mission_revision: 1,
        permission: Uuid::from_u128(0x31),
        call_ids: vec![reservation.generation_call, reservation.repair_call],
        bindings: vec![bound.digest, bound_repair.digest],
        proposal,
        disposition: "validated_proposal",
        publication_obligation: "trajectory.reasoning_history.v1",
    };
    store.publish_atomic(&dec).unwrap();
    let published = store.inspect(&s).unwrap().unwrap();
    assert_eq!(published.disposition, "validated_proposal");

    let mut history = duskweave::postgres_trajectory::PgTrajectory::new(native::client(
        "DW_TRAJECTORY_DATABASE_URL",
    ));
    use duskweave::m1_reasoning::inference::ReasoningHistory;
    history.accept_committed_source(&dec).unwrap();
    broker.finish(&s).unwrap();
}

//! Real command composition with registered opaque input and a counted provider.
use super::*;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[path = "registration_db.rs"]
mod db_support;
#[path = "m1_native_inference.rs"]
mod native;
#[path = "database_wait_proxy.rs"]
mod proxy;

#[test]
fn invalid_input_never_constructs_provider() {
    for command in ["m1-infer", "m1-infer-inspect", "m1-infer-recover"] {
        assert!(
            command_with::<reasoning::transport::DeepSeekProvider>(command, &[], || panic!(
                "provider must not be constructed"
            ))
            .is_err()
        );
    }
}

struct CountedProvider(Arc<AtomicUsize>);
impl inference::Provider for CountedProvider {
    fn prepare(
        &mut self,
        _: &reasoning::request::BoundCall,
        _: Instant,
    ) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        b: &reasoning::request::BoundCall,
        _: Instant,
        basis: &mut dyn reasoning::proposal::ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        release()?;
        self.0.fetch_add(1, Ordering::SeqCst);
        let refs = reasoning::request::reference_ids(&b.pack)?;
        assert!(!refs.is_empty());
        let proposal = json!({"version":"m1_inference_proposal_v1","hypothesis":"bounded hypothesis",
            "considered_alternative":"alternative","references":[refs[0]],"expected_observation":"expected",
            "falsifying_observation":"falsifying","uncertainty":"unknown","rationale":"bounded explanation",
            "next_intent":{"category":"observe_more","references":[refs[0]],"manifest_reference":null,"view_request":null}});
        let envelope = json!({"status":"completed","model":"deepseek-flash","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":proposal.to_string()}]}],"usage":{"input_tokens":10,"output_tokens":20}});
        reasoning::response::admit_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            100,
            Duration::ZERO,
            basis,
        )
    }
}

fn args(q: &Value, operation: &str) -> (std::path::PathBuf, Vec<String>) {
    let path = std::env::temp_dir().join(format!(
        "dw-native-cli-{}-{}.json",
        std::process::id(),
        q["mission"]["campaign_id"].as_str().unwrap()
    ));
    std::fs::write(&path, serde_json::to_vec(q).unwrap()).unwrap();
    let args = vec![
        "duskweave".into(),
        "m1-infer".into(),
        "--input".into(),
        path.to_str().unwrap().into(),
        "--operation".into(),
        operation.into(),
    ];
    (path, args)
}

#[test]
fn injected_provider_lifecycle() {
    let _guard = native::db();
    let (_, q) = native::registered(0x9810);
    let typed: ContextRequest = serde_json::from_value(q.clone()).unwrap();
    let diagnostic = broker_connection().unwrap().query_one("SELECT mission.current_inference_publication($1,$2,$3),execution.inference_capacity_snapshot($1,$2)",&[&typed.mission.engagement_id.0,&typed.mission.campaign_id.0,&Uuid::from_u128(0x9810)]);
    assert!(
        diagnostic.is_ok(),
        "restricted context query: {:?}",
        diagnostic
            .err()
            .map(|e| e.as_db_error().map(|d| d.message().to_owned()))
    );
    let (path, arguments) = args(&q, "new");
    let factories = AtomicUsize::new(0);
    let calls = Arc::new(AtomicUsize::new(0));
    let receipt = command_with("m1-infer", &arguments, || {
        factories.fetch_add(1, Ordering::SeqCst);
        CountedProvider(calls.clone())
    })
    .expect("registered native command succeeds");
    assert_eq!(
        (
            factories.load(Ordering::SeqCst),
            calls.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
    assert_eq!(
        (
            receipt.proposal_disposition,
            receipt.publication,
            receipt.history
        ),
        ("validated_proposal", "committed", "completed")
    );
    assert!(!receipt.dispatch_granted);
    let mut admin = native::admin();
    let row = admin.query_one("SELECT q.payload,(SELECT count(*) FROM cognition.reasoning_outbox o WHERE o.event=q.event),(SELECT count(*) FROM trajectory.reasoning_history h WHERE h.operation_id=q.d AND h.event_id=q.event AND h.status='accepted' AND h.contract=q.payload) FROM cognition.reasoning_decisions q WHERE q.d=$1",&[&receipt.decision]).unwrap();
    let payload: Value = row.get(0);
    assert_eq!(payload["call_ids"].as_array().unwrap().len(), 2);
    assert_eq!(payload["bindings"].as_array().unwrap().len(), 1);
    assert_eq!((row.get::<_, i64>(1), row.get::<_, i64>(2)), (1, 1));
    for command in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let (_, arguments) = args(&q, &receipt.decision.to_string());
        let r = command_with::<CountedProvider>(command, &arguments, || {
            panic!("inspection/recovery cannot construct provider")
        })
        .unwrap();
        assert_eq!(
            (r.decision, r.event, r.history),
            (receipt.decision, receipt.event, "completed")
        );
        assert!(!r.dispatch_granted);
    }
    assert_eq!(
        (
            factories.load(Ordering::SeqCst),
            calls.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
    assert_eq!(
        admin
            .query_one(
                "SELECT count(*) FROM execution.inference_closures WHERE d=$1",
                &[&receipt.decision]
            )
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    admin.execute("UPDATE trajectory.reasoning_history SET contract=jsonb_set(contract,'{occurred_at_ms}','1') WHERE operation_id=$1 AND status='accepted'",&[&receipt.decision]).unwrap();
    let (_, arguments) = args(&q, &receipt.decision.to_string());
    let conflict = command_with::<CountedProvider>("m1-infer-recover", &arguments, || {
        panic!("conflict recovery cannot construct provider")
    })
    .unwrap();
    assert_eq!(conflict.history, "anomaly");
    // Restore only the injected corruption; the verified anomaly remains dominant.
    admin.execute("UPDATE trajectory.reasoning_history h SET contract=q.payload FROM cognition.reasoning_decisions q WHERE h.operation_id=q.d AND q.d=$1 AND h.status='accepted'",&[&receipt.decision]).unwrap();
    let before: i64 = admin.query_one("SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",&[&typed.mission.engagement_id.0,&typed.mission.campaign_id.0]).unwrap().get(0);
    let (_, arguments) = args(&q, "new");
    assert!(
        command_with("m1-infer", &arguments, || {
            factories.fetch_add(1, Ordering::SeqCst);
            CountedProvider(calls.clone())
        })
        .is_err()
    );
    assert_eq!(
        (
            factories.load(Ordering::SeqCst),
            calls.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
    assert_eq!(admin.query_one("SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",&[&typed.mission.engagement_id.0,&typed.mission.campaign_id.0]).unwrap().get::<_,i64>(0),before);
    assert_eq!(admin.query_one("SELECT count(*) FROM trajectory.reasoning_history WHERE operation_id=$1 AND status='anomaly'",&[&receipt.decision]).unwrap().get::<_,i64>(0),1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn changed_candidates_cannot_claim_a_binding_before_publication() {
    let _guard = native::db();
    let (_, mut q) = native::registered(0x9811);
    let typed: ContextRequest = serde_json::from_value(q.clone()).unwrap();
    let decision = allocate("new").unwrap();
    let s = scope(&typed, decision).unwrap();
    bind_input(
        &mut broker_connection().unwrap(),
        &s,
        &identity(&typed).unwrap(),
    )
    .unwrap();
    let (_, arguments) = args(&q, &decision.to_string());
    let original = command_with::<CountedProvider>("m1-infer-inspect", &arguments, || {
        panic!("binding-only inspection cannot construct provider")
    })
    .unwrap();
    assert_eq!(original.publication, "absent_not_proof_of_no_send");
    q["candidates"][0]["priority"] = json!("low");
    let (path, arguments) = args(&q, &decision.to_string());
    let result = command_with::<CountedProvider>("m1-infer-recover", &arguments, || {
        panic!("changed input cannot construct provider")
    });
    assert!(matches!(
        result,
        Err(Fail::Conflict("inference_identity_conflict"))
    ));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_owner_qualification_refuses_siblings_and_foreign_mutation() {
    let _guard = native::db();
    assert!(broker_role(&mut native::client("DW_TEST_DATABASE_URL")).is_err());
    assert!(cognition_role(&mut native::client("DW_NATIVE_SIBLING_DATABASE_URL")).is_err());
    assert!(broker_role(&mut native::client("DW_NATIVE_SIBLING_DATABASE_URL")).is_err());
    assert!(cognition_role(&mut native::client("DW_NATIVE_FOREIGN_DATABASE_URL")).is_err());
    let mut setonly = native::client("DW_NATIVE_SETONLY_DATABASE_URL");
    let inherited: bool = setonly.query_one("SELECT has_table_privilege(session_user,(SELECT t.oid FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname='mission' AND t.relname='missions'),'INSERT')",&[]).unwrap().get(0);
    assert!(
        !inherited,
        "negative fixture reaches mutation only through SET ROLE"
    );
    assert!(cognition_role(&mut setonly).is_err());
    let mut column = native::client("DW_NATIVE_COLUMN_DATABASE_URL");
    let grants = column.query_one("SELECT has_table_privilege(session_user,'mission.missions','UPDATE'),has_any_column_privilege(session_user,'mission.missions','UPDATE')",&[]).unwrap();
    assert!(!grants.get::<_, bool>(0) && grants.get::<_, bool>(1));
    let column_rejected = broker_role(&mut column).is_err();
    let core_rejected = qualify(
        &mut native::client("DW_NATIVE_COREBAD_DATABASE_URL"),
        "dw_runtime",
    )
    .is_err();
    assert_eq!(
        (column_rejected, core_rejected),
        (true, true),
        "column-only Mission mutation and Core foreign INSERT must be rejected"
    );
    assert!(core_connection().is_ok());
    assert!(broker_role(&mut native::client("DW_INFERENCE_BROKER_DATABASE_URL")).is_ok());
    assert!(cognition_role(&mut native::client("DW_COGNITION_DATABASE_URL")).is_ok());
    assert!(trajectory_role(&mut native::client("DW_TRAJECTORY_DATABASE_URL")).is_ok());
}

#[test]
fn input_binding_commit_ack_loss_never_enters_reservation_or_provider() {
    let _guard = native::db();
    let (_, q) = native::registered(0x9812);
    let typed: ContextRequest = serde_json::from_value(q.clone()).unwrap();
    let id = allocate("new").unwrap();
    let s = scope(&typed, id).unwrap();
    let hash = identity(&typed).unwrap();
    let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
    let mut c = native::through("DW_INFERENCE_BROKER_DATABASE_URL", relay.port());
    let observed = s.clone();
    let result = std::thread::scope(|threads| {
        let worker = threads.spawn(move || bind_input(&mut c, &s, &hash));
        assert!(relay.observed(Duration::from_secs(3)));
        assert!(
            relay.consumed(Duration::from_secs(3)),
            "binding COMMIT durable before ACK loss"
        );
        assert_eq!(
            native::admin()
                .query_one(
                    "SELECT count(*) FROM execution.native_input_bindings WHERE d=$1",
                    &[&observed.decision]
                )
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        assert_eq!(
            native::admin()
                .query_one(
                    "SELECT count(*) FROM execution.inference_reservations WHERE operation_id=$1",
                    &[&observed.decision]
                )
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        drop(relay);
        worker.join().unwrap()
    });
    assert!(matches!(
        result,
        Err(Fail::Store("inference_input_binding_unknown"))
    ));
    let (path, arguments) = args(&q, &id.to_string());
    for command in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let receipt = command_with::<CountedProvider>(command, &arguments, || {
            panic!("unknown binding ACK cannot cause replay")
        })
        .unwrap();
        assert_eq!(receipt.publication, "absent_not_proof_of_no_send");
    }
    std::fs::remove_file(path).unwrap();
}

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
    assert!(
        qualify(
            &mut native::client("DW_NATIVE_TERRAINCOLUMN_DATABASE_URL"),
            "dw_m1_terrain"
        )
        .is_err()
    );
    assert!(
        qualify(
            &mut native::client("DW_NATIVE_TERRAINSETONLY_DATABASE_URL"),
            "dw_m1_terrain"
        )
        .is_err()
    );
    assert!(
        qualify(
            &mut native::client("DW_TERRAIN_DATABASE_URL"),
            "dw_m1_terrain"
        )
        .is_ok()
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

fn register_terrain_campaign(n: u128) -> (Value, Uuid) {
    use duskweave::registration;
    let (e, c) = db_support::scope(n);
    let now: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let perm_id = Uuid::from_u128(e.0.as_u128().wrapping_add(100));
    let m1_perm = json!({
        "policy_version": 1, "ct_base_domain": "example.invalid", "provider_disclosure": "crt_sh",
        "vantage_ref": Uuid::from_u128(0x31), "resolver_ipv4": "192.0.2.53",
        "discovery_rules": [{"label_suffix": "example.invalid"}],
        "contact_rules": [
            {"asset_ref": Uuid::from_u128(0x21), "rule": {"exact": "www.example.invalid"}, "priority": 1}
        ],
        "excluded_names": [{"label_suffix": "excluded.example.invalid"}], "approved_path": "/",
        "starts_at": now - 3600, "ends_at": now + 3600,
        "campaign_limits": {"episodes": 2, "provider_calls": 3, "dns_questions": 10,
            "dns_followups": 2, "tcp_connections": 5, "head_requests": 2}, "concurrency": 1
    });
    let limits = json!({
        "context_bytes": 4096, "candidates": 8, "request_bytes": 16384, "configuration_bytes": 16384,
        "response_bytes": 65536, "calls_per_decision": 2, "output_tokens_per_call": 4096,
        "decision_wall_time_ms": 20000, "connect_time_ms": 5000, "proposal_bytes": 8192,
        "repair_bytes": 2048, "text_field_bytes": 1024, "repair_error_codes": 8, "json_depth": 32,
        "collection_entries": 64, "concurrency": 1
    });
    let inf_perm = json!({
        "version": "m1_inference_permission_v2",
        "permission_id": perm_id,
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 1,
        "mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "exercise_mode": "blind",
        "recipient": "https://api.deepseek.com/responses",
        "profile": "deepseek-responses-envelope32-v1",
        "field_classes": [
            "mission_question_constraints",
            "scoped_opaque_references",
            "owner_qualified_summaries",
            "omissions_unknown",
            "capacity",
            "admitted_operator_premises",
            "admitted_repair_fields_codes"
        ],
        "starts_at": now - 30,
        "ends_at": now + 1800,
        "provider_terms_ref": Uuid::from_u128(0x32),
        "provider_terms_approved": true,
        "campaign_limits": {
            "calls": 10, "request_bytes": 100000, "response_bytes": 500000,
            "output_token_capacity": 20000, "wall_time_ms": 100000
        },
        "operation_limits": limits
    });
    let raw = json!({
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21)],
        "excluded_assets": [],
        "exercise_mode": "blind",
        "starts_at": now - 7200,
        "ends_at": now + 7200,
        "m1_permission": m1_perm,
        "m1_inference_permission": inf_perm
    });
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut a, mut m, mut h) = db_support::ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut m, &mut h, op, &input).unwrap();

    let dns_val = json!({
        "version": "m1_terrain_dns_observation_v1",
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "expected_mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "exercise_mode": "blind",
        "correlation_id": Uuid::from_u128(0x41),
        "source_kind": "operator_reviewed_dns",
        "vantage_ref": Uuid::from_u128(0x31),
        "resolver_ipv4": "192.0.2.53",
        "subject_name": "www.example.invalid",
        "asset_ref": Uuid::from_u128(0x21),
        "expected_claim_revision": 0,
        "perceived_at": now - 10,
        "uncertainty_seconds": 2,
        "transport": "udp",
        "qtype": "A",
        "qclass": "IN",
        "rcode": "noerror",
        "truncated": false,
        "completeness": "complete",
        "limitations": ["single_vantage_only"],
        "answers": [{"ipv4": "198.51.100.10", "ttl_seconds": 60}]
    });
    let dns_req = duskweave::m1_terrain_input::parse_terrain_dns_request(
        &serde_json::to_vec(&dns_val).unwrap(),
    )
    .unwrap();
    let dns_op = OperationId(
        PgAllocator::new(native::client("DW_TEST_DATABASE_URL"))
            .allocate()
            .unwrap(),
    );
    let mut m_store = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut t_store = duskweave::postgres_m1_terrain::PgTerrainStore::new(native::client(
        "DW_TERRAIN_DATABASE_URL",
    ));
    let (ev, _, _, _) = t_store
        .record_dns(&mut m_store, &dns_req, dns_op)
        .unwrap()
        .unwrap();
    let mut tj = native::client("DW_TRAJECTORY_DATABASE_URL");
    assert_eq!(
        duskweave::postgres_terrain_history::record_terrain_history(&mut tj, &ev).unwrap(),
        duskweave::trajectory::HistoryStatus::Completed
    );

    let request = json!({
        "version": "m1_context_prepare_request_v1",
        "mission": {
            "version": "m1_inference_reservation_request_v1",
            "engagement_id": e,
            "campaign_id": c,
            "operator_ref": Uuid::from_u128(0x11),
            "expected_mission_revision": 1,
            "permission_id": perm_id,
            "goal_ref": Uuid::from_u128(0x13),
            "exercise_mode": "blind"
        },
        "candidates": [{"asset_ref": Uuid::from_u128(0x21), "priority": "high"}]
    });
    (request, ev.claim.claim_id)
}

struct CountedTerrainProvider {
    calls: Arc<AtomicUsize>,
    seen_pack: Arc<std::sync::Mutex<Option<String>>>,
}

impl inference::Provider for CountedTerrainProvider {
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        let pack_str = std::str::from_utf8(&b.pack)
            .map_err(|_| "pack_utf8")?
            .to_owned();
        for forbidden in ["www.example.invalid", "192.0.2.53", "198.51.100.10"] {
            assert!(!pack_str.contains(forbidden));
        }
        *self.seen_pack.lock().unwrap() = Some(pack_str);
        let refs = reasoning::request::reference_ids(&b.pack)?;
        assert!(refs.iter().any(|r| r == "terrain_0"));
        let proposal = json!({
            "version": "m1_inference_proposal_v1",
            "hypothesis": "Terrain-qualified DNS summary supports candidate access validation",
            "considered_alternative": "Defer until additional vantage observation",
            "references": ["mission", "asset_0", "terrain_0"],
            "expected_observation": "Consistent HTTPS reachability under permitted envelope",
            "falsifying_observation": "Applicability conflict or authority withdrawal",
            "uncertainty": "Bounded single-vantage TTL window",
            "rationale": "Owner-qualified DNS summary is fresh and unconflicted",
            "next_intent": {
                "category": "observe_more",
                "references": ["asset_0", "terrain_0"],
                "manifest_reference": null,
                "view_request": null
            }
        });
        let envelope = json!({
            "status": "completed",
            "model": "deepseek-flash",
            "output": [{
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{
                    "type": "output_text",
                    "text": proposal.to_string()
                }]
            }],
            "usage": {"input_tokens": 16, "output_tokens": 24}
        });
        reasoning::response::admit_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            100,
            Duration::ZERO,
            basis,
        )
    }
}

#[test]
fn terrain_connected_command_with_lifecycle() {
    let _guard = native::db();
    let (q, claim_id) = register_terrain_campaign(0x9813);
    let typed: ContextRequest = serde_json::from_value(q.clone()).unwrap();
    let preview = m1_context::prepare(
        &mut PgContextReader::new(broker_connection().unwrap()),
        &typed,
        OperationId(Uuid::from_u128(0x0098_1399)),
    )
    .expect("prepare terrain pack via real reader");
    assert_eq!(preview.terrain_dependencies().len(), 1);
    assert_eq!(preview.terrain_dependencies()[0].claim_id, claim_id);

    let (path, arguments) = args(&q, "new");
    let factories = AtomicUsize::new(0);
    let calls = Arc::new(AtomicUsize::new(0));
    let seen_pack = Arc::new(std::sync::Mutex::new(None));
    let receipt = command_with("m1-infer", &arguments, || {
        factories.fetch_add(1, Ordering::SeqCst);
        CountedTerrainProvider {
            calls: calls.clone(),
            seen_pack: seen_pack.clone(),
        }
    })
    .expect("terrain-connected m1-infer succeeds");
    assert_eq!(
        (
            factories.load(Ordering::SeqCst),
            calls.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
    let bound_pack = seen_pack
        .lock()
        .unwrap()
        .clone()
        .expect("provider saw pack");
    assert_eq!(receipt.proposal_disposition, "validated_proposal");
    assert_eq!(receipt.publication, "committed");
    assert_eq!(receipt.history, "completed");
    assert_eq!(receipt.pack_state, "available");
    assert_eq!(receipt.pack_utf8.as_deref(), Some(bound_pack.as_str()));
    assert_eq!(
        receipt.proposal.as_ref().unwrap().references,
        vec!["mission", "asset_0", "terrain_0"]
    );
    assert!(!receipt.dispatch_granted);

    for command in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let (_, inspect_args) = args(&q, &receipt.decision.to_string());
        let r = command_with::<CountedTerrainProvider>(command, &inspect_args, || {
            panic!("inspection/recovery must never construct provider")
        })
        .unwrap();
        assert_eq!(r.decision, receipt.decision);
        assert_eq!(r.event, receipt.event);
        assert_eq!(r.pack_state, "available");
        assert_eq!(r.pack_utf8.as_deref(), Some(bound_pack.as_str()));
        assert_eq!(
            r.proposal.as_ref().unwrap().references,
            vec!["mission", "asset_0", "terrain_0"]
        );
        assert_eq!(r.history, "completed");
        assert!(!r.dispatch_granted);
    }
    let mut admin = native::admin();
    admin
        .batch_execute(
            "REVOKE SELECT ON execution.inference_calls FROM dw_m1_broker; \
             GRANT SELECT(e,c,d,state) ON execution.inference_calls TO dw_m1_broker",
        )
        .unwrap();
    let (_, inspect_args) = args(&q, &receipt.decision.to_string());
    let unavail_results: Vec<_> = ["m1-infer-inspect", "m1-infer-recover", "m1-infer"]
        .into_iter()
        .map(|command| {
            command_with::<CountedTerrainProvider>(command, &inspect_args, || {
                panic!("unavailable-pack continuation must never construct provider")
            })
        })
        .collect();
    admin
        .batch_execute(
            "REVOKE SELECT(e,c,d,state) ON execution.inference_calls FROM dw_m1_broker; \
             GRANT SELECT ON execution.inference_calls TO dw_m1_broker",
        )
        .unwrap();
    for res in unavail_results {
        let r = res.expect("unavailable-pack continuation receipt");
        assert_eq!(r.decision, receipt.decision);
        assert_eq!(r.event, receipt.event);
        assert_eq!(r.pack_state, "unavailable");
        assert!(r.pack_utf8.is_none());
        assert_eq!(
            r.proposal.as_ref().unwrap().references,
            vec!["mission", "asset_0", "terrain_0"]
        );
        assert_eq!(r.history, "completed");
        assert_eq!(r.consumption, "completed_charged_unused_repair_reserved");
        assert_eq!(r.recovery_category, "no_provider_replay");
        assert!(!r.dispatch_granted);
    }
    assert_eq!(
        (
            factories.load(Ordering::SeqCst),
            calls.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
    std::fs::remove_file(path).unwrap();
}

#[path = "m1_terrain_live_contract.rs"]
mod m1_terrain_live_contract;

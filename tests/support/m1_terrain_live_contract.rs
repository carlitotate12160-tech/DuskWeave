//! Real native command/DB/provider-to-Python contract and pre-reservation floor tests.
use super::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn register_terrain_campaign_with_context_bytes(
    n: u128,
    context_bytes: usize,
) -> (Value, Uuid, Uuid, i64) {
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
        "context_bytes": context_bytes, "candidates": 8, "request_bytes": 16384,
        "configuration_bytes": 16384, "response_bytes": 65536, "calls_per_decision": 2,
        "output_tokens_per_call": 4096, "decision_wall_time_ms": 20000, "connect_time_ms": 5000,
        "proposal_bytes": 8192, "repair_bytes": 2048, "text_field_bytes": 1024,
        "repair_error_codes": 8, "json_depth": 32, "collection_entries": 64, "concurrency": 1
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
            "mission_question_constraints", "scoped_opaque_references",
            "owner_qualified_summaries", "omissions_unknown", "capacity",
            "admitted_operator_premises", "admitted_repair_fields_codes"
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
        "engagement_id": e, "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11), "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 1, "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21)], "excluded_assets": [],
        "exercise_mode": "blind", "starts_at": now - 7200, "ends_at": now + 7200,
        "m1_permission": m1_perm, "m1_inference_permission": inf_perm
    });
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut a, mut m, mut h) = db_support::ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut m, &mut h, op, &input).unwrap();

    let dns_val = json!({
        "version": "m1_terrain_dns_observation_v1",
        "engagement_id": e, "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11), "expected_mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13), "exercise_mode": "blind",
        "correlation_id": Uuid::from_u128(0x41), "source_kind": "operator_reviewed_dns",
        "vantage_ref": Uuid::from_u128(0x31), "resolver_ipv4": "192.0.2.53",
        "subject_name": "www.example.invalid", "asset_ref": Uuid::from_u128(0x21),
        "expected_claim_revision": 0, "perceived_at": now - 10, "uncertainty_seconds": 2,
        "transport": "udp", "qtype": "A", "qclass": "IN", "rcode": "noerror",
        "truncated": false, "completeness": "complete",
        "limitations": ["cache_remaining_ttl", "single_vantage_only"],
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
            "engagement_id": e, "campaign_id": c,
            "operator_ref": Uuid::from_u128(0x11), "expected_mission_revision": 1,
            "permission_id": perm_id, "goal_ref": Uuid::from_u128(0x13),
            "exercise_mode": "blind"
        },
        "candidates": [{"asset_ref": Uuid::from_u128(0x21), "priority": "high"}]
    });
    let terrain_meta = json!({
        "operation_id": ev.operation_id.0, "event_id": ev.event_id.0,
        "observation_id": ev.observation.observation_id,
        "material_id": ev.material.material_id,
        "evaluation_id": ev.evaluation.evaluation_id,
        "claim_id": ev.claim.claim_id, "asset_ref": Uuid::from_u128(0x21),
        "vantage_ref": Uuid::from_u128(0x31), "claim_revision": 1,
        "perceived_at": now - 10, "valid_until": now + 50,
        "temporal_frontier": now - 10, "evaluated_at": now,
        "source_kind": "operator_reviewed_dns", "status": "observed",
        "freshness": "fresh", "uncertainty_seconds": 2, "ttl_seconds": 60
    });
    (
        json!({"request": request, "terrain": terrain_meta}),
        e.0,
        c.0,
        now,
    )
}

#[test]
fn command_with_under_smaller_consent_reaches_floor_with_zero_provider_or_reservation_effects() {
    let _guard = native::db();
    let (bundle, e, c, _) = register_terrain_campaign_with_context_bytes(0x9820, 3500);
    let q = &bundle["request"];
    let (path, arguments) = args(q, "new");
    let prepares = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));

    struct NeverCalledProvider {
        prepares: Arc<AtomicUsize>,
        calls: Arc<AtomicUsize>,
    }
    impl inference::Provider for NeverCalledProvider {
        fn prepare(
            &mut self,
            _: &reasoning::request::BoundCall,
            _: Instant,
        ) -> Result<(), &'static str> {
            self.prepares.fetch_add(1, Ordering::SeqCst);
            panic!(
                "provider prepare must not be called when prepare hits context_bytes_insufficient"
            );
        }
        fn call(
            &mut self,
            _: &reasoning::request::BoundCall,
            _: Instant,
            _: &mut dyn reasoning::proposal::ProposalBasis,
            _: &mut dyn FnMut() -> Result<(), &'static str>,
        ) -> Result<inference::Received, &'static str> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            panic!("provider call must not be called when prepare hits context_bytes_insufficient");
        }
    }

    let outcome = command_with("m1-infer", &arguments, || NeverCalledProvider {
        prepares: prepares.clone(),
        calls: calls.clone(),
    });
    assert!(matches!(
        outcome,
        Err(Fail::State("context_bytes_insufficient"))
    ));
    assert_eq!(
        (
            prepares.load(Ordering::SeqCst),
            calls.load(Ordering::SeqCst)
        ),
        (0, 0)
    );

    let mut admin = native::admin();
    let bindings: Vec<Uuid> = admin
        .query(
            "SELECT d FROM execution.native_input_bindings WHERE e=$1 AND c=$2",
            &[&e, &c],
        )
        .unwrap()
        .into_iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(
        bindings.len(),
        1,
        "allocated decision input binding is preserved"
    );
    let d = bindings[0];
    assert!(!d.is_nil());

    let counts = admin
        .query_one(
            "SELECT (SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2),\
                    (SELECT count(*) FROM execution.inference_calls WHERE e=$1 AND c=$2),\
                    (SELECT count(*) FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2),\
                    (SELECT count(*) FROM cognition.reasoning_outbox o JOIN cognition.reasoning_decisions q USING(event) WHERE q.e=$1 AND q.c=$2),\
                    (SELECT count(*) FROM trajectory.reasoning_history WHERE operation_id=$3),\
                    (SELECT count(*) FROM execution.inference_closures WHERE e=$1 AND c=$2),\
                    execution.inference_capacity_snapshot($1,$2)",
            &[&e, &c, &d],
        )
        .unwrap();
    for idx in 0..6 {
        assert_eq!(
            counts.get::<_, i64>(idx),
            0,
            "effect column {idx} must be 0"
        );
    }
    let snap: Value = counts.get(6);
    assert_eq!(snap["active_holds"], 0);
    assert_eq!(
        snap["charged"],
        json!({"calls": 0, "request_bytes": 0, "response_bytes": 0, "output_token_capacity": 0, "wall_time_ms": 0})
    );

    // Inspect and recover on the preserved identity never construct a provider or regenerate work.
    for cmd in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let (_, inspect_args) = args(q, &d.to_string());
        let r = command_with::<NeverCalledProvider>(cmd, &inspect_args, || {
            panic!("{cmd} must not construct provider")
        })
        .unwrap();
        assert_eq!(r.decision, d);
        assert!(r.call_ids.is_empty());
        assert_eq!(r.pack_state, "not_bound");
        assert!(r.pack_utf8.is_none());
        assert_eq!(r.publication, "absent_not_proof_of_no_send");
        assert_eq!(r.recovery_category, "no_provider_replay");
        assert!(!r.dispatch_granted);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn terrain_connected_command_with_receipt_and_sql_pass_python_verifier() {
    let _guard = native::db();
    let (bundle, e, c, _) = register_terrain_campaign_with_context_bytes(0x9821, 4096);
    let q = &bundle["request"];
    let terrain_meta = &bundle["terrain"];
    let (path, arguments) = args(q, "new");
    let calls = Arc::new(AtomicUsize::new(0));
    let seen_pack = Arc::new(std::sync::Mutex::new(None));
    let initial = command_with("m1-infer", &arguments, || CountedTerrainProvider {
        calls: calls.clone(),
        seen_pack: seen_pack.clone(),
    })
    .expect("terrain-connected m1-infer with 2 limitations succeeds at 4096");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let bound_pack = seen_pack
        .lock()
        .unwrap()
        .clone()
        .expect("provider saw pack");
    let pack_json: Value = serde_json::from_str(&bound_pack).unwrap();
    assert_eq!(
        pack_json["pending_unknown"],
        json!(["other_owner_views_not_loaded_by_profile"])
    );
    assert_eq!(initial.call_ids.len(), 2);
    assert_ne!(initial.call_ids[0], initial.call_ids[1]);

    let mut historical = Vec::new();
    for cmd in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let (_, inspect_args) = args(q, &initial.decision.to_string());
        let r = command_with::<CountedTerrainProvider>(cmd, &inspect_args, || {
            panic!("inspection/recovery must never construct provider")
        })
        .unwrap();
        assert_eq!(r.call_ids, initial.call_ids);
        historical.push(serde_json::to_value(&r).unwrap());
    }
    std::fs::remove_file(path).unwrap();

    let mut admin = native::admin();
    let d = initial.decision;
    let res_rows: Value = admin
        .query_one(
            "SELECT coalesce(json_agg(json_build_object('generation',generation_call_id,'repair',repair_call_id)),'[]'::json)\
             FROM execution.inference_reservations WHERE (engagement_id,campaign_id,operation_id)=($1,$2,$3)\
             AND record_revision=1 AND kind='reserved_not_sent'",
            &[&e, &c, &d],
        )
        .unwrap()
        .get(0);
    let call_rows: Value = admin
        .query_one(
            "SELECT coalesce(json_agg(json_build_object('call',call,'p',p)),'[]'::json)\
             FROM execution.inference_calls WHERE (e,c,d)=($1,$2,$3)",
            &[&e, &c, &d],
        )
        .unwrap()
        .get(0);
    assert_eq!(res_rows.as_array().unwrap().len(), 1);
    assert_eq!(call_rows.as_array().unwrap().len(), 1);

    let payload = json!({
        "e": e, "c": c, "d": d,
        "terrain": terrain_meta,
        "initial": serde_json::to_value(&initial).unwrap(),
        "historical": historical,
        "reservation_rows": res_rows,
        "call_rows": call_rows
    });
    let py_bin = if cfg!(windows) { "python" } else { "python3" };
    let script = r#"
import json, sys
from pathlib import Path
sys.path.insert(0, str(Path("tests/support").resolve()))
import m1_terrain_live as live

data = json.loads(sys.stdin.read(65536))
class SqlFixture:
    def sql(self, stmt):
        if "inference_reservations" in stmt and "generation_call_id" in stmt:
            return json.dumps(data["reservation_rows"])
        if "inference_calls" in stmt and "json_agg" in stmt:
            return json.dumps(data["call_rows"])
        raise AssertionError(stmt)

fx = SqlFixture()
e, c, d = data["e"], data["c"], data["d"]
reserved = live.reserved_calls(fx, e, c, d)
original, actual = live.retained_packs(fx, e, c, d, reserved)
refs = live.terrain_references(original, data["terrain"], e, c, d)
ev = live.verify_receipt(data["initial"], d, None, original, reserved, False)
for h in data["historical"]:
    ev = live.verify_receipt(h, d, ev, original, reserved, True)

bad = dict(data["initial"], call_ids=list(reversed(reserved)))
reversed_rejected = False
try:
    live.verify_receipt(bad, d, ev, original, reserved, False)
except live.QualificationFailure as exc:
    reversed_rejected = (exc.args[0] == "inference_calls_changed")

print(json.dumps({
    "reserved": reserved,
    "actual": actual,
    "refs": refs,
    "reversed_rejected": reversed_rejected
}))
"#;
    let mut child = Command::new(py_bin)
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn python verifier");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&payload).unwrap())
        .unwrap();
    let out = child.wait_with_output().expect("wait python verifier");
    assert!(out.status.success() && out.stderr.is_empty(), "{out:?}");
    let verified: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        verified["reserved"],
        json!([initial.call_ids[0], initial.call_ids[1]])
    );
    assert_eq!(verified["actual"], json!([initial.call_ids[0]]));
    assert_eq!(verified["refs"], json!(["terrain_0"]));
    assert_eq!(verified["reversed_rejected"], true);
}

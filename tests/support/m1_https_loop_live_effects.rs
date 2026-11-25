use super::*;
use duskweave::m1_autonomous_context::{AutonomousContext, RunContextBinding};
use duskweave::m1_inference_profile::{FIXTURE_PRO_V2_PROFILE, HTTPS_LAB_RESOLVER};
use duskweave::m1_reasoning::inference::{self, InferenceInput, Provider, Received, Scope};
use duskweave::m1_reasoning::postgres::{Basis, PgBroker, PgDecisionStore};
use duskweave::m1_reasoning::postgres_terrain::TerrainBasis;
use duskweave::m1_reasoning::proposal::{ProposalBasis, RecordingPremises};
use duskweave::m1_reasoning::request::{BoundCall, reference_ids};
use duskweave::m1_reasoning::response;
use duskweave::m1_run::{RunRequest, RunSeed};
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::OperationId;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_mission::{PgAllocator, PgContextReader, PgMissionStore};
use duskweave::postgres_terrain_history::record_terrain_history;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::trajectory::HistoryStatus;
use postgres::Client;
use serde_json::{Value, json};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn pg_client(var: &str) -> Client {
    std::env::var(var)
        .expect("dsn")
        .parse::<postgres::Config>()
        .unwrap()
        .connect(postgres::NoTls)
        .unwrap()
}

fn pg_admin() -> Client {
    let mut cfg: postgres::Config = std::env::var("DW_TEST_ADMIN_DATABASE_URL")
        .unwrap()
        .parse()
        .unwrap();
    let db: postgres::Config = std::env::var("DW_TEST_DATABASE_URL")
        .unwrap()
        .parse()
        .unwrap();
    cfg.dbname(db.get_dbname().unwrap());
    cfg.connect(postgres::NoTls).unwrap()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

struct LiveBasis(TerrainBasis);
impl ProposalBasis for LiveBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.0.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp" || r == duskweave::m1_https::INTENT_HTTPS_HEAD_V1
    }
    fn safe_explanation(&self, s: &str) -> bool {
        self.0.safe_explanation(s)
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.0.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.0.recording_integrity().map(|_| ())
    }
    fn current_authority_for_references(&mut self, r: &[String]) -> Result<(), &'static str> {
        self.0.recording_references(r)
    }
    fn recording_premises(&mut self) -> Result<RecordingPremises, &'static str> {
        self.0.recording_integrity()
    }
    fn expected_proposal_version(&self) -> &'static str {
        duskweave::m1_https::VERSION_HTTPS_LOOP
    }
}

struct LiveSynthProvider(Value, Value);
impl Provider for LiveSynthProvider {
    fn prepare(&mut self, _: &BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        let prop = json!({
            "version": duskweave::m1_https::VERSION_HTTPS_LOOP, "hypothesis": "Admitted LAB target check.",
            "considered_alternative": "Target unreachable.", "references": self.0,
            "expected_observation": "Qualified observation.", "falsifying_observation": "Refusal or error.",
            "uncertainty": "Synthetic LAB check.", "rationale": "Step bounded validation.", "next_intent": self.1
        });
        let env = json!({"status":"completed","model":"deepseek-v4-pro","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":prop.to_string()}]}],"usage":{"input_tokens":10,"output_tokens":20}});
        let raw = serde_json::to_vec(&env).unwrap();
        let mut received = response::admit_envelope(&raw, raw.len(), Duration::ZERO, basis)?;
        received.usage.requested_model = "deepseek-v4-pro";
        Ok(received)
    }
}

fn sample_run_record() -> RunRecord {
    let id = Uuid::new_v4();
    let req_json = json!({
        "version": "m1_https_loop_v1", "run_id": id, "run_profile": HTTPS_LAB_RUN_PROFILE,
        "execution_mode": "live", "inference_profile": FIXTURE_PRO_V2_PROFILE, "deadline_at_ms": now_ms() + 600_000,
        "live_binding": {"qualification_ref": Uuid::new_v4(), "release_sha256": "a".repeat(64), "resolver_ipv4": HTTPS_LAB_RESOLVER},
        "context": {"version": "m1_context_prepare_request_v1", "mission": {"version": "m1_inference_reservation_request_v1", "engagement_id": id, "campaign_id": id, "operator_ref": id, "expected_mission_revision": 1, "permission_id": id, "goal_ref": id, "exercise_mode": "blind"}, "candidates": [{"asset_ref": id, "priority": "high"}]},
        "seed_names": [{"asset_ref": id, "name": "head.duskweave.test"}]
    });
    let req = RunRequest::parse(&serde_json::to_vec(&req_json).unwrap()).unwrap();
    RunRecord {
        request: RunRequest {
            seed_names: Vec::<RunSeed>::new(),
            ..req
        },
        decision_one: Uuid::new_v4(),
        decision_two: Uuid::new_v4(),
        attempt: Uuid::new_v4(),
        result: Uuid::new_v4(),
        terrain_operation: Uuid::new_v4(),
        release_sha256: "a".repeat(64),
        qualification_ref: Uuid::new_v4(),
        status: "running".into(),
        deadline_at_ms: now_ms() + 600_000,
    }
}

fn seed_qual(qref: Uuid, rel: &str, now: i64) {
    let hex = "b".repeat(64);
    let ip: std::net::IpAddr = HTTPS_LAB_RESOLVER.parse().unwrap();
    pg_admin().execute(
        "INSERT INTO execution.live_qualifications (qualification_ref,run_profile,purpose,release_sha256,launcher_sha256,capture_sha256,exporter_sha256,image_digest,manifest_sha256,boot_id,resolver_ipv4,valid_from_ms,valid_until_ms) VALUES ($1,$2,'HTTPS_LOOP_LAB',$3,$4,$4,$4,$5,$4,$6,$7,$8,$9) ON CONFLICT (qualification_ref) DO NOTHING",
        &[&qref, &HTTPS_LAB_RUN_PROFILE, &rel, &hex, &format!("sha256:{hex}"), &Uuid::from_u128(0x1), &ip, &(now - 60_000), &(now + 3_600_000)],
    ).unwrap();
}

fn seed_run(qref: Uuid) -> (RunRecord, Value, PgRunStore) {
    let (rel, now) = ("a".repeat(64), now_ms());
    seed_qual(qref, &rel, now);
    let [e, c, run_id, op, auth, perm, goal, asset] = [(); 8].map(|_| Uuid::new_v4());
    let s = now / 1000;
    let reg: Value = serde_json::from_str(&format!(
        r#"{{"engagement_id":"{e}","campaign_id":"{c}","operator_ref":"{op}","authority_ref":"{auth}","authority_revision":1,"goal_ref":"{goal}","included_assets":["{asset}"],"excluded_assets":[],"exercise_mode":"blind","starts_at":{},"ends_at":{},"m1_permission":{{"policy_version":1,"ct_base_domain":"duskweave.test","provider_disclosure":"crt_sh","vantage_ref":"{}","resolver_ipv4":"{HTTPS_LAB_RESOLVER}","approved_path":"/","starts_at":{},"ends_at":{},"excluded_names":[],"concurrency":1,"contact_rules":[{{"asset_ref":"{asset}","rule":{{"exact":"head.duskweave.test"}},"priority":1}}],"discovery_rules":[{{"label_suffix":"duskweave.test"}}],"campaign_limits":{{"episodes":3,"provider_calls":8,"dns_questions":3,"dns_followups":1,"tcp_connections":2,"head_requests":2}}}},"m1_inference_permission":{{"version":"m1_https_loop_v1","permission_id":"{perm}","engagement_id":"{e}","campaign_id":"{c}","operator_ref":"{op}","authority_ref":"{auth}","authority_revision":1,"mission_revision":1,"goal_ref":"{goal}","exercise_mode":"blind","run_profile":"{HTTPS_LAB_RUN_PROFILE}","execution_mode":"live","profile":"{FIXTURE_PRO_V2_PROFILE}","recipient":"http://127.0.0.1:18083/responses","starts_at":{},"ends_at":{},"provider_terms_approved":true,"provider_terms_ref":"{}","field_classes":["mission_question_constraints","scoped_opaque_references","owner_qualified_summaries","admitted_operator_premises","manifest_availability","omissions_unknown","capacity","admitted_repair_fields_codes","admitted_campaign_decision_feedback"],"live_binding":{{"qualification_ref":"{qref}","release_sha256":"{rel}","resolver_ipv4":"{HTTPS_LAB_RESOLVER}"}},"campaign_limits":{{"calls":8,"output_token_capacity":65536,"wall_time_ms":996000,"request_bytes":131072,"response_bytes":524288}},"operation_limits":{{"context_bytes":8192,"candidates":8,"request_bytes":16384,"configuration_bytes":16384,"response_bytes":65536,"calls_per_decision":2,"output_tokens_per_call":8192,"decision_wall_time_ms":249000,"per_call_wall_time_ms":122000,"soft_completion_ms":120000,"shared_wall_time_ms":244000,"phase_wall_time_ms":254000,"connect_time_ms":5000,"proposal_bytes":8192,"repair_bytes":2048,"text_field_bytes":1024,"repair_error_codes":8,"json_depth":32,"collection_entries":64,"concurrency":1,"cancel_grace_ms":2000,"accounting_window_ms":3000}}}}}}"#,
        s - 120, s + 3600, Uuid::new_v4(), s - 90, s + 3500, s - 90, s + 3500, Uuid::new_v4()
    )).unwrap();
    let (mut alloc, mut miss, mut traj) = (
        PgAllocator::new(pg_client("DW_TEST_DATABASE_URL")),
        PgMissionStore::new(pg_client("DW_TEST_DATABASE_URL")),
        PgTrajectory::new(pg_client("DW_TEST_DATABASE_URL")),
    );
    let input = duskweave::input::parse_register(&serde_json::to_vec(&reg).unwrap()).unwrap();
    let op_id = duskweave::registration::prepare_operation(&mut alloc).unwrap();
    assert_eq!(
        duskweave::registration::register(&mut alloc, &mut miss, &mut traj, op_id, &input)
            .unwrap()
            .history,
        HistoryStatus::Completed
    );
    let req_val = json!({
        "version":"m1_https_loop_v1","run_id":run_id,"run_profile":HTTPS_LAB_RUN_PROFILE,"execution_mode":"live","inference_profile":FIXTURE_PRO_V2_PROFILE,"deadline_at_ms":now+600_000,
        "live_binding":{"qualification_ref":qref,"release_sha256":rel,"resolver_ipv4":HTTPS_LAB_RESOLVER},
        "context":{"version":"m1_context_prepare_request_v1","mission":{"version":"m1_inference_reservation_request_v1","engagement_id":e,"campaign_id":c,"operator_ref":op,"expected_mission_revision":1,"permission_id":perm,"goal_ref":goal,"exercise_mode":"blind"},"candidates":[{"asset_ref":asset,"priority":"high"}]},
        "seed_names":[{"asset_ref":asset,"name":"head.duskweave.test"}]
    });
    let mut runs = PgRunStore::new(pg_client("DW_INFERENCE_BROKER_DATABASE_URL"));
    assert!(
        runs.start(
            &RunRequest::parse(&serde_json::to_vec(&req_val).unwrap()).unwrap(),
            &rel
        )
        .unwrap()
    );
    (runs.inspect(run_id).unwrap(), reg, runs)
}

fn commit_step(
    runs: &mut PgRunStore,
    rec: &RunRecord,
    reg: &Value,
    step: usize,
    intent: &str,
    refresh: &str,
) {
    let phase = if step == 1 {
        RunPhase::DecisionOne
    } else {
        RunPhase::DecisionTwo
    };
    runs.renew(rec.request.run_id).unwrap();
    runs.start_phase(rec.request.run_id, phase).unwrap();
    runs.claim_phase(rec.request.run_id, phase, &"a".repeat(64))
        .unwrap();
    let d_id = runs
        .https_step(rec.request.run_id, step)
        .unwrap()
        .decision_id;
    let qref = rec.request.live_binding.as_ref().unwrap().qualification_ref;
    let predecessor = if step == 1 {
        None
    } else {
        runs.https_predecessor(rec.request.run_id, step).unwrap()
    };
    let auto_ctx = AutonomousContext {
        binding: RunContextBinding {
            engagement_id: rec.request.context.mission.engagement_id,
            campaign_id: rec.request.context.mission.campaign_id,
            run_profile: HTTPS_LAB_RUN_PROFILE.into(),
            execution_mode: "live".into(),
            inference_profile: rec.request.inference_profile.clone(),
            release_sha256: "a".repeat(64),
            qualification_ref: qref,
        },
        predecessor,
    };
    let mut reader = PgContextReader::new(pg_client("DW_INFERENCE_BROKER_DATABASE_URL"));
    let prep = duskweave::m1_context::https_loop::prepare(
        &mut reader,
        &rec.request.context,
        OperationId(d_id),
        rec.request.run_id,
        &auto_ctx,
    )
    .unwrap();
    let pack = prep.prepared.utf8().as_bytes();
    runs.bind_https_input(
        rec.request.context.mission.engagement_id.0,
        rec.request.context.mission.campaign_id.0,
        d_id,
        rec.request.run_id,
        &"c".repeat(64),
        &prep.endpoint_map,
        pack,
    )
    .unwrap();
    let perm_id: Uuid =
        serde_json::from_value(reg["m1_inference_permission"]["permission_id"].clone()).unwrap();
    let scope = Scope {
        engagement: rec.request.context.mission.engagement_id.0,
        campaign: rec.request.context.mission.campaign_id.0,
        decision: d_id,
        mission_revision: 1,
        permission: perm_id,
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut port = PgBroker {
        client: pg_client("DW_INFERENCE_BROKER_DATABASE_URL"),
        mission: PgMissionStore::new(pg_client("DW_TEST_DATABASE_URL")),
        reservations: PgInferenceReservationStore::new(pg_client(
            "DW_INFERENCE_BROKER_DATABASE_URL",
        )),
        request: rec.request.context.mission.clone(),
        deadline,
    };
    let mut basis = LiveBasis(TerrainBasis {
        mission: Basis {
            client: pg_client("DW_INFERENCE_BROKER_DATABASE_URL"),
            scope: scope.clone(),
            references: reference_ids(pack).unwrap(),
            deadline,
        },
        dependencies: prep.prepared.terrain_dependencies().to_vec(),
    });
    let mut store = PgDecisionStore {
        client: pg_client("DW_COGNITION_DATABASE_URL"),
        input_hash: "c".repeat(64),
    };
    let mut traj = PgTrajectory::new(pg_client("DW_TRAJECTORY_DATABASE_URL"));
    let mut prov = if step == 1 {
        LiveSynthProvider(
            json!(["mission", "asset_0"]),
            json!({"category":"observe_more","references":["mission","asset_0"],"manifest_reference":intent,"view_request":null,"dependency_refresh":refresh}),
        )
    } else {
        LiveSynthProvider(
            json!(["previous_decision", "asset_0", "endpoint_0"]),
            json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":intent,"dependency_refresh":refresh}),
        )
    };
    let receipt = inference::infer(
        &mut port,
        &mut prov,
        &mut basis,
        &mut store,
        &mut traj,
        InferenceInput {
            scope: &scope,
            pack,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now_ms() as u64,
            source_mode: "blind".into(),
        },
    );
    assert_eq!(receipt.publication, "committed");
    runs.complete_phase(rec.request.run_id, phase).unwrap();
    let _ = runs.resolve_https_step_action(rec, step).unwrap();
}

fn complete_dns_sql(b: &mut Client, run_id: Uuid, step: i32, kind: &str, ttl: i64) {
    let rec: Value = b
        .query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,$2,$3)",
            &[&run_id, &step, &kind],
        )
        .unwrap()
        .get(0);
    let att = Uuid::parse_str(rec["request"]["attempt"].as_str().unwrap()).unwrap();
    let g = rec["request"]["session_generation"].as_i64().unwrap();
    let dl: i64 = b
        .query_one(
            "SELECT execution.live_attempt_may_start($1,$2)",
            &[&att, &g],
        )
        .unwrap()
        .get(0);
    let obs = dl - 2000;
    let res = json!({"schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":att,"session_generation":g,"sequence":3,"status":"completed","observed_at_ms":obs,"rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,"answers":[{"ipv4":"198.51.100.10","ttl_seconds":ttl}],"aliases":[]});
    b.execute(
        "SELECT execution.live_attempt_dns_result($1,$2,$3)",
        &[&att, &g, &res],
    )
    .unwrap();
    b.execute(
        "SELECT execution.live_attempt_stop($1,$2,true)",
        &[&att, &g],
    )
    .unwrap();
}

fn prepare_step2(ttl: i64, refresh: &str) -> (RunRecord, PgRunStore) {
    let (rec, reg, mut runs) = seed_run(Uuid::new_v4());
    let run_id = rec.request.run_id;
    let mut b = pg_client("DW_INFERENCE_BROKER_DATABASE_URL");
    commit_step(&mut runs, &rec, &reg, 1, "dns_udp", "none");
    runs.protect_next_decision(run_id, 1).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionOne).unwrap();
    runs.claim_phase(run_id, RunPhase::AcquisitionOne, &"a".repeat(64))
        .unwrap();
    let (mut dns_ok, mut noop_h, mut ok_d, mut ok_hd) = (
        |rid, st: u32, kind| {
            complete_dns_sql(&mut b, rid, st as i32, kind, ttl);
            Ok(())
        },
        |_, _| Ok(()),
        |_: &RunRecord, _| Ok(()),
        |_: &mut PgRunStore, _: &RunRecord, _| Ok(()),
    );
    acquire_with(
        &mut runs,
        &rec,
        RunPhase::AcquisitionOne,
        &mut LiveOps {
            run_dns: &mut dns_ok,
            run_head: &mut noop_h,
            deliver_dns: &mut ok_d,
            deliver_head: &mut ok_hd,
        },
    )
    .unwrap();
    let step1 = runs.https_step(run_id, 1).unwrap();
    let qm = &rec.request.context.mission;
    let iref = IssuedDnsReference::live(
        qm.engagement_id,
        qm.campaign_id,
        step1.dns_result_id.unwrap(),
        HTTPS_LAB_RUN_PROFILE,
    )
    .unwrap();
    let mut ms = PgMissionStore::new(pg_client("DW_TEST_DATABASE_URL"));
    let mut pts = PgTerrainStore::new(pg_client("DW_TERRAIN_DATABASE_URL"));
    let (ev, _, _, _) = pts.record_issued(&mut ms, &iref).unwrap().unwrap();
    record_terrain_history(&mut pg_client("DW_TRAJECTORY_DATABASE_URL"), &ev).unwrap();
    runs.complete_phase(run_id, RunPhase::AcquisitionOne)
        .unwrap();
    if ttl <= 1 {
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(450));
            runs.renew(run_id).unwrap();
        }
    }
    commit_step(&mut runs, &rec, &reg, 2, "https-head-v1", refresh);
    runs.protect_next_decision(run_id, 2).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionTwo).unwrap();
    runs.claim_phase(run_id, RunPhase::AcquisitionTwo, &"a".repeat(64))
        .unwrap();
    (rec, runs)
}

#[test]
fn acquire_rejects_wrong_mode_and_pure_helpers_cover_variants() {
    let mut rec = sample_run_record();
    rec.request.execution_mode = "controlled_fixture".into();
    assert_eq!(
        acquire(&rec, RunPhase::AcquisitionOne).unwrap_err(),
        Fail::State("live_effect_refused")
    );
    rec.request.execution_mode = "live".into();
    rec.request.run_profile = "autonomous-dns-genai-lab-v1".into();
    assert_eq!(
        acquire(&rec, RunPhase::AcquisitionOne).unwrap_err(),
        Fail::State("live_effect_refused")
    );
    rec.request.version = "m1_run_request_v1".into();
    assert_eq!(
        acquire(&rec, RunPhase::AcquisitionOne).unwrap_err(),
        Fail::State("live_effect_refused")
    );
    assert!(
        is_pre_effect_refusal("live_head_exhausted") && !is_pre_effect_refusal("https_ipc_failure")
    );
    assert_eq!(failure_tag(&Fail::State("s")), "s");
    assert_eq!(failure_tag(&Fail::Unresolved("u")), "u");
    assert_eq!(failure_tag(&Fail::Input("i")), "i");
    assert_eq!(failure_tag(&Fail::Store("st")), "st");
    assert_eq!(failure_tag(&Fail::Config("c")), "c");
    assert_eq!(failure_tag(&Fail::Conflict("cf")), "cf");
}

fn run_two(
    r: &mut PgRunStore,
    rec: &RunRecord,
    run_dns: &mut dyn FnMut(Uuid, u32, &'static str) -> Res<()>,
    run_head: &mut dyn FnMut(Uuid, u32) -> Res<()>,
) -> Res<()> {
    let (mut ok_d, mut ok_hd) = (
        |_: &RunRecord, _| Ok(()),
        |_: &mut PgRunStore, _: &RunRecord, _| Ok(()),
    );
    acquire_with(
        r,
        rec,
        RunPhase::AcquisitionTwo,
        &mut LiveOps {
            run_dns,
            run_head,
            deliver_dns: &mut ok_d,
            deliver_head: &mut ok_hd,
        },
    )
}

#[ignore = "requires schema 0038"]
#[test]
fn acquire_with_covers_refresh_and_head_paths() {
    let (mut noop_d, mut noop_h) = (|_, _, _| Ok(()), |_, _| Ok(()));
    let (rec1, mut r1) = prepare_step2(1, "dns_once_if_stale");
    run_two(
        &mut r1,
        &rec1,
        &mut |_, _, _| Err(Fail::State("live_refresh_exhausted")),
        &mut noop_h,
    )
    .unwrap();
    let _ = r1.stop(rec1.request.run_id);
    let (rec2, mut r2) = prepare_step2(1, "dns_once_if_stale");
    let mut b2 = pg_client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut post_ref = |rid, st: u32, kind| {
        let _ = b2.query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,$2,$3)",
            &[&rid, &(st as i32), &kind],
        );
        Err(Fail::State("dns_ipc_failure"))
    };
    assert!(run_two(&mut r2, &rec2, &mut post_ref, &mut noop_h).is_err());
    let _ = r2.stop(rec2.request.run_id);
    let (rec3, mut r3) = prepare_step2(1, "dns_once_if_stale");
    let mut b3 = pg_client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut ref_ok = |rid, st: u32, kind| {
        complete_dns_sql(&mut b3, rid, st as i32, kind, 60);
        Ok(())
    };
    run_two(&mut r3, &rec3, &mut ref_ok, &mut noop_h).unwrap();
    let _ = r3.stop(rec3.request.run_id);
    let (rec4, mut r4) = prepare_step2(60, "none");
    run_two(&mut r4, &rec4, &mut noop_d, &mut |_, _| {
        Err(Fail::State("live_head_exhausted"))
    })
    .unwrap();
    let _ = r4.stop(rec4.request.run_id);
    let (rec5, mut r5) = prepare_step2(60, "none");
    let mut b5 = pg_client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut head_post = |rid, st: u32| {
        let _ = b5.query_one(
            "SELECT execution.live_https_loop_head_reserve($1,$2)",
            &[&rid, &(st as i32)],
        );
        Err(Fail::State("https_ipc_failure"))
    };
    assert!(run_two(&mut r5, &rec5, &mut noop_d, &mut head_post).is_err());
    let _ = r5.stop(rec5.request.run_id);
}

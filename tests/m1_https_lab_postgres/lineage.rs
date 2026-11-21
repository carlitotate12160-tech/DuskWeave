use crate::admission::{broker, db_err, now_ms, start_lab_run};
use crate::genai::envelope;
use crate::lab_support;
use crate::terrain::terrain_support::native;
use duskweave::m1_inference_profile::{HTTPS_LAB_IPV4, HTTPS_LAB_RUN_PROFILE};
use duskweave::m1_reasoning::{
    inference::{self, InferenceInput, Provider, Received, Scope},
    postgres::{Basis, PgBroker, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::{ProposalBasis, RecordingPremises},
    request::BoundCall,
    response,
};
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::{AssetRef, CampaignId, EngagementId, OperationId};
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_m1_run::{PgRunStore, https_loop::StepAction};
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_mission::{PgContextReader, PgMissionStore};
use duskweave::postgres_terrain_history::record_terrain_history;
use duskweave::postgres_trajectory::PgTrajectory;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

struct LabBasis(TerrainBasis);
impl ProposalBasis for LabBasis {
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

struct SynthProvider(Value);
impl Provider for SynthProvider {
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
            "version": duskweave::m1_https::VERSION_HTTPS_LOOP,
            "hypothesis": "Admitted LAB target check.",
            "considered_alternative": "Target unreachable.",
            "references": self.0["references"],
            "expected_observation": "Qualified observation.",
            "falsifying_observation": "Refusal or error.",
            "uncertainty": "Synthetic LAB check.",
            "rationale": "Step bounded validation.",
            "next_intent": self.0
        });
        let raw = serde_json::to_vec(&envelope(&prop)).unwrap();
        let mut received = response::admit_envelope(&raw, raw.len(), Duration::ZERO, basis)?;
        received.usage.requested_model = "deepseek-v4-pro";
        Ok(received)
    }
}

pub(crate) fn commit_synthetic_step_decision(
    runs: &mut PgRunStore,
    rec: &RunRecord,
    reg: &Value,
    step_index: usize,
    next_intent: Value,
) -> StepAction {
    let phase = match step_index {
        1 => RunPhase::DecisionOne,
        2 => RunPhase::DecisionTwo,
        _ => RunPhase::DecisionThree,
    };
    runs.start_phase(rec.request.run_id, phase).unwrap();
    runs.claim_phase(rec.request.run_id, phase, &rec.release_sha256)
        .unwrap();
    let d_id = runs
        .https_step(rec.request.run_id, step_index)
        .unwrap()
        .decision_id;
    let qref = rec
        .request
        .live_binding
        .as_ref()
        .map(|b| b.qualification_ref)
        .unwrap();
    let predecessor = if step_index == 1 {
        None
    } else {
        runs.https_predecessor(rec.request.run_id, step_index)
            .unwrap()
    };
    let auto_ctx = duskweave::m1_autonomous_context::AutonomousContext {
        binding: duskweave::m1_autonomous_context::RunContextBinding {
            engagement_id: rec.request.context.mission.engagement_id,
            campaign_id: rec.request.context.mission.campaign_id,
            run_profile: HTTPS_LAB_RUN_PROFILE.into(),
            execution_mode: "live".into(),
            inference_profile: rec.request.inference_profile.clone(),
            release_sha256: rec.release_sha256.clone(),
            qualification_ref: qref,
        },
        predecessor,
    };
    let mut reader = PgContextReader::new(broker());
    let prep = duskweave::m1_context::https_loop::prepare(
        &mut reader,
        &rec.request.context,
        OperationId(d_id),
        rec.request.run_id,
        &auto_ctx,
    )
    .unwrap();
    let pack = prep.prepared.utf8().as_bytes();
    let input_hash = lab_support::sha256_hex(format!("lab-input-{step_index}").as_bytes());
    runs.bind_https_input(
        rec.request.context.mission.engagement_id.0,
        rec.request.context.mission.campaign_id.0,
        d_id,
        rec.request.run_id,
        &input_hash,
        &prep.endpoint_map,
        pack,
    )
    .unwrap();
    let perm_id = Uuid::parse_str(
        reg["m1_inference_permission"]["permission_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let scope = Scope {
        engagement: rec.request.context.mission.engagement_id.0,
        campaign: rec.request.context.mission.campaign_id.0,
        decision: d_id,
        mission_revision: 1,
        permission: perm_id,
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut port = PgBroker {
        client: broker(),
        mission: PgMissionStore::new(native::client("DW_TEST_DATABASE_URL")),
        reservations: PgInferenceReservationStore::new(broker()),
        request: rec.request.context.mission.clone(),
        deadline,
    };
    let mut basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: scope.clone(),
            references: duskweave::m1_reasoning::request::reference_ids(pack).unwrap(),
            deadline,
        },
        dependencies: prep.prepared.terrain_dependencies().to_vec(),
    });
    let mut store = PgDecisionStore {
        client: native::client("DW_COGNITION_DATABASE_URL"),
        input_hash,
    };
    let mut traj = PgTrajectory::new(native::client("DW_TRAJECTORY_DATABASE_URL"));
    let mut prov = SynthProvider(next_intent);
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
    runs.resolve_https_step_action(rec, step_index).unwrap()
}

pub(crate) fn drive_step1_live_dns(
    e: Uuid,
    c: Uuid,
    run_id: Uuid,
    reg: &Value,
    runs: &mut PgRunStore,
    rec: &RunRecord,
    ttl_seconds: u32,
) -> (Uuid, i32) {
    let rel = lab_support::sample_release_sha();
    drive_step1_live_dns_with_release(e, c, run_id, reg, runs, rec, &rel, ttl_seconds)
}

pub(crate) fn drive_step1_live_dns_with_release(
    e: Uuid,
    c: Uuid,
    run_id: Uuid,
    reg: &Value,
    runs: &mut PgRunStore,
    rec: &RunRecord,
    release: &str,
    ttl_seconds: u32,
) -> (Uuid, i32) {
    commit_synthetic_step_decision(
        runs,
        rec,
        reg,
        1,
        json!({"category":"observe_more","references":["mission","asset_0"],"manifest_reference":"dns_udp","view_request":null,"dependency_refresh":"none"}),
    );
    runs.protect_next_decision(run_id, 1).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionOne).unwrap();
    runs.claim_phase(run_id, RunPhase::AcquisitionOne, release)
        .unwrap();
    let mut b = broker();
    let dns_req: Value = b
        .query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,1,'dns')",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    let att_dns = Uuid::parse_str(dns_req["request"]["attempt"].as_str().unwrap()).unwrap();
    let g_dns = dns_req["request"]["session_generation"].as_i64().unwrap();
    let dl_dns: i64 = b
        .query_one(
            "SELECT execution.live_attempt_may_start($1,$2)",
            &[&att_dns, &g_dns],
        )
        .unwrap()
        .get(0);
    let dns_res = json!({
        "schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":att_dns,"session_generation":g_dns,
        "sequence":3,"status":"completed","observed_at_ms":dl_dns - 2000,"rcode":0,"truncated":false,
        "complete":true,"withheld_aliases":0,"answers":[{"ipv4":HTTPS_LAB_IPV4,"ttl_seconds":ttl_seconds}],"aliases":[]
    });
    b.execute(
        "SELECT execution.live_attempt_dns_result($1,$2,$3)",
        &[&att_dns, &g_dns, &dns_res],
    )
    .unwrap();
    b.execute(
        "SELECT execution.live_attempt_stop($1,$2,true)",
        &[&att_dns, &g_dns],
    )
    .unwrap();
    let dup_dns: Value = b
        .query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,1,'dns')",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(dup_dns["request"]["attempt"], dns_req["request"]["attempt"]);
    assert!(
        b.query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,1,'dns_refresh')",
            &[&run_id]
        )
        .is_err()
    );

    let step1 = runs.https_step(run_id, 1).unwrap();
    let iref = IssuedDnsReference::live(
        EngagementId(e),
        CampaignId(c),
        step1.dns_result_id.unwrap(),
        HTTPS_LAB_RUN_PROFILE,
    )
    .unwrap();
    let mut m_store = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut pts = PgTerrainStore::new(native::client("DW_TERRAIN_DATABASE_URL"));
    let (dns_ev, _, _, _) = pts.record_issued(&mut m_store, &iref).unwrap().unwrap();
    record_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &dns_ev).unwrap();
    runs.complete_phase(run_id, RunPhase::AcquisitionOne)
        .unwrap();

    native::admin()
        .query_one(
            "SELECT claim_id, claim_revision FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .map(|r| (r.get(0), r.get(1)))
        .unwrap()
}

#[ignore]
#[test]
fn live_https_loop_head_phase_monotonicity_f3_f4_and_cardinality_caps() {
    let _guard = native::qualified_existing_db();
    let qref = Uuid::new_v4();
    let (e, c, run_id, reg, mut runs, rec) = start_lab_run(qref);
    let renew = crate::recovery::Renewals::start(run_id);
    let asset = Uuid::parse_str(reg["included_assets"][0].as_str().unwrap()).unwrap();
    let mut b = broker();
    let step1_claim = drive_step1_live_dns(e, c, run_id, &reg, &mut runs, &rec, 30);

    let ep_ref = "endpoint_0";
    commit_synthetic_step_decision(
        &mut runs,
        &rec,
        &reg,
        2,
        json!({"category":"validate","references":["asset_0",ep_ref],"manifest_reference":"https-head-v1","dependency_refresh":"dns_once_if_stale"}),
    );
    runs.protect_next_decision(run_id, 2).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionTwo).unwrap();
    runs.claim_phase(
        run_id,
        RunPhase::AcquisitionTwo,
        &lab_support::sample_release_sha(),
    )
    .unwrap();

    let head_rec: Value = b
        .query_one(
            "SELECT execution.live_https_loop_head_reserve($1,2)",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    let att_h = Uuid::parse_str(head_rec["request"]["attempt"].as_str().unwrap()).unwrap();
    let g_h = head_rec["request"]["session_generation"].as_i64().unwrap();
    b.query_one(
        "SELECT execution.live_attempt_may_start($1,$2)",
        &[&att_h, &g_h],
    )
    .unwrap();

    let e_head_early = b
        .query_one(
            "SELECT execution.live_attempt_head_phase($1,$2,'head')",
            &[&att_h, &g_h],
        )
        .unwrap_err();
    assert!(
        db_err(&e_head_early).contains("live_effect_refused"),
        "{}",
        db_err(&e_head_early)
    );

    let ipc_req: duskweave::m1_https::HttpsIpcRequest =
        serde_json::from_value(head_rec["request"].clone()).unwrap();
    let obs_ms = ipc_req.deadline_at_ms - i64::from(ipc_req.max_duration_ms);
    let safe = serde_json::to_value(lab_support::sample_https_safe_result(
        &ipc_req,
        duskweave::m1_https::HttpsOutcome::HeadResponse,
        obs_ms,
        200,
        "none",
        None,
    ))
    .unwrap();
    let head_res = json!({
        "version": "m1_https_head_result_v1", "engagement_id": e, "campaign_id": c,
        "run_id": run_id, "decision_id": runs.https_step(run_id, 2).unwrap().decision_id,
        "attempt_id": att_h, "result_id": head_rec["request"]["result_id"],
        "asset_ref": AssetRef(asset).0, "endpoint_ref": ep_ref,
        "hostname": "head.duskweave.test", "ipv4": HTTPS_LAB_IPV4, "port": 443,
        "approved_path": "/", "method": "HEAD",
        "parent_claim_id": step1_claim.0, "parent_claim_revision": step1_claim.1,
        "perceived_at": obs_ms / 1000, "uncertainty_seconds": 1,
        "outcome": "head_response", "status_code": 200, "redirect": "none",
        "source_kind": "issued_https_acquisition", "limitations": safe["limitations"]
    });
    let e_pre_tls = b
        .execute(
            "SELECT execution.live_attempt_head_result($1,$2,$3,$4)",
            &[&att_h, &g_h, &safe, &head_res],
        )
        .unwrap_err();
    assert!(
        db_err(&e_pre_tls).contains("live_result_refused"),
        "{}",
        db_err(&e_pre_tls)
    );

    b.query_one(
        "SELECT execution.live_attempt_head_phase($1,$2,'tls')",
        &[&att_h, &g_h],
    )
    .unwrap();
    b.query_one(
        "SELECT execution.live_attempt_head_phase($1,$2,'head')",
        &[&att_h, &g_h],
    )
    .unwrap();
    let e_reg = b
        .query_one(
            "SELECT execution.live_attempt_head_phase($1,$2,'tls')",
            &[&att_h, &g_h],
        )
        .unwrap_err();
    assert!(
        db_err(&e_reg).contains("live_effect_refused"),
        "{}",
        db_err(&e_reg)
    );

    b.execute(
        "SELECT execution.live_attempt_head_result($1,$2,$3,$4)",
        &[&att_h, &g_h, &safe, &head_res],
    )
    .unwrap();
    b.execute(
        "SELECT execution.live_attempt_stop($1,$2,true)",
        &[&att_h, &g_h],
    )
    .unwrap();
    let dup_h: Value = b
        .query_one(
            "SELECT execution.live_https_loop_head_reserve($1,2)",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(dup_h["request"]["attempt"], head_rec["request"]["attempt"]);

    // Check NotNeeded/Fresh before TTL expiry, and verify head result readback + Terrain publication.
    let refresh_d = format!("{:?}", runs.https_refresh_decision(run_id, 2).unwrap());
    assert!(
        refresh_d == "NotNeeded" || refresh_d == "Fresh",
        "{refresh_d}"
    );
    let read_back: Value = b
        .query_one(
            "SELECT execution.read_https_loop_head_result($1,2)",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(read_back["status_code"], json!(200));
    assert_eq!(read_back["source_kind"], json!("issued_https_acquisition"));

    let head_committed = runs
        .read_committed_https_result(run_id, 2)
        .unwrap()
        .unwrap();
    let step2 = runs.https_step(run_id, 2).unwrap();
    let mut m_store = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut pts = PgTerrainStore::new(native::client("DW_TERRAIN_DATABASE_URL"));
    let mut phts = duskweave::postgres_m1_terrain::https::PgHttpsTerrainStore::new(&mut pts);
    let https_ev = phts
        .record_https(
            &mut m_store,
            &head_committed,
            OperationId(step2.head_terrain_operation_id.unwrap()),
        )
        .unwrap();
    duskweave::postgres_terrain_history::record_https_terrain_history(
        &mut native::client("DW_TRAJECTORY_DATABASE_URL"),
        &https_ev,
    )
    .unwrap();
    let ter_src: String = native::admin()
        .query_one(
            "SELECT source_kind FROM terrain.https_observations WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    assert_eq!(ter_src, "issued_https_acquisition");
    renew.stop();
    let _ = runs.stop(run_id);
}

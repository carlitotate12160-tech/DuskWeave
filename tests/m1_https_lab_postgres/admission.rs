use crate::lab_support;
use crate::terrain::terrain_support::native;
use duskweave::Fail;
use duskweave::m1_inference_profile::{
    FIXTURE_PRO_V2_PROFILE, HTTPS_LAB_RESOLVER, HTTPS_LAB_RUN_PROFILE,
};
use duskweave::m1_reasoning::{inference::Broker, inference_intent, postgres::PgBroker};
use duskweave::m1_run::RunRecord;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_mission::{PgAllocator, PgContextReader, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub(crate) fn broker() -> postgres::Client {
    native::client("DW_INFERENCE_BROKER_DATABASE_URL")
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

pub(crate) fn db_err(err: &postgres::Error) -> String {
    err.as_db_error()
        .map(|d| {
            format!(
                "{}:{}:{}",
                d.code().code(),
                d.message(),
                d.detail().unwrap_or("")
            )
        })
        .unwrap_or_else(|| err.to_string())
}

pub(crate) fn assert_42501<T: std::fmt::Debug>(res: Result<T, postgres::Error>, ctx: &str) {
    let err = res.expect_err(&format!("expected 42501 insufficient privilege: {ctx}"));
    assert_eq!(
        err.code(),
        Some(&postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
        "{ctx}: {}",
        db_err(&err)
    );
}

pub(crate) fn seed_qual(
    qref: Uuid,
    run_profile: &str,
    purpose: &str,
    resolver: &str,
    release: &str,
    valid_from_ms: i64,
    valid_until_ms: i64,
) {
    let mut a = native::admin();
    let hex = "b".repeat(64);
    let ip: std::net::IpAddr = resolver.parse().unwrap();
    a.execute(
        "INSERT INTO execution.live_qualifications
         (qualification_ref,run_profile,purpose,release_sha256,launcher_sha256,
          capture_sha256,exporter_sha256,image_digest,manifest_sha256,boot_id,
          resolver_ipv4,valid_from_ms,valid_until_ms)
         VALUES ($1,$2,$3,$4,$5,$5,$5,$6,$5,$7,$8,$9,$10)
         ON CONFLICT (qualification_ref) DO UPDATE SET
          run_profile=EXCLUDED.run_profile, purpose=EXCLUDED.purpose,
          release_sha256=EXCLUDED.release_sha256, resolver_ipv4=EXCLUDED.resolver_ipv4,
          valid_from_ms=EXCLUDED.valid_from_ms, valid_until_ms=EXCLUDED.valid_until_ms",
        &[
            &qref,
            &run_profile,
            &purpose,
            &release,
            &hex,
            &format!("sha256:{hex}"),
            &Uuid::from_u128(0x1111),
            &ip,
            &valid_from_ms,
            &valid_until_ms,
        ],
    )
    .unwrap();
}

pub(crate) fn seed_lab_qual(qref: Uuid) {
    let now = now_ms();
    seed_qual(
        qref,
        HTTPS_LAB_RUN_PROFILE,
        "HTTPS_LOOP_LAB",
        HTTPS_LAB_RESOLVER,
        &lab_support::sample_release_sha(),
        now - 60_000,
        now + 3_600_000,
    );
}

pub(crate) fn base_registration_json(e: Uuid, c: Uuid, qref: Uuid, release: &str) -> Value {
    let now_s = now_ms() / 1000;
    let mut reg = crate::terrain::combined_registration_json(
        duskweave::mission::EngagementId(e),
        duskweave::mission::CampaignId(c),
        now_s,
        true,
    );
    let asset = reg["included_assets"][0].clone();
    reg["starts_at"] = json!(now_s - 120);
    reg["ends_at"] = json!(now_s + 3600);
    let mp = &mut reg["m1_permission"];
    mp["ct_base_domain"] = json!("duskweave.test");
    mp["resolver_ipv4"] = json!(HTTPS_LAB_RESOLVER);
    mp["approved_path"] = json!("/");
    mp["starts_at"] = json!(now_s - 90);
    mp["ends_at"] = json!(now_s + 3500);
    mp["contact_rules"] = json!([{
        "asset_ref": asset,
        "rule": {"exact": "head.duskweave.test"},
        "priority": 1
    }]);
    mp["discovery_rules"] = json!([{"label_suffix": "duskweave.test"}]);
    mp["excluded_names"] = json!([]);
    for (k, v) in [
        ("episodes", 3),
        ("provider_calls", 8),
        ("dns_questions", 3),
        ("dns_followups", 1),
        ("tcp_connections", 2),
        ("head_requests", 2),
    ] {
        mp["campaign_limits"][k] = json!(v);
    }
    let ip = &mut reg["m1_inference_permission"];
    ip["version"] = json!("m1_https_loop_v1");
    ip["run_profile"] = json!(HTTPS_LAB_RUN_PROFILE);
    ip["execution_mode"] = json!("live");
    ip["profile"] = json!(FIXTURE_PRO_V2_PROFILE);
    ip["recipient"] = json!("http://127.0.0.1:18083/responses");
    ip["starts_at"] = json!(now_s - 90);
    ip["ends_at"] = json!(now_s + 3500);
    ip["field_classes"] = json!([
        "mission_question_constraints",
        "scoped_opaque_references",
        "owner_qualified_summaries",
        "admitted_operator_premises",
        "manifest_availability",
        "omissions_unknown",
        "capacity",
        "admitted_repair_fields_codes",
        "admitted_campaign_decision_feedback"
    ]);
    ip["live_binding"] = json!({
        "qualification_ref": qref,
        "release_sha256": release,
        "resolver_ipv4": HTTPS_LAB_RESOLVER
    });
    for (k, v) in [
        ("calls", 8),
        ("output_token_capacity", 65536),
        ("wall_time_ms", 996000),
        ("request_bytes", 131072),
        ("response_bytes", 524288),
    ] {
        ip["campaign_limits"][k] = json!(v);
    }
    for (k, v) in [
        ("output_tokens_per_call", 8192),
        ("decision_wall_time_ms", 249000),
        ("per_call_wall_time_ms", 122000),
        ("soft_completion_ms", 120000),
        ("shared_wall_time_ms", 244000),
        ("phase_wall_time_ms", 254000),
        ("request_bytes", 16384),
        ("response_bytes", 65536),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
        ("context_bytes", 8192),
        ("json_depth", 32),
    ] {
        ip["operation_limits"][k] = json!(v);
    }
    reg
}

pub(crate) fn lab_run_request(reg: &Value, run_id: Uuid, qref: Uuid, release: &str) -> Value {
    let asset = &reg["included_assets"][0];
    json!({
        "version": "m1_https_loop_v1",
        "run_id": run_id,
        "run_profile": HTTPS_LAB_RUN_PROFILE,
        "execution_mode": "live",
        "inference_profile": FIXTURE_PRO_V2_PROFILE,
        "deadline_at_ms": now_ms() + 600_000,
        "live_binding": {
            "qualification_ref": qref,
            "release_sha256": release,
            "resolver_ipv4": HTTPS_LAB_RESOLVER
        },
        "context": {
            "version": "m1_context_prepare_request_v1",
            "mission": {
                "version": "m1_inference_reservation_request_v1",
                "engagement_id": reg["engagement_id"],
                "campaign_id": reg["campaign_id"],
                "operator_ref": reg["operator_ref"],
                "expected_mission_revision": 1,
                "permission_id": reg["m1_inference_permission"]["permission_id"],
                "goal_ref": reg["goal_ref"],
                "exercise_mode": "blind"
            },
            "candidates": [{"asset_ref": asset, "priority": "high"}]
        },
        "seed_names": [{"asset_ref": asset, "name": "head.duskweave.test"}]
    })
}

pub(crate) fn register_lab_campaign(reg: &Value) -> duskweave::Res<()> {
    let mut allocator = PgAllocator::new(native::client("DW_TEST_DATABASE_URL"));
    let mut mission = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut history = PgTrajectory::new(native::client("DW_TEST_DATABASE_URL"));
    let bytes = serde_json::to_vec(reg).map_err(|_| Fail::Input("invalid_registration"))?;
    let input = duskweave::input::parse_register(&bytes)?;
    let op = duskweave::registration::prepare_operation(&mut allocator)?;
    let receipt =
        duskweave::registration::register(&mut allocator, &mut mission, &mut history, op, &input)?;
    assert_eq!(
        receipt.history,
        duskweave::trajectory::HistoryStatus::Completed
    );
    Ok(())
}

pub(crate) fn start_lab_run(qref: Uuid) -> (Uuid, Uuid, Uuid, Value, PgRunStore, RunRecord) {
    let release = lab_support::sample_release_sha();
    seed_lab_qual(qref);
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let reg = base_registration_json(e, c, qref, &release);
    register_lab_campaign(&reg).unwrap();
    let req_val = lab_run_request(&reg, run_id, qref, &release);
    let req = duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&req_val).unwrap()).unwrap();
    let mut runs = PgRunStore::new(broker());
    assert!(runs.start(&req, &release).unwrap());
    let rec = runs.inspect(run_id).unwrap();
    (e, c, run_id, reg, runs, rec)
}

#[ignore]
#[test]
fn lab_admission_requires_matching_qualification_profile_seed_and_budget() {
    let _guard = native::qualified_existing_db();
    let qref = Uuid::new_v4();
    let release = lab_support::sample_release_sha();
    let (_, _, run_id, _, mut runs, _) = start_lab_run(qref);
    let summary = runs.summary(run_id).unwrap();
    assert_eq!(summary["live_qualified"], json!(true));
    assert_eq!(summary["execution_mode"], json!("live"));
    assert_eq!(summary["run_profile"], json!(HTTPS_LAB_RUN_PROFILE));

    let e2 = Uuid::new_v4();
    let c2 = Uuid::new_v4();
    let qref_missing = Uuid::new_v4();
    let reg2 = base_registration_json(e2, c2, qref_missing, &release);
    register_lab_campaign(&reg2).unwrap();
    let base_req = lab_run_request(&reg2, Uuid::new_v4(), qref_missing, &release);
    let mut brk = broker();
    assert!(
        brk.query_one("SELECT execution.run_start($1,$2)", &[&base_req, &release])
            .is_err()
    );

    let qref_dns = Uuid::new_v4();
    seed_qual(
        qref_dns,
        "autonomous-dns-genai-lab-v1",
        "LAB",
        HTTPS_LAB_RESOLVER,
        &release,
        now_ms() - 60_000,
        now_ms() + 3_600_000,
    );
    let mut reg_dns = base_registration_json(Uuid::new_v4(), Uuid::new_v4(), qref_dns, &release);
    register_lab_campaign(&reg_dns).unwrap();
    let req_dns = lab_run_request(&reg_dns, Uuid::new_v4(), qref_dns, &release);
    assert!(
        brk.query_one("SELECT execution.run_start($1,$2)", &[&req_dns, &release])
            .is_err()
    );

    let qref_exp = Uuid::new_v4();
    seed_qual(
        qref_exp,
        HTTPS_LAB_RUN_PROFILE,
        "HTTPS_LOOP_LAB",
        HTTPS_LAB_RESOLVER,
        &release,
        10,
        20,
    );
    let reg_exp = base_registration_json(Uuid::new_v4(), Uuid::new_v4(), qref_exp, &release);
    register_lab_campaign(&reg_exp).unwrap();
    let req_exp = lab_run_request(&reg_exp, Uuid::new_v4(), qref_exp, &release);
    assert!(
        brk.query_one("SELECT execution.run_start($1,$2)", &[&req_exp, &release])
            .is_err()
    );

    // Wrong release_sha256 in qualification vs executing release is rejected.
    let qref_bad_rel = Uuid::new_v4();
    let other_rel = "c".repeat(64);
    seed_qual(
        qref_bad_rel,
        HTTPS_LAB_RUN_PROFILE,
        "HTTPS_LOOP_LAB",
        HTTPS_LAB_RESOLVER,
        &other_rel,
        now_ms() - 60_000,
        now_ms() + 3_600_000,
    );
    let reg_bad_rel =
        base_registration_json(Uuid::new_v4(), Uuid::new_v4(), qref_bad_rel, &other_rel);
    register_lab_campaign(&reg_bad_rel).unwrap();
    let req_bad_rel = lab_run_request(&reg_bad_rel, Uuid::new_v4(), qref_bad_rel, &other_rel);
    assert!(
        brk.query_one(
            "SELECT execution.run_start($1,$2)",
            &[&req_bad_rel, &release]
        )
        .is_err()
    );

    reg_dns["m1_inference_permission"]["campaign_limits"]["calls"] = json!(1);
    assert!(register_lab_campaign(&reg_dns).is_err());

    let mut bad_seed = lab_run_request(&reg2, Uuid::new_v4(), qref, &release);
    bad_seed["seed_names"][0]["name"] = json!("seed.duskweave.test");
    let mut bad_mode = lab_run_request(&reg2, Uuid::new_v4(), qref, &release);
    bad_mode["execution_mode"] = json!("controlled_fixture");
    let mut bad_resolver = lab_run_request(&reg2, Uuid::new_v4(), qref, &release);
    bad_resolver["live_binding"]["resolver_ipv4"] = json!("1.1.1.1");
    let mut bad_inf_profile = lab_run_request(&reg2, Uuid::new_v4(), qref, &release);
    bad_inf_profile["inference_profile"] = json!("fixture-genai-responses-v1");
    for bad in [bad_seed, bad_mode, bad_resolver, bad_inf_profile] {
        assert!(
            brk.query_one("SELECT execution.run_start($1,$2)", &[&bad, &release])
                .is_err()
        );
    }
    let _ = runs.stop(run_id);
}

#[ignore]
#[test]
fn lab_intent_binding_and_pack_verification_enforce_pro_v2_contract() {
    let _guard = native::qualified_existing_db();
    let qref = Uuid::new_v4();
    let (e, c, run_id, reg, mut runs, rec) = start_lab_run(qref);
    let d1 = rec.decision_one;
    runs.start_phase(run_id, duskweave::m1_run::RunPhase::DecisionOne)
        .unwrap();
    runs.claim_phase(
        run_id,
        duskweave::m1_run::RunPhase::DecisionOne,
        &lab_support::sample_release_sha(),
    )
    .unwrap();

    let mut reader = PgContextReader::new(broker());
    let auto_ctx = duskweave::m1_autonomous_context::AutonomousContext {
        binding: duskweave::m1_autonomous_context::RunContextBinding {
            engagement_id: duskweave::mission::EngagementId(e),
            campaign_id: duskweave::mission::CampaignId(c),
            run_profile: HTTPS_LAB_RUN_PROFILE.into(),
            execution_mode: "live".into(),
            inference_profile: Some(FIXTURE_PRO_V2_PROFILE.into()),
            release_sha256: lab_support::sample_release_sha(),
            qualification_ref: qref,
        },
        predecessor: None,
    };
    let prepared = duskweave::m1_context::https_loop::prepare(
        &mut reader,
        &rec.request.context,
        duskweave::mission::OperationId(d1),
        run_id,
        &auto_ctx,
    )
    .unwrap();

    let pack_bytes = prepared.prepared.utf8().as_bytes();
    let input_hash = lab_support::sha256_hex(b"lab-admission-input");
    runs.bind_https_input(
        e,
        c,
        d1,
        run_id,
        &input_hash,
        &prepared.endpoint_map,
        pack_bytes,
    )
    .unwrap();

    let resv_store = PgInferenceReservationStore::new(broker());
    let perm_id = Uuid::parse_str(
        reg["m1_inference_permission"]["permission_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let scope = duskweave::m1_reasoning::inference::Scope {
        engagement: e,
        campaign: c,
        decision: d1,
        mission_revision: 1,
        permission: perm_id,
    };
    let mut port = PgBroker {
        client: broker(),
        mission: PgMissionStore::new(native::client("DW_TEST_DATABASE_URL")),
        reservations: resv_store,
        request: rec.request.context.mission.clone(),
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(60),
    };
    let reservation = port.reserve(&scope, pack_bytes).unwrap();
    let bound = inference_intent::bind(
        &scope,
        &reservation,
        reservation.generation_call,
        pack_bytes,
        None,
    )
    .unwrap();
    let cfg: Value = serde_json::from_slice(&bound.configuration).unwrap();
    assert_eq!(cfg["model"], "deepseek-v4-pro");
    assert_eq!(cfg["recipient"], "http://127.0.0.1:18083/responses");

    port.may_send(&scope, reservation.generation_call, &bound)
        .unwrap();

    let mut bad_model = cfg.clone();
    bad_model["model"] = json!("deepseek-flash");
    let bad_cfg_bytes = serde_json::to_vec(&bad_model).unwrap();
    let bad_digest = inference_intent::digest(&bound.pack, &bad_cfg_bytes);
    let mut brk = broker();
    assert!(
        brk.query_one(
            "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &e,
                &c,
                &d1,
                &reservation.repair_call,
                &String::from_utf8_lossy(&bound.pack).into_owned(),
                &String::from_utf8_lossy(&bad_cfg_bytes).into_owned(),
                &String::from_utf8_lossy(&bound.body).into_owned(),
                &bad_digest,
            ],
        )
        .is_err()
    );
    let _ = runs.stop(run_id);
}

//! Closed actual Pro-v2 tuple admission, producer/SQL/intent binding, and capacity checks.
use crate::provider_upgrade::{
    ACTUAL, ACTUAL_URL, BROKER, FIXTURE, FIXTURE_URL, actual_pro_v2_registered_with,
    ensure_0039_overlay, lab_permission, live_binding, permission_json, sql_matches,
};
use crate::terrain::terrain_support::native;
use duskweave::Fail;
use duskweave::input::parse_register;
use duskweave::m1_autonomous_context::{AutonomousContext, RunContextBinding};
use duskweave::m1_context::https_loop::{self, PreparedHttpsLoop};
use duskweave::m1_context_input::ContextRequest;
use duskweave::m1_inference_profile::{
    HTTPS_LAB_HOSTNAME, HTTPS_LAB_RUN_PROFILE, PRO_V2_TIMING_PROFILE,
};
use duskweave::m1_reasoning::inference::{Reservation, Scope};
use duskweave::m1_reasoning::inference_intent;
use duskweave::m1_run::{RunPhase, RunRequest};
use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_mission::PgContextReader;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

const LOOP: &str = "m1_https_loop_v1";
const MODE: &str = "controlled_fixture";
const LAB: &str = HTTPS_LAB_RUN_PROFILE;
const V1: &str = "deepseek-genai-v4-pro-low-v1";
const BOGUS: &str = "bogus-profile";
fn u(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn register_json(p: &str, r: &str, rp: &str, mode: &str, live: Option<Value>) -> Value {
    let mut raw = crate::terrain::combined_registration_json(
        EngagementId(u(0xe101)),
        CampaignId(u(0xe102)),
        1_700_000_000,
        true,
    );
    crate::https_support::configure_permission(&mut raw);
    let ip = &mut raw["m1_inference_permission"];
    (
        ip["profile"],
        ip["recipient"],
        ip["run_profile"],
        ip["execution_mode"],
    ) = (json!(p), json!(r), json!(rp), json!(mode));
    if let Some(b) = live {
        ip["live_binding"] = b;
    }
    raw
}

fn run_json(p: &str, rp: &str, mode: &str, live: Option<Value>) -> Value {
    let (e, c, a) = (u(0xe101), u(0xe102), u(0x21));
    let mut req = json!({
        "version": LOOP, "run_id": u(0xe104), "run_profile": rp,
        "execution_mode": mode, "inference_profile": p, "deadline_at_ms": 1_700_000_600_000i64,
        "context": {
            "version": "m1_context_prepare_request_v1",
            "mission": {
                "version": "m1_inference_reservation_request_v1", "engagement_id": e,
                "campaign_id": c, "operator_ref": u(0x11), "expected_mission_revision": 1,
                "permission_id": u(0xe103), "goal_ref": u(0x13), "exercise_mode": "blind"
            },
            "candidates": [{"asset_ref": a, "priority": "high"}]
        },
        "seed_names": [{"asset_ref": a, "name": HTTPS_LAB_HOSTNAME}]
    });
    if let Some(b) = live {
        req["live_binding"] = b;
    }
    req
}

fn pack_json(p: &str, rp: &str, mode: &str, live: Option<Value>, purp: Option<Value>) -> Vec<u8> {
    let mut v = json!({
        "version": LOOP, "engagement_id": u(0xe101), "campaign_id": u(0xe102),
        "decision_id": u(0xe105), "permission_id": u(0xe103), "mission_revision": 1,
        "run_profile": rp, "execution_mode": mode, "inference_profile": p,
        "question": "Select next authorized candidate.",
        "references": [{"id": "asset_0", "owner": "operator"}],
        "manifests": [{"reference": "dns_udp"}],
        "candidates": [{"reference": "asset_0", "priority": "high"}],
        "predecessor": {"availability": "absent", "reason": "first_decision"}
    });
    if let Some(b) = live {
        v["live_binding"] = b;
    }
    if let Some(q) = purp {
        v["qualification_purpose"] = q;
    }
    serde_json::to_vec(&v).unwrap()
}

fn prepare_d1(q: &Value) -> (PgRunStore, Uuid, Uuid, Uuid, Uuid, PreparedHttpsLoop) {
    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    let m = &q["context"]["mission"];
    let e = Uuid::parse_str(m["engagement_id"].as_str().unwrap()).unwrap();
    let c = Uuid::parse_str(m["campaign_id"].as_str().unwrap()).unwrap();
    let mut store = PgRunStore::new(native::client(BROKER));
    let req: RunRequest = serde_json::from_value(q.clone()).unwrap();
    assert!(store.start(&req, &"a".repeat(64)).unwrap());
    store.start_phase(run_id, RunPhase::DecisionOne).unwrap();
    store
        .claim_phase(run_id, RunPhase::DecisionOne, &"a".repeat(64))
        .unwrap();
    let d1 = store.https_step(run_id, 1).unwrap().decision_id;
    let ctx_req: ContextRequest = serde_json::from_value(q["context"].clone()).unwrap();
    let auto = AutonomousContext {
        binding: RunContextBinding {
            engagement_id: EngagementId(e),
            campaign_id: CampaignId(c),
            run_profile: LOOP.into(),
            execution_mode: MODE.into(),
            inference_profile: Some(ACTUAL.into()),
            release_sha256: "a".repeat(64),
            qualification_ref: Uuid::new_v4(),
        },
        predecessor: None,
    };
    let mut reader = PgContextReader::new(native::client(BROKER));
    let prep = https_loop::prepare(&mut reader, &ctx_req, OperationId(d1), run_id, &auto).unwrap();
    let pb = prep.prepared.utf8().as_bytes();
    store
        .bind_https_input(e, c, d1, run_id, &"b".repeat(64), &prep.endpoint_map, pb)
        .unwrap();
    (store, run_id, e, c, d1, prep)
}

#[test]
fn actual_pro_v2_rust_ingress_and_rejections_enforce_closed_tuple() {
    let reg_ok = |v: &Value| parse_register(&serde_json::to_vec(v).unwrap()).is_ok();
    let run_ok = |v: &Value| RunRequest::parse(&serde_json::to_vec(v).unwrap()).is_ok();
    let pack_ok = |b: &[u8]| inference_intent::preflight(b, 8192, 16384).is_ok();
    let lb = live_binding();
    for (p, r, rp, mode, live) in [
        (ACTUAL, ACTUAL_URL, LOOP, MODE, None),
        (FIXTURE, FIXTURE_URL, LOOP, MODE, None),
        (FIXTURE, FIXTURE_URL, LAB, "live", Some(lb.clone())),
    ] {
        assert!(reg_ok(&register_json(p, r, rp, mode, live)));
    }
    for (p, r, rp, mode, live) in [
        (ACTUAL, FIXTURE_URL, LOOP, MODE, None),
        (FIXTURE, ACTUAL_URL, LOOP, MODE, None),
        (ACTUAL, ACTUAL_URL, LAB, "live", Some(lb.clone())),
        (ACTUAL, FIXTURE_URL, LAB, "live", Some(lb.clone())),
        (ACTUAL, ACTUAL_URL, LOOP, MODE, Some(lb.clone())),
        (ACTUAL, ACTUAL_URL, LOOP, MODE, Some(Value::Null)),
        (V1, ACTUAL_URL, LOOP, MODE, None),
        (BOGUS, ACTUAL_URL, LOOP, MODE, None),
    ] {
        assert!(!reg_ok(&register_json(p, r, rp, mode, live)), "reject {p}");
    }
    let mut bad_lim = register_json(ACTUAL, ACTUAL_URL, LOOP, MODE, None);
    bad_lim["m1_inference_permission"]["operation_limits"]["output_tokens_per_call"] = json!(4096);
    assert!(!reg_ok(&bad_lim));

    let ok_run =
        RunRequest::parse(&serde_json::to_vec(&run_json(ACTUAL, LOOP, MODE, None)).unwrap())
            .unwrap();
    assert_eq!(
        ok_run.timing_profile().unwrap().unwrap().name,
        PRO_V2_TIMING_PROFILE
    );
    assert!(run_ok(&run_json(FIXTURE, LOOP, MODE, None)));
    assert!(run_ok(&run_json(FIXTURE, LAB, "live", Some(lb.clone()))));
    for (p, rp, mode, live) in [
        (ACTUAL, LAB, "live", Some(lb.clone())),
        (ACTUAL, LOOP, "live", Some(lb.clone())),
        (ACTUAL, LOOP, MODE, Some(lb.clone())),
        (ACTUAL, LOOP, MODE, Some(Value::Null)),
        (ACTUAL, LAB, MODE, None),
        (V1, LOOP, MODE, None),
        (BOGUS, LOOP, MODE, None),
    ] {
        assert!(!run_ok(&run_json(p, rp, mode, live)), "reject run {p}");
    }

    assert!(pack_ok(&pack_json(ACTUAL, LOOP, MODE, None, None)));
    assert!(pack_ok(&pack_json(FIXTURE, LOOP, MODE, None, None)));
    for (p, rp, mode, live, purp) in [
        (
            ACTUAL,
            LAB,
            "live",
            Some(lb.clone()),
            Some(json!("HTTPS_LOOP_LAB")),
        ),
        (ACTUAL, LOOP, MODE, Some(lb), None),
        (ACTUAL, LOOP, MODE, Some(Value::Null), None),
        (ACTUAL, LOOP, MODE, None, Some(json!("HTTPS_LOOP_LAB"))),
        (ACTUAL, LOOP, MODE, None, Some(Value::Null)),
        (BOGUS, LOOP, MODE, None, None),
    ] {
        assert!(!pack_ok(&pack_json(p, rp, mode, live, purp)));
    }
}

#[ignore]
#[test]
fn actual_pro_v2_producer_sql_reservation_intent_and_sql_rejections() {
    let _guard = native::qualified_existing_db();
    ensure_0039_overlay();
    let q = actual_pro_v2_registered_with(0xe101, |_| {});
    let m = &q["context"]["mission"];
    let perm = Uuid::parse_str(m["permission_id"].as_str().unwrap()).unwrap();
    let (_store, _run_id, e, c, d1, prep) = prepare_d1(&q);

    let pack_bytes = prep.prepared.utf8().as_bytes();
    let pv: Value = serde_json::from_slice(pack_bytes).unwrap();
    assert_eq!(
        (&pv["version"], &pv["inference_profile"]),
        (&json!(LOOP), &json!(ACTUAL))
    );
    assert!(pv.get("live_binding").is_none() && pv.get("qualification_purpose").is_none());

    let mut broker_client = native::client(BROKER);
    let r_row = broker_client
        .query_one("SELECT generation, generation_call_id, repair_call_id FROM execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))", &[&d1, m, &e, &c])
        .unwrap();
    let res = Reservation {
        generation: r_row.get::<_, i64>(0) as u64,
        generation_call: r_row.get(1),
        repair_call: r_row.get(2),
        shared_deadline: Instant::now() + Duration::from_secs(120),
    };
    let scope = Scope {
        engagement: e,
        campaign: c,
        decision: d1,
        mission_revision: 1,
        permission: perm,
    };
    let bound_cap = inference_intent::preflight(pack_bytes, 8192, 16384).unwrap();
    let gen_call =
        inference_intent::bind(&scope, &res, res.generation_call, pack_bytes, None).unwrap();
    let rep_call = inference_intent::bind(
        &scope,
        &res,
        res.repair_call,
        pack_bytes,
        Some(&["proposal_shape"]),
    )
    .unwrap();
    inference_intent::verify_owned(&gen_call, &scope, &res, res.generation_call).unwrap();
    inference_intent::verify_owned(&rep_call, &scope, &res, res.repair_call).unwrap();
    assert!(pack_bytes.len() <= 8192 && bound_cap <= 16384);
    println!(
        "ACTUAL_PRO_V2_BYTES pack={} gen_intent={} rep_intent={} preflight_8repair_bound={}",
        pack_bytes.len(),
        gen_call.configuration.len(),
        rep_call.configuration.len(),
        bound_cap
    );

    let intent: Value = serde_json::from_slice(&gen_call.configuration).unwrap();
    assert_eq!(
        (
            &intent["profile"],
            &intent["recipient"],
            &intent["model"],
            &intent["timing_profile"]
        ),
        (
            &json!(ACTUAL),
            &json!(ACTUAL_URL),
            &json!("deepseek-v4-pro"),
            &json!(PRO_V2_TIMING_PROFILE)
        )
    );
    assert_eq!(
        intent["settings"],
        json!({"reasoning_effort": "low", "max_output_tokens": 8192, "stream": false, "store": false, "tools": []})
    );
    assert!(intent.get("live_binding").is_none() && intent.get("qualification_purpose").is_none());

    let mut admin = native::admin();
    let binding_ok = |a: &mut postgres::Client, cid, cfg: &str, dig: &str| -> bool {
        a.query_one(
            "SELECT execution.genai_binding_valid($1,$2,$3,$4,$5,$6,'',$7)",
            &[&e, &c, &d1, &cid, &prep.prepared.utf8(), &cfg, &dig],
        )
        .unwrap()
        .get(0)
    };
    for (cid, call) in [
        (res.generation_call, &gen_call),
        (res.repair_call, &rep_call),
    ] {
        let cfg = std::str::from_utf8(&call.configuration).unwrap();
        assert!(binding_ok(&mut admin, cid, cfg, &call.digest));
    }
    let bad_cfg = std::str::from_utf8(&gen_call.configuration)
        .unwrap()
        .replace(ACTUAL_URL, FIXTURE_URL);
    assert!(!binding_ok(
        &mut admin,
        res.generation_call,
        &bad_cfg,
        &gen_call.digest
    ));

    for ok in [
        permission_json(ACTUAL, ACTUAL_URL),
        permission_json(FIXTURE, FIXTURE_URL),
        lab_permission(permission_json(FIXTURE, FIXTURE_URL)),
    ] {
        assert!(sql_matches(&mut broker_client, &ok));
    }
    let mut with_live = permission_json(ACTUAL, ACTUAL_URL);
    with_live["live_binding"] = live_binding();
    let mut with_purpose = permission_json(ACTUAL, ACTUAL_URL);
    with_purpose["qualification_purpose"] = json!("HTTPS_LOOP_LAB");
    for bad in [
        permission_json(ACTUAL, FIXTURE_URL),
        permission_json(FIXTURE, ACTUAL_URL),
        lab_permission(permission_json(ACTUAL, ACTUAL_URL)),
        lab_permission(permission_json(ACTUAL, FIXTURE_URL)),
        permission_json(BOGUS, ACTUAL_URL),
        with_live,
        with_purpose,
    ] {
        assert!(
            !sql_matches(&mut broker_client, &bad),
            "SQL must reject {bad}"
        );
    }

    let run = |b: &mut postgres::Client, v: &Value| {
        b.query_one("SELECT execution.run_start($1,$2)", &[v, &"a".repeat(64)])
            .is_err()
    };
    let mut q_mismatch = actual_pro_v2_registered_with(0xe102, |_| {});
    q_mismatch["inference_profile"] = json!(FIXTURE);
    assert!(run(&mut broker_client, &q_mismatch));
    let mut q_lab_actual = q_mismatch;
    (
        q_lab_actual["run_profile"],
        q_lab_actual["execution_mode"],
        q_lab_actual["inference_profile"],
        q_lab_actual["live_binding"],
    ) = (json!(LAB), json!("live"), json!(ACTUAL), live_binding());
    assert!(run(&mut broker_client, &q_lab_actual));
}

#[ignore]
#[test]
fn actual_pro_v2_preserves_protected_capacity_and_rejects_short_budget_or_time() {
    let _guard = native::qualified_existing_db();
    ensure_0039_overlay();
    let mut broker = native::client(BROKER);
    let q_ok = actual_pro_v2_registered_with(0xe201, |raw| {
        let c = &mut raw["m1_inference_permission"]["campaign_limits"];
        (
            c["calls"],
            c["request_bytes"],
            c["response_bytes"],
            c["output_token_capacity"],
            c["wall_time_ms"],
        ) = (
            json!(4),
            json!(65536),
            json!(262144),
            json!(32768),
            json!(498000),
        );
    });
    let (mut store, r_ok, e_ok, c_ok, d1, _prep) = prepare_d1(&q_ok);
    broker.query_one("SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))", &[&d1, &q_ok["context"]["mission"], &e_ok, &c_ok]).unwrap();
    store.protect_next_decision(r_ok, 1).unwrap();
    let held: Value = broker
        .query_one(
            "SELECT execution.https_loop_protected_held($1,$2)",
            &[&e_ok, &c_ok],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        held,
        json!({"calls": 2, "request_bytes": 32768, "response_bytes": 131072, "output_token_capacity": 16384, "wall_time_ms": 249000})
    );

    let q_short = actual_pro_v2_registered_with(0xe202, |raw| {
        raw["m1_inference_permission"]["campaign_limits"]["calls"] = json!(3);
    });
    let (mut short_store, r_short, e_short, c_short, d1_short, _prep) = prepare_d1(&q_short);
    broker
        .query_one(
            "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
            &[&d1_short, &q_short["context"]["mission"], &e_short, &c_short],
        )
        .unwrap();
    assert_eq!(
        short_store.protect_next_decision(r_short, 1).unwrap_err(),
        Fail::State("inference_capacity_exhausted")
    );

    let now_s: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let mut q_over = actual_pro_v2_registered_with(0xe203, |_| {});
    q_over["deadline_at_ms"] = json!(now_s * 1000 + 645_000);
    assert!(
        broker
            .query_one(
                "SELECT execution.run_start($1,$2)",
                &[&q_over, &"a".repeat(64)]
            )
            .is_err()
    );

    let q_low = actual_pro_v2_registered_with(0xe204, |_| {});
    let r_low = Uuid::parse_str(q_low["run_id"].as_str().unwrap()).unwrap();
    let a_low = Uuid::parse_str(q_low["seed_names"][0]["asset_ref"].as_str().unwrap()).unwrap();
    let seed_name = q_low["seed_names"][0]["name"].as_str().unwrap().to_owned();
    store
        .start(&serde_json::from_value(q_low).unwrap(), &"a".repeat(64))
        .unwrap();
    store.start_phase(r_low, RunPhase::DecisionOne).unwrap();
    store
        .claim_phase(r_low, RunPhase::DecisionOne, &"a".repeat(64))
        .unwrap();
    store.protect_next_decision(r_low, 1).unwrap();
    native::admin()
        .execute(
            "UPDATE execution.autonomous_runs SET deadline_at_ms = floor(extract(epoch FROM clock_timestamp())*1000)::bigint + 260000 WHERE run_id=$1",
            &[&r_low],
        )
        .unwrap();
    broker
        .execute(
            "SELECT execution.record_https_loop_dns_intent($1,1,$2,$3)",
            &[&r_low, &a_low, &seed_name],
        )
        .unwrap();
    native::admin().execute("INSERT INTO execution.autonomous_phases VALUES ($1,'decision_one','completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint) ON CONFLICT DO NOTHING", &[&r_low]).unwrap();
    store.start_phase(r_low, RunPhase::AcquisitionOne).unwrap();
    store
        .claim_phase(r_low, RunPhase::AcquisitionOne, &"a".repeat(64))
        .unwrap();
    assert_eq!(
        store.reserve_https_dns(r_low, 1, "initial", 0).unwrap_err(),
        Fail::State("fixture_time_insufficient")
    );
}

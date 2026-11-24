use crate::admission::{assert_42501, broker, db_err, now_ms};
use crate::episode::{
    CliRun, ControlDir, FakeHttpsBoundary, assert_inspect_counts, cli_json, drive_d1_dns, pack_in,
    pg_now_ms, run_decision_phase, start_cli_lab_run, step_prop,
};
use crate::genai::{Reply, Server, envelope};
use crate::lab_support;
use crate::recovery::Renewals;
use crate::terrain::terrain_support::native;
use duskweave::m1_autonomous_context::{AutonomousContext, RunContextBinding};
use duskweave::m1_context::https_loop::prepare;
use duskweave::m1_effect_control::https::acquire_with_clock;
use duskweave::m1_https::HttpsOutcome;
use duskweave::m1_inference_profile::FIXTURE_PRO_V2_PROFILE;
use duskweave::m1_reasoning::inference::{Reservation, Scope};
use duskweave::m1_reasoning::inference_intent;
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::postgres_m1_effect::https_loop::PgHttpsAttemptStore;
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_terrain::https::PgHttpsTerrainStore;
use duskweave::postgres_mission::PgContextReader;
use duskweave::postgres_terrain_history::record_https_terrain_history;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(crate) fn drive_d1_d2(
    (e, c, run_id, rel): (Uuid, Uuid, Uuid, &str),
    runs: &mut PgRunStore,
    rec: &RunRecord,
    ctrl: &ControlDir,
    (outcome, obs_ms, perceived_s): (HttpsOutcome, Option<i64>, Option<i64>),
    check_refresh: bool,
) {
    let (mut m_store, mut pts) = drive_d1_dns(e, c, run_id, runs, rec, rel, ctrl);

    run_decision_phase(runs, run_id, RunPhase::DecisionTwo, ctrl);
    runs.resolve_https_step_action(rec, 2).unwrap();
    if check_refresh {
        let r = format!("{:?}", runs.https_refresh_decision(run_id, 2).unwrap());
        assert!(r == "NotNeeded" || r == "Fresh", "{r}");
    }
    runs.protect_next_decision(run_id, 2).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionTwo).unwrap();
    runs.claim_phase(run_id, RunPhase::AcquisitionTwo, rel)
        .unwrap();
    let offset = pg_now_ms() - now_ms();
    let mut h_s = PgHttpsAttemptStore::new(broker());
    let mut h_b = FakeHttpsBoundary::with_observed_ms(outcome, obs_ms.unwrap_or(0));
    let mut clk = || Ok(now_ms() + offset + 10);
    acquire_with_clock(&mut h_s, &mut h_b, run_id, 2, &mut clk).unwrap();
    let mut head_res = runs
        .read_committed_https_result(run_id, 2)
        .unwrap()
        .unwrap();
    if let Some(p_s) = perceived_s {
        head_res.perceived_at = p_s;
    }
    let op = OperationId(
        runs.https_step(run_id, 2)
            .unwrap()
            .head_terrain_operation_id
            .unwrap(),
    );
    let https_ev = PgHttpsTerrainStore::new(&mut pts)
        .record_https(&mut m_store, &head_res, op)
        .unwrap();
    record_https_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &https_ev)
        .unwrap();
    runs.complete_phase(run_id, RunPhase::AcquisitionTwo)
        .unwrap();
}

fn drive_d1_d2_https_outcome(
    server: &Server,
    outcome: HttpsOutcome,
    observed_override_ms: Option<i64>,
    perceived_override_s: Option<i64>,
) -> (CliRun, Uuid, ControlDir, Renewals) {
    let before = server.count();
    let qref = Uuid::new_v4();
    let mut run = start_cli_lab_run(qref);
    let (ctrl, renewals) = (ControlDir::new(), Renewals::start(run.2));
    drive_d1_d2(
        (run.0, run.1, run.2, &run.6),
        &mut run.4,
        &run.5,
        &ctrl,
        (outcome, observed_override_ms, perceived_override_s),
        false,
    );
    assert_eq!(server.count() - before, 2);
    (run, qref, ctrl, renewals)
}

fn handler_for_d1_d2_d3_retain(req_body: &[u8]) -> Reply {
    let wire: Value = serde_json::from_slice(req_body).unwrap();
    let pack = pack_in(&wire).expect("https_loop context pack");
    let refs = pack["references"].as_array().unwrap();
    let h_opt = refs.iter().find(|r| r["id"] == "https_0");
    let feedback = pack["predecessor"]["availability"] == "committed";
    let prop = if !feedback && h_opt.is_none() {
        step_prop(
            &["mission", "asset_0"],
            json!({"category":"observe_more","references":["mission","asset_0"],"manifest_reference":"dns_udp","view_request":null,"dependency_refresh":"none"}),
        )
    } else if h_opt.is_none() {
        step_prop(
            &["previous_decision", "asset_0", "endpoint_0"],
            json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":"https-head-v1","dependency_refresh":"dns_once_if_stale"}),
        )
    } else {
        let h0 = h_opt.unwrap();
        assert_eq!(h0["status"], "withheld");
        assert_eq!(h0["outcome"], "execution_refused");
        assert_eq!(h0["source"], "issued_https_acquisition");
        assert_eq!(h0["endpoint_reachable"], false);
        assert_eq!(h0["status_code"], Value::Null);
        assert_eq!(h0["redirect"], "none");
        assert_eq!(h0["whole_host_proven"], false);
        assert_eq!(h0["objective_completed"], false);
        assert!(!h0["limitations"].as_array().unwrap().is_empty());
        step_prop(
            &["asset_0", "previous_decision", "https_0"],
            json!({"category":"retain","references":["asset_0","https_0"],"manifest_reference":null,"dependency_refresh":"none"}),
        )
    };
    Reply::Length(200, serde_json::to_vec(&envelope(&prop)).unwrap())
}

#[ignore]
#[test]
fn cli_m1_run_phase_execution_refused_feedback_reaches_d3_retain_and_zero_replay_recover() {
    let _guard = native::qualified_existing_db();
    let server = Server::lab(|req| handler_for_d1_d2_d3_retain(&req.body));
    let ((e, c, run_id, _, mut runs, _, _), _, ctrl, renewals) =
        drive_d1_d2_https_outcome(&server, HttpsOutcome::ExecutionRefused, None, None);

    let step2 = runs.https_step(run_id, 2).unwrap();
    let mut admin = native::admin();
    let payload: Value = admin
        .query_one("SELECT payload FROM terrain.https_observations WHERE engagement_id=$1 AND campaign_id=$2", &[&e, &c])
        .unwrap().get(0);
    assert_eq!(payload["claim"]["claim_status"], "withheld");
    assert_eq!(payload["evaluation"]["outcome"], "execution_refused");
    assert_eq!(payload["evaluation"]["usable_endpoint_premise"], false);
    assert_eq!(payload["claim"]["endpoint_reachable"], false);
    assert_eq!(payload["claim"]["status_code"], Value::Null);

    run_decision_phase(&mut runs, run_id, RunPhase::DecisionThree, &ctrl);
    runs.terminate_https_loop(run_id, 3, "retained").unwrap();
    renewals.stop();
    runs.stop(run_id).unwrap();
    assert_eq!(server.count(), 3);

    let (ok_i, inspect) = cli_json(&["m1-run-inspect", "--run", &run_id.to_string()]);
    assert!(ok_i, "{inspect}");
    for _ in 0..2 {
        let (ok_r, recover) = cli_json(&["m1-run-recover", "--run", &run_id.to_string()]);
        assert!(ok_r, "{recover}");
        assert_eq!(recover, inspect);
    }
    assert_eq!(server.count(), 3);
    assert_inspect_counts(&inspect, "retained", 3, 1, 1);
    assert_eq!(inspect["decision_history_count"], 3);
    assert_eq!(inspect["terrain_history_count"], 2);
    assert_eq!(inspect["steps"][2]["disposition"], "retained");
    let hr = &inspect["https_results"][0];
    assert_eq!(hr["outcome"], "execution_refused");
    assert_eq!(hr["status_code"], Value::Null);
    assert_eq!(hr["whole_host_proven"], false);
    assert_eq!(hr["objective_completed"], false);
    assert_eq!(hr["attempt"], json!(step2.head_attempt_id.unwrap()));
    assert_eq!(hr["result"], json!(step2.head_result_id.unwrap()));

    let payload_after: Value = admin
        .query_one("SELECT payload FROM terrain.https_observations WHERE engagement_id=$1 AND campaign_id=$2", &[&e, &c])
        .unwrap().get(0);
    assert_eq!(payload_after, payload);
}

fn try_broker_may_send(
    run: &CliRun,
    d3: Uuid,
    base_map: &duskweave::m1_https::DecisionEndpointMap,
    pack_val: &Value,
) -> Result<(), String> {
    let (e, c, run_id, reg, _, rec, _) = run;
    let pack_bytes = serde_json::to_vec(pack_val).unwrap();
    let pack_digest = lab_support::sha256_hex(&pack_bytes);
    let mut ep_map = base_map.clone();
    ep_map.pack_digest = pack_digest.clone();
    let (_, template_json, template_digest) =
        inference_intent::semantic_template(&pack_bytes).map_err(|e| e.to_owned())?;
    let map_json = serde_json::to_value(&ep_map).unwrap();
    let input_hash = lab_support::sha256_hex(format!("tamper-probe-{pack_digest}").as_bytes());
    let perm = Uuid::parse_str(
        reg["m1_inference_permission"]["permission_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let q_json = serde_json::to_value(&rec.request.context.mission).unwrap();
    let mut brk = broker();
    let mut tx = brk.transaction().unwrap();
    let src: Value = tx
        .query_one(
            "SELECT mission.current_inference_publication($1,$2,$3)",
            &[e, c, &d3],
        )
        .unwrap()
        .get(0);
    tx.execute(
        "SELECT execution.bind_https_loop_input($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        &[
            e,
            c,
            &d3,
            run_id,
            &input_hash,
            &pack_digest,
            &map_json,
            &template_json,
            &template_digest,
        ],
    )
    .map_err(|e| db_err(&e))?;
    let r_val: Value = tx
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation($1,$2,$3,$4))",
            &[&"reserve", &d3, &q_json, &src],
        )
        .map_err(|e| db_err(&e))?
        .get(0);
    let resv = Reservation {
        generation: r_val["generation"].as_u64().unwrap(),
        generation_call: Uuid::parse_str(r_val["generation_call_id"].as_str().unwrap()).unwrap(),
        repair_call: Uuid::parse_str(r_val["repair_call_id"].as_str().unwrap()).unwrap(),
        shared_deadline: Instant::now() + Duration::from_secs(60),
    };
    let scope = Scope {
        engagement: *e,
        campaign: *c,
        decision: d3,
        mission_revision: 1,
        permission: perm,
    };
    let b = inference_intent::bind(&scope, &resv, resv.generation_call, &pack_bytes, None)
        .map_err(|e| e.to_owned())?;
    fn utf8(x: &[u8]) -> &str {
        std::str::from_utf8(x).unwrap()
    }
    tx.query_one(
        "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
        &[
            e,
            c,
            &d3,
            &resv.generation_call,
            &utf8(&b.pack),
            &utf8(&b.configuration),
            &utf8(&b.body),
            &b.digest,
        ],
    )
    .map_err(|e| db_err(&e))?;
    tx.rollback().unwrap();
    Ok(())
}

fn prepare_d3_pack(
    run: &mut CliRun,
    d3: Uuid,
) -> (Value, duskweave::m1_https::DecisionEndpointMap) {
    let (e, c, run_id, _, runs, rec, rel) = run;
    let mut reader = PgContextReader::new(broker());
    let pred = runs.https_predecessor(*run_id, 3).unwrap();
    let auto_ctx = AutonomousContext {
        binding: RunContextBinding {
            engagement_id: EngagementId(*e),
            campaign_id: CampaignId(*c),
            run_profile: rec.request.run_profile.clone(),
            execution_mode: if rec.request.live() {
                "live"
            } else {
                "controlled_fixture"
            }
            .into(),
            inference_profile: Some(FIXTURE_PRO_V2_PROFILE.into()),
            release_sha256: rel.clone(),
            qualification_ref: rec.qualification_ref,
        },
        predecessor: pred,
    };
    let prepared = prepare(
        &mut reader,
        &rec.request.context,
        OperationId(d3),
        *run_id,
        &auto_ctx,
    )
    .unwrap();
    let pack_val: Value = serde_json::from_slice(prepared.prepared.utf8().as_bytes()).unwrap();
    (pack_val, prepared.endpoint_map)
}

#[ignore]
#[test]
fn restricted_admission_rejects_forged_https_withheld_and_tampered_feedback() {
    let _guard = native::qualified_existing_db();
    let server = Server::lab(|req| handler_for_d1_d2_d3_retain(&req.body));

    // Separate campaign with another ExecutionRefused claim to test cross-campaign entity swap.
    let (mut other, _, o_ctrl, o_ren) =
        drive_d1_d2_https_outcome(&server, HttpsOutcome::ExecutionRefused, None, None);
    let other_claim: Uuid = native::admin()
        .query_one("SELECT claim_id FROM terrain.https_observations WHERE engagement_id=$1 AND campaign_id=$2", &[&other.0, &other.1])
        .unwrap().get(0);
    o_ren.stop();
    let _ = (o_ctrl, other.4.stop(other.2));

    // Separate campaign producing genuine `scope_or_policy_withheld` (perceived before m1_permission.starts_at).
    let pre_start_s = pg_now_ms() / 1000 - 120;
    let (mut scoped, s_ctrl, s_ren) = crate::upgrade::drive_fixture_scoped_d1_d2(pre_start_s);
    let scoped_obs: Value = native::admin()
        .query_one("SELECT payload FROM terrain.https_observations WHERE engagement_id=$1 AND campaign_id=$2", &[&scoped.0, &scoped.1])
        .unwrap().get(0);
    assert_eq!(scoped_obs["claim"]["claim_status"], "withheld");
    assert_eq!(
        scoped_obs["evaluation"]["outcome"],
        "scope_or_policy_withheld"
    );
    let scoped_d3 = scoped.4.https_step(scoped.2, 3).unwrap().decision_id;
    scoped
        .4
        .start_phase(scoped.2, RunPhase::DecisionThree)
        .unwrap();
    scoped
        .4
        .claim_phase(scoped.2, RunPhase::DecisionThree, &scoped.6)
        .unwrap();
    let (scoped_pack, scoped_map) = prepare_d3_pack(&mut scoped, scoped_d3);
    let scoped_err = try_broker_may_send(&scoped, scoped_d3, &scoped_map, &scoped_pack)
        .expect_err("scope_or_policy_withheld must be refused");
    assert!(
        scoped_err.contains("fixture_stale_premises"),
        "{scoped_err}"
    );
    s_ren.stop();
    let _ = (s_ctrl, scoped.4.stop(scoped.2));

    // Primary run (`observed_at_ms = None` captures real D2 acquisition time, well after `occurred_at`).
    let (mut s, _, ctrl, renewals) =
        drive_d1_d2_https_outcome(&server, HttpsOutcome::ExecutionRefused, None, None);
    let d3 = s.4.https_step(s.2, 3).unwrap().decision_id;
    s.4.start_phase(s.2, RunPhase::DecisionThree).unwrap();
    s.4.claim_phase(s.2, RunPhase::DecisionThree, &s.6).unwrap();
    let (base_pack, base_map) = prepare_d3_pack(&mut s, d3);
    let h0_case = |k: &str, v: Value| -> Value {
        let mut p = base_pack.clone();
        p["references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["id"] == "https_0")
            .unwrap()[k] = v;
        p
    };
    let cur_p = base_pack["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "https_0")
        .unwrap()["perceived_at"]
        .as_i64()
        .unwrap();
    let mut future_pack = base_pack.clone();
    future_pack["evaluated_at"] = json!(cur_p - 1);
    for v in future_pack["owner_views"].as_array_mut().unwrap() {
        if v["owner"] == "mission" {
            v["evaluated_at"] = json!(cur_p - 1);
        }
    }
    let (conn, fsrc, past) = (
        json!("connection_failed"),
        json!("fixture_https_acquisition"),
        json!(cur_p - 1),
    );
    for (label, key, val) in [
        ("forged_observed_status", "status", json!("observed")),
        ("forged_reachable_true", "endpoint_reachable", json!(true)),
        ("invented_status_code", "status_code", json!(200)),
        ("invented_redirect", "redirect", json!("refused_redirect")),
        ("stale_revision", "revision", json!(99)),
        ("forged_entity", "entity", json!(Uuid::new_v4())),
        ("cross_campaign_entity", "entity", json!(other_claim)),
        ("mismatched_outcome_conn", "outcome", conn),
        ("mismatched_outcome_head", "outcome", json!("head_response")),
        ("wrong_parent_revision", "parent_revision", json!(99)),
        ("tampered_source", "source", fsrc),
        ("tampered_perceived_at", "perceived_at", past),
        ("tampered_limitations", "limitations", json!(["forged"])),
    ] {
        let err = try_broker_may_send(&s, d3, &base_map, &h0_case(key, val))
            .expect_err(&format!("{label} must fail"));
        assert!(!err.contains("42501"), "{label} failed on privilege: {err}");
        assert!(
            err.contains("fixture_stale_premises")
                || (label == "tampered_source" && err.contains("run_pack_source_refused")),
            "{label} unexpected: {err}"
        );
    }
    let fut_err = try_broker_may_send(&s, d3, &base_map, &future_pack)
        .expect_err("future_perception must fail");
    assert!(
        !fut_err.contains("42501") && fut_err.contains("fixture_stale_premises"),
        "future_perception: {fut_err}"
    );
    try_broker_may_send(&s, d3, &base_map, &base_pack).unwrap();
    assert_42501(
        broker().query_one(
            "SELECT execution.assert_current_pack_terrain($1,$2,$3,$4)",
            &[&s.0, &s.1, &d3, &base_pack],
        ),
        "broker direct assert_current_pack_terrain",
    );
    renewals.stop();
    let _ = (ctrl, s.4.stop(s.2));
}

use crate::admission::{
    assert_42501, base_registration_json, broker, lab_run_request, now_ms, register_lab_campaign,
    seed_qual, start_lab_run,
};
use crate::denied_release::{claim_cap_until_s, sleep_until_db_ms};
use crate::episode::{
    ControlDir, assert_inspect_counts, cli_json, drive_d1_dns, pack_in, run_decision_phase,
    start_cli_lab_run, step_prop,
};
use crate::genai::{Reply, Server, envelope};
use crate::lab_support::{sample_https_safe_result, sample_release_sha, sha256_hex};
use crate::lineage::{commit_synthetic_step_decision, drive_step1_live_dns};
use crate::recovery::Renewals;
use crate::terrain::terrain_support::native;
use duskweave::m1_https::{HttpsIpcRequest, HttpsOutcome};
use duskweave::m1_inference_profile::{FIXTURE_PRO_V2_PROFILE, HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER};
use duskweave::m1_run::{RunPhase, RunRecord, RunRequest};
use duskweave::postgres_m1_run::PgRunStore;
use postgres::{Client, types::ToSql};
use serde_json::{Value, json};
use uuid::Uuid;

struct ActiveHead {
    e: Uuid,
    c: Uuid,
    id: Uuid,
    runs: PgRunStore,
    att: Uuid,
    gen_id: i64,
    charged: Value,
}

fn qv(c: &mut Client, q: &str, p: &[&(dyn ToSql + Sync)]) -> Value {
    c.query_one(q, p).unwrap().get(0)
}
fn ex(c: &mut Client, q: &str, p: &[&(dyn ToSql + Sync)]) {
    c.execute(q, p).unwrap();
}
fn uid(v: &Value, k: &str) -> Uuid {
    Uuid::parse_str(v[k].as_str().unwrap()).unwrap()
}
fn cli(cmd: &str, id: Uuid) -> (bool, Value) {
    cli_json(&[cmd, "--run", &id.to_string()])
}
fn may_start(b: &mut Client, att: Uuid, g: i64) -> i64 {
    let q = "SELECT execution.live_attempt_may_start($1,$2)";
    b.query_one(q, &[&att, &g]).unwrap().get(0)
}
fn stop_att(b: &mut Client, att: Uuid, g: i64) {
    let q = "SELECT execution.live_attempt_stop($1,$2,true)";
    ex(b, q, &[&att, &g]);
}
fn charged(e: Uuid, c: Uuid) -> Value {
    let q = "SELECT execution.inference_capacity_snapshot($1,$2)";
    qv(&mut broker(), q, &[&e, &c])["charged"].clone()
}
fn ctrl(id: Uuid) -> (String, i64) {
    let q = "SELECT status, lease_until_ms FROM execution.autonomous_control WHERE run_id=$1";
    let r = native::admin().query_one(q, &[&id]).unwrap();
    (r.get(0), r.get(1))
}
fn unk(runs: &mut PgRunStore, id: Uuid, step: usize, kind: &str, reason: &str) {
    runs.mark_https_effect_unknown(id, step, kind, reason)
        .unwrap();
}

fn enter_acq(runs: &mut PgRunStore, id: Uuid, step: usize, ph: RunPhase, rel: &str) {
    runs.protect_next_decision(id, step).unwrap();
    runs.start_phase(id, ph).unwrap();
    runs.claim_phase(id, ph, rel).unwrap();
}

fn reserve_head(b: &mut Client, id: Uuid) -> (Uuid, Uuid, i64, Value) {
    let q = "SELECT execution.live_https_loop_head_reserve($1,2)";
    let rq = qv(b, q, &[&id])["request"].clone();
    let (att, res) = (uid(&rq, "attempt"), uid(&rq, "result_id"));
    let g = rq["session_generation"].as_i64().unwrap();
    may_start(b, att, g);
    let qp = "SELECT execution.live_attempt_head_phase($1,$2,$3)";
    for ph in ["tls", "head"] {
        b.query_one(qp, &[&att, &g, &ph]).unwrap();
    }
    (att, res, g, rq)
}

fn prep_step2(runs: &mut PgRunStore, rec: &RunRecord, reg: &Value, id: Uuid, dep: &str) {
    let intent = json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":"https-head-v1","dependency_refresh":dep});
    commit_synthetic_step_decision(runs, rec, reg, 2, intent);
    enter_acq(runs, id, 2, RunPhase::AcquisitionTwo, &sample_release_sha());
}

fn start_active_head_step2() -> ActiveHead {
    let (e, c, id, reg, mut runs, rec) = start_lab_run(Uuid::new_v4());
    let renew = Renewals::start(id);
    drive_step1_live_dns(e, c, id, &reg, &mut runs, &rec, 30);
    prep_step2(&mut runs, &rec, &reg, id, "none");
    let (att, _, gen_id, _) = reserve_head(&mut broker(), id);
    let charged = charged(e, c);
    // Stop renewal worker before run_unknown so Renewals does not hit non-stopped unknown lease refusal.
    renew.stop();
    ActiveHead {
        e,
        c,
        id,
        runs,
        att,
        gen_id,
        charged,
    }
}

const ACTIVE_SQL: &str = "
SELECT lec.phase, lec.granted_phase, lec.possibly_started, lec.disposal_confirmed,
  EXISTS(SELECT 1 FROM execution.live_effect_events ev WHERE ev.attempt=$1 AND ev.phase='may_start'),
  EXISTS(SELECT 1 FROM execution.live_effect_events ev WHERE ev.attempt=$1 AND ev.phase='phase_head'),
  EXISTS(SELECT 1 FROM execution.live_effect_events ev WHERE ev.attempt=$1 AND ev.phase='result_recorded'),
  EXISTS(SELECT 1 FROM execution.autonomous_phases ap WHERE ap.run_id=$2 AND ap.phase='acquisition_two' AND ap.status='unknown')
FROM execution.live_effect_sessions les JOIN execution.live_effect_control lec USING (attempt)
WHERE les.attempt=$1 AND les.run_id=$2 AND les.generation=$3";

fn assert_stopped(ctx: &mut ActiveHead, out: Option<&str>, disp: Option<&str>, prot: &str) {
    assert_eq!(ctrl(ctx.id), ("stopped".into(), 0));
    assert_eq!(ctx.runs.inspect(ctx.id).unwrap().status, "stopped");
    assert!(ctx.runs.check(ctx.id).is_err() && ctx.runs.renew(ctx.id).is_err());
    let st = ctx.runs.https_steps(ctx.id).unwrap();
    let (s0, s1, s2) = (&st[0], &st[1], &st[2]);
    assert!(s0.status == "completed" && s0.dns_result_id.is_some());
    assert!(s1.status == "stopped" && s1.head_attempt_id == Some(ctx.att));
    assert!(s1.head_result_id.is_none() && s1.head_outcome.as_deref() == out);
    assert!(s1.disposition.as_deref() == disp && s1.protection_state == prot);
    assert!(s2.status == "stopped" && charged(ctx.e, ctx.c) == ctx.charged);
    let r = native::admin()
        .query_one(ACTIVE_SQL, &[&ctx.att, &ctx.id, &ctx.gen_id])
        .unwrap();
    let ok = |i| r.get::<_, bool>(i);
    assert_eq!(r.get::<_, String>(0), "may_start");
    assert_eq!(r.get::<_, Option<String>>(1).as_deref(), Some("head"));
    assert!(ok(2) && !ok(3) && ok(4) && ok(5) && !ok(6) && ok(7));
}

#[ignore]
#[test]
fn stop_and_unknown_ordering_matrix_converges_on_independent_runs() {
    let _guard = native::qualified_existing_db();
    for (stop_first, mark_eff) in [(true, false), (false, false), (true, true), (false, true)] {
        let mut r = start_active_head_step2();
        if stop_first {
            r.runs.stop(r.id).unwrap();
        }
        if mark_eff {
            unk(&mut r.runs, r.id, 2, "head", "live_head_stopped");
        }
        r.runs.unknown(r.id, RunPhase::AcquisitionTwo).unwrap();
        r.runs.stop(r.id).unwrap();
        // Repeated stop/unknown/effect-mark operations stay idempotent and preserve first reason.
        if mark_eff {
            unk(&mut r.runs, r.id, 2, "head", "second");
        }
        r.runs.unknown(r.id, RunPhase::AcquisitionTwo).unwrap();
        r.runs.stop(r.id).unwrap();
        let (out, disp, prot) = if mark_eff {
            (Some("unknown"), Some("live_head_stopped"), "unknown")
        } else {
            (None, None, "protected")
        };
        assert_stopped(&mut r, out, disp, prot);
    }
}

fn check_legacy_non_https_stays_unknown() {
    let (rel, qref) = (sample_release_sha(), Uuid::new_v4());
    let (f, u) = (now_ms() - 60_000, now_ms() + 3_600_000);
    let (prof, inf) = ("autonomous-dns-genai-lab-v1", "fixture-genai-responses-v1");
    seed_qual(qref, prof, "LAB", HTTPS_LAB_RESOLVER, &rel, f, u);
    let (e, c, id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut reg = base_registration_json(e, c, qref, &rel);
    let ip = &mut reg["m1_inference_permission"];
    ip["version"] = json!("m1_inference_permission_v6");
    (ip["run_profile"], ip["profile"]) = (json!(prof), json!(inf));
    let lim: Value = serde_json::from_str(r#"{"output_tokens_per_call":4096,"decision_wall_time_ms":129000,"per_call_wall_time_ms":62000,"soft_completion_ms":60000,"shared_wall_time_ms":124000,"phase_wall_time_ms":134000}"#).unwrap();
    let m = ip["operation_limits"].as_object_mut().unwrap();
    m.extend(lim.as_object().unwrap().clone());
    let asset = reg["included_assets"][0].clone();
    let rule =
        json!([{"asset_ref": asset, "rule": {"exact": "seed.duskweave.test"}, "priority": 1}]);
    reg["m1_permission"]["contact_rules"] = rule;
    register_lab_campaign(&reg).unwrap();
    let mut rv = lab_run_request(&reg, id, qref, &rel);
    rv["version"] = json!("m1_autonomous_run_request_v4");
    (rv["run_profile"], rv["inference_profile"]) = (json!(prof), json!(inf));
    rv["deadline_at_ms"] = json!(now_ms() + 300_000);
    rv["seed_names"][0]["name"] = json!("seed.duskweave.test");
    let req = RunRequest::parse(&serde_json::to_vec(&rv).unwrap()).unwrap();
    let mut runs = PgRunStore::new(broker());
    assert!(runs.start(&req, &rel).unwrap());
    runs.start_phase(id, RunPhase::DecisionOne).unwrap();
    runs.unknown(id, RunPhase::DecisionOne).unwrap();
    runs.stop(id).unwrap();
    assert_eq!(ctrl(id), ("unknown".into(), 0));
}

fn check_role_and_scope_refusals(id: Uuid) {
    let qs = [
        "SELECT execution.run_stop($1)",
        "SELECT execution.run_unknown($1,'acquisition_two')",
        "SELECT execution.mark_https_loop_effect_unknown($1,2,'head','x')",
    ];
    let vars = [
        "DW_TEST_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_STOP_WRITER_DATABASE_URL",
    ];
    for var in vars {
        let mut c = native::client(var);
        for q in qs {
            assert_42501(c.execute(q, &[&id]), var);
        }
    }
    let mut other = PgRunStore::new(native::client("DW_NATIVE_OTHER_BROKER_DATABASE_URL"));
    assert!(other.stop(id).is_err() && other.unknown(id, RunPhase::AcquisitionTwo).is_err());
    assert!(other.mark_https_effect_unknown(id, 2, "head", "x").is_err());
    let m = Uuid::new_v4();
    let mut same = PgRunStore::new(broker());
    assert!(same.stop(m).is_err() && same.unknown(m, RunPhase::DecisionOne).is_err());
    assert!(same.mark_https_effect_unknown(m, 1, "dns", "x").is_err());
}

fn check_terminal_preserved(step: usize, phase: RunPhase, disp: &str, intent: Value) {
    let (e, c, id, reg, mut runs, rec) = start_lab_run(Uuid::new_v4());
    let renew = Renewals::start(id);
    if step == 2 {
        drive_step1_live_dns(e, c, id, &reg, &mut runs, &rec, 30);
    }
    commit_synthetic_step_decision(&mut runs, &rec, &reg, step, intent);
    runs.terminate_https_loop(id, step, disp).unwrap();
    renew.stop();
    runs.stop(id).unwrap();
    runs.unknown(id, phase).unwrap();
    if step == 2 {
        unk(&mut runs, id, 1, "dns", "late_completed");
        unk(&mut runs, id, 2, "head", "late_terminal");
        let s1 = runs.https_step(id, 1).unwrap();
        assert!(s1.status == "completed" && s1.disposition.is_none());
    } else {
        unk(&mut runs, id, 1, "dns", "late_unknown");
    }
    assert_eq!(runs.inspect(id).unwrap().status, disp);
    let st = runs.https_step(id, step).unwrap();
    assert!(st.status == "terminal" && st.disposition.as_deref() == Some(disp));
    assert!(st.head_outcome.is_none());
}

#[ignore]
#[test]
fn negative_controls_preserve_unknown_terminal_legacy_and_role_fences() {
    let _guard = native::qualified_existing_db();
    let mut u = start_active_head_step2();
    unk(&mut u.runs, u.id, 2, "head", "control_loss");
    u.runs.unknown(u.id, RunPhase::AcquisitionTwo).unwrap();
    assert_eq!(ctrl(u.id), ("unknown".into(), 0));
    let steps = u.runs.https_steps(u.id).unwrap();
    assert!(steps[1].status == "unknown" && steps[2].status == "pending");
    assert!(u.runs.check(u.id).is_err() && u.runs.renew(u.id).is_err());

    let def = json!({"category":"defer","references":["mission"],"manifest_reference":null,"dependency_refresh":"none"});
    let ret = json!({"category":"retain","references":["asset_0","endpoint_0"],"manifest_reference":null,"dependency_refresh":"none"});
    check_terminal_preserved(1, RunPhase::DecisionOne, "deferred", def);
    check_terminal_preserved(2, RunPhase::DecisionTwo, "retained", ret);
    check_legacy_non_https_stays_unknown();
    check_role_and_scope_refusals(u.id);
}

fn commit_safe_head_result(
    e: Uuid,
    c: Uuid,
    id: Uuid,
    d_id: Uuid,
    asset: Uuid,
    claim: (Uuid, i32),
) -> (Uuid, Uuid) {
    let mut b = broker();
    let (att, res, g, rq_v) = reserve_head(&mut b, id);
    let req: HttpsIpcRequest = serde_json::from_value(rq_v).unwrap();
    let obs = req.deadline_at_ms - i64::from(req.max_duration_ms);
    let sr = sample_https_safe_result(&req, HttpsOutcome::HeadResponse, obs, 200, "none", None);
    let safe = serde_json::to_value(sr).unwrap();
    let head_res = json!({
        "version": "m1_https_head_result_v1", "engagement_id": e, "campaign_id": c, "run_id": id,
        "decision_id": d_id, "attempt_id": att, "result_id": res, "asset_ref": asset,
        "endpoint_ref": "endpoint_0", "hostname": "head.duskweave.test", "ipv4": HTTPS_LAB_IPV4, "port": 443,
        "approved_path": "/", "method": "HEAD", "parent_claim_id": claim.0, "parent_claim_revision": claim.1,
        "perceived_at": obs / 1000, "uncertainty_seconds": 1, "outcome": "head_response",
        "status_code": 200, "redirect": "none", "source_kind": "issued_https_acquisition", "limitations": safe["limitations"]
    });
    let qh = "SELECT execution.live_attempt_head_result($1,$2,$3,$4)";
    ex(&mut b, qh, &[&att, &g, &safe, &head_res]);
    stop_att(&mut b, att, g);
    (att, res)
}

fn run_cli_lab_server() -> Server {
    Server::lab(|req| {
        let wire: Value = serde_json::from_slice(&req.body).unwrap();
        let pack = pack_in(&wire).expect("https_loop context pack");
        let refs = pack["references"].as_array().unwrap();
        let ep0 = refs.iter().any(|r| r["id"] == "endpoint_0");
        let prop = if !ep0 {
            let i = json!({"category":"observe_more","references":["mission","asset_0"],"manifest_reference":"dns_udp","view_request":null,"dependency_refresh":"none"});
            step_prop(&["mission", "asset_0"], i)
        } else {
            let i = json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":"https-head-v1","dependency_refresh":"dns_once_if_stale"});
            step_prop(&["previous_decision", "asset_0", "endpoint_0"], i)
        };
        Reply::Length(200, serde_json::to_vec(&envelope(&prop)).unwrap())
    })
}

fn check_refresh_pending_and_head_error_isolated() {
    let (e, c, id, reg, mut runs, rec) = start_lab_run(Uuid::new_v4());
    let renew = Renewals::start(id);
    drive_step1_live_dns(e, c, id, &reg, &mut runs, &rec, 1);
    sleep_until_db_ms(&mut broker(), claim_cap_until_s(e, c) * 1000 + 1100);
    prep_step2(&mut runs, &rec, &reg, id, "dns_once_if_stale");
    let mut b = broker();
    let qr = "SELECT execution.live_https_loop_dns_reserve($1,2,'dns_refresh')";
    let rq = qv(&mut b, qr, &[&id])["request"].clone();
    let (att, g) = (
        uid(&rq, "attempt"),
        rq["session_generation"].as_i64().unwrap(),
    );
    let dl = may_start(&mut b, att, g);
    let dns = json!({
        "schema": "m1_dns_ipc_v1", "kind": "RESULT", "attempt": att, "session_generation": g,
        "sequence": 3, "status": "completed", "observed_at_ms": dl - 2000, "rcode": 0,
        "truncated": false, "complete": true, "withheld_aliases": 0,
        "answers": [{"ipv4": HTTPS_LAB_IPV4, "ttl_seconds": 30}], "aliases": []
    });
    let qd = "SELECT execution.live_attempt_dns_result($1,$2,$3)";
    ex(&mut b, qd, &[&att, &g, &dns]);
    stop_att(&mut b, att, g);
    renew.stop();
    runs.stop(id).unwrap();
    unk(&mut runs, id, 2, "head", "live_head_stopped");
    runs.unknown(id, RunPhase::AcquisitionTwo).unwrap();
    let s2 = runs.https_step(id, 2).unwrap();
    assert!(s2.status == "stopped" && s2.refresh_outcome.as_deref() == Some("completed"));
    assert!(s2.refresh_result_id.is_some() && s2.head_result_id.is_none());
    assert_eq!(s2.head_outcome.as_deref(), Some("unknown"));
    let (ok_r, rec_r) = cli("m1-run-recover", id);
    assert!(ok_r && rec_r["terrain_history_count"] == json!(2));
    assert_inspect_counts(&rec_r, "stopped", 2, 2, 0);
}

#[ignore]
#[test]
fn committed_result_pending_delivery_survives_uncertainty_stop_and_cli_recover() {
    let _guard = native::qualified_existing_db();
    let server = run_cli_lab_server();
    let (e, c, id, reg, mut runs, rec, rel) = start_cli_lab_run(Uuid::new_v4());
    let (ctrl_d, renew) = (ControlDir::new(), Renewals::start(id));
    drive_d1_dns(e, c, id, &mut runs, &rec, &rel, &ctrl_d);
    run_decision_phase(&mut runs, id, RunPhase::DecisionTwo, &ctrl_d);
    runs.resolve_https_step_action(&rec, 2).unwrap();
    enter_acq(&mut runs, id, 2, RunPhase::AcquisitionTwo, &rel);
    let qc = "SELECT claim_id, claim_revision FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2";
    let cr = native::admin().query_one(qc, &[&e, &c]).unwrap();
    let claim = (cr.get::<_, Uuid>(0), cr.get::<_, i32>(1));
    let d2_id = runs.https_step(id, 2).unwrap().decision_id;
    let asset = Uuid::parse_str(reg["included_assets"][0].as_str().unwrap()).unwrap();
    let (att_h, res_h) = commit_safe_head_result(e, c, id, d2_id, asset, claim);
    let pre = charged(e, c);
    renew.stop();

    runs.stop(id).unwrap();
    unk(&mut runs, id, 2, "head", "live_post_commit_unknown");
    runs.unknown(id, RunPhase::AcquisitionTwo).unwrap();
    unk(&mut runs, id, 1, "dns", "late_completed_step");
    let sp = runs.https_steps(id).unwrap();
    let (s0, s1, s2) = (&sp[0], &sp[1], &sp[2]);
    assert!(s0.status == "completed" && s0.disposition.is_none());
    assert!(s1.status == "stopped" && s1.head_result_id == Some(res_h));
    assert_eq!(s1.head_outcome.as_deref(), Some("head_response"));
    assert_eq!(s1.disposition.as_deref(), Some("live_post_commit_unknown"));
    assert!(s1.protection_state == "unknown" && s2.status == "stopped");

    let (ok_i, insp) = cli("m1-run-inspect", id);
    assert!(ok_i && insp["terrain_history_count"] == json!(1));
    assert_inspect_counts(&insp, "stopped", 2, 1, 1);
    let i1 = &insp["steps"][1];
    assert_eq!(i1["head_outcome"], json!("head_response"));
    assert_eq!(i1["disposition"], json!("live_post_commit_unknown"));

    let (ok1, rec1) = cli("m1-run-recover", id);
    let (ok2, rec2) = cli("m1-run-recover", id);
    assert!(ok1 && ok2 && rec1 == rec2 && rec1["terrain_history_count"] == json!(2));
    assert_inspect_counts(&rec1, "stopped", 2, 1, 1);
    let hr = &rec1["https_results"][0];
    assert!(hr["attempt"] == json!(att_h) && hr["result"] == json!(res_h));
    assert!(hr["outcome"] == json!("head_response") && hr["status_code"] == json!(200));
    assert!(server.count() == 2 && charged(e, c) == pre);
    check_refresh_pending_and_head_error_isolated();
}

#[ignore]
#[test]
fn cli_m1_run_stop_transitions_unknown_https_fixture_run_and_recovers() {
    let _guard = native::qualified_existing_db();
    let server = run_cli_lab_server();
    let rel = sha256_hex(&std::fs::read(env!("CARGO_BIN_EXE_duskweave")).unwrap());
    let (e, c, id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut reg = base_registration_json(e, c, Uuid::new_v4(), &rel);
    let ip = &mut reg["m1_inference_permission"];
    (ip["version"], ip["run_profile"]) = (json!("m1_https_loop_v1"), json!("m1_https_loop_v1"));
    ip["execution_mode"] = json!("controlled_fixture");
    ip["profile"] = json!(FIXTURE_PRO_V2_PROFILE);
    ip.as_object_mut().unwrap().remove("live_binding");
    register_lab_campaign(&reg).unwrap();
    let mut rv = lab_run_request(&reg, id, Uuid::new_v4(), &rel);
    rv["run_profile"] = json!("m1_https_loop_v1");
    rv["execution_mode"] = json!("controlled_fixture");
    rv.as_object_mut().unwrap().remove("live_binding");
    let req = RunRequest::parse(&serde_json::to_vec(&rv).unwrap()).unwrap();
    let mut runs = PgRunStore::new(broker());
    assert!(runs.start(&req, &rel).unwrap());
    let rec = runs.inspect(id).unwrap();
    let (ctrl_d, renew) = (ControlDir::new(), Renewals::start(id));
    run_decision_phase(&mut runs, id, RunPhase::DecisionOne, &ctrl_d);
    runs.resolve_https_step_action(&rec, 1).unwrap();
    enter_acq(&mut runs, id, 1, RunPhase::AcquisitionOne, &rel);
    let pre = charged(e, c);
    renew.stop();
    unk(&mut runs, id, 1, "initial", "fixture_dns_unknown");
    runs.unknown(id, RunPhase::AcquisitionOne).unwrap();
    assert_eq!(ctrl(id).0, "unknown");

    let (ok_s, sr) = cli("m1-run-stop", id);
    assert!(ok_s && sr["stop_pending"] == json!(false) && sr["status"] == json!("stopped"));
    assert_eq!(ctrl(id), ("stopped".into(), 0));
    let st = runs.https_steps(id).unwrap();
    assert_eq!(st[0].status, "stopped");
    assert_eq!(st[0].disposition.as_deref(), Some("fixture_dns_unknown"));
    assert!(st[0].protection_state == "unknown" && st[1].status == "stopped");
    assert_eq!(charged(e, c), pre);

    let (ok_i, insp) = cli("m1-run-inspect", id);
    let (ok1, rec1) = cli("m1-run-recover", id);
    let (ok2, rec2) = cli("m1-run-recover", id);
    assert!(ok_i && ok1 && ok2 && rec1 == rec2 && insp["status"] == json!("stopped"));
    assert!(rec1["status"] == json!("stopped") && rec1["decision_count"] == json!(1));
    assert!(rec1["dns_effect_count"] == json!(0) && rec1["head_effect_count"] == json!(0));
    assert!(rec1.get("live_qualified").is_none() && server.count() == 1);
}

fn fn_body(sql: &str, name: &str) -> String {
    let norm = sql.replace("\r\n", "\n");
    let hdr = format!("CREATE OR REPLACE FUNCTION {name}(");
    let s = norm.rfind(&hdr).unwrap_or_else(|| panic!("missing {name}"));
    let bs = norm[s..].find("AS $$").unwrap() + s + 5;
    let be = norm[bs..].find("$$;").unwrap() + bs;
    norm[bs..be].to_owned()
}

const CAT_SQL: &str = "
SELECT p.proowner=(SELECT p0.proowner FROM pg_catalog.pg_proc p0 JOIN pg_catalog.pg_namespace n0
    ON n0.oid=p0.pronamespace WHERE n0.nspname='execution' AND p0.proname='run_start'),
  l.lanname, p.prosecdef, coalesce(p.proconfig, ARRAY[]::text[]),
  has_function_privilege('public', p.oid, 'EXECUTE'),
  has_function_privilege('dw_m1_broker', p.oid, 'EXECUTE'),
  (SELECT bool_or(has_function_privilege(r, p.oid, 'EXECUTE')) FROM unnest(ARRAY[
    'dw_runtime','dw_m1_cognition','dw_m1_terrain','dw_m1_trajectory','dw_m1_stop_writer']) r),
  encode(sha256(convert_to(replace(p.prosrc, E'\\r\\n', E'\\n'), 'UTF8')), 'hex')
FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid=p.pronamespace
JOIN pg_catalog.pg_language l ON l.oid=p.prolang WHERE n.nspname='execution' AND p.proname=$1";

#[ignore]
#[test]
fn catalog_matches_0038_bodies_attributes_owner_and_acl_for_stop_unknown_functions() {
    let _guard = native::qualified_existing_db();
    let mut admin = native::admin();
    let s38 = include_str!("../../migrations/0038_https_lab.sql");
    let s37 = include_str!("../../migrations/0037_https_loop.sql");
    for f in ["run_unknown", "run_stop", "mark_https_loop_effect_unknown"] {
        let rows = admin.query(CAT_SQL, &[&f]).unwrap();
        assert_eq!(rows.len(), 1, "single overload expected for {f}");
        let r = &rows[0];
        let (cfg, sha): (Vec<String>, String) = (r.get(3), r.get(7));
        assert!(r.get::<_, bool>(0) && r.get::<_, String>(1) == "plpgsql" && r.get::<_, bool>(2));
        assert!(!r.get::<_, bool>(4) && r.get::<_, bool>(5) && !r.get::<_, bool>(6));
        assert!(cfg.iter().any(|c| c == "search_path=pg_catalog, pg_temp"));
        let fq = format!("execution.{f}");
        let (e38, e37) = (
            sha256_hex(fn_body(s38, &fq).as_bytes()),
            sha256_hex(fn_body(s37, &fq).as_bytes()),
        );
        assert!(sha == e38 && sha != e37, "{f} SHA");
    }
}

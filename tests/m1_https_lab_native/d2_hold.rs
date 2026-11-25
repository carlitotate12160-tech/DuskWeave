#![allow(dead_code)]
//! Narrow loopback D2-hold SDK fixture, pre-STOP run-state validators, and regressions.

use super::support::extract_https_context_pack;
use crate::lab_support::lab_proposal_for_pack;
use crate::sdk_fixture::{Incoming, Reply, Server, envelope};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) const D2_HOLD_WAIT_LIMIT: Duration = Duration::from_secs(8);
pub(super) const STOP_EXIT_WAIT_LIMIT: Duration = Duration::from_secs(10);
pub(super) const HOLD_SELF_EXPIRY: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct D2RequestIdentity {
    pub engagement_id: Uuid,
    pub campaign_id: Uuid,
    pub decision_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HeldD2Observation {
    pub identity: D2RequestIdentity,
    pub d1_decision_id: Uuid,
    pub hold_selected_at: Instant,
}

pub(super) struct D2HoldServer {
    server: Server,
    release: Arc<AtomicBool>,
    hold_marks: Arc<Mutex<Vec<Option<Instant>>>>,
}

impl D2HoldServer {
    #[cfg(target_os = "linux")]
    pub(super) fn lab() -> Self {
        Self::start("127.0.0.1:18083")
    }

    fn start(address: &str) -> Self {
        let release = Arc::new(AtomicBool::new(false));
        let hold_marks = Arc::new(Mutex::new(Vec::new()));
        let (rel, marks) = (release.clone(), hold_marks.clone());
        let server = Server::start(address, move |inc| handle_req(inc, &rel, &marks));
        Self {
            server,
            release,
            hold_marks,
        }
    }

    pub(super) fn release(&self) {
        self.release.store(true, Ordering::Release);
    }

    pub(super) fn hold_state(&self) -> (usize, bool, usize) {
        let count = self.server.count();
        let released = self.release.load(Ordering::Acquire);
        let written = self.server.wire.lock().unwrap().write_complete;
        (count, released, written)
    }

    fn snapshot(&self) -> (Vec<Vec<u8>>, Vec<Option<Instant>>) {
        // Lock requests before hold_marks: handle_req pushes mark before Server::start pushes request.
        let bodies = {
            let reqs = self.server.requests.lock().unwrap();
            reqs.iter().map(|r| r.body.clone()).collect()
        };
        let marks = self.hold_marks.lock().unwrap().clone();
        (bodies, marks)
    }

    pub(super) fn wait_for_held_d2(
        &self,
        timeout: Duration,
        mut check_parent: impl FnMut() -> Result<(), String>,
    ) -> Result<HeldD2Observation, String> {
        let deadline = Instant::now() + timeout;
        loop {
            check_parent()?;
            let (bodies, marks) = self.snapshot();
            if let Some(obs) = evaluate_recorded_requests(&bodies, &marks)? {
                return Ok(obs);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for D2 hold (n={})",
                    bodies.len()
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for D2HoldServer {
    fn drop(&mut self) {
        self.release();
    }
}

fn handle_req(
    inc: &Incoming,
    rel: &Arc<AtomicBool>,
    marks: &Arc<Mutex<Vec<Option<Instant>>>>,
) -> Reply {
    let Ok(req) = serde_json::from_slice::<Value>(&inc.body) else {
        marks.lock().unwrap().push(None);
        return Reply::Length(400, br#"{"error":"invalid_sdk_json"}"#.to_vec());
    };
    let Ok(pack) = extract_https_context_pack(&req) else {
        marks.lock().unwrap().push(None);
        return Reply::Length(400, br#"{"error":"invalid_context_pack"}"#.to_vec());
    };
    if is_d2_pack(&pack) {
        marks.lock().unwrap().push(Some(Instant::now()));
        return Reply::Hold(rel.clone());
    }
    marks.lock().unwrap().push(None);
    let prop = lab_proposal_for_pack(&pack, "none");
    Reply::Length(200, serde_json::to_vec(&envelope(&prop)).unwrap())
}

fn check_all(checks: &[(bool, &str)]) -> Result<(), String> {
    for &(ok, msg) in checks {
        if !ok {
            return Err((*msg).to_string());
        }
    }
    Ok(())
}

fn check_str(pairs: &[(&Value, &str, &str)]) -> Result<(), String> {
    for &(val, expected, msg) in pairs {
        if val.as_str() != Some(expected) {
            return Err(format!("{msg}: got {val}, want {expected}"));
        }
    }
    Ok(())
}

fn has_https_ref(pack: &Value) -> bool {
    pack["references"].as_array().is_some_and(|r| {
        r.iter()
            .any(|x| x["id"].as_str().is_some_and(|id| id.starts_with("https_")))
    })
}

fn is_d2_pack(pack: &Value) -> bool {
    pack["references"]
        .as_array()
        .is_some_and(|r| r.iter().any(|x| x["id"] == "endpoint_0"))
        && !has_https_ref(pack)
}

fn parse_uuid(obj: &Value, key: &str) -> Result<Uuid, String> {
    let raw = obj[key].as_str().ok_or_else(|| format!("missing {key}"))?;
    let id = Uuid::parse_str(raw).map_err(|_| format!("invalid {key}"))?;
    if id.is_nil() {
        return Err(format!("nil {key}"));
    }
    Ok(id)
}

fn extract_pack_identity(body: &[u8]) -> Result<(Value, D2RequestIdentity), String> {
    let req: Value = serde_json::from_slice(body).map_err(|_| "malformed recorded SDK JSON")?;
    let pack = extract_https_context_pack(&req).map_err(str::to_string)?;
    let id = D2RequestIdentity {
        engagement_id: parse_uuid(&pack, "engagement_id")?,
        campaign_id: parse_uuid(&pack, "campaign_id")?,
        decision_id: parse_uuid(&pack, "decision_id")?,
    };
    Ok((pack, id))
}

fn evaluate_recorded_requests(
    bodies: &[Vec<u8>],
    marks: &[Option<Instant>],
) -> Result<Option<HeldD2Observation>, String> {
    if bodies.is_empty() {
        return Ok(None);
    }
    check_all(&[
        (bodies.len() <= 2, "unexpected extra SDK request"),
        (marks.len() >= bodies.len(), "missing hold mark"),
    ])?;
    let (d1_pack, d1_id) = extract_pack_identity(&bodies[0])?;
    let d1_ok = !is_d2_pack(&d1_pack) && !has_https_ref(&d1_pack);
    check_all(&[
        (marks[0].is_none(), "first SDK request unexpectedly held"),
        (d1_ok, "first SDK request was not D1"),
    ])?;
    if bodies.len() == 1 {
        return Ok(None);
    }
    let hold_selected_at = marks[1].ok_or_else(|| "second request was not held".to_string())?;
    let (d2_pack, d2_id) = extract_pack_identity(&bodies[1])?;
    check_all(&[
        (is_d2_pack(&d2_pack), "second request is not D2 pack"),
        (
            d2_id.engagement_id == d1_id.engagement_id,
            "D2 engagement_id mismatch",
        ),
        (
            d2_id.campaign_id == d1_id.campaign_id,
            "D2 campaign_id mismatch",
        ),
        (d2_id.decision_id != d1_id.decision_id, "D2 reused D1 id"),
    ])?;
    Ok(Some(HeldD2Observation {
        identity: d2_id,
        d1_decision_id: d1_id.decision_id,
        hold_selected_at,
    }))
}

fn find_step(arr: &Value, step_index: i64) -> Option<&Value> {
    arr.as_array()?
        .iter()
        .find(|s| s["step_index"].as_i64() == Some(step_index))
}

fn zero_https(hc: &Value) -> bool {
    hc["connections"] == 0 && hc["tls_handshakes"] == 0 && hc["head_requests"] == 0
}

fn check_pre_stop_sql(snap: &Value, ids: &[String; 5]) -> Result<(), String> {
    let [run, eng, cmp, d1, d2] = ids;
    let ctl = &snap["control"];
    let s1 = find_step(&snap["steps"], 1).ok_or("sql missing step 1")?;
    let s2 = find_step(&snap["steps"], 2).ok_or("sql missing step 2")?;
    let head_sess = snap["sessions"]
        .as_array()
        .is_some_and(|r| r.iter().any(|x| x["attempt_kind"] == "head"));
    check_str(&[
        (&ctl["run_id"], run, "ctl run_id"),
        (&ctl["status"], "running", "ctl status"),
        (&ctl["phase"], "decision_two", "ctl phase"),
        (&s1["status"], "completed", "sql s1 status"),
        (&s1["e"], eng, "sql s1 e"),
        (&s1["c"], cmp, "sql s1 c"),
        (&s1["decision_id"], d1, "sql s1 dec"),
        (&s2["status"], "decision_running", "sql s2 status"),
        (&s2["decision_phase"], "decision_two", "sql s2 phase"),
        (&s2["e"], eng, "sql s2 e"),
        (&s2["c"], cmp, "sql s2 c"),
        (&s2["decision_id"], d2, "sql s2 dec"),
    ])?;
    let s1_ok = s1["dns_attempt_id"].is_string()
        && s1["dns_result_id"].is_string()
        && s1["terrain_operation_id"].is_string();
    let s2_null = s2["head_attempt_id"].is_null() && s2["head_result_id"].is_null();
    check_all(&[
        (s1_ok, "sql s1 dns"),
        (s2_null && !head_sess, "sql s2 head"),
    ])
}

fn check_pre_stop_inspect_and_counters(
    snap: &Value,
    insp: &Value,
    hc: &Value,
    ids: &[String; 5],
) -> Result<(), String> {
    let [run, _, _, d1, d2] = ids;
    let s1_sql = find_step(&snap["steps"], 1).ok_or("sql missing s1")?;
    let s1 = find_step(&insp["steps"], 1).ok_or("inspect missing s1")?;
    let s2 = find_step(&insp["steps"], 2).ok_or("inspect missing s2")?;
    check_str(&[
        (&insp["run_id"], run, "insp run_id"),
        (&insp["status"], "running", "insp status"),
        (&insp["phase"], "decision_two", "insp phase"),
        (&s1["status"], "completed", "insp s1 status"),
        (&s1["decision_id"], d1, "insp s1 dec"),
        (&s2["status"], "decision_running", "insp s2 status"),
        (&s2["decision_id"], d2, "insp s2 dec"),
        (&hc["run_id"], run, "cnt run_id"),
    ])?;
    let fx_ok = insp["dns_effect_count"] == 1 && insp["head_effect_count"] == 0;
    let s1_ok = s1["dns_result_id"].is_string()
        && s1["terrain_operation_id"] == s1_sql["terrain_operation_id"];
    let s2_ok = s2["head_attempt_id"].is_null() && s2["head_result_id"].is_null();
    let cnt_ok = hc["dns_answered"] == 1 && zero_https(hc);
    check_all(&[
        (fx_ok && insp["terrain_history_count"] == 1, "insp counts"),
        (s1_ok && s2_ok, "insp step fields"),
        (cnt_ok, "pre-STOP counters"),
    ])
}

pub(super) fn validate_pre_stop_state(
    run_id: Uuid,
    held: &HeldD2Observation,
    sql_snapshot: &Value,
    inspect_receipt: &Value,
    counters: &Value,
    hold_state: (usize, bool, usize),
) -> Result<(), String> {
    let (sdk_count, released, write_complete) = hold_state;
    check_all(&[
        (sdk_count == 2, "pre-STOP SDK request count != 2"),
        (!released, "D2 hold already released before STOP"),
        (write_complete == 1, "D2 hold completed write pre-STOP"),
    ])?;
    let ids = [
        run_id.to_string(),
        held.identity.engagement_id.to_string(),
        held.identity.campaign_id.to_string(),
        held.d1_decision_id.to_string(),
        held.identity.decision_id.to_string(),
    ];
    check_pre_stop_sql(sql_snapshot, &ids)?;
    check_pre_stop_inspect_and_counters(sql_snapshot, inspect_receipt, counters, &ids)
}

pub(super) fn validate_parent_termination(
    exited: bool,
    exited_at: Instant,
    held: &HeldD2Observation,
    hold_state: (usize, bool, usize),
) -> Result<(), String> {
    let (sdk_count, released, write_complete) = hold_state;
    let before_expiry = exited_at < held.hold_selected_at + HOLD_SELF_EXPIRY;
    check_all(&[
        (exited, "m1-run did not terminate before STOP deadline"),
        (sdk_count == 2, "SDK request count changed during STOP"),
        (!released, "D2 hold released before parent termination"),
        (write_complete == 1, "D2 response completed without cancel"),
        (before_expiry, "exit at/after Reply::Hold 10s self-expiry"),
    ])
}

pub(super) fn check_zero_https_counters(
    hc: &Value,
    run_id: Uuid,
    sdk: usize,
) -> Result<(), String> {
    let ok =
        hc["run_id"] == run_id.to_string() && hc["dns_answered"] == 1 && zero_https(hc) && sdk == 2;
    check_all(&[(ok, "post-stop counters/sdk mismatch")])
}

#[cfg(test)]
mod tests {
    use super::*;
    use duskweave::m1_https::VERSION_HTTPS_LOOP;
    use genai::{
        Client, ModelIden, ModelSpec, ServiceTarget,
        adapter::AdapterKind,
        chat::ChatRequest,
        resolver::{AuthData, Endpoint},
    };
    use serde_json::json;
    use std::sync::mpsc;

    fn send_pack(addr: SocketAddr, pack: Value) -> Result<String, String> {
        let client = Client::new().map_err(|e| e.to_string())?;
        let target = ServiceTarget {
            endpoint: Endpoint::from_owned(format!("http://{addr}/")),
            auth: AuthData::from_single("synthetic-key"),
            model: ModelIden::new(AdapterKind::OpenAIResp, "deepseek-v4-pro"),
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let req = ChatRequest::from_user(pack.to_string()).with_system("sys");
        let res = rt
            .block_on(client.exec_chat_observed(ModelSpec::Target(target), req, None))
            .result;
        Ok(res
            .map_err(|e| e.to_string())?
            .first_text()
            .unwrap_or_default()
            .to_string())
    }

    fn pack(e: Uuid, c: Uuid, d: Uuid, refs: Value) -> Value {
        json!({"version": VERSION_HTTPS_LOOP, "engagement_id": e, "campaign_id": c, "decision_id": d, "references": refs})
    }

    fn start_with_d1(e: Uuid, c: Uuid, d: Uuid) -> (D2HoldServer, Value) {
        let srv = D2HoldServer::start("127.0.0.1:0");
        let r1 = json!([{"id": "asset_0"}]);
        let raw = send_pack(srv.server.address, pack(e, c, d, r1)).unwrap();
        (srv, serde_json::from_str(&raw).unwrap())
    }

    fn wait_ms(srv: &D2HoldServer, ms: u64) -> Result<HeldD2Observation, String> {
        srv.wait_for_held_d2(Duration::from_millis(ms), || Ok(()))
    }

    fn sample_pre_stop(run_id: Uuid, h: &HeldD2Observation, op: Uuid) -> (Value, Value, Value) {
        let id = &h.identity;
        let sql = json!({
            "control": {"run_id": run_id, "status": "running", "phase": "decision_two"},
            "steps": [
                {"step_index": 1, "status": "completed", "e": id.engagement_id, "c": id.campaign_id, "decision_id": h.d1_decision_id, "dns_attempt_id": Uuid::new_v4(), "dns_result_id": Uuid::new_v4(), "terrain_operation_id": op},
                {"step_index": 2, "status": "decision_running", "decision_phase": "decision_two", "e": id.engagement_id, "c": id.campaign_id, "decision_id": id.decision_id, "head_attempt_id": null, "head_result_id": null}
            ],
            "sessions": []
        });
        let insp = json!({
            "run_id": run_id, "status": "running", "phase": "decision_two", "dns_effect_count": 1, "head_effect_count": 0, "terrain_history_count": 1,
            "steps": [
                {"step_index": 1, "status": "completed", "decision_id": h.d1_decision_id, "dns_result_id": Uuid::new_v4(), "terrain_operation_id": op},
                {"step_index": 2, "status": "decision_running", "decision_id": id.decision_id, "head_attempt_id": null, "head_result_id": null}
            ]
        });
        let cnt = json!({"run_id": run_id, "dns_answered": 1, "connections": 0, "tls_handshakes": 0, "head_requests": 0});
        (sql, insp, cnt)
    }

    #[test]
    fn d2_hold_barrier_requires_d2_holds_until_release_and_matches_run_state() {
        let u = Uuid::new_v4;
        let (eng, cmp, d1, d2) = (u(), u(), u(), u());
        let (srv, p1) = start_with_d1(eng, cmp, d1);
        assert_eq!(p1["next_intent"]["manifest_reference"], "dns_udp");
        assert!(wait_ms(&srv, 30).is_err());

        let (tx, rx) = mpsc::channel();
        let addr = srv.server.address;
        let r2 = json!([{"id": "asset_0"}, {"id": "endpoint_0"}]);
        let d2_pack = pack(eng, cmp, d2, r2);
        let worker = std::thread::spawn(move || tx.send(send_pack(addr, d2_pack)));

        let held = wait_ms(&srv, 2000).unwrap();
        let id = &held.identity;
        assert!(id.engagement_id == eng && id.campaign_id == cmp && id.decision_id == d2);
        assert_eq!((held.d1_decision_id, srv.hold_state()), (d1, (2, false, 1)));
        assert!(rx.try_recv().is_err(), "D2 client must stay held");

        let run_id = u();
        let (sql, insp, cnt) = sample_pre_stop(run_id, &held, u());
        let st = srv.hold_state();
        assert!(validate_pre_stop_state(run_id, &held, &sql, &insp, &cnt, st).is_ok());
        assert!(check_zero_https_counters(&cnt, run_id, 2).is_ok());
        assert!(validate_parent_termination(true, Instant::now(), &held, (2, false, 1)).is_ok());

        let bad = |f: fn(&mut Value, &mut Value, &mut Value)| {
            let (mut s, mut i, mut c) = (sql.clone(), insp.clone(), cnt.clone());
            f(&mut s, &mut i, &mut c);
            assert!(validate_pre_stop_state(run_id, &held, &s, &i, &c, (2, false, 1)).is_err());
        };
        bad(|s, _, _| s["control"]["phase"] = json!("acquisition_one"));
        bad(|s, _, _| s["steps"][0]["status"] = json!("running"));
        bad(|s, _, _| s["steps"][1]["status"] = json!("pending"));
        bad(|s, _, _| s["steps"][1]["decision_id"] = json!(Uuid::nil()));
        bad(|_, i, _| i["terrain_history_count"] = json!(0));
        bad(|_, _, c| c["head_requests"] = json!(1));
        assert!(validate_pre_stop_state(run_id, &held, &sql, &insp, &cnt, (2, true, 1)).is_err());

        let term = |ex, at, wc| validate_parent_termination(ex, at, &held, (2, false, wc));
        assert!(term(false, Instant::now(), 1).is_err());
        assert!(term(true, held.hold_selected_at + HOLD_SELF_EXPIRY, 1).is_err());
        assert!(term(true, Instant::now(), 2).is_err());

        srv.release();
        assert!(rx.recv_timeout(Duration::from_secs(2)).unwrap().is_err());
        worker.join().unwrap().unwrap();
    }

    #[test]
    fn d2_hold_barrier_rejects_non_d2_malformed_wrong_identity_and_early_exit() {
        let u = Uuid::new_v4;
        let (eng, cmp, d1, d2) = (u(), u(), u(), u());
        let (srv_early, _) = start_with_d1(eng, cmp, d1);
        let early = srv_early.wait_for_held_d2(Duration::from_secs(1), || Err("exited".into()));
        assert!(early.unwrap_err().contains("exited"));

        let r3 = json!([{"id": "asset_0"}, {"id": "endpoint_0"}, {"id": "https_0"}]);
        let malformed = json!({"version": "m1_context_pack_autonomous_v4", "references": []});
        for bad_req in [pack(eng, cmp, d2, r3), malformed] {
            let (srv, _) = start_with_d1(eng, cmp, d1);
            let _ = send_pack(srv.server.address, bad_req);
            assert!(wait_ms(&srv, 50).is_err());
        }

        let r2 = json!([{"id": "asset_0"}, {"id": "endpoint_0"}]);
        let missing_dec = json!({"version": VERSION_HTTPS_LOOP, "engagement_id": eng, "campaign_id": cmp, "references": [{"id": "endpoint_0"}]});
        for (req, expected_err) in [
            (pack(eng, u(), d2, r2), "campaign_id mismatch"),
            (missing_dec, "decision_id"),
        ] {
            let (srv, _) = start_with_d1(eng, cmp, d1);
            let addr = srv.server.address;
            let t = std::thread::spawn(move || send_pack(addr, req));
            assert!(wait_ms(&srv, 2000).unwrap_err().contains(expected_err));
            drop(srv);
            let _ = t.join().unwrap();
        }
    }
}

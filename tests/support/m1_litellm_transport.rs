//! Private adapter probe against the isolated LiteLLM `/v1/responses` container.
//! Synthetic private construction tests wire/admission compatibility only; it does
//! not qualify production credential loading or a CLI gateway configuration lane.
use super::*;
use crate::m1_reasoning::inference::{
    IncompleteReason, Provider, ResponseStatus, UsageAvailability,
};
use crate::m1_reasoning::{
    proposal::Admission, provider_diagnostic::Classification, request, response,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const GATEWAY_URL: &str = "http://127.0.0.1:14000/v1/responses";
const CONTROL_ADDR: &str = "127.0.0.1:14082";
const GATEWAY_KEY: &str = "sk-dw-probe-gateway-sentinel-9002";
const UPSTREAM_KEY: &str = "sk-dw-probe-upstream-sentinel-9001";
const REPORT_DIR: &str = "D:/DuskWeave-build/rust-1.94.1/reports/litellm-native-probe-20261027a";
const CONTAINER: &str = "dw-litellm-probe-20261027a";

struct ProbeBasis(AtomicUsize, bool);
impl ProbeBasis {
    fn new(ok: bool) -> Self {
        Self(AtomicUsize::new(0), ok)
    }
}
impl ProposalBasis for ProbeBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        matches!(
            r,
            "mission" | "asset_0" | "terrain_0" | "previous_decision" | "ref-1"
        )
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        self.1.then_some(()).ok_or("current_authority_unavailable")
    }
    fn safe_explanation(&self, s: &str) -> bool {
        ![GATEWAY_KEY, UPSTREAM_KEY, "SENTINEL_PRIVATE"]
            .iter()
            .any(|k| s.contains(k))
    }
}

type Conns = Arc<Mutex<HashMap<u32, TcpStream>>>;

fn send_mux(stdin: &Arc<Mutex<ChildStdin>>, cmd: u8, cid: u32, data: &[u8]) -> bool {
    let Ok(mut w) = stdin.lock() else {
        return false;
    };
    let mut hdr = [0u8; 7];
    hdr[0] = cmd;
    hdr[1..5].copy_from_slice(&cid.to_be_bytes());
    hdr[5..7].copy_from_slice(&(data.len() as u16).to_be_bytes());
    w.write_all(&hdr).is_ok() && w.write_all(data).is_ok() && w.flush().is_ok()
}

fn mux_client_pump(mut reader: TcpStream, cid: u32, tx: Arc<Mutex<ChildStdin>>, map: Conns) {
    let mut b = [0u8; 16384];
    while let Ok(n) = reader.read(&mut b) {
        if n == 0 || !send_mux(&tx, 2, cid, &b[..n]) {
            break;
        }
    }
    let _ = send_mux(&tx, 3, cid, &[]);
    if let Some(s) = map.lock().unwrap().remove(&cid) {
        let _ = s.shutdown(Shutdown::Both);
    }
}

fn spawn_mux_listener(
    listener: TcpListener,
    port: u16,
    stdin: Arc<Mutex<ChildStdin>>,
    conns: Conns,
    next_id: Arc<AtomicUsize>,
) {
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let _ = client.set_read_timeout(Some(Duration::from_secs(15)));
            let _ = client.set_write_timeout(Some(Duration::from_secs(15)));
            let Ok(reader) = client.try_clone() else {
                continue;
            };
            let cid = next_id.fetch_add(1, Ordering::SeqCst) as u32;
            conns.lock().unwrap().insert(cid, client);
            if !send_mux(&stdin, 1, cid, &port.to_be_bytes()) {
                conns.lock().unwrap().remove(&cid);
                continue;
            }
            let (tx, map) = (Arc::clone(&stdin), Arc::clone(&conns));
            std::thread::spawn(move || mux_client_pump(reader, cid, tx, map));
        }
    });
}

fn mux_stdout_pump(mut out: std::process::ChildStdout, mut child: std::process::Child, map: Conns) {
    let mut hdr = [0u8; 7];
    while out.read_exact(&mut hdr).is_ok() {
        let cid = u32::from_be_bytes(hdr[1..5].try_into().unwrap());
        let len = u16::from_be_bytes(hdr[5..7].try_into().unwrap()) as usize;
        let mut buf = vec![0u8; len];
        if len > 0 && out.read_exact(&mut buf).is_err() {
            break;
        }
        if hdr[0] == 2
            && let Some(s) = map.lock().unwrap().get_mut(&cid)
        {
            let _ = s.write_all(&buf);
            let _ = s.flush();
        } else if hdr[0] == 3
            && let Some(s) = map.lock().unwrap().remove(&cid)
        {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn ensure_gateway() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        if let (Ok(l_gw), Ok(l_ctl)) = (
            TcpListener::bind(("127.0.0.1", 14000)),
            TcpListener::bind(("127.0.0.1", 14082)),
        ) {
            let args = ["exec", "-i", CONTAINER, "python", "-u", "/probe/mock_upstream.py", "--mux"];
            let mut child = Command::new("docker")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn docker mux bridge");
            let stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
            let mut stdout = child.stdout.take().unwrap();
            stdout.read_exact(&mut [0u8; 7]).expect("mux ready");
            let conns: Conns = Arc::new(Mutex::new(HashMap::new()));
            let rc = Arc::clone(&conns);
            std::thread::spawn(move || mux_stdout_pump(stdout, child, rc));
            let next_id = Arc::new(AtomicUsize::new(1));
            spawn_mux_listener(l_gw, 4000, Arc::clone(&stdin), Arc::clone(&conns), Arc::clone(&next_id));
            spawn_mux_listener(l_ctl, 18082, stdin, conns, next_id);
        }
        let end = Instant::now() + Duration::from_secs(30);
        while Instant::now() < end {
            if let Ok(mut s) = TcpStream::connect("127.0.0.1:14000") {
                let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
                let req = b"GET /health/liveliness HTTP/1.1\r\nHost: 127.0.0.1:14000\r\nConnection: close\r\n\r\n";
                let _ = s.write_all(req);
                let mut buf = Vec::new();
                let _ = s.read_to_end(&mut buf);
                if buf.starts_with(b"HTTP/1.1 200") && TcpStream::connect(CONTROL_ADDR).is_ok() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        panic!("BLOCKED: LiteLLM probe container or control listener unavailable on loopback");
    });
}

fn control_req(method: &str, body: Option<&Value>) -> Value {
    ensure_gateway();
    let payload = body
        .map(|v| serde_json::to_vec(v).unwrap())
        .unwrap_or_default();
    let mut s = TcpStream::connect(CONTROL_ADDR).expect("connect mock control API");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let head = format!(
        "{method} /state HTTP/1.1\r\nHost: {CONTROL_ADDR}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    s.write_all(head.as_bytes()).unwrap();
    s.write_all(&payload).unwrap();
    s.flush().unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).unwrap();
    let i = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    serde_json::from_slice(&raw[i..]).unwrap()
}

fn set_mock(cfg: Value) -> Value {
    control_req("POST", Some(&cfg))
}
fn mock_state() -> Value {
    control_req("GET", None)
}
fn hex_sha256(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn proposal_json(category: &str, refs: &[&str], hypothesis: &str) -> String {
    let mut intent = json!({"category": category, "references": refs, "manifest_reference": null});
    if category == "observe_more" {
        intent["view_request"] = Value::Null;
    }
    json!({
        "version": "m1_inference_proposal_v1", "hypothesis": hypothesis,
        "considered_alternative": "The premise may remain unverified.", "references": refs,
        "expected_observation": "Bounded synthetic observation.",
        "falsifying_observation": "Contrary synthetic observation.",
        "uncertainty": "Synthetic gateway probe only.",
        "rationale": "Retain strict deterministic authority separation.",
        "next_intent": intent
    })
    .to_string()
}

fn completed_envelope(proposal: &str, model: &str, usage: Value) -> String {
    let rs = json!({"id": "rs_probe_1", "type": "reasoning", "summary": [{"type": "summary_text", "text": "SENTINEL_PRIVATE_REASONING"}]});
    let msg = json!({"id": "msg_probe_1", "type": "message", "role": "assistant", "status": "completed", "content": [{"type": "output_text", "text": proposal, "annotations": []}]});
    json!({"id": "resp_probe_ok", "object": "response", "created_at": 1792335054, "status": "completed", "model": model, "output": [rs, msg], "usage": usage}).to_string()
}

fn incomplete_envelope(reason: &str, model: &str, usage: Value) -> String {
    let rs = json!({"id": "rs_probe_inc", "type": "reasoning", "summary": [{"type": "summary_text", "text": "SENTINEL_PRIVATE_REASONING"}]});
    json!({"id": "resp_probe_inc", "object": "response", "created_at": 1792335054, "status": "incomplete", "model": model, "incomplete_details": {"reason": reason}, "output": [rs], "usage": usage}).to_string()
}

fn wire_cases() -> Vec<Value> {
    let raw = include_str!("../../docs/contracts/fixtures/m1-autonomous-wire-examples-v1.json");
    serde_json::from_str::<Value>(raw).unwrap()["cases"]
        .as_array()
        .unwrap()
        .clone()
}

fn successor_pack(case_pack: &str, profile: &str) -> Vec<u8> {
    let mut p: Value = serde_json::from_str(case_pack).unwrap();
    p["version"] = json!("m1_context_pack_autonomous_v2");
    p["run_profile"] = json!("autonomous-dns-fixture-v2");
    p["execution_mode"] = json!("controlled_fixture");
    p["inference_profile"] = json!(profile);
    serde_json::to_vec(&p).unwrap()
}

fn exact_8192_pack(profile: &str) -> Vec<u8> {
    let cases = wire_cases();
    let base = cases
        .iter()
        .find(|c| c["name"] == "provider_fixture_quote_escape")
        .unwrap()["pack"]
        .as_str()
        .unwrap();
    let mut v: Value = serde_json::from_slice(&successor_pack(base, profile)).unwrap();
    let pad = 8192 - serde_json::to_vec(&v).unwrap().len();
    let q = v["question"].as_str().unwrap().to_owned();
    v["question"] = json!(format!("{q}{}", "x".repeat(pad)));
    let out = serde_json::to_vec(&v).unwrap();
    assert_eq!(out.len(), 8192);
    out
}

fn permit_active(secs: u64) -> Permit {
    let end = Instant::now() + Duration::from_secs(secs);
    let p = Permit::new(end);
    assert!(p.publish(Instant::now(), end));
    let renew = p.clone();
    std::thread::spawn(move || {
        while !renew.stopped() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(300));
            if !renew.publish(Instant::now(), end) {
                break;
            }
        }
    });
    p
}

fn gateway_provider(call: &BoundCall, permit: Permit, with_clock: bool) -> AutonomousProvider {
    autonomous_binding(call).unwrap();
    let mut h = configure(call, GATEWAY_KEY, Duration::from_secs(20)).unwrap();
    h.url(GATEWAY_URL).unwrap();
    let shared = Instant::now() + Duration::from_millis(124000);
    let clock = with_clock.then(|| CallClock::arm(permit.clone(), shared, None).unwrap());
    AutonomousProvider {
        diagnostic: ProviderDiagnostic::default(),
        prepared: Some((h, GATEWAY_KEY.into(), call.digest.clone())),
        permit,
        pre_send: Box::new(|| Ok(())),
        clock,
        admission_guard: None,
        reporter: None,
    }
}

fn call_gw(p: &mut AutonomousProvider, c: &BoundCall, ms: u64) -> Res<Received> {
    let res = p.call(
        c,
        Instant::now() + Duration::from_millis(ms),
        &mut ProbeBasis::new(true),
        &mut || Ok(()),
    );
    p.admission_guard = None;
    res
}

fn raw_exchange(call: &BoundCall) -> (Result<(), curl::Error>, Vec<u8>, bool, Duration) {
    let mut h = configure(call, GATEWAY_KEY, Duration::from_secs(20)).unwrap();
    h.url(GATEWAY_URL).unwrap();
    exchange(&mut h, &mut || true).unwrap()
}

fn assert_wire(call: &BoundCall, st: &Value, effort: &str) {
    let counts = (
        st["calls"].as_u64(),
        st["responses_calls"].as_u64(),
        st["chat_calls"].as_u64(),
        st["other_calls"].as_u64(),
    );
    assert_eq!(counts, (Some(1), Some(1), Some(0), Some(0)));
    assert_eq!(
        (st["last_path"].as_str(), st["last_auth_matched"].as_bool()),
        (Some("/v1/responses"), Some(true))
    );
    let sent: Value = serde_json::from_slice(&call.body).unwrap();
    let got: Value = serde_json::from_str(st["last_body"].as_str().unwrap()).unwrap();
    for k in ["instructions", "input", "text"] {
        assert_eq!(got[k], sent[k]);
    }
    assert_eq!(
        (&got["model"], &got["max_output_tokens"], &got["stream"]),
        (&json!("deepseek-flash"), &json!(4096), &json!(false))
    );
    assert_eq!(
        (
            &got["tools"],
            &got["tool_choice"],
            &got["reasoning"]["effort"]
        ),
        (&json!([]), &json!("none"), &json!(effort))
    );
    assert!(got.get("temperature").is_none() && got.get("top_p").is_none());
    let p_str = std::str::from_utf8(&call.pack).unwrap();
    if sent["input"].is_array() {
        assert_eq!(got["input"][0]["content"].as_str(), Some(p_str));
        assert_eq!(got["input"][1]["content"], sent["input"][1]["content"]);
    } else {
        assert_eq!(got["input"].as_str(), Some(p_str));
    }
}

fn run_wire_case(
    name: &str,
    pack: Vec<u8>,
    repair: Option<&[&str]>,
    effort: &str,
    body: &str,
) -> Value {
    let call = request::bind(&pack, repair).unwrap();
    assert!(
        call.pack.len() <= 8192 && call.configuration.len() <= 16384 && call.body.len() <= 16384
    );
    set_mock(json!({"status": 200, "body": body}));
    let mut p = gateway_provider(&call, permit_active(130), true);
    let rec = call_gw(&mut p, &call, 124000).unwrap();
    assert!(matches!(rec.admission, Admission::Accepted(_)));
    let st = mock_state();
    assert_wire(&call, &st, effort);
    json!({
        "case": name, "pack_bytes": call.pack.len(), "pack_margin_bytes": 8192 - call.pack.len(),
        "config_bytes": call.configuration.len(), "config_margin_bytes": 16384 - call.configuration.len(),
        "outgoing_body_bytes": call.body.len(), "request_margin_bytes": 16384 - call.body.len(),
        "outgoing_body_sha256": hex_sha256(&call.body), "binding_digest": call.digest,
        "mock_received_bytes": st["last_len"], "mock_received_sha256": st["last_sha256"],
        "mock_path": st["last_path"], "gateway_response_bytes": rec.usage.response_bytes,
        "gateway_response_margin_bytes": 65536 - rec.usage.response_bytes, "wall_time_ms": rec.usage.wall_time_ms
    })
}

#[test]
#[ignore = "requires disposable LiteLLM probe container on 127.0.0.1:14000/14082"]
fn native_wire_and_terminal_matrix() {
    ensure_gateway();
    let cases = wire_cases();
    let (d1, d2_min) = (
        cases[0]["pack"].as_str().unwrap(),
        cases[1]["pack"].as_str().unwrap(),
    );
    let find_pack = |n: &str| {
        cases.iter().find(|c| c["name"] == n).unwrap()["pack"]
            .as_str()
            .unwrap()
    };
    let (d2_quote, d2_multi) = (
        find_pack("decision2_quote_escape_width"),
        find_pack("decision2_utf8_width"),
    );
    let codes8: &[&str] = &[
        "json_size",
        "json_structure",
        "json_trailing",
        "proposal_shape",
        "proposal_version",
        "unexpected_view_request",
        "proposal_text",
        "json_size",
    ];
    let retain_prop = proposal_json("retain", &["asset_0"], "Bounded probe hypothesis.");
    let std_u = json!({"input_tokens": 12, "output_tokens": 23, "total_tokens": 35, "output_tokens_details": {"reasoning_tokens": 5}});
    let ok_env = completed_envelope(&retain_prop, "deepseek-flash", std_u.clone());
    let (low, high) = (
        "deepseek-responses-low-v1",
        "deepseek-responses-reasoning-v1",
    );
    let wire_records: Vec<Value> = [
        ("d1_low_gen", successor_pack(d1, low), None, "low"),
        ("d1_high_gen", successor_pack(d1, high), None, "high"),
        (
            "d2_feedback_low_gen",
            successor_pack(d2_min, low),
            None,
            "low",
        ),
        (
            "d2_feedback_low_repair1",
            successor_pack(d2_min, low),
            Some(&["proposal_shape"][..]),
            "low",
        ),
        (
            "d2_quote_width_high_gen",
            successor_pack(d2_quote, high),
            None,
            "high",
        ),
        (
            "d2_multibyte_low_gen",
            successor_pack(d2_multi, low),
            None,
            "low",
        ),
        (
            "exact_8192_low_repair8",
            exact_8192_pack(low),
            Some(codes8),
            "low",
        ),
    ]
    .into_iter()
    .map(|(n, pk, rp, ef)| run_wire_case(n, pk, rp, ef, &ok_env))
    .collect();

    set_mock(json!({"status": 200}));
    let mut over_pack = exact_8192_pack(low);
    over_pack.push(b' ');
    assert_eq!(
        request::bind(&over_pack, None).err(),
        Some("request_encode")
    );
    let legacy_over = format!(
        r#"{{"references":[{{"id":"ref-1"}}],"pad":"{}"}}"#,
        "a".repeat(4070)
    );
    assert_eq!(
        request::bind(legacy_over.as_bytes(), None).err(),
        Some("request_encode")
    );
    assert_eq!(
        request::bind(d1.as_bytes(), Some(&["json_size"; 9])).err(),
        Some("repair_code_limit")
    );
    assert_eq!(
        request::bind(d1.as_bytes(), Some(&["bad_code"])).err(),
        Some("repair_code_ineligible")
    );
    assert_eq!(mock_state()["calls"], 0);

    let probe_call = request::bind(&successor_pack(d1, low), None).unwrap();
    let defer_prop = proposal_json(
        "defer",
        &[],
        "Defer until current owner premises are refreshed.",
    );
    set_mock(
        json!({"status": 200, "body": completed_envelope(&defer_prop, "deepseek-flash", std_u)}),
    );
    let (res_ok, raw_ok, over_ok, el_ok) = raw_exchange(&probe_call);
    assert!(res_ok.is_ok() && !over_ok && raw_ok.len() <= 65536);
    let mut diag_ok = ProviderDiagnostic::new(probe_call.digest.clone());
    let adm_ok = admit_without_key(
        &raw_ok,
        el_ok,
        GATEWAY_KEY,
        &mut ProbeBasis::new(true),
        &mut diag_ok,
    )
    .unwrap();
    let Admission::Accepted(ref prop_ok) = adm_ok.admission else {
        panic!("expected Accepted")
    };
    assert!(
        !serde_json::to_string(prop_ok).unwrap().contains("SENTINEL")
            && !serde_json::to_string(&diag_ok)
                .unwrap()
                .contains("SENTINEL")
    );

    let inc_u = json!({"input_tokens": 12, "output_tokens": 23, "total_tokens": 35, "output_tokens_details": {"reasoning_tokens": 20}});
    set_mock(
        json!({"status": 200, "body": incomplete_envelope("max_output_tokens", "deepseek-flash", inc_u)}),
    );
    let (res_inc, raw_inc, over_inc, el_inc) = raw_exchange(&probe_call);
    assert!(res_inc.is_ok() && !over_inc && raw_inc.len() <= 65536);
    let mut diag_inc = ProviderDiagnostic::new(probe_call.digest.clone());
    assert_eq!(
        admit_without_key(
            &raw_inc,
            el_inc,
            GATEWAY_KEY,
            &mut ProbeBasis::new(true),
            &mut diag_inc
        )
        .err(),
        Some("provider_incomplete")
    );
    let rep_inc = diag_inc.report.as_ref().unwrap();
    assert!(matches!(
        rep_inc.provider_response.status,
        ResponseStatus::Incomplete
    ));
    assert!(matches!(
        rep_inc.provider_response.incomplete_reason,
        Some(IncompleteReason::MaxOutputTokens)
    ));
    assert!(matches!(
        rep_inc.provider_response.usage_availability,
        UsageAvailability::Reported
    ));
    assert_eq!(
        (
            rep_inc.usage.input_tokens,
            rep_inc.usage.output_tokens,
            rep_inc.usage.reasoning_tokens
        ),
        (Some(12), Some(23), Some(20))
    );
    assert!(
        !serde_json::to_string(&diag_inc)
            .unwrap()
            .contains("SENTINEL")
    );

    for (u_in, avail, exp, m_name) in [
        (
            Value::Null,
            UsageAvailability::Unavailable,
            (None, None, None),
            "deepseek-flash",
        ),
        (
            json!({"input_tokens": 15, "output_tokens": 29, "total_tokens": 44}),
            UsageAvailability::Partial,
            (Some(15), Some(29), None),
            "deepseek-flash",
        ),
        (
            json!({"input_tokens": "bad", "output_tokens": -1, "total_tokens": 0, "output_tokens_details": {"reasoning_tokens": 7}}),
            UsageAvailability::Partial,
            (None, None, Some(7)),
            "deepseek-reasoner-v99",
        ),
    ] {
        set_mock(
            json!({"status": 200, "body": incomplete_envelope("max_output_tokens", m_name, u_in)}),
        );
        let mut p = gateway_provider(&probe_call, permit_active(130), true);
        assert_eq!(
            call_gw(&mut p, &probe_call, 124000).err(),
            Some("provider_incomplete")
        );
        let r = p.terminal_report(&probe_call.digest).unwrap();
        assert_eq!(
            serde_json::to_value(r.provider_response.usage_availability).unwrap(),
            serde_json::to_value(avail).unwrap()
        );
        assert_eq!(
            (
                r.usage.input_tokens,
                r.usage.output_tokens,
                r.usage.reasoning_tokens
            ),
            exp
        );
        assert_eq!(
            (r.usage.requested_model, r.usage.reported_model.as_deref()),
            ("deepseek-flash", Some("deepseek-flash"))
        );
    }

    set_mock(
        json!({"status": 200, "body": completed_envelope(&"x".repeat(66000), "deepseek-flash", Value::Null)}),
    );
    let mut p_over = gateway_provider(&probe_call, permit_active(130), true);
    assert_eq!(
        call_gw(&mut p_over, &probe_call, 124000).err(),
        Some("provider_response_limit")
    );
    assert!(p_over.terminal_report(&probe_call.digest).is_none());

    for (hyp, code) in [
        (&*"h".repeat(1025), "json_structure"),
        ("   ", "proposal_text"),
    ] {
        set_mock(
            json!({"status": 200, "body": completed_envelope(&proposal_json("retain", &["asset_0"], hyp), "deepseek-flash", Value::Null)}),
        );
        let mut p = gateway_provider(&probe_call, permit_active(130), true);
        let rec = call_gw(&mut p, &probe_call, 124000).unwrap();
        assert!(matches!(rec.admission, Admission::Repairable(ref c) if c == &[code]));
    }

    let root = std::path::Path::new(REPORT_DIR);
    std::fs::write(root.join("gateway-completed-defer.http"), &raw_ok).unwrap();
    std::fs::write(
        root.join("gateway-incomplete-max-output-tokens.http"),
        &raw_inc,
    )
    .unwrap();
    let manifest = json!({
        "completed_defer": {"file": "gateway-completed-defer.http", "bytes": raw_ok.len(), "sha256": hex_sha256(&raw_ok)},
        "incomplete_max_output_tokens": {"file": "gateway-incomplete-max-output-tokens.http", "bytes": raw_inc.len(), "sha256": hex_sha256(&raw_inc)},
        "wire_matrix": wire_records,
    });
    std::fs::write(
        root.join("gateway-artifacts-manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires disposable LiteLLM probe container on 127.0.0.1:14000/14082"]
fn single_send_and_no_cache_matrix() {
    ensure_gateway();
    let d1 = wire_cases()[0]["pack"].as_str().unwrap().to_owned();
    let call = request::bind(&successor_pack(&d1, "deepseek-responses-low-v1"), None).unwrap();

    for status in [400u16, 401, 408, 429, 500] {
        set_mock(
            json!({"status": status, "body": r#"{"error":{"message":"synthetic error","type":"server_error"}}"#}),
        );
        let mut p = gateway_provider(&call, permit_active(130), true);
        assert_eq!(
            call_gw(&mut p, &call, 124000).err(),
            Some("provider_protocol")
        );
        assert!(
            matches!(p.diagnostic.classification, Classification::HttpError)
                && p.diagnostic.final_http_status == Some(status)
        );
        let st = mock_state();
        assert_eq!(
            (
                st["calls"].as_u64(),
                st["responses_calls"].as_u64(),
                st["chat_calls"].as_u64()
            ),
            (Some(1), Some(1), Some(0))
        );
    }

    for cfg in [
        json!({"reset": true}),
        json!({"status": 302, "headers": {"Location": "http://127.0.0.1:18081/v1/redirected"}, "body": "{}"}),
    ] {
        set_mock(cfg);
        let mut p = gateway_provider(&call, permit_active(130), true);
        assert_eq!(
            call_gw(&mut p, &call, 124000).err(),
            Some("provider_protocol")
        );
        let st = mock_state();
        assert_eq!(
            (st["calls"].as_u64(), st["other_calls"].as_u64()),
            (Some(1), Some(0))
        );
    }

    let prop = proposal_json("retain", &["asset_0"], "Delayed & no-cache check.");
    let ok_env = completed_envelope(
        &prop,
        "deepseek-flash",
        json!({"input_tokens": 10, "output_tokens": 20, "total_tokens": 30, "output_tokens_details": {"reasoning_tokens": 4}}),
    );
    set_mock(json!({"status": 200, "delay_ms": 350, "body": ok_env}));
    let mut p_delay = gateway_provider(&call, permit_active(130), true);
    let rec = call_gw(&mut p_delay, &call, 124000).unwrap();
    assert!(
        matches!(rec.admission, Admission::Accepted(_))
            && rec.usage.wall_time_ms >= 300
            && mock_state()["calls"] == 1
    );

    for alias in ["unknown-model-alias", "deepseek-flash-unannotated"] {
        set_mock(json!({"status": 200}));
        let mut body_v: Value = serde_json::from_slice(&call.body).unwrap();
        body_v["model"] = json!(alias);
        let body_b = serde_json::to_vec(&body_v).unwrap();
        let fake = BoundCall {
            pack: call.pack.clone(),
            configuration: call.configuration.clone(),
            digest: request::compute_binding(&call.pack, &call.configuration, &body_b),
            body: body_b,
        };
        assert!(autonomous_binding(&fake).is_err());
        let (p_s, c_s, b_s) = (
            std::str::from_utf8(&fake.pack).unwrap(),
            std::str::from_utf8(&fake.configuration).unwrap(),
            std::str::from_utf8(&fake.body).unwrap(),
        );
        assert!(request::verify_binding(p_s, c_s, b_s, &fake.digest, false).is_err());
        let (_, raw, _, _) = raw_exchange(&fake);
        let mut d = ProviderDiagnostic::default();
        assert_eq!(
            response::admit_http(&raw, Duration::ZERO, &mut ProbeBasis::new(true), &mut d).err(),
            Some("provider_protocol")
        );
        assert_eq!(
            (d.final_http_status, mock_state()["calls"].as_u64()),
            (Some(400), Some(0))
        );
    }

    for (field, val, in_cfg) in [
        ("effort", json!("medium"), false),
        ("recipient", json!(GATEWAY_URL), true),
        ("profile", json!("litellm-custom-v1"), true),
    ] {
        let mut cfg_v: Value = serde_json::from_slice(&call.configuration).unwrap();
        let mut body_v: Value = serde_json::from_slice(&call.body).unwrap();
        if in_cfg {
            cfg_v[field] = val;
        } else {
            body_v["reasoning"][field] = val;
        }
        let (cfg_b, body_b) = (
            serde_json::to_vec(&cfg_v).unwrap(),
            serde_json::to_vec(&body_v).unwrap(),
        );
        let digest = request::compute_binding(&call.pack, &cfg_b, &body_b);
        let tampered = BoundCall {
            pack: call.pack.clone(),
            configuration: cfg_b,
            body: body_b,
            digest,
        };
        assert!(autonomous_binding(&tampered).is_err());
        let (p_s, c_s, b_s) = (
            std::str::from_utf8(&tampered.pack).unwrap(),
            std::str::from_utf8(&tampered.configuration).unwrap(),
            std::str::from_utf8(&tampered.body).unwrap(),
        );
        assert!(request::verify_binding(p_s, c_s, b_s, &tampered.digest, false).is_err());
    }

    set_mock(json!({"status": 200, "body": ok_env}));
    for expected in [1u64, 2] {
        let mut p = gateway_provider(&call, permit_active(130), true);
        assert!(matches!(
            call_gw(&mut p, &call, 124000).unwrap().admission,
            Admission::Accepted(_)
        ));
        let st = mock_state();
        assert_eq!(
            (st["calls"].as_u64(), st["responses_calls"].as_u64()),
            (Some(expected), Some(expected))
        );
    }
}

#[test]
#[ignore = "requires disposable LiteLLM probe container on 127.0.0.1:14000/14082"]
fn stop_disconnect_and_redaction_matrix() {
    if let Ok(mode) = std::env::var("DW_LITELLM_PROBE_CHILD") {
        return probe_child_worker(&mode);
    }
    ensure_gateway();
    let d1 = wire_cases()[0]["pack"].as_str().unwrap().to_owned();
    let call = request::bind(&successor_pack(&d1, "deepseek-responses-low-v1"), None).unwrap();

    set_mock(json!({"status": 200}));
    let stopped = permit_active(130);
    let mut p_before = gateway_provider(&call, stopped.clone(), false);
    stopped.stop();
    assert_eq!(
        call_gw(&mut p_before, &call, 40000).err(),
        Some("current_authority_unavailable")
    );
    assert_eq!(mock_state()["calls"], 0);

    set_mock(json!({"status": 200, "delay_ms": 1500}));
    let live = permit_active(130);
    let stop_h = live.clone();
    let mut p_after = gateway_provider(&call, live, true);
    let stopper = std::thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            let st = mock_state();
            if st["calls"] == 1 && st["in_flight"] == 1 {
                stop_h.stop();
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    });
    let err_after = call_gw(&mut p_after, &call, 124000).err();
    assert!(stopper.join().unwrap() && err_after == Some("provider_cancelled_uncertain"));
    assert_eq!(
        p_after
            .prepare(&call, Instant::now() + Duration::from_millis(124000))
            .err(),
        Some("inference_deadline")
    );
    let settle = Instant::now() + Duration::from_secs(3);
    while mock_state()["in_flight"] != 0 && Instant::now() < settle {
        std::thread::sleep(Duration::from_millis(50));
    }
    let st_after = mock_state();
    assert_eq!(st_after["calls"], 1);
    assert_eq!(
        st_after["upstream_finished"].as_u64().unwrap()
            + st_after["upstream_disconnected"].as_u64().unwrap(),
        1
    );

    set_mock(json!({"status": 200, "delay_ms": 1500}));
    let soft_permit = permit_active(130);
    let now = Instant::now();
    let (soft, hard) = (
        now + Duration::from_millis(450),
        now + Duration::from_millis(2450),
    );
    let _soft_guard = TransportGuard::arm_successor(soft_permit.clone(), soft, hard, None).unwrap();
    let mut p_soft = gateway_provider(&call, soft_permit, true);
    let soft_err = call_gw(&mut p_soft, &call, 124000).err();
    drop(_soft_guard);
    assert_eq!(soft_err, Some("provider_cancelled_uncertain"));
    let settle_s = Instant::now() + Duration::from_secs(3);
    while mock_state()["in_flight"] != 0 && Instant::now() < settle_s {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(mock_state()["calls"], 1);

    for mode in ["parent_loss", "lease_expiry"] {
        set_mock(json!({"status": 200, "delay_ms": 3500}));
        let target = "m1_reasoning::transport::litellm_tests::stop_disconnect_and_redaction_matrix";
        let mut child = Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("SystemRoot", std::env::var_os("SystemRoot").unwrap())
            .env("DW_LITELLM_PROBE_CHILD", mode)
            .args(["--exact", target, "--ignored", "--nocapture"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let arrive = Instant::now() + Duration::from_secs(5);
        while mock_state()["calls"] == 0 {
            assert!(Instant::now() < arrive, "child never sent: {mode}");
            std::thread::sleep(Duration::from_millis(25));
        }
        let t0 = Instant::now();
        if mode == "parent_loss" {
            drop(child.stdin.take());
        }
        assert_eq!(child.wait().unwrap().code(), Some(124), "mode={mode}");
        assert!(t0.elapsed() <= Duration::from_millis(2600));
        let drain = Instant::now() + Duration::from_secs(4);
        while mock_state()["in_flight"] != 0 && Instant::now() < drain {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(mock_state()["calls"], 1);
    }

    let raw_ref = proposal_json("retain", &["asset_0"], GATEWAY_KEY);
    let esc_key: String = GATEWAY_KEY
        .chars()
        .map(|c| format!("\\u{:04x}", u32::from(c)))
        .collect();
    let esc_ref = raw_ref.replace(GATEWAY_KEY, &esc_key);
    let err_both = json!({"error": {"message": format!("bad {GATEWAY_KEY} {UPSTREAM_KEY} SENTINEL_ERR"), "type": "invalid_request_error", "param": "model"}}).to_string();
    let err_up = json!({"error": {"message": format!("up {UPSTREAM_KEY} SENTINEL_ERR"), "type": "invalid_request_error", "param": "model"}}).to_string();
    for (cfg, exp) in [
        (
            json!({"status": 200, "body": completed_envelope(&raw_ref, "deepseek-flash", Value::Null)}),
            "provider_unsafe_response",
        ),
        (
            json!({"status": 200, "body": completed_envelope(&esc_ref, "deepseek-flash", Value::Null)}),
            "provider_unsafe_response",
        ),
        (
            json!({"status": 400, "body": err_both}),
            "provider_protocol",
        ),
        (json!({"status": 400, "body": err_up}), "provider_protocol"),
    ] {
        set_mock(cfg);
        let mut p = gateway_provider(&call, permit_active(130), true);
        assert_eq!(call_gw(&mut p, &call, 124000).err(), Some(exp));
        let diag = serde_json::to_string(&p.diagnostic).unwrap();
        assert!(
            !diag.contains(GATEWAY_KEY)
                && !diag.contains(UPSTREAM_KEY)
                && !diag.contains("SENTINEL")
        );
    }
    let (_, raw_hdr, _, el_hdr) = raw_exchange(&call);
    let mut raw_with_key_hdr = b"HTTP/1.1 200 OK\r\nX-Probe-Echo: ".to_vec();
    raw_with_key_hdr.extend_from_slice(GATEWAY_KEY.as_bytes());
    raw_with_key_hdr.extend_from_slice(&raw_hdr[15..]);
    let mut diag_hdr = ProviderDiagnostic::new(call.digest.clone());
    let hdr_res = finish_transfer(
        Ok(()),
        raw_with_key_hdr,
        false,
        GATEWAY_KEY,
        el_hdr,
        &mut ProbeBasis::new(true),
        &mut diag_hdr,
    );
    assert_eq!(hdr_res.err(), Some("provider_unsafe_response"));
    assert!(diag_hdr.report.is_none());
}

fn probe_child_worker(mode: &str) {
    let d1 = wire_cases()[0]["pack"].as_str().unwrap().to_owned();
    let call = request::bind(&successor_pack(&d1, "deepseek-responses-low-v1"), None).unwrap();
    let end = Instant::now() + Duration::from_millis(124000);
    let permit = if mode == "parent_loss" {
        let p = permit_active(130);
        let p_in = p.clone();
        std::thread::spawn(move || {
            let _ = std::io::stdin().read(&mut [0u8; 1]);
            p_in.stop();
        });
        p
    } else {
        let p = Permit::new(end);
        assert!(p.publish(Instant::now(), end));
        p
    };
    let _guard = TransportGuard::arm_successor(permit.clone(), end, end, None).unwrap();
    let mut provider = gateway_provider(&call, permit, true);
    let _ = call_gw(&mut provider, &call, 124000);
    loop {
        std::thread::sleep(Duration::from_millis(50));
    }
}

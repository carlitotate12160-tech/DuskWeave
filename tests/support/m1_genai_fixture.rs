#![allow(dead_code)]
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub const FIXTURE_PROFILE: &str = "fixture-genai-responses-v1";

pub fn run_request() -> Value {
    let id = "00000000-0000-0000-0000-000000000021";
    json!({"version":"m1_autonomous_run_request_v3","run_id":id,
        "run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
        "inference_profile":FIXTURE_PROFILE,"deadline_at_ms":1,
        "context":{"version":"m1_context_prepare_request_v1","mission":{
            "version":"m1_inference_reservation_request_v1","engagement_id":id,"campaign_id":id,
            "operator_ref":id,"expected_mission_revision":1,"permission_id":id,"goal_ref":id,
            "exercise_mode":"blind"},"candidates":[{"asset_ref":id,"priority":"high"}]},
        "seed_names":[{"asset_ref":id,"name":"seed.example.invalid"}]})
}

pub fn configure_permission(value: &mut Value) {
    let p = &mut value["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v5");
    p["profile"] = json!(FIXTURE_PROFILE);
    p["recipient"] = json!("http://127.0.0.1:18083/responses");
    p["run_profile"] = json!("autonomous-dns-fixture-v2");
    p["execution_mode"] = json!("controlled_fixture");
    p["field_classes"] = json!([
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
    p["campaign_limits"]["wall_time_ms"] = json!(500000);
    for (name, n) in [
        ("json_depth", 32),
        ("context_bytes", 8192),
        ("decision_wall_time_ms", 129000),
        ("per_call_wall_time_ms", 62000),
        ("shared_wall_time_ms", 124000),
        ("phase_wall_time_ms", 134000),
        ("soft_completion_ms", 60000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        p["operation_limits"][name] = json!(n);
    }
}

// Raw HTTP framing is fixture-owned; only synthetic bytes are kept in memory.
pub struct Incoming {
    pub headers: String,
    pub body: Vec<u8>,
}
pub enum Reply {
    Length(u16, Vec<u8>),
    Chunked(Vec<u8>, usize),
    Close(Vec<u8>),
    Misleading(Vec<u8>, usize),
    Reset,
    Hold(Arc<AtomicBool>),
}
#[derive(Default, Debug)]
pub struct WireCounts {
    pub read_complete: usize,
    pub read_failed: usize,
    pub write_complete: usize,
    pub write_failed: usize,
    pub read_failure: Option<&'static str>,
    pub read_io: Option<std::io::ErrorKind>,
    pub write_failure: Option<std::io::ErrorKind>,
}
pub struct Server {
    pub wire: Arc<Mutex<WireCounts>>,
    pub address: SocketAddr,
    pub requests: Arc<Mutex<Vec<Incoming>>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub fn start(
        address: &str,
        mut reply: impl FnMut(&Incoming) -> Reply + Send + 'static,
    ) -> Self {
        let listener =
            TcpListener::bind(address).expect("owned fixture endpoint must be available");
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let wire = Arc::new(Mutex::new(WireCounts::default()));
        let progress = wire.clone();
        let (stopped, recorded) = (stop.clone(), requests.clone());
        let worker = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Winsock accept inherits the listener's nonblocking mode.
                        // Connected fixture I/O is blocking with finite timeouts.
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut read_io = None;
                        match read_request(&mut stream, &mut read_io) {
                            Ok(request) => {
                                progress.lock().unwrap().read_complete += 1;
                                let response = reply(&request);
                                recorded.lock().unwrap().push(request);
                                match send_reply(&mut stream, response, &stopped) {
                                    Ok(()) => progress.lock().unwrap().write_complete += 1,
                                    Err(e) => {
                                        let mut w = progress.lock().unwrap();
                                        w.write_failed += 1;
                                        w.write_failure = Some(e.kind());
                                    }
                                }
                            }
                            Err(reason) => {
                                let mut w = progress.lock().unwrap();
                                w.read_failed += 1;
                                w.read_failure = Some(reason);
                                w.read_io = read_io;
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            wire,
            worker: Some(worker),
        }
    }
    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
    pub fn await_count(&self, count: usize) {
        let end = Instant::now() + Duration::from_secs(5);
        while self.count() < count && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(self.count(), count, "fixture request barrier");
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn read_request(
    stream: &mut TcpStream,
    read_io: &mut Option<std::io::ErrorKind>,
) -> Result<Incoming, &'static str> {
    let mut raw = Vec::new();
    let mut byte = [0];
    while !raw.ends_with(b"\r\n\r\n") {
        if raw.len() > 32768 {
            return Err("header_limit");
        }
        stream.read_exact(&mut byte).map_err(|e| {
            *read_io = Some(e.kind());
            "header_read"
        })?;
        raw.push(byte[0]);
    }
    let headers = String::from_utf8(raw).map_err(|_| "header_utf8")?;
    let len = headers
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .ok_or("missing_length")?
        .1
        .trim()
        .parse::<usize>()
        .map_err(|_| "invalid_length")?;
    if len > 32768 {
        return Err("request_limit");
    }
    let mut body = vec![0; len];
    stream.read_exact(&mut body).map_err(|e| {
        *read_io = Some(e.kind());
        "body_read"
    })?;
    Ok(Incoming { headers, body })
}
fn send_reply(stream: &mut TcpStream, reply: Reply, stopped: &AtomicBool) -> std::io::Result<()> {
    let result: std::io::Result<()> = (|| {
        match reply {
            Reply::Length(status, body) => {
                let location = if status == 302 {
                    "Location: /redirected\r\n"
                } else {
                    ""
                };
                write!(
                    stream,
                    "HTTP/1.1 {status} Synthetic\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{location}\r\n",
                    body.len()
                )?;
                stream.write_all(&body)?;
            }
            Reply::Chunked(body, size) => {
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")?;
                for bytes in body.chunks(size) {
                    write!(stream, "{:x}\r\n", bytes.len())?;
                    stream.write_all(bytes)?;
                    stream.write_all(b"\r\n")?;
                }
                stream.write_all(b"0\r\n\r\n")?;
            }
            Reply::Close(body) => {
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n")?;
                stream.write_all(&body)?;
            }
            Reply::Misleading(body, size) => {
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n"
                )?;
                stream.write_all(&body)?;
            }
            Reply::Reset => {}
            Reply::Hold(release) => {
                let end = Instant::now() + Duration::from_secs(10);
                while !release.load(Ordering::SeqCst)
                    && !stopped.load(Ordering::SeqCst)
                    && Instant::now() < end
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        }
        Ok(())
    })();
    result // Client cancellation may deliberately close the socket mid-response.
}
pub fn envelope(proposal: &Value) -> Value {
    json!({"id":"synthetic-response","model":"deepseek-flash","status":"completed",
        "output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":proposal.to_string()}]}],
        "usage":{"input_tokens":0,"output_tokens":0,"output_tokens_details":{"reasoning_tokens":0}}})
}
pub fn pack_in(value: &Value) -> Option<Value> {
    match value {
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .filter(|p| p["version"] == "m1_context_pack_autonomous_v3"),
        Value::Array(a) => a.iter().find_map(pack_in),
        Value::Object(o) => o.values().find_map(pack_in),
        _ => None,
    }
}
pub fn proposal(pack: &Value) -> Value {
    let feedback = pack["predecessor"]["availability"] == "committed";
    let observed = pack["references"].as_array().unwrap().iter().find(|r| {
        r["owner"] == "cyber_terrain"
            && r["source"] == "fixture_dns_acquisition"
            && r["status"] == "observed"
    });
    let (refs, intent) = if feedback {
        let id = observed.expect("D2 must receive qualified Terrain feedback")["id"].clone();
        (
            json!(["previous_decision", id]),
            json!({"category":"retain","references":[id],"manifest_reference":null}),
        )
    } else {
        (
            json!(["asset_0"]),
            json!({"category":"observe_more","references":["mission","asset_0"],"manifest_reference":"dns_udp","view_request":null}),
        )
    };
    json!({"version":"m1_inference_proposal_v1","hypothesis":if feedback {"Qualified fixture feedback supports the admitted routing hypothesis."} else {"The admitted routing premise merits bounded fixture observation."},
        "considered_alternative":"Routing may remain unavailable.","references":refs,
        "expected_observation":"Qualified fixture routing feedback.","falsifying_observation":"Missing or conflicting owner feedback.",
        "uncertainty":"Synthetic plumbing only; live applicability is unknown.",
        "rationale":if feedback {"Qualified fixture feedback supports retention."} else {"Observe one admitted seed."},"next_intent":intent})
}

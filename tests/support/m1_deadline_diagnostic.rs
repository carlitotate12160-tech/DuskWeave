//! Offline deadline diagnostics (DW-DIAGNOSE-M1-SCHEMA-DEADLINE): real
//! `decision_attempt::decide` two-call sequence through the concrete
//! transport against loopback fixtures, plus sacrificial child cases for the
//! hard watchdog exits. Test Broker records finite safe checkpoints only —
//! phase enums, monotonic elapsed/remaining ms, request counts and exit
//! categories; this isolates transport/control timing and does NOT qualify
//! PostgreSQL transactions, the full CLI composition or startup latency.
use super::*;
use crate::m1_reasoning::decision_attempt;
use crate::m1_reasoning::inference::{
    Broker, Provider, Received, ReleaseFailure, Reservation, Scope, Usage,
};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use uuid::Uuid;

const PACK: &[u8] = br#"{"references":[{"id":"ref-1"}]}"#;

fn envelope(text: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"status":"completed","model":"deepseek-flash",
        "output":[{"type":"message","role":"assistant","status":"completed",
        "content":[{"type":"output_text","text":text}]}],
        "usage":{"input_tokens":12,"output_tokens":23}}))
    .unwrap()
}

fn valid_proposal() -> String {
    json!({"version":"m1_inference_proposal_v1",
        "hypothesis":"bounded synthetic hypothesis",
        "considered_alternative":"synthetic alternative",
        "references":["ref-1"],"expected_observation":"expected",
        "falsifying_observation":"falsifying","uncertainty":"unknown",
        "rationale":"safe rationale","next_intent":{"category":"observe_more",
        "references":["ref-1"],"manifest_reference":null,
        "view_request":{"owner":"cyber_terrain","selectors":["ref-1"]}}})
    .to_string()
}

fn defective_proposal() -> String {
    // Missing required `rationale` -> decoder must yield proposal_shape.
    json!({"version":"m1_inference_proposal_v1",
        "hypothesis":"h","considered_alternative":"a","references":["ref-1"],
        "expected_observation":"e","falsifying_observation":"f",
        "uncertainty":"u","next_intent":{"category":"defer",
        "references":["ref-1"],"manifest_reference":null}})
    .to_string()
}

/// Ordered (delay_ms, Option<body>) per accepted connection; None holds the
/// socket for delay_ms then closes without a response (injected stall).
struct TwoShot {
    url: String,
    accepted: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

impl TwoShot {
    fn start(responses: Vec<(u64, Option<Vec<u8>>)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/responses", listener.local_addr().unwrap());
        let accepted = Arc::new(AtomicUsize::new(0));
        let observed = accepted.clone();
        let worker = std::thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(90);
            for (delay_ms, body) in responses {
                let mut stream = loop {
                    if Instant::now() >= end {
                        return;
                    }
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(_) => return,
                    }
                };
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                observed.fetch_add(1, Ordering::SeqCst);
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 4096];
                let read_end = Instant::now() + Duration::from_secs(8);
                while Instant::now() < read_end {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            bytes.extend_from_slice(&chunk[..n]);
                            if let Some(pos) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                                let head = String::from_utf8_lossy(&bytes[..pos]);
                                let len = head
                                    .lines()
                                    .find_map(|line| {
                                        let (k, v) = line.split_once(':')?;
                                        k.eq_ignore_ascii_case("content-length")
                                            .then(|| v.trim().parse().ok())
                                            .flatten()
                                    })
                                    .unwrap_or(0);
                                if bytes.len() >= pos + 4 + len {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                std::thread::sleep(Duration::from_millis(delay_ms));
                if let Some(body) = body {
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&body);
                }
            }
        });
        Self {
            url,
            accepted,
            worker: Some(worker),
        }
    }
}

impl Drop for TwoShot {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Real provider path per call: production prepare equivalent with a
/// loopback URL and synthetic token (private test construction only).
struct LoopbackProvider {
    url: String,
    inner: DeepSeekProvider,
}

impl Provider for LoopbackProvider {
    fn prepare(&mut self, call: &BoundCall, deadline: Instant) -> Res<()> {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or("inference_deadline")?;
        let mut h = configure(call, "synthetic-fixture-token", left)
            .map_err(|_| "inference_transport_unavailable")?;
        h.url(&self.url)
            .map_err(|_| "inference_transport_unavailable")?;
        self.inner = DeepSeekProvider {
            prepared: Some((h, "synthetic-fixture-token".into(), call.digest.clone())),
        };
        Ok(())
    }
    fn call(
        &mut self,
        call: &BoundCall,
        deadline: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Received> {
        self.inner.call(call, deadline, basis, release)
    }
}

/// Narrow test Broker: finite reservation, may-send markers, deadline gate
/// mirroring the real release path, and safe checkpoint events only.
struct DiagBroker {
    generation_call: Uuid,
    repair_call: Uuid,
    deadline: Instant,
    t0: Instant,
    events: Vec<String>,
    log: Option<PathBuf>,
    /// Injected deterministic blocking inside repair may_send (scenario C).
    block_repair_ms: u64,
}

impl DiagBroker {
    fn new(deadline_ms: u64, log: Option<PathBuf>) -> Self {
        Self {
            generation_call: Uuid::from_u128(0xd001),
            repair_call: Uuid::from_u128(0xd002),
            deadline: Instant::now() + Duration::from_millis(deadline_ms),
            t0: Instant::now(),
            events: Vec::new(),
            log,
            block_repair_ms: 0,
        }
    }
    fn mark(&mut self, phase: &str) {
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        self.events.push(phase.to_owned());
        if let Some(path) = &self.log {
            let line = json!({"phase": phase,
                "elapsed_ms": self.t0.elapsed().as_millis(),
                "remaining_ms": remaining});
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }
    }
    fn label(&self, call: Uuid) -> &'static str {
        if call == self.generation_call {
            "generation"
        } else if call == self.repair_call {
            "repair"
        } else {
            "unknown"
        }
    }
}

impl Broker for DiagBroker {
    fn reserve(&mut self, _: &Scope, _: &[u8]) -> Res<Reservation> {
        self.mark("reserve");
        Ok(Reservation {
            generation: 1,
            generation_call: self.generation_call,
            repair_call: self.repair_call,
            shared_deadline: self.deadline,
        })
    }
    fn may_send(&mut self, _: &Scope, call: Uuid, _: &BoundCall) -> Res<()> {
        if call == self.repair_call && self.block_repair_ms > 0 {
            // Deterministic injected delay so the shared deadline expires
            // between the marker and the release gate.
            std::thread::sleep(Duration::from_millis(self.block_repair_ms));
        }
        self.mark(match self.label(call) {
            "generation" => "may_send_generation",
            "repair" => "may_send_repair",
            _ => "may_send_unknown",
        });
        Ok(())
    }
    fn release_into(
        &mut self,
        _: &Scope,
        r: &Reservation,
        _: Uuid,
        bound: &BoundCall,
        provider: &mut dyn Provider,
        basis: &mut dyn ProposalBasis,
    ) -> Result<Received, ReleaseFailure> {
        self.mark("release_enter");
        if Instant::now() >= r.shared_deadline {
            self.mark("release_refused_deadline");
            return Err(ReleaseFailure::NeverReleased("inference_deadline"));
        }
        let mut phases = Vec::new();
        let result = provider.call(bound, r.shared_deadline, basis, &mut || {
            phases.push("release_callback");
            Ok(())
        });
        for phase in phases {
            self.mark(phase);
        }
        self.mark(if result.is_ok() {
            "send_completed"
        } else {
            "send_failed_may_have_sent"
        });
        result.map_err(ReleaseFailure::MayHaveSent)
    }
    fn never_released(&mut self, _: &Scope, _: Uuid, _: &'static str) -> Res<()> {
        self.mark("accounting_never_released");
        Ok(())
    }
    fn completed(&mut self, _: &Scope, _: Uuid, _: &Usage) -> Res<()> {
        self.mark("accounting_completed");
        Ok(())
    }
    fn uncertain(&mut self, _: &Scope, _: Uuid, _: &'static str) -> Res<()> {
        self.mark("accounting_uncertain");
        Ok(())
    }
    fn close_unused_repair(&mut self, _: &Scope, _: &Reservation) -> Res<()> {
        Ok(())
    }
    fn provenance(&mut self, _: &Scope) -> Res<serde_json::Value> {
        Ok(json!({}))
    }
    fn finish(&mut self, _: &Scope) -> Res<()> {
        Ok(())
    }
    fn inspect(&mut self, _: &Scope) -> Res<&'static str> {
        Ok("synthetic")
    }
}

fn scope() -> Scope {
    Scope {
        engagement: Uuid::from_u128(0xe001),
        campaign: Uuid::from_u128(0xe002),
        decision: Uuid::from_u128(0xe003),
        mission_revision: 1,
        permission: Uuid::from_u128(0xe004),
    }
}

fn summarize(outcome: &Res<(Admission, Vec<Uuid>, Vec<String>)>) -> String {
    match outcome {
        Ok((Admission::Accepted(_), _, _)) => "accepted".to_owned(),
        Ok((Admission::Repairable(codes), _, _)) => {
            format!("repairable:{}", codes.join(","))
        }
        Ok((Admission::Rejected(code), _, _)) | Err(code) => {
            format!("rejected:{code}")
        }
    }
}

fn run_decide(url: &str, deadline_ms: u64, log: Option<PathBuf>) -> (DiagBroker, String) {
    let mut broker = DiagBroker::new(deadline_ms, log);
    let mut provider = LoopbackProvider {
        url: url.to_owned(),
        inner: DeepSeekProvider::default(),
    };
    let mut basis = Basis(true);
    let outcome = decision_attempt::decide(&mut broker, &mut provider, &mut basis, &scope(), PACK);
    broker.mark("decide_returned");
    let summary = summarize(&outcome);
    (broker, summary)
}

/// Bounded terminal record appended to the checkpoint stream on a normal
/// return. Write errors fail the child/test: absent or malformed records
/// must not count as success. Not called on the watchdog-interrupted path.
fn write_terminal(log: &PathBuf, broker: &DiagBroker, summary: &str) {
    let line = json!({"phase": "terminal", "outcome": summary,
        "elapsed_ms": broker.t0.elapsed().as_millis(),
        "remaining_ms": broker
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis()});
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(log)
        .expect("terminal_record_write");
    writeln!(f, "{line}").expect("terminal_record_write");
    f.flush().expect("terminal_record_write");
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Sacrificial-child dispatch: only runs under DW_DIAG_SCENARIO.
#[test]
fn deadline_diag_child() {
    let Ok(scenario) = std::env::var("DW_DIAG_SCENARIO") else {
        return;
    };
    match scenario.as_str() {
        "transport_watchdog_only" => {
            let arm = Instant::now() + Duration::from_millis(env_u64("DW_DIAG_ARM_MS", 200));
            let _deadline = Deadline::arm(arm).expect("armed");
            // Controlled blocking: plain sleep; only the armed watchdog can end it.
            std::thread::sleep(Duration::from_secs(30));
        }
        "decide" | "decide_skip_terminal" => {
            let url = std::env::var("DW_DIAG_URL").unwrap();
            let deadline_ms = env_u64("DW_DIAG_DEADLINE_MS", 1500);
            let log = std::env::var("DW_DIAG_CHECKPOINT").ok().map(PathBuf::from);
            if let Some(path) = &log {
                std::fs::write(path, "").unwrap();
            }
            let mut broker = DiagBroker::new(deadline_ms, log.clone());
            let mut provider = LoopbackProvider {
                url: url.clone(),
                inner: DeepSeekProvider::default(),
            };
            let mut basis = Basis(true);
            let outcome =
                decision_attempt::decide(&mut broker, &mut provider, &mut basis, &scope(), PACK);
            broker.mark("decide_returned");
            if scenario == "decide" {
                // Terminal evidence is required on a normal return; a write
                // failure must fail the child, never become silent success.
                write_terminal(
                    log.as_ref().expect("checkpoint_path_required"),
                    &broker,
                    &summarize(&outcome),
                );
            }
        }
        _ => {}
    }
}

fn spawn_child(envs: &[(&str, String)]) -> std::process::Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "m1_reasoning::transport::tests::m1_deadline_diagnostic::deadline_diag_child",
            "--nocapture",
        ])
        .env("DW_DIAG_SCENARIO", envs[0].1.as_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in envs.iter().skip(1) {
        command.env(key, value);
    }
    command.spawn().unwrap()
}

/// Bounded parent wait: polls exit and kills only the owned child on expiry.
fn wait_bounded(child: &mut std::process::Child, bound: Duration) -> (Option<i32>, Duration) {
    let start = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return (status.code(), start.elapsed());
        }
        if start.elapsed() > bound {
            let _ = child.kill();
            let _ = child.wait();
            return (None, start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Optional evidence export: when DW_DIAG_REPORT_DIR names an existing
/// directory, copy the child's checkpoint timeline and a bounded outcome
/// record into it. Absent variable keeps tests hermetic.
fn export_outcome(
    name: &str,
    ckpt: &PathBuf,
    code: Option<i32>,
    elapsed: Duration,
    calls: usize,
    oracle: &Result<(), String>,
) {
    let Some(dir) = std::env::var_os("DW_DIAG_REPORT_DIR") else {
        return;
    };
    let dir = PathBuf::from(dir);
    if !dir.is_dir() {
        return;
    }
    if let Ok(text) = std::fs::read(ckpt) {
        let _ = std::fs::write(dir.join(format!("{name}-checkpoints.jsonl")), text);
    }
    let outcome = json!({"case": name,
        "exit_code": code,
        "elapsed_ms": elapsed.as_millis(),
        "accepted_connections": calls,
        "oracle": match oracle { Ok(()) => "pass".to_owned(), Err(e) => format!("rejected: {e}") }});
    let _ = std::fs::write(
        dir.join(format!("{name}-outcome.json")),
        serde_json::to_string_pretty(&outcome).unwrap(),
    );
}

/// Deadline-crossing oracle: a successful child observation must prove the
/// intended outcome, not just exit. Two accepted loopback connections and the
/// generation-complete/repair-entry checkpoints are required on both paths.
/// Exit 124 additionally needs near-deadline elapsed; a normal return needs
/// a well-formed terminal record consistent with deadline-limited transport
/// rejection plus the uncertain accounting callback. Anything else is a
/// rejected observation (false-positive guard).
fn verify_crossing(
    ckpt_text: &str,
    code: Option<i32>,
    elapsed: Duration,
    connections: usize,
    budget_ms: u64,
    window_ms: (u64, u64),
) -> Result<(), String> {
    for phase in [
        "may_send_generation",
        "release_callback",
        "send_completed",
        "accounting_completed",
        "may_send_repair",
        "release_enter",
    ] {
        if !ckpt_text.contains(phase) {
            return Err(format!("missing phase {phase}"));
        }
    }
    if connections != 2 {
        return Err(format!("accepted loopback connections {connections} != 2"));
    }
    match code {
        Some(124) => {
            // Interrupted preterminal path: watchdog fired inside the send.
            // Missing terminal evidence is expected only here.
            let ms = elapsed.as_millis() as u64;
            if !(window_ms.0..=window_ms.1).contains(&ms) {
                return Err(format!("exit 124 elapsed {ms}ms outside {window_ms:?}"));
            }
            Ok(())
        }
        Some(0) => {
            // Normal return: deadline-limited transport rejection is the
            // only consistent terminal outcome; prove it from the record.
            let line = ckpt_text
                .lines()
                .rev()
                .find(|l| l.contains("\"phase\":\"terminal\""))
                .ok_or("terminal record absent")?;
            let record: Value =
                serde_json::from_str(line).map_err(|_| "terminal record malformed")?;
            let outcome = record["outcome"]
                .as_str()
                .ok_or("terminal outcome missing")?;
            if outcome != "rejected:provider_transport_unknown" {
                return Err(format!(
                    "terminal outcome {outcome} is not a deadline rejection"
                ));
            }
            let elapsed_ms = record["elapsed_ms"]
                .as_u64()
                .ok_or("terminal elapsed missing")?;
            let floor = budget_ms.saturating_sub(100);
            if !(floor..=budget_ms + 4000).contains(&elapsed_ms) {
                return Err(format!(
                    "terminal elapsed {elapsed_ms}ms outside {floor}..={} ",
                    budget_ms + 4000
                ));
            }
            let remaining_ms = record["remaining_ms"]
                .as_u64()
                .ok_or("terminal remaining missing")?;
            if remaining_ms > 100 {
                return Err(format!("terminal remaining {remaining_ms}ms > 100"));
            }
            if !ckpt_text.contains("accounting_uncertain") {
                return Err("uncertain accounting callback missing".to_owned());
            }
            Ok(())
        }
        Some(other) => Err(format!("unexpected exit code {other}")),
        None => Err("child did not exit within the parent bound".to_owned()),
    }
}

fn unique_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dw-diag-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn generation_shape_defect_then_admitted_repair_one_reservation() {
    // Scenario A: defective generation -> proposal_shape -> fast valid repair.
    let server = TwoShot::start(vec![
        (0, Some(envelope(&defective_proposal()))),
        (0, Some(envelope(&valid_proposal()))),
    ]);
    let (broker, summary) = run_decide(&server.url, 5_000, None);
    assert_eq!(summary, "accepted");
    assert_eq!(server.accepted.load(Ordering::SeqCst), 2);
    for phase in [
        "reserve",
        "may_send_generation",
        "release_enter",
        "release_callback",
        "send_completed",
        "accounting_completed",
        "may_send_repair",
        "decide_returned",
    ] {
        assert!(
            broker.events.contains(&phase.to_owned()),
            "missing phase {phase}: {:?}",
            broker.events
        );
    }
    let g_pos = broker
        .events
        .iter()
        .position(|e| e == "may_send_generation")
        .unwrap();
    let r_pos = broker
        .events
        .iter()
        .position(|e| e == "may_send_repair")
        .unwrap();
    assert!(g_pos < r_pos, "generation marker precedes repair marker");
}

#[test]
fn expired_deadline_before_repair_release_prevents_second_send() {
    // Scenario C: injected may_send delay expires the SAME absolute deadline
    // before release -> NeverReleased, no second loopback request.
    let server = TwoShot::start(vec![(0, Some(envelope(&defective_proposal())))]);
    let mut provider = LoopbackProvider {
        url: server.url.clone(),
        inner: DeepSeekProvider::default(),
    };
    let mut broker = DiagBroker::new(600, None);
    broker.block_repair_ms = 900;
    let mut basis = Basis(true);
    let outcome = decision_attempt::decide(&mut broker, &mut provider, &mut basis, &scope(), PACK);
    // decide maps the recorded NeverReleased failure to Rejected(code).
    assert!(matches!(
        outcome,
        Ok((Admission::Rejected("inference_deadline"), _, _))
    ));
    assert_eq!(server.accepted.load(Ordering::SeqCst), 1);
    for phase in [
        "may_send_repair",
        "release_enter",
        "release_refused_deadline",
        "accounting_never_released",
    ] {
        assert!(
            broker.events.contains(&phase.to_owned()),
            "missing phase {phase}: {:?}",
            broker.events
        );
    }
}

#[test]
fn repair_send_crossing_scaled_deadline_records_exit_and_phase() {
    // Scenario B (scaled): generation completes, repair send stalls past the
    // shared deadline inside the sacrificial child.
    let dir = unique_dir("scaled");
    std::fs::create_dir(&dir).unwrap();
    let ckpt = dir.join("checkpoints.jsonl");
    let server = TwoShot::start(vec![
        (0, Some(envelope(&defective_proposal()))),
        (8_000, Some(envelope(&valid_proposal()))),
    ]);
    let mut child = spawn_child(&[
        ("DW_DIAG_SCENARIO", "decide".to_owned()),
        ("DW_DIAG_URL", server.url.clone()),
        ("DW_DIAG_DEADLINE_MS", "1500".to_owned()),
        (
            "DW_DIAG_CHECKPOINT",
            ckpt.to_string_lossy().replace('\\', "/"),
        ),
    ]);
    let (code, elapsed) = wait_bounded(&mut child, Duration::from_secs(12));
    let text = std::fs::read_to_string(&ckpt).unwrap_or_default();
    let connections = server.accepted.load(Ordering::SeqCst);
    let verdict = verify_crossing(&text, code, elapsed, connections, 1500, (1000, 5500));
    export_outcome(
        "repair_send_crossing_scaled",
        &ckpt,
        code,
        elapsed,
        connections,
        &verdict,
    );
    verdict.expect("scaled crossing observation must prove the deadline outcome");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn isolated_transport_watchdog_exits_124_in_child() {
    // Scenario D: only the transport Deadline is armed; blocking is a plain
    // sleep. The only possible terminator is the watchdog's process::exit.
    let mut child = spawn_child(&[
        ("DW_DIAG_SCENARIO", "transport_watchdog_only".to_owned()),
        ("DW_DIAG_ARM_MS", "200".to_owned()),
    ]);
    let (code, elapsed) = wait_bounded(&mut child, Duration::from_secs(10));
    assert_eq!(code, Some(124), "armed watchdog must exit(124)");
    assert!(
        elapsed < Duration::from_secs(5),
        "exit must occur near the armed bound, not the sleep bound"
    );
}

#[test]
fn disarmed_transport_deadline_never_exits() {
    // Scenario D control: Drop disarms the watchdog in-process (safe).
    let deadline = Deadline::arm(Instant::now() + Duration::from_millis(100)).unwrap();
    drop(deadline);
    std::thread::sleep(Duration::from_millis(250));
    // Reaching this assertion proves the armed watchdog was disarmed.
}

#[test]
fn unscaled_twenty_second_shared_deadline_child() {
    // Representative unscaled case: real 20 s shared deadline; generation is
    // fast and defective; the repair response is held beyond the deadline.
    // The child either exits by watchdog (124) or returns a transport error.
    let dir = unique_dir("unscaled");
    std::fs::create_dir(&dir).unwrap();
    let ckpt = dir.join("checkpoints.jsonl");
    let server = TwoShot::start(vec![
        (0, Some(envelope(&defective_proposal()))),
        (23_000, Some(envelope(&valid_proposal()))),
    ]);
    let mut child = spawn_child(&[
        ("DW_DIAG_SCENARIO", "decide".to_owned()),
        ("DW_DIAG_URL", server.url.clone()),
        ("DW_DIAG_DEADLINE_MS", "20000".to_owned()),
        (
            "DW_DIAG_CHECKPOINT",
            ckpt.to_string_lossy().replace('\\', "/"),
        ),
    ]);
    let (code, elapsed) = wait_bounded(&mut child, Duration::from_secs(26));
    let text = std::fs::read_to_string(&ckpt).unwrap_or_default();
    let connections = server.accepted.load(Ordering::SeqCst);
    let verdict = verify_crossing(&text, code, elapsed, connections, 20000, (17000, 24000));
    export_outcome(
        "unscaled_shared_deadline",
        &ckpt,
        code,
        elapsed,
        connections,
        &verdict,
    );
    verdict.expect("unscaled crossing observation must prove the deadline outcome");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Shared parent spawn for oracle-regression children: same verification
/// path as the crossing tests, with the fixture responses chosen per case.
fn spawn_decide_child(
    server: &TwoShot,
    deadline_ms: &str,
    scenario: &str,
) -> (std::process::Child, PathBuf, PathBuf) {
    let dir = unique_dir("regression");
    std::fs::create_dir(&dir).unwrap();
    let ckpt = dir.join("checkpoints.jsonl");
    let child = spawn_child(&[
        ("DW_DIAG_SCENARIO", scenario.to_owned()),
        ("DW_DIAG_URL", server.url.clone()),
        ("DW_DIAG_DEADLINE_MS", deadline_ms.to_owned()),
        (
            "DW_DIAG_CHECKPOINT",
            ckpt.to_string_lossy().replace('\\', "/"),
        ),
    ]);
    (child, dir, ckpt)
}

#[test]
fn fast_repair_child_is_rejected_as_deadline_crossing() {
    // Genuine regression: a real child whose repair completes fast and is
    // admitted exits 0 — but that is NOT deadline-crossing evidence. The
    // shared oracle must reject it (wrong outcome class and remaining time).
    let server = TwoShot::start(vec![
        (0, Some(envelope(&defective_proposal()))),
        (0, Some(envelope(&valid_proposal()))),
    ]);
    let (mut child, dir, ckpt) = spawn_decide_child(&server, "1500", "decide");
    let (code, elapsed) = wait_bounded(&mut child, Duration::from_secs(12));
    let text = std::fs::read_to_string(&ckpt).unwrap_or_default();
    let connections = server.accepted.load(Ordering::SeqCst);
    let verdict = verify_crossing(&text, code, elapsed, connections, 1500, (1000, 5500));
    export_outcome(
        "regression_fast_repair",
        &ckpt,
        code,
        elapsed,
        connections,
        &verdict,
    );
    assert_eq!(code, Some(0), "child must return normally");
    assert_eq!(connections, 2, "both calls must reach the listener");
    let reason = verdict.unwrap_err();
    assert!(
        reason.contains("not a deadline rejection"),
        "rejection must name the accepted outcome, observed: {reason}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn early_transport_failure_child_is_rejected_as_deadline_crossing() {
    // Unrelated early failure: the repair connection closes immediately, so
    // the transport error arrives long before the deadline. Same oracle path
    // must reject it on the terminal elapsed bound.
    let server = TwoShot::start(vec![(0, Some(envelope(&defective_proposal()))), (0, None)]);
    let (mut child, dir, ckpt) = spawn_decide_child(&server, "1500", "decide");
    let (code, elapsed) = wait_bounded(&mut child, Duration::from_secs(12));
    let text = std::fs::read_to_string(&ckpt).unwrap_or_default();
    let connections = server.accepted.load(Ordering::SeqCst);
    let verdict = verify_crossing(&text, code, elapsed, connections, 1500, (1000, 5500));
    export_outcome(
        "regression_early_failure",
        &ckpt,
        code,
        elapsed,
        connections,
        &verdict,
    );
    assert_eq!(code, Some(0), "child must return normally");
    assert_eq!(connections, 2, "both calls must reach the listener");
    let reason = verdict.unwrap_err();
    assert!(
        reason.contains("elapsed") && reason.contains("outside"),
        "rejection must name the elapsed bound, observed: {reason}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_terminal_record_child_is_rejected() {
    // Oracle integrity control: a child that returns normally but omits the
    // required terminal record must be rejected — absent evidence is not
    // success.
    let server = TwoShot::start(vec![
        (0, Some(envelope(&defective_proposal()))),
        (0, Some(envelope(&valid_proposal()))),
    ]);
    let (mut child, dir, ckpt) = spawn_decide_child(&server, "1500", "decide_skip_terminal");
    let (code, elapsed) = wait_bounded(&mut child, Duration::from_secs(12));
    let text = std::fs::read_to_string(&ckpt).unwrap_or_default();
    let connections = server.accepted.load(Ordering::SeqCst);
    let verdict = verify_crossing(&text, code, elapsed, connections, 1500, (1000, 5500));
    export_outcome(
        "regression_missing_terminal",
        &ckpt,
        code,
        elapsed,
        connections,
        &verdict,
    );
    assert_eq!(code, Some(0), "child must return normally");
    assert_eq!(connections, 2, "both calls must reach the listener");
    let reason = verdict.unwrap_err();
    assert!(
        reason.contains("terminal record absent"),
        "rejection must name the missing terminal record, observed: {reason}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn release_callback_is_the_send_linearization_point() {
    // The release callback runs only once, inside the send window, before any
    // accounting transition — mirrors the production ordering claim.
    let server = TwoShot::start(vec![(0, Some(envelope(&valid_proposal())))]);
    let (broker, summary) = run_decide(&server.url, 5_000, None);
    assert_eq!(summary, "accepted");
    let callback = broker
        .events
        .iter()
        .position(|e| e == "release_callback")
        .unwrap();
    let completed = broker
        .events
        .iter()
        .position(|e| e == "accounting_completed")
        .unwrap();
    assert!(callback < completed);
    assert_eq!(server.accepted.load(Ordering::SeqCst), 1);
}

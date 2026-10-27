//! Attempt orchestration: validate the issued request, build the pair, run
//! the ADR-017 phase exchange under the host lease and STOP latch, then
//! dispose and emit a safe receipt. Raw bytes never appear in the receipt.
use super::{docker, msgs, nft, stop, wire};
use serde_json::{json, Value};
use std::io::{BufReader, Write};
use std::process::{ChildStdin, ChildStdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_LEASE_MS: u64 = 2_000;
const MAX_RENEW_MS: u64 = 500;
const MAX_DEADLINE_MS: i64 = 120_000;
const REAP_BOUND_MS: u64 = 1_000;

type Outcome = Result<msgs::WorkerResult, &'static str>;

pub fn preflight() -> Result<Value, &'static str> {
    let docker_ok = docker::live();
    let nft_probe = nft::EgressTable::create("pf0probe", "lo");
    let nft_ok = nft_probe.is_ok();
    if let Ok(t) = nft_probe {
        let _ = t.destroy();
    }
    let cgroup_v2 = std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists();
    Ok(json!({
        "version": "dw_boundary_receipt_v1",
        "kind": "preflight",
        "docker": docker_ok,
        "nftables": nft_ok,
        "cgroup_v2": cgroup_v2,
        "qualified": docker_ok && nft_ok && cgroup_v2,
    }))
}

pub fn cleanup(session: &str) -> Result<Value, &'static str> {
    let removed = docker::sweep(session)?;
    Ok(json!({"version":"dw_boundary_receipt_v1","cleanup":removed,"session":session}))
}

pub fn run(req: &msgs::AttemptRequest) -> Result<Value, &'static str> {
    let tag = validate(req).map(|_| tag_of(req))?;
    if stop::is_latched(&req.control_root, &req.session_id) {
        return Err("stopped");
    }
    let _ = docker::sweep(&req.session_id);
    let t0 = Instant::now();
    let (net, _gw) = docker::network_create(&tag, &req.session_id)?;
    let result = run_pair(req, &tag, &net, t0);
    docker::network_remove(&net);
    result
}

fn run_pair(
    req: &msgs::AttemptRequest,
    tag: &str,
    net: &str,
    t0: Instant,
) -> Result<Value, &'static str> {
    let table = nft::EgressTable::create(tag, &format!("dwcap{tag}"))?;
    match spawn_pair(req, tag, net) {
        Ok((mut cap, mut exp)) => {
            let out = exchange(req, tag, table.lease(), &mut cap, &mut exp);
            let drop_packets = table.drop_counter();
            let reap = reap_pair(&mut cap, &mut exp);
            let r = receipt(req, &out, t0, drop_packets, reap);
            table.destroy();
            r
        }
        Err(code) => {
            table.destroy();
            Err(code)
        }
    }
}

fn spawn_pair(
    req: &msgs::AttemptRequest,
    tag: &str,
    net: &str,
) -> Result<(docker::Spawned, docker::Spawned), &'static str> {
    let exp =
        docker::spawn("exporter", tag, &req.session_id, docker::EXPORTER_UID, "none", &req.image)?;
    match docker::spawn("capture", tag, &req.session_id, docker::CAPTURE_UID, net, &req.image) {
        Ok(cap) => Ok((cap, exp)),
        Err(code) => {
            let mut exp = exp;
            docker::kill(&exp.name);
            let _ = exp.child.wait();
            Err(code)
        }
    }
}

struct Pipes {
    cap_in: ChildStdin,
    cap_out: BufReader<ChildStdout>,
    exp_in: ChildStdin,
    exp_out: BufReader<ChildStdout>,
}

fn take_pipes(cap: &mut docker::Spawned, exp: &mut docker::Spawned) -> Result<Pipes, &'static str> {
    let (ci, co) = (cap.child.stdin.take(), cap.child.stdout.take());
    let (ei, eo) = (exp.child.stdin.take(), exp.child.stdout.take());
    match (ci, co, ei, eo) {
        (Some(a), Some(b), Some(c), Some(d)) => Ok(Pipes {
            cap_in: a,
            cap_out: BufReader::new(b),
            exp_in: c,
            exp_out: BufReader::new(d),
        }),
        _ => Err("pipes_missing"),
    }
}

/// The drive thread owns all four pipe ends and runs the phase sequence:
/// REQUEST to both, READY gate, START, blind raw forwarding, RESULT, ACK.
fn exchange(
    req: &msgs::AttemptRequest,
    tag: &str,
    lease: nft::LeaseRef,
    cap: &mut docker::Spawned,
    exp: &mut docker::Spawned,
) -> Result<msgs::WorkerResult, &'static str> {
    let pipes = take_pipes(cap, exp)?;
    let stop_flag = Arc::new(AtomicBool::new(false));
    let (result_tx, result_rx) = mpsc::channel();
    let (arm_tx, arm_rx) = mpsc::channel();
    let req_owned = owned(req);
    let flag = stop_flag.clone();
    let lease2 = lease.clone();
    let drive = std::thread::spawn(move || {
        drive(pipes, &req_owned, lease2, arm_tx, result_tx, flag)
    });
    let _renew = spawn_renew(req, lease.clone(), arm_rx, &stop_flag);
    let _watch = spawn_watch(req, &stop_flag, tag, lease.clone());
    let _ctl = spawn_ctl_reader(req, &stop_flag, tag, lease);
    let outcome = await_result(req, &result_rx, &stop_flag);
    stop_flag.store(true, Ordering::Relaxed);
    let _ = drive.join();
    outcome
}

fn drive(
    mut p: Pipes,
    req: &msgs::Owned,
    lease: nft::LeaseRef,
    ready: mpsc::Sender<bool>,
    result_tx: mpsc::Sender<msgs::WorkerResult>,
    stop: Arc<AtomicBool>,
) {
    let _ = wire::write_json(&mut p.exp_in, &exp_request(req));
    let _ = wire::write_json(&mut p.cap_in, &cap_request(req));
    let ok = relay_ready(&mut p.cap_out, &mut p.exp_in, req);
    if ok {
        // The lease must admit the endpoint before START: the first egress
        // packet is judged by the live element, not by intent.
        let _ = lease.renew(&req.endpoint_ipv4, req.endpoint_port, req.lease_ms);
    }
    let _ = ready.send(ok);
    if ok {
        let phase = if stop.load(Ordering::Relaxed) {
            "cancel"
        } else {
            "start"
        };
        let ctl = msgs::Control {
            version: msgs::REQUEST_VERSION,
            phase,
            attempt_id: &req.attempt_id,
        };
        let _ = wire::write_json(&mut p.cap_in, &ctl);
    }
    wire::blind_forward(&mut p.cap_out, &mut p.exp_in, &stop);
    read_results(&mut p.exp_out, req, &result_tx);
    let ack = msgs::Control {
        version: msgs::REQUEST_VERSION,
        phase: "ack",
        attempt_id: &req.attempt_id,
    };
    let _ = wire::write_json(&mut p.exp_in, &ack);
}

fn relay_ready(
    cap_out: &mut BufReader<ChildStdout>,
    exp_in: &mut ChildStdin,
    req: &msgs::Owned,
) -> bool {
    let mut buf = Vec::new();
    let ok = wire::read_frame(cap_out, &mut buf)
        .ok()
        .and_then(|_| serde_json::from_slice::<msgs::Phase>(&buf).ok())
        .is_some_and(|ph| {
            ph.version == msgs::REQUEST_VERSION
                && ph.phase == "ready"
                && ph.kind == "capture"
                && ph.attempt_id == req.attempt_id
        });
    if ok {
        let bytes = [
            &(buf.len() as u32).to_be_bytes()[..],
            &buf[..],
        ]
        .concat();
        let _ = exp_in.write_all(&bytes);
    }
    ok
}

fn read_results(
    out: &mut BufReader<ChildStdout>,
    req: &msgs::Owned,
    tx: &mpsc::Sender<msgs::WorkerResult>,
) {
    let mut buf = Vec::new();
    loop {
        match wire::read_frame(out, &mut buf) {
            Err(_) => return,
            Ok(_) => {
                // Route on the phase field only: the RESULT frame carries
                // fields the strict Phase struct rejects.
                let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf) else {
                    return;
                };
                let phase = v.get("phase").and_then(|p| p.as_str());
                let attempt = v.get("attempt_id").and_then(|a| a.as_str());
                let kind = v.get("kind").and_then(|k| k.as_str());
                match phase {
                    Some("ready")
                        if kind == Some("exporter")
                            && attempt == Some(req.attempt_id.as_str()) =>
                    {
                        continue
                    }
                    Some("result") if attempt == Some(req.attempt_id.as_str()) => {
                        if let Ok(res) = serde_json::from_slice::<msgs::WorkerResult>(&buf) {
                            let valid = res.version == msgs::RESULT_VERSION
                                && res.phase == "result"
                                && res.attempt_id == req.attempt_id;
                            if valid {
                                let _ = tx.send(res);
                            }
                        }
                        return;
                    }
                    _ => return,
                }
            }
        }
    }
}

fn await_result(
    req: &msgs::AttemptRequest,
    rx: &mpsc::Receiver<msgs::WorkerResult>,
    stop: &Arc<AtomicBool>,
) -> Outcome {
    loop {
        let latched = || {
            stop.load(Ordering::Relaxed)
                || stop::is_latched(&req.control_root, &req.session_id)
        };
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(r) => return if latched() { Err("stopped") } else { Ok(r) },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if latched() {
                    return Err("stopped");
                }
                if wire::now_ms() >= req.deadline_ms {
                    // Grace: the worker's exchange budget ends at the same
                    // epoch, so an in-flight RESULT (e.g. no_data/timeout)
                    // lands just past the deadline. Give it a bounded window
                    // before declaring the attempt timed out.
                    return match rx.recv_timeout(Duration::from_millis(1500)) {
                        Ok(r) => {
                            if latched() {
                                Err("stopped")
                            } else {
                                Ok(r)
                            }
                        }
                        _ => Err("timeout"),
                    };
                }
            }
            // The stop path kills the pair, so a disconnect is expected there;
            // check the flag before blaming the phase pump.
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return if latched() {
                    Err("stopped")
                } else {
                    Err("phase_failed")
                }
            }
        }
    }
}

fn spawn_watch(
    req: &msgs::AttemptRequest,
    flag: &Arc<AtomicBool>,
    tag: &str,
    lease: nft::LeaseRef,
) -> std::thread::JoinHandle<()> {
    let (root, session, deadline) = (
        req.control_root.clone(),
        req.session_id.clone(),
        req.deadline_ms,
    );
    let (ip, port) = (req.endpoint.ipv4.clone(), req.endpoint.port);
    let (cap_name, exp_name) = (format!("dw-capture-{tag}"), format!("dw-exporter-{tag}"));
    let f = flag.clone();
    std::thread::spawn(move || {
        while !f.load(Ordering::Relaxed) {
            if stop::is_latched(&root, &session) {
                // ADR-016 order: latch STOP, remove egress, then cancel —
                // the lease element leaves before the pair is killed.
                lease.revoke(&ip, port);
                docker::kill(&cap_name);
                docker::kill(&exp_name);
                f.store(true, Ordering::Relaxed);
                return;
            }
            if wire::now_ms() >= deadline {
                // Deadline expiry is a timeout, not a STOP: revoke and kill
                // without flagging, so the receipt stays honest.
                lease.revoke(&ip, port);
                docker::kill(&cap_name);
                docker::kill(&exp_name);
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })
}

/// In-flight STOP arrives on the attempt's exclusive stdin channel as a
/// framed control message (ADR-017). Ordering matches the watchdog: revoke
/// the lease, kill the pair, then raise the flag. A closed channel ends the
/// reader quietly; the file latch remains the between-attempt check.
fn spawn_ctl_reader(
    req: &msgs::AttemptRequest,
    flag: &Arc<AtomicBool>,
    tag: &str,
    lease: nft::LeaseRef,
) -> std::thread::JoinHandle<()> {
    let attempt = req.attempt_id.clone();
    let (ip, port) = (req.endpoint.ipv4.clone(), req.endpoint.port);
    let (cap_name, exp_name) = (format!("dw-capture-{tag}"), format!("dw-exporter-{tag}"));
    let f = flag.clone();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = Vec::new();
        loop {
            if f.load(Ordering::Relaxed) {
                return;
            }
            match wire::read_frame(&mut stdin, &mut buf) {
                Err(_) => return,
                Ok(_) => {
                    let Ok(ctl) = serde_json::from_slice::<msgs::Phase>(&buf) else {
                        continue;
                    };
                    if ctl.version == msgs::REQUEST_VERSION
                        && ctl.phase == "stop"
                        && ctl.attempt_id == attempt
                    {
                        lease.revoke(&ip, port);
                        docker::kill(&cap_name);
                        docker::kill(&exp_name);
                        f.store(true, Ordering::Relaxed);
                        return;
                    }
                }
            }
        }
    })
}

fn spawn_renew(
    req: &msgs::AttemptRequest,
    lease: nft::LeaseRef,
    arm_rx: mpsc::Receiver<bool>,
    flag: &Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    let (ip, port, lease_ms, every) = (
        req.endpoint.ipv4.clone(),
        req.endpoint.port,
        req.lease_ms,
        req.renew_ms,
    );
    let f = flag.clone();
    std::thread::spawn(move || {
        if arm_rx.recv() != Ok(true) {
            return;
        }
        while !f.load(Ordering::Relaxed) {
            if lease.renew(&ip, port, lease_ms).is_err() {
                f.store(true, Ordering::Relaxed);
                return;
            }
            std::thread::sleep(Duration::from_millis(every));
        }
    })
}

fn reap_pair(cap: &mut docker::Spawned, exp: &mut docker::Spawned) -> Value {
    let t = Instant::now();
    docker::kill(&cap.name);
    docker::kill(&exp.name);
    let bound = Instant::now() + Duration::from_millis(REAP_BOUND_MS);
    let mut reaped = false;
    while Instant::now() < bound {
        let a = cap.child.try_wait().ok().flatten().is_some();
        let b = exp.child.try_wait().ok().flatten().is_some();
        if a && b {
            reaped = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    json!({"kill_reap_ms": wire::elapsed(t), "reaped_within_bound": reaped})
}

fn receipt(
    req: &msgs::AttemptRequest,
    outcome: &Outcome,
    t0: Instant,
    drop_packets: Option<u64>,
    reap: Value,
) -> Result<Value, &'static str> {
    let mut base = json!({
        "version": "dw_boundary_receipt_v1",
        "kind": "attempt",
        "session_id": req.session_id,
        "attempt_id": req.attempt_id,
        "elapsed_ms": wire::elapsed(t0),
        "teardown": reap,
        "egress_dropped_packets": drop_packets,
    });
    let obj = base.as_object_mut().ok_or("receipt_failed")?;
    match outcome {
        Ok(r) => {
            obj.insert("disposition".into(), json!(r.disposition));
            obj.insert("rcode".into(), json!(r.rcode));
            obj.insert("truncated".into(), json!(r.truncated));
            obj.insert("answers".into(), serde_json::to_value(&r.answers).unwrap_or(json!([])));
            obj.insert(
                "counters".into(),
                serde_json::to_value(&r.counters).unwrap_or(json!({})),
            );
            obj.insert("worker_elapsed_ms".into(), json!(r.elapsed_ms));
        }
        Err(code) => {
            obj.insert("disposition".into(), json!(code));
        }
    }
    Ok(base)
}

fn owned(req: &msgs::AttemptRequest) -> msgs::Owned {
    msgs::Owned {
        attempt_id: req.attempt_id.clone(),
        session_id: req.session_id.clone(),
        effect: req.effect.clone(),
        qname: req.qname.clone(),
        qtype: req.qtype.clone(),
        endpoint_ipv4: req.endpoint.ipv4.clone(),
        endpoint_port: req.endpoint.port,
        endpoint_transport: req.endpoint.transport.clone(),
        txid: req.txid,
        deadline_ms: req.deadline_ms,
        max_wire: req.max_wire,
        max_raw: req.max_raw,
        lease_ms: req.lease_ms,
    }
}

fn cap_request<'a>(req: &'a msgs::Owned) -> msgs::CaptureRequest<'a> {
    msgs::CaptureRequest {
        version: msgs::REQUEST_VERSION,
        kind: "capture",
        attempt_id: &req.attempt_id,
        session_id: &req.session_id,
        effect: &req.effect,
        qname: &req.qname,
        qtype: &req.qtype,
        endpoint: msgs::EndpointRef {
            ipv4: &req.endpoint_ipv4,
            port: req.endpoint_port,
            transport: &req.endpoint_transport,
        },
        txid: req.txid,
        deadline_ms: req.deadline_ms,
        max_wire: req.max_wire,
    }
}

fn exp_request<'a>(req: &'a msgs::Owned) -> msgs::ExporterRequest<'a> {
    msgs::ExporterRequest {
        version: msgs::REQUEST_VERSION,
        kind: "exporter",
        attempt_id: &req.attempt_id,
        session_id: &req.session_id,
        effect: &req.effect,
        qname: &req.qname,
        qtype: &req.qtype,
        txid: req.txid,
        deadline_ms: req.deadline_ms,
        max_raw: req.max_raw,
    }
}

fn validate(req: &msgs::AttemptRequest) -> Result<(), &'static str> {
    let checks: [(&'static str, bool); 10] = [
        ("version", req.version == msgs::ATTEMPT_VERSION),
        ("session_id", id_ok(&req.session_id)),
        ("attempt_id", id_ok(&req.attempt_id)),
        ("effect", req.effect == "dns_udp"),
        ("qtype", req.qtype == "A"),
        ("transport", req.endpoint.transport == "udp"),
        ("endpoint_ipv4", req.endpoint.ipv4.parse::<std::net::Ipv4Addr>().is_ok()),
        ("qname", qname_ok(&req.qname)),
        ("bounds", bounds_ok(req)),
        ("image_pin", image_pinned(&req.image)),
    ];
    match checks.iter().find(|(_, ok)| !ok) {
        Some((name, _)) => Err(Box::leak(format!("request_refused_{name}").into_boxed_str())),
        None => Ok(()),
    }
}

fn tag_of(req: &msgs::AttemptRequest) -> String {
    let t: String = req
        .attempt_id
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(10)
        .collect();
    if t.len() >= 8 {
        return t;
    }
    let mut fallback = format!("a{:09}", wire::now_ms() % 1_000_000_000);
    fallback.truncate(10);
    fallback
}

fn id_ok(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn qname_ok(s: &str) -> bool {
    (1..=253).contains(&s.len())
        && s.contains('.')
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

fn bounds_ok(req: &msgs::AttemptRequest) -> bool {
    req.max_wire > 0
        && req.max_wire <= 512
        && req.max_raw > 0
        && req.max_raw <= 4096
        && req.lease_ms <= MAX_LEASE_MS
        && req.renew_ms <= MAX_RENEW_MS
        && req.renew_ms < req.lease_ms
        && req.deadline_ms > wire::now_ms()
        && req.deadline_ms - wire::now_ms() <= MAX_DEADLINE_MS
        && std::path::Path::new(&req.control_root).is_absolute()
}

fn image_pinned(image: &str) -> bool {
    // A digest-pinned reference (name@sha256:) or a bare local image id.
    (image.contains("@sha256:") || image.starts_with("sha256:")) && image.len() < 200
}

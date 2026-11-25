use super::*;
use crate::attempt::tests::TestRootGuard;
use crate::custody::tests::install_fixture_map;
use crate::network::tests::fixture::Network;
use crate::qualification::now_ms;
use duskweave::m1_https::ipc::{HttpsOutcome, HttpsPhase, HttpsPhaseReady, HttpsPhaseStart};
use duskweave::m1_https::result::LAB_SCOPE_LIMITATION;
use duskweave::m1_https::{RedirectDisposition, VERSION_HTTPS_IPC};
use std::cell::RefCell;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum HttpsFault {
    None,
    ReadyConnect,
    ReadyTls,
    BadKind,
    ExtraAfterResult,
    MissingResult,
    WrongStart,
}

struct Resources {
    image: String,
    attempt: Uuid,
    sockets: Option<(UnixStream, UnixStream)>,
}

thread_local! {
    static RESOURCES: RefCell<Option<Resources>> = const { RefCell::new(None) };
}

pub(crate) struct ResourceScope {
    previous: Option<Resources>,
    workers: Vec<std::thread::JoinHandle<Res<()>>>,
}

impl Drop for ResourceScope {
    fn drop(&mut self) {
        let current = RESOURCES.with(|slot| slot.replace(self.previous.take()));
        drop(current);
        let mut errors = Vec::new();
        for worker in self.workers.drain(..) {
            match worker.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => errors.push(format!("{error:?}")),
                Err(_) => errors.push("peer panicked".into()),
            }
        }
        if std::thread::panicking() {
            eprintln!("https protocol peer cleanup: {errors:?}");
        } else {
            assert!(errors.is_empty(), "owned https peers failed: {errors:?}");
        }
    }
}

pub(crate) fn resources(
    image: &str,
    q: &HttpsIpcRequest,
    end: Instant,
) -> Option<Res<(Attach, Attach)>> {
    RESOURCES.with(|slot| {
        slot.borrow_mut().as_mut().map(|resource| {
            assert_eq!(image, resource.image);
            assert_eq!(q.attempt, resource.attempt);
            let (capture, exporter) = resource.sockets.take().expect("one resource acquisition");
            Ok((
                Attach::connect_on(capture, "capture", end)?,
                Attach::connect_on(exporter, "exporter", end)?,
            ))
        })
    })
}

fn multiplex(socket: &mut UnixStream, value: &impl Serialize, limit: usize) -> Res<()> {
    let bytes = encode_frame(value, limit)?;
    socket
        .write_all(&[1, 0, 0, 0])
        .and_then(|_| socket.write_all(&(bytes.len() as u32).to_be_bytes()))
        .and_then(|_| socket.write_all(&bytes))
        .map_err(|_| Fail::State("https_ipc_failure"))
}

fn upgraded(socket: &mut UnixStream) -> Res<()> {
    let mut req = Vec::new();
    while !req.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        socket
            .read_exact(&mut byte)
            .map_err(|_| Fail::State("https_ipc_failure"))?;
        req.push(byte[0]);
        assert!(req.len() < 1024);
    }
    socket
        .write_all(b"HTTP/1.1 101 Switching Protocols\r\n\r\n")
        .map_err(|_| Fail::State("https_ipc_failure"))
}

fn read_packet(socket: &mut UnixStream) -> Res<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    socket
        .read_exact(&mut len_buf)
        .map_err(|_| Fail::State("https_ipc_failure"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    socket
        .read_exact(&mut payload)
        .map_err(|_| Fail::State("https_ipc_failure"))?;
    Ok(payload)
}

fn read_start(
    socket: &mut UnixStream,
    expected: HttpsPhase,
    fault: HttpsFault,
) -> Res<Option<i64>> {
    let raw = match read_packet(socket) {
        Ok(raw) => raw,
        Err(_) if fault != HttpsFault::None => return Ok(None),
        Err(e) => return Err(e),
    };
    let start: HttpsPhaseStart = decode_payload(&raw, HTTPS_IPC_MAX_CONTROL)?;
    assert_eq!(start.phase, expected);
    Ok(Some(start.deadline_at_ms))
}

pub(crate) fn safe_result(q: &HttpsIpcRequest, observed_at_ms: i64) -> HttpsSafeResult {
    HttpsSafeResult {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "RESULT".into(),
        attempt: q.attempt,
        session_generation: q.session_generation,
        sequence: 7,
        outcome: HttpsOutcome::HeadResponse,
        observed_at_ms,
        network_bytes: 128,
        header_bytes: 64,
        status_code: 200,
        tls_category: "none".into(),
        redirect: RedirectDisposition::None,
        limitations: vec![
            LAB_SCOPE_LIMITATION.into(),
            "head_response_does_not_prove_access_or_objective_completion".into(),
        ],
    }
}

fn worker_handshake(
    socket: &mut UnixStream,
    q: &HttpsIpcRequest,
    exporter: bool,
    fault: HttpsFault,
) -> Res<bool> {
    upgraded(socket)?;
    let req: HttpsIpcRequest = decode_payload(&read_packet(socket)?, HTTPS_IPC_MAX_FRAME)?;
    assert_eq!(req.attempt, q.attempt);
    let mut ready = HttpsPhaseReady::new(q, HttpsPhase::Connect);
    if fault == HttpsFault::ReadyConnect && exporter {
        ready.session_generation += 1;
    }
    multiplex(socket, &ready, HTTPS_IPC_MAX_CONTROL)?;
    Ok(read_start(socket, HttpsPhase::Connect, fault)?.is_some())
}

fn capture_phases(
    socket: &mut UnixStream,
    q: &HttpsIpcRequest,
    fault: HttpsFault,
) -> Res<Option<Vec<u8>>> {
    let second = if fault == HttpsFault::ReadyTls {
        HttpsPhase::Head
    } else {
        HttpsPhase::Tls
    };
    multiplex(
        socket,
        &HttpsPhaseReady::new(q, second),
        HTTPS_IPC_MAX_CONTROL,
    )?;
    if fault == HttpsFault::ReadyTls {
        let _ = read_start(socket, HttpsPhase::Tls, fault);
        return Ok(None);
    }
    if read_start(socket, HttpsPhase::Tls, fault)?.is_none() {
        return Ok(None);
    }
    multiplex(
        socket,
        &HttpsPhaseReady::new(q, HttpsPhase::Head),
        HTTPS_IPC_MAX_CONTROL,
    )?;
    if read_start(socket, HttpsPhase::Head, fault)?.is_none() {
        return Ok(None);
    }
    let kind = if fault == HttpsFault::BadKind {
        "BOGUS"
    } else {
        "RESULT"
    };
    let marker = serde_json::json!({"schema": VERSION_HTTPS_IPC, "kind": kind, "opaque": true});
    let raw = serde_json::to_vec(&marker).expect("raw json");
    multiplex(socket, &marker, HTTPS_IPC_MAX_FRAME)?;
    if fault == HttpsFault::ExtraAfterResult {
        socket
            .write_all(&[1, 0, 0, 0, 0, 0, 0, 1, b'x'])
            .map_err(|_| Fail::State("https_ipc_failure"))?;
    }
    let _ = socket.shutdown(Shutdown::Write);
    Ok(Some(raw))
}

fn exporter_export(
    socket: &mut UnixStream,
    q: &HttpsIpcRequest,
    raw: Vec<u8>,
    fault: HttpsFault,
    gate: Option<Arc<AtomicBool>>,
) -> Res<()> {
    for phase in [HttpsPhase::Tls, HttpsPhase::Head] {
        if read_start(socket, phase, fault)?.is_none() {
            return Ok(());
        }
    }
    let forwarded = match read_packet(socket) {
        Ok(pkt) => pkt,
        Err(_) if fault != HttpsFault::None => return Ok(()),
        Err(e) => return Err(e),
    };
    assert_eq!(forwarded, raw);
    if fault == HttpsFault::MissingResult {
        return Ok(());
    }
    if let Some(gate) = gate {
        let end = Instant::now() + Duration::from_secs(2);
        while !gate.load(Ordering::SeqCst) {
            if Instant::now() >= end {
                return Err(Fail::State("fixture_renewal_not_observed"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let obs = now_ms()?.min(q.deadline_at_ms);
    multiplex(socket, &safe_result(q, obs), HTTPS_IPC_MAX_FRAME)?;
    let ack: HttpsAck = decode_payload(&read_packet(socket)?, HTTPS_IPC_MAX_CONTROL)?;
    assert_eq!((ack.attempt, ack.sequence), (q.attempt, 8));
    Ok(())
}

fn worker(
    mut socket: UnixStream,
    q: HttpsIpcRequest,
    raw: Vec<u8>,
    exporter: bool,
    fault: HttpsFault,
    gate: Option<Arc<AtomicBool>>,
) -> Res<()> {
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    if !worker_handshake(&mut socket, &q, exporter, fault)? {
        return Ok(());
    }
    if !exporter {
        let _ = capture_phases(&mut socket, &q, fault)?;
        return Ok(());
    }
    exporter_export(&mut socket, &q, raw, fault, gate)
}

pub(crate) fn resource_scope(
    image: &str,
    q: &HttpsIpcRequest,
    fault: HttpsFault,
    gate: Option<Arc<AtomicBool>>,
) -> ResourceScope {
    let raw = serde_json::to_vec(&serde_json::json!({
        "schema": VERSION_HTTPS_IPC, "kind": "RESULT", "opaque": true
    }))
    .expect("raw");
    let mut sockets = Vec::new();
    let mut workers = Vec::new();
    for exporter in [false, true] {
        let (host, peer) = UnixStream::pair().unwrap();
        let req = q.clone();
        let raw_copy = raw.clone();
        let gate_copy = gate.clone();
        workers.push(std::thread::spawn(move || {
            worker(peer, req, raw_copy, exporter, fault, gate_copy)
        }));
        sockets.push(host);
    }
    let exporter = sockets.pop().unwrap();
    let capture = sockets.pop().unwrap();
    let previous = RESOURCES.with(|slot| {
        slot.replace(Some(Resources {
            image: image.into(),
            attempt: q.attempt,
            sockets: Some((capture, exporter)),
        }))
    });
    ResourceScope { previous, workers }
}

fn new_session(q: &HttpsIpcRequest) -> HttpsSession {
    HttpsSession {
        preparation_deadline: Some(Instant::now() + Duration::from_secs(3)),
        run_id: q.run_id,
        request: Box::new(q.clone()),
        granted_phase: None,
        opened_at_ms: q.authority_evaluated_at_ms,
        ready_at_ms: None,
        attempt_deadline_at_ms: None,
        handling_deadline_at_ms: q.deadline_at_ms,
    }
}

#[test]
fn pair_prepare_drive_phases_ack_and_dispose_succeeds() {
    let _root = TestRootGuard::new();
    let _net = Network::new();
    let _custody = install_fixture_map();
    let now = now_ms().unwrap();
    let q = crate::attempt::https::tests::sample_request(Uuid::new_v4(), now);
    let image = format!("sha256:{}", "a".repeat(64));
    let gate = Arc::new(AtomicBool::new(false));
    let _scope = resource_scope(&image, &q, HttpsFault::None, Some(gate.clone()));

    let mut pair = HttpsPair::prepare(&image, &new_session(&q)).expect("prepare");
    pair.start_connect(q.deadline_at_ms).expect("start_connect");

    let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel(1);
    let (evt_tx, evt_rx) = std::sync::mpsc::sync_channel(4);
    let deadline = q.deadline_at_ms;
    std::thread::scope(|s| {
        let tx = evt_tx.clone();
        let p = &mut pair;
        s.spawn(move || {
            let res = p.drive(deadline, &tx, &cmd_rx, &|_| {});
            let _ = tx.send(PairEvt::Done(res));
        });
        assert!(matches!(
            evt_rx.recv().unwrap(),
            PairEvt::Ready(HttpsPhase::Tls)
        ));
        cmd_tx
            .send(PairCmd::StartPhase(HttpsPhase::Tls, deadline))
            .unwrap();
        assert!(matches!(
            evt_rx.recv().unwrap(),
            PairEvt::Ready(HttpsPhase::Head)
        ));
        cmd_tx
            .send(PairCmd::StartPhase(HttpsPhase::Head, deadline))
            .unwrap();
        gate.store(true, Ordering::SeqCst);
        let done = match evt_rx.recv().unwrap() {
            PairEvt::Done(res) => res.expect("result"),
            _ => panic!("expected Done"),
        };
        assert_eq!(done.status_code, 200);
    });
    pair.acknowledge().expect("ack");
    let _ = pair.dispose();
}

#[test]
fn pair_rejects_protocol_faults_and_enforces_frame_limit() {
    let now = now_ms().unwrap();
    let image = format!("sha256:{}", "b".repeat(64));
    for fault in [
        HttpsFault::ReadyConnect,
        HttpsFault::ReadyTls,
        HttpsFault::BadKind,
        HttpsFault::ExtraAfterResult,
        HttpsFault::MissingResult,
    ] {
        let q = crate::attempt::https::tests::sample_request(Uuid::new_v4(), now);
        let _scope = resource_scope(&image, &q, fault, None);
        let prepared = HttpsPair::prepare(&image, &new_session(&q));
        if fault == HttpsFault::ReadyConnect {
            assert!(prepared.is_err());
            continue;
        }
        let mut pair = prepared.expect("prepare");
        pair.start_connect(q.deadline_at_ms).expect("start");
        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel(1);
        let (evt_tx, evt_rx) = std::sync::mpsc::sync_channel(4);
        let dl = q.deadline_at_ms;
        std::thread::scope(|s| {
            let tx = evt_tx.clone();
            s.spawn(move || {
                let r = pair.drive(dl, &tx, &cmd_rx, &|_| {});
                let _ = tx.send(PairEvt::Done(r));
            });
            while let Ok(evt) = evt_rx.recv() {
                match evt {
                    PairEvt::Ready(p) => {
                        let _ = cmd_tx.send(PairCmd::StartPhase(p, dl));
                    }
                    PairEvt::Done(res) => {
                        assert!(res.is_err(), "{fault:?} must fail");
                        break;
                    }
                }
            }
        });
    }
    let mut frames = 0;
    for _ in 0..HTTPS_IPC_MAX_FRAMES {
        charge_frame(&mut frames).expect("within ceiling");
    }
    assert_eq!(
        charge_frame(&mut frames).unwrap_err(),
        Fail::State("https_ipc_failure")
    );
    assert_eq!(
        frame_kind(b"not-json").unwrap_err(),
        Fail::State("https_ipc_failure")
    );
}

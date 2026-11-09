//! Real Attach/FrameReader protocol over private sockets; no Docker daemon or DNS.
use super::*;
use std::cell::RefCell;
use std::os::unix::net::UnixStream;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Fault {
    None,
    Ready,
    Result,
    Ack,
    WrongStart,
}
struct Resources {
    image: String,
    attempt: uuid::Uuid,
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
            eprintln!("protocol peer cleanup: {errors:?}");
        } else {
            assert!(errors.is_empty(), "owned protocol peers failed: {errors:?}");
        }
    }
}
pub(crate) fn resources(
    image: &str,
    q: &DnsRequest,
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
fn multiplex(socket: &mut UnixStream, value: &impl serde::Serialize, limit: usize) -> Res<()> {
    let mut bytes = Vec::new();
    FrameWriter::new(&mut bytes).write(value, limit)?;
    opaque(socket, &bytes)
}
fn opaque(socket: &mut UnixStream, bytes: &[u8]) -> Res<()> {
    socket
        .write_all(&[1, 0, 0, 0])
        .and_then(|_| socket.write_all(&(bytes.len() as u32).to_be_bytes()))
        .and_then(|_| socket.write_all(bytes))
        .map_err(|_| Fail::State("dns_ipc_failure"))
}
fn upgraded(socket: &mut UnixStream) -> Res<()> {
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        socket
            .read_exact(&mut byte)
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        request.push(byte[0]);
        assert!(request.len() < 1024);
    }
    socket
        .write_all(b"HTTP/1.1 101 Switching Protocols\r\n\r\n")
        .map_err(|_| Fail::State("dns_ipc_failure"))
}
pub(crate) fn result(q: &DnsRequest) -> DnsResult {
    serde_json::from_value(serde_json::json!({
        "schema":"m1_dns_ipc_v1", "kind":"RESULT", "attempt":q.attempt,
        "session_generation":q.session_generation,"sequence":3,"status":"completed",
        "observed_at_ms":super::super::qualification::now_ms().unwrap(),
        "rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,
        "answers":[{"ipv4":"198.51.100.7","ttl_seconds":30}],"aliases":[]
    }))
    .unwrap()
}
fn worker(
    mut socket: UnixStream,
    q: DnsRequest,
    exporter: bool,
    fault: Fault,
    gate: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Res<()> {
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    upgraded(&mut socket)?;
    let received = FrameReader::new(&mut socket).request()?;
    assert_eq!(received.attempt, q.attempt);
    assert_eq!(received.allowed_names, q.allowed_names);
    let mut ready = Control::new(&q, "READY", 1);
    if fault == Fault::Ready && exporter {
        ready.session_generation += 1;
    }
    multiplex(&mut socket, &ready, MAX_CONTROL)?;
    let start = match FrameReader::new(&mut socket).control() {
        Ok(start) => start,
        Err(_) if matches!(fault, Fault::Ready | Fault::WrongStart) => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut running = q.clone();
    running.deadline_at_ms = start.deadline_at_ms;
    start.validate(&running, "START", 2)?;
    assert!(start.deadline_at_ms > 0 && start.deadline_at_ms <= q.deadline_at_ms);
    let raw = [7u32.to_be_bytes().as_slice(), b"opaque!"].concat();
    if !exporter {
        opaque(&mut socket, &raw)?;
        socket.shutdown(std::net::Shutdown::Write).unwrap();
        return Ok(());
    }
    let mut received = vec![0; raw.len()];
    socket
        .read_exact(&mut received)
        .map_err(|_| Fail::State("dns_ipc_failure"))?;
    assert_eq!(
        received, raw,
        "opaque complete capture frame reaches only exporter"
    );
    if fault == Fault::Result {
        return Ok(());
    }
    if let Some(gate) = gate {
        let end = Instant::now() + Duration::from_secs(2);
        while !gate.load(std::sync::atomic::Ordering::SeqCst) {
            if Instant::now() >= end {
                return Err(Fail::State("fixture_renewal_not_observed"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    multiplex(&mut socket, &result(&q), MAX_FRAME)?;
    match FrameReader::new(&mut socket).control() {
        Ok(ack) => ack.validate(&q, "ACK", 4)?,
        Err(_) if fault == Fault::Ack => {}
        Err(error) => return Err(error),
    }
    Ok(())
}
pub(crate) fn resource_scope(
    image: &str,
    q: &DnsRequest,
    fault: Fault,
    gate: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> ResourceScope {
    let mut sockets = Vec::new();
    let mut workers = Vec::new();
    for exporter in [false, true] {
        let (host, peer) = UnixStream::pair().unwrap();
        let request = q.clone();
        let gate = gate.clone();
        workers.push(std::thread::spawn(move || {
            worker(peer, request, exporter, fault, gate)
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

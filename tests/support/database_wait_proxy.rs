//! Test-only bounded PostgreSQL wire relay for the wait-bound fault cases.
//! Binds an ephemeral loopback listener, forwards the length-prefixed
//! startup packet and authentication exchange verbatim between the real CLI
//! and the owned local test cluster, then forwards type/length-framed
//! protocol messages in both directions. Once a configured safe SQL shape
//! is observed inside a client Q or P message, that session's backend
//! replies are withheld from the caller while both sockets stay open.
//! Frame sizes are bounded; credentials, startup/auth payloads, backend
//! keys and raw results move through transient buffers only and are never
//! recorded. Included as a module by the real test crates; not a
//! standalone target.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const MAX_PACKET: usize = 1 << 20;
const POLL: Duration = Duration::from_millis(100);

fn blocked(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

/// Bounded exact read that re-checks shutdown on every poll timeout; None
/// means close, error or shutdown, never partial data.
fn fill(stream: &TcpStream, buf: &mut [u8], stop: &AtomicBool) -> Option<()> {
    let mut src = stream;
    let mut got = 0;
    while got < buf.len() {
        match src.read(&mut buf[got..]) {
            Ok(0) => return None,
            Ok(n) => got += n,
            Err(e) if blocked(&e) && !stop.load(Ordering::Relaxed) => continue,
            Err(_) => return None,
        }
    }
    Some(())
}

fn forward(dst: &TcpStream, head: &[u8], payload: &[u8]) -> Option<()> {
    let mut dst = dst;
    dst.write_all(head).ok()?;
    dst.write_all(payload).ok()?;
    dst.flush().ok()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Owns the relay listener, session threads and sockets for one fixture
/// lifetime; Drop stops every session and joins its threads.
pub struct Relay {
    port: u16,
    observed: Receiver<()>,
    consumed: Receiver<()>,
    stop: Arc<AtomicBool>,
    sessions: Arc<Mutex<Vec<JoinHandle<()>>>>,
    accept: Option<JoinHandle<()>>,
}

impl Relay {
    /// `needle` is a safe ASCII SQL shape observed in client Q/P payloads.
    pub fn start(target: std::net::SocketAddr, needle: &'static [u8]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (observed_tx, observed_rx) = channel();
        let (consumed_tx, consumed_rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let sessions: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
        let accept = {
            let stop = stop.clone();
            let sessions = sessions.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((client, _)) => {
                            let handle = std::thread::spawn({
                                let stop = stop.clone();
                                let observed = observed_tx.clone();
                                let consumed = consumed_tx.clone();
                                move || session(client, target, needle, observed, consumed, stop)
                            });
                            sessions.lock().unwrap().push(handle);
                        }
                        Err(_) => std::thread::sleep(POLL),
                    }
                }
            })
        };
        Self {
            port,
            observed: observed_rx,
            consumed: consumed_rx,
            stop,
            sessions,
            accept: Some(accept),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Barrier: true once a session observed and forwarded the needle frame.
    pub fn observed(&self, within: Duration) -> bool {
        self.observed.recv_timeout(within).is_ok()
    }

    /// Barrier: true once a session received a backend reply it withheld.
    /// Because the relay arms before forwarding the needle frame, this is
    /// server confirmation that the needle query (for example COMMIT)
    /// completed — the honest durable-state point, not merely client send.
    pub fn consumed(&self, within: Duration) -> bool {
        self.consumed.recv_timeout(within).is_ok()
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
        for session in self.sessions.lock().unwrap().drain(..) {
            let _ = session.join();
        }
    }
}

/// One session: forward the startup packet verbatim (SSLRequest/GSSENC
/// negotiation packets are length-prefixed too), then run framed
/// bidirectional forwarding until close or relay shutdown.
fn session(
    client: TcpStream,
    target: std::net::SocketAddr,
    needle: &'static [u8],
    observed: Sender<()>,
    consumed: Sender<()>,
    stop: Arc<AtomicBool>,
) {
    let server = match TcpStream::connect(target) {
        Ok(server) => server,
        Err(_) => return,
    };
    for stream in [&client, &server] {
        let _ = stream.set_read_timeout(Some(POLL));
        let _ = stream.set_write_timeout(Some(POLL));
    }
    loop {
        let mut len = [0u8; 4];
        if fill(&client, &mut len, &stop).is_none() {
            return;
        }
        let size = i32::from_be_bytes(len) as usize;
        if !(8..=MAX_PACKET).contains(&size) {
            return;
        }
        let mut packet = vec![0u8; size - 4];
        if fill(&client, &mut packet, &stop).is_none() {
            return;
        }
        if forward(&server, &len, &packet).is_none() {
            return;
        }
        let code = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);
        if !matches!(code, 80877103 | 80877104) {
            break;
        }
    }
    let withheld = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    threads.push({
        let (client, server, withheld, stop) = (
            client.try_clone().unwrap(),
            server.try_clone().unwrap(),
            withheld.clone(),
            stop.clone(),
        );
        std::thread::spawn(move || c2s(client, server, needle, observed, withheld, stop))
    });
    threads.push({
        let stop = stop.clone();
        std::thread::spawn(move || s2c(server, client, withheld, consumed, stop))
    });
    for thread in threads {
        let _ = thread.join();
    }
}

/// Client to server: forward every framed message; a Q/P payload matching
/// the needle arms backend-response withholding before it is forwarded.
fn c2s(
    client: TcpStream,
    server: TcpStream,
    needle: &'static [u8],
    observed: Sender<()>,
    withheld: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) {
    loop {
        let mut head = [0u8; 5];
        if fill(&client, &mut head, &stop).is_none() {
            break;
        }
        let size = i32::from_be_bytes(head[1..5].try_into().unwrap()) as usize;
        if !(4..=MAX_PACKET).contains(&size) {
            break;
        }
        let mut payload = vec![0u8; size - 4];
        if fill(&client, &mut payload, &stop).is_none() {
            break;
        }
        if matches!(head[0], b'Q' | b'P') && contains(&payload, needle) {
            withheld.store(true, Ordering::Relaxed);
            let _ = observed.send(());
        }
        if forward(&server, &head, &payload).is_none() {
            break;
        }
    }
    let _ = server.shutdown(Shutdown::Both);
}

/// Server to client: forward backend frames until the session's needle
/// fired, then discard replies while the sockets stay open. The first
/// discarded frame is also the fixture's server-completion signal.
fn s2c(
    server: TcpStream,
    client: TcpStream,
    withheld: Arc<AtomicBool>,
    consumed: Sender<()>,
    stop: Arc<AtomicBool>,
) {
    loop {
        let mut head = [0u8; 5];
        if fill(&server, &mut head, &stop).is_none() {
            break;
        }
        let size = i32::from_be_bytes(head[1..5].try_into().unwrap()) as usize;
        if !(4..=MAX_PACKET).contains(&size) {
            break;
        }
        let mut payload = vec![0u8; size - 4];
        if fill(&server, &mut payload, &stop).is_none() {
            break;
        }
        if withheld.load(Ordering::Relaxed) {
            let _ = consumed.send(());
            continue;
        }
        if forward(&client, &head, &payload).is_none() {
            break;
        }
    }
    let _ = client.shutdown(Shutdown::Both);
}

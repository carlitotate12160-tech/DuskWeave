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

    /// Barrier: true once a withheld backend `CommandComplete` carried the
    /// needle as its exact completion tag — the server's report that the
    /// needle command itself succeeded (for COMMIT, a durable commit under
    /// `synchronous_commit=on`). Merely receiving any frame is not proof:
    /// an `ErrorResponse` or another tag such as `ROLLBACK` (an aborted
    /// transaction's COMMIT result) never signals.
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
    if configure_streams(&client, &server).is_err() {
        return;
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
        std::thread::spawn(move || s2c(server, client, withheld, needle, consumed, stop))
    });
    for thread in threads {
        let _ = thread.join();
    }
}

/// Configure both directions before forwarding or cloning either socket.
fn configure_streams(client: &TcpStream, server: &TcpStream) -> std::io::Result<()> {
    for stream in [client, server] {
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(POLL))?;
        stream.set_write_timeout(Some(POLL))?;
    }
    Ok(())
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
/// fired, then discard replies while the sockets stay open. A discarded
/// `CommandComplete` whose NUL-terminated tag equals the needle is the
/// fixture's server-completion signal; every other frame is only
/// discarded, never reported as completion.
fn s2c(
    server: TcpStream,
    client: TcpStream,
    withheld: Arc<AtomicBool>,
    needle: &'static [u8],
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
            if head[0] == b'C'
                && payload.len() == needle.len() + 1
                && payload[..needle.len()] == *needle
                && payload[needle.len()] == 0
            {
                let _ = consumed.send(());
            }
            continue;
        }
        if forward(&client, &head, &payload).is_none() {
            break;
        }
    }
    let _ = client.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let server = listener.accept().unwrap().0;
        for stream in [&client, &server] {
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
        }
        (client, server)
    }

    #[test]
    fn session_configuration_disables_nagle_on_both_streams() {
        let (client, _client_peer) = pair();
        let (server, _server_peer) = pair();
        client.set_nodelay(false).unwrap();
        server.set_nodelay(false).unwrap();
        configure_streams(&client, &server).unwrap();
        assert!(
            client.nodelay().unwrap(),
            "accepted client must use TCP_NODELAY"
        );
        assert!(
            server.nodelay().unwrap(),
            "upstream server must use TCP_NODELAY"
        );
        assert_eq!(client.read_timeout().unwrap(), Some(POLL));
        assert_eq!(server.write_timeout().unwrap(), Some(POLL));
    }

    fn frame(tag: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![tag];
        bytes.extend_from_slice(&((payload.len() + 4) as u32).to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn read_exact(stream: &mut TcpStream, expected: &[u8]) {
        let mut actual = vec![0; expected.len()];
        stream.read_exact(&mut actual).unwrap();
        assert_eq!(actual, expected);
    }

    fn fragmented(stream: &mut TcpStream, bytes: &[u8]) {
        for byte in bytes {
            stream.write_all(&[*byte]).unwrap();
        }
    }

    #[test]
    fn fragmented_startup_and_nonmatching_frames_round_trip_verbatim() {
        let backend = TcpListener::bind("127.0.0.1:0").unwrap();
        let relay = Relay::start(backend.local_addr().unwrap(), b"COMMIT");
        let mut client = TcpStream::connect(("127.0.0.1", relay.port())).unwrap();
        client.set_nodelay(true).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let startup = [0, 0, 0, 8, 0, 3, 0, 0];
        let query = frame(b'Q', b"SELECT 1\0");
        let reply = frame(b'C', b"SELECT 1\0");
        let worker = std::thread::spawn(move || {
            let mut server = backend.accept().unwrap().0;
            server.set_nodelay(true).unwrap();
            server
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            read_exact(&mut server, &startup);
            for _ in 0..40 {
                read_exact(&mut server, &query);
                fragmented(&mut server, &reply);
            }
        });
        let started = Instant::now();
        fragmented(&mut client, &startup);
        for _ in 0..40 {
            fragmented(&mut client, &frame(b'Q', b"SELECT 1\0"));
            read_exact(&mut client, &frame(b'C', b"SELECT 1\0"));
        }
        println!(
            "relay synthetic round trips=40 wire_bytes={} elapsed_ms={}",
            startup.len()
                + 40 * (frame(b'Q', b"SELECT 1\0").len() + frame(b'C', b"SELECT 1\0").len()),
            started.elapsed().as_millis()
        );
        assert!(!relay.observed(Duration::ZERO));
        assert!(!relay.consumed(Duration::ZERO));
        worker.join().unwrap();
    }

    #[test]
    fn matching_query_and_parse_park_replies_and_require_exact_completion() {
        for tag in [b'Q', b'P'] {
            let backend = TcpListener::bind("127.0.0.1:0").unwrap();
            let relay = Relay::start(backend.local_addr().unwrap(), b"COMMIT");
            let mut client = TcpStream::connect(("127.0.0.1", relay.port())).unwrap();
            client.set_nodelay(true).unwrap();
            client.set_read_timeout(Some(POLL)).unwrap();
            let mut server = backend.accept().unwrap().0;
            server
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let startup = [0, 0, 0, 8, 0, 3, 0, 0];
            fragmented(&mut client, &startup);
            read_exact(&mut server, &startup);
            // The needle in a non-Q/P message must neither arm nor park replies.
            let other = frame(b'd', b"COMMIT\0");
            fragmented(&mut client, &other);
            read_exact(&mut server, &other);
            fragmented(&mut server, &frame(b'C', b"COMMIT\0"));
            read_exact(&mut client, &frame(b'C', b"COMMIT\0"));
            assert!(!relay.observed(Duration::ZERO));
            assert!(!relay.consumed(Duration::ZERO));

            let payload = if tag == b'P' {
                &b"\0COMMIT\0\0\0"[..]
            } else {
                &b"COMMIT\0"[..]
            };
            let matched = frame(tag, payload);
            fragmented(&mut client, &matched);
            read_exact(&mut server, &matched);
            assert!(relay.observed(Duration::from_secs(2)));
            // Error, rollback, prefix and non-NUL tags cannot prove completion.
            for reply in [
                frame(b'E', b"COMMIT\0"),
                frame(b'C', b"ROLLBACK\0"),
                frame(b'C', b"COMMIT extra\0"),
                frame(b'C', b"COMMIT"),
            ] {
                fragmented(&mut server, &reply);
            }
            assert!(!relay.consumed(POLL));
            let mut byte = [0];
            assert!(blocked(&client.read(&mut byte).unwrap_err()));
            fragmented(&mut server, &frame(b'C', b"COMMIT\0"));
            assert!(relay.consumed(Duration::from_secs(2)));
            // Both sockets stay open and parked after the completion barrier.
            assert!(blocked(&client.read(&mut byte).unwrap_err()));
        }
    }
}

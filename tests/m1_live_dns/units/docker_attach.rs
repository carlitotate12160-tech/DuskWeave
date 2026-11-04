use super::*;
use duskweave::m1_dns::{Control, DnsRequest, FrameReader, FrameWriter, MAX_CONTROL};
use uuid::Uuid;

fn attach_io(io: UnixStream) -> Attach {
    Attach {
        socket: TimedSocket {
            io,
            end: Instant::now() + Duration::from_secs(5),
        },
        remaining: 0,
        stream: 0,
        chunks: 0,
        total: 0,
        stderr: 0,
    }
}

#[test]
fn multiplex_headers_are_parsed_exactly() {
    use std::io::Write;
    let (a, mut b) = UnixStream::pair().unwrap();
    // stdout frame: stream id 1, 4-byte big-endian payload length.
    b.write_all(&[1, 0, 0, 0, 0, 0, 0, 3]).unwrap();
    let mut attach = attach_io(a);
    assert!(attach.header().unwrap(), "stdout frame parsed");
    assert_eq!(attach.remaining, 3);
    assert_eq!(attach.stream, 1);
    // A truncated stream refuses the protocol instead of guessing a frame.
    let (a, mut b) = UnixStream::pair().unwrap();
    b.write_all(&[1, 0, 0]).unwrap();
    drop(b);
    let mut attach = attach_io(a);
    assert!(attach.header().is_err());
}

#[test]
fn multiplex_stdout_reading_and_stderr_discard() {
    use std::io::{Read, Write};
    let (a, mut b) = UnixStream::pair().unwrap();

    // 1. Write stderr frame (stream 2, len 4: "err!")
    b.write_all(&[2, 0, 0, 0, 0, 0, 0, 4]).unwrap();
    b.write_all(b"err!").unwrap();

    // 2. Write stdout frame (stream 1, len 5: "hello")
    b.write_all(&[1, 0, 0, 0, 0, 0, 0, 5]).unwrap();
    b.write_all(b"hello").unwrap();

    let mut attach = attach_io(a);

    // Reading from attach should automatically discard stderr and return stdout
    let mut buf = [0u8; 10];
    let n = attach.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"hello");
    assert_eq!(attach.stderr, 4);

    // Empty buffer returns 0
    let mut empty = [0u8; 0];
    assert_eq!(attach.read(&mut empty).unwrap(), 0);

    // Write through attach
    attach.write_all(b"response").unwrap();
    attach.flush().unwrap();
    let mut received = [0u8; 8];
    b.read_exact(&mut received).unwrap();
    assert_eq!(&received, b"response");

    // Nonblocking and deadline configuration
    assert!(attach.nonblocking(true).is_ok());
    assert!(attach.nonblocking(false).is_ok());
    assert!(attach.deadline(1000).is_ok());
}

fn ready_request() -> DnsRequest {
    DnsRequest {
        schema: "m1_dns_ipc_v1".into(),
        kind: "REQUEST".into(),
        attempt: Uuid::from_u128(0x0a77ac4),
        session_generation: 3,
        sequence: 0,
        engagement_id: Uuid::from_u128(0x0e6a4e1),
        campaign_id: Uuid::from_u128(0x0ca9a16),
        decision_id: Uuid::from_u128(0x0dec1de),
        profile: "dns-routing-v2".into(),
        execution_mode: "live".into(),
        run_profile: "autonomous-dns-genai-lab-v1".into(),
        release_sha256: "c".repeat(64),
        qualification_ref: Uuid::from_u128(0x0a11f1ed),
        name: "probe.duskweave.test".into(),
        resolver_ipv4: "192.0.2.53".into(),
        transport: "udp".into(),
        query_id: 0x1234,
        allowed_names: vec!["probe.duskweave.test".into()],
        deadline_at_ms: i64::MAX,
        max_duration_ms: 1500,
    }
}

/// One Docker multiplexed stdout frame: stream id 1 + big-endian payload length.
fn stdout_frame(payload: &[u8], framed: &mut Vec<u8>) {
    framed.extend_from_slice(&[1, 0, 0, 0]);
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(payload);
}

/// After its first positive read through the real Attach, this adapter elapses
/// that same Attach socket's absolute end and releases the remaining framed
/// bytes to the still-open peer. Every read delegates to the real Attach; it
/// never manufactures an I/O error itself.
struct ExpireThenRelease<'a> {
    attach: &'a mut Attach,
    peer: &'a mut UnixStream,
    rest: &'a [u8],
    fired: bool,
}

impl Read for ExpireThenRelease<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let count = self.attach.read(out)?;
        if count > 0 && !self.fired {
            self.fired = true;
            self.attach.socket.end = Instant::now() - Duration::from_millis(1);
            self.peer.write_all(self.rest)?;
        }
        Ok(count)
    }
}

#[test]
fn ready_control_survives_chunked_stdout_but_not_an_elapsed_absolute_end() {
    let q = ready_request();
    // The exporter's READY control reaches the launcher as FrameWriter bytes
    // carried inside Docker stdout multiplex chunks.
    let mut wire = Vec::new();
    FrameWriter::new(&mut wire)
        .write(&Control::new(&q, "READY", 1), MAX_CONTROL)
        .unwrap();
    let chunks: [&[u8]; 3] = [&wire[..3], &wire[3..11], &wire[11..]];
    let mut framed = Vec::new();
    for chunk in chunks {
        stdout_frame(chunk, &mut framed);
    }
    let rest_at = 8 + chunks[0].len();

    // Identical framed bytes on a live end validate, so malformed framing
    // cannot explain the negative case below.
    let (a, mut b) = UnixStream::pair().unwrap();
    b.write_all(&framed).unwrap();
    let mut attach = attach_io(a);
    FrameReader::new(&mut attach)
        .control()
        .unwrap()
        .validate(&q, "READY", 1)
        .unwrap();

    // Same framed bytes: after one positive partial read, this Attach's own
    // absolute end elapses and the remaining chunks become available. The
    // read still refuses with bytes queued and the peer open, so neither EOF
    // nor missing input can explain the rejection.
    let (a, mut b) = UnixStream::pair().unwrap();
    b.write_all(&framed[..rest_at]).unwrap();
    let mut attach = attach_io(a);
    let mut adapter = ExpireThenRelease {
        attach: &mut attach,
        peer: &mut b,
        rest: &framed[rest_at..],
        fired: false,
    };
    assert_eq!(
        FrameReader::new(&mut adapter).control().err(),
        Some(Fail::State("dns_ipc_failure"))
    );
    assert!(
        adapter.fired,
        "the first partial stdout read must succeed before the end elapses"
    );
}

#[test]
fn timed_socket_writes_never_extend_the_absolute_end() {
    let (a, mut b) = UnixStream::pair().unwrap();
    let mut attach = attach_io(a);
    // The first fragment reaches the peer through the real TimedSocket write.
    attach.write_all(b"part-").unwrap();
    let mut first = [0u8; 5];
    b.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"part-");

    // Elapse the same absolute end: a later fragment must refuse even though
    // the peer is still connected and the socket remains writable.
    attach.socket.end = Instant::now() - Duration::from_millis(1);
    let error = attach.write_all(b"rest").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(error.to_string(), "dns_ipc_failure");

    // Nonblocking observation: no later bytes ever reached the open peer.
    b.set_nonblocking(true).unwrap();
    let mut extra = [0u8; 1];
    assert!(matches!(
        b.read(&mut extra),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));

    // A fresh end accepts writes again, so the refusal was the elapsed
    // deadline and not a dead socket.
    attach.socket.end = Instant::now() + Duration::from_secs(5);
    attach.write_all(b"fin").unwrap();
    b.set_nonblocking(false).unwrap();
    let mut tail = [0u8; 3];
    b.read_exact(&mut tail).unwrap();
    assert_eq!(&tail, b"fin");
}

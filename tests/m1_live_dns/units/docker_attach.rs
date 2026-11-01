use super::*;

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

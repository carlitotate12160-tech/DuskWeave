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

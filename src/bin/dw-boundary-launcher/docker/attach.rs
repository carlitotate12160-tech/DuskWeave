//! Raw-facing Docker upgrade and finite stdout/stderr multiplexing.
use super::{command, connection};
use duskweave::{Fail, Res};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
/// Each short socket read/write consumes the same absolute phase deadline.
pub(crate) struct TimedSocket {
    pub(crate) io: UnixStream,
    pub(crate) end: Instant,
}
impl Read for TimedSocket {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.io
            .set_read_timeout(Some(command::remaining(self.end).map_err(|_| protocol())?))?;
        self.io.read(bytes)
    }
}
impl Write for TimedSocket {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let left = command::remaining(self.end).map_err(|_| protocol())?;
        self.io
            .set_write_timeout(Some(left.min(Duration::from_millis(100))))?;
        self.io.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.io.flush()
    }
}
pub(crate) struct Attach {
    socket: TimedSocket,
    remaining: u32,
    stream: u8,
    chunks: u16,
    total: u32,
    stderr: u32,
}

impl Attach {
    pub fn connect(id: &str, deadline: Instant) -> Res<Self> {
        let io = connection::acquire(deadline)?;
        let mut socket = TimedSocket { io, end: deadline };
        let request = format!(
            "POST /containers/{id}/attach?stream=1&stdin=1&stdout=1&stderr=1 HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: tcp\r\nContent-Length: 0\r\n\r\n"
        );
        socket
            .write_all(request.as_bytes())
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            if response.len() == 4096 {
                return Err(Fail::State("dns_ipc_failure"));
            }
            let mut byte = [0; 1];
            socket
                .read_exact(&mut byte)
                .map_err(|_| Fail::State("dns_ipc_failure"))?;
            response.push(byte[0]);
        }
        if !response.starts_with(b"HTTP/1.1 101 ") {
            return Err(Fail::State("dns_ipc_failure"));
        }
        Ok(Self {
            socket,
            remaining: 0,
            stream: 0,
            chunks: 0,
            total: 0,
            stderr: 0,
        })
    }

    pub fn nonblocking(&self, enabled: bool) -> Res<()> {
        self.socket
            .io
            .set_nonblocking(enabled)
            .map_err(|_| Fail::State("dns_ipc_failure"))
    }

    pub fn deadline(&mut self, milliseconds: u64) -> Res<()> {
        self.socket.end = Instant::now() + Duration::from_millis(milliseconds.max(1));
        Ok(())
    }

    fn header(&mut self) -> std::io::Result<bool> {
        let mut header = [0; 8];
        if self.socket.read(&mut header[..1])? == 0 {
            return Ok(false);
        }
        self.socket.read_exact(&mut header[1..])?;
        self.admit_header(header)?;
        Ok(true)
    }
    fn admit_header(&mut self, header: [u8; 8]) -> std::io::Result<()> {
        self.chunks += 1;
        let size = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
        self.total = self.total.checked_add(size).ok_or_else(protocol)?;
        let valid = [
            self.chunks <= 64,
            self.total <= 262208,
            size <= 16388,
            header[1..4] == [0, 0, 0],
            [1, 2].contains(&header[0]),
        ];
        if !valid.into_iter().all(|value| value) {
            return Err(protocol());
        }
        self.remaining = size;
        self.stream = header[0];
        Ok(())
    }

    fn discard_stderr(&mut self) -> std::io::Result<()> {
        self.stderr = self
            .stderr
            .checked_add(self.remaining)
            .ok_or_else(protocol)?;
        if self.stderr > 4096 {
            return Err(protocol());
        }
        let mut discard = [0; 256];
        while self.remaining > 0 {
            let count = (self.remaining as usize).min(discard.len());
            self.socket.read_exact(&mut discard[..count])?;
            self.remaining -= count as u32;
        }
        Ok(())
    }
}

fn protocol() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, "dns_ipc_failure")
}

impl Attach {
    fn next_frame(&mut self) -> std::io::Result<bool> {
        if self.remaining > 0 {
            return Ok(true);
        }
        self.header()
    }
    fn stdout_ready(&mut self) -> std::io::Result<bool> {
        loop {
            if !self.next_frame()? {
                return Ok(false);
            }
            if self.stream == 2 {
                self.discard_stderr()?;
                continue;
            }
            if self.remaining > 0 {
                return Ok(true);
            }
        }
    }
}
impl Read for Attach {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() || !self.stdout_ready()? {
            return Ok(0);
        }
        let count = (self.remaining as usize).min(out.len());
        let got = self.socket.read(&mut out[..count])?;
        if got == 0 {
            return Err(protocol());
        }
        self.remaining -= got as u32;
        Ok(got)
    }
}
impl Write for Attach {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.socket.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.socket.flush()
    }
}

#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/docker_attach.rs"]
mod tests;

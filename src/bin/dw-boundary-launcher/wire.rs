//! ADR-017 frame I/O: four-byte big-endian length then one strict UTF-8 JSON
//! object. Reads bound before allocation; malformed frames end the stream
//! without being echoed anywhere.
use serde::Serialize;
use std::io::{Read, Write};
use std::time::Instant;

pub const MAX_JSON_FRAME: usize = 16 * 1024;

#[derive(Debug)]
pub enum WireErr {
    Io,
    Oversize,
    Invalid,
}

pub fn read_frame<R: Read>(r: &mut R, buf: &mut Vec<u8>) -> Result<usize, WireErr> {
    let mut hdr = [0u8; 4];
    r.read_exact(&mut hdr).map_err(|_| WireErr::Io)?;
    let n = u32::from_be_bytes(hdr) as usize;
    if n > MAX_JSON_FRAME {
        return Err(WireErr::Oversize);
    }
    buf.clear();
    buf.resize(n, 0);
    r.read_exact(buf).map_err(|_| WireErr::Io)?;
    Ok(n)
}

pub fn write_json<W: Write, T: Serialize>(w: &mut W, v: &T) -> Result<(), WireErr> {
    let body = serde_json::to_vec(v).map_err(|_| WireErr::Invalid)?;
    if body.len() > MAX_JSON_FRAME {
        return Err(WireErr::Oversize);
    }
    w.write_all(&(body.len() as u32).to_be_bytes())
        .and_then(|_| w.write_all(&body))
        .and_then(|_| w.flush())
        .map_err(|_| WireErr::Io)
}

/// Copy bytes verbatim from src to dst until EOF, never inspecting content.
/// Returns bytes moved. Used only for the capture->exporter raw channel.
pub fn blind_forward<R: Read, W: Write>(
    src: &mut R,
    dst: &mut W,
    stop: &std::sync::atomic::AtomicBool,
) -> u64 {
    let mut moved = 0u64;
    let mut chunk = [0u8; 4096];
    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        match src.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if dst.write_all(&chunk[..n]).is_err() {
                    break;
                }
                moved += n as u64;
            }
        }
    }
    moved
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

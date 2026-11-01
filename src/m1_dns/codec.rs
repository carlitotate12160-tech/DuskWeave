//! Ordinary-side framing accepts only the trusted exporter's closed result types.
use super::{DnsRequest, DnsResult, MAX_CONTROL, MAX_FRAME, SCHEMA};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::io::{Read, Write};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
    pub deadline_at_ms: i64,
}
impl Control {
    pub fn new(q: &DnsRequest, kind: &str, sequence: u32) -> Self {
        Self {
            schema: SCHEMA.into(),
            kind: kind.into(),
            attempt: q.attempt,
            session_generation: q.session_generation,
            sequence,
            deadline_at_ms: q.deadline_at_ms,
        }
    }
    pub fn validate(&self, q: &DnsRequest, kind: &str, sequence: u32) -> Res<()> {
        let valid = [
            self.schema == SCHEMA,
            self.kind == kind,
            self.sequence == sequence,
            self.attempt == q.attempt,
            self.session_generation == q.session_generation,
            self.deadline_at_ms == q.deadline_at_ms,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::State("dns_ipc_failure"))
    }
}
pub struct FrameReader<R> {
    source: R,
    frames: u8,
}
pub struct FrameWriter<W> {
    sink: W,
    frames: u8,
}
impl<R: Read> FrameReader<R> {
    pub fn new(source: R) -> Self {
        Self { source, frames: 0 }
    }
    fn read<T: DeserializeOwned>(&mut self, limit: usize) -> Res<T> {
        if self.frames >= 16 {
            return Err(Fail::State("dns_ipc_failure"));
        }
        let mut header = [0; 4];
        self.source
            .read_exact(&mut header)
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        let len = u32::from_be_bytes(header) as usize;
        if !(1..=limit).contains(&len) {
            return Err(Fail::State("dns_ipc_failure"));
        }
        let mut data = vec![0; len];
        self.source
            .read_exact(&mut data)
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        if data.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
            return Err(Fail::State("dns_ipc_failure"));
        }
        let result = serde_json::from_slice(&data).map_err(|_| Fail::State("dns_ipc_failure"))?;
        self.frames += 1;
        Ok(result)
    }
    pub fn control(&mut self) -> Res<Control> {
        self.read(MAX_CONTROL)
    }
    pub fn request(&mut self) -> Res<DnsRequest> {
        self.read(MAX_FRAME)
    }
    pub fn result(&mut self) -> Res<DnsResult> {
        self.read(MAX_FRAME)
    }
}
impl<W: Write> FrameWriter<W> {
    pub fn new(sink: W) -> Self {
        Self { sink, frames: 0 }
    }
    pub fn write<T: Serialize>(&mut self, value: &T, limit: usize) -> Res<()> {
        if self.frames >= 16 {
            return Err(Fail::State("dns_ipc_failure"));
        }
        let data = serde_json::to_vec(value).map_err(|_| Fail::State("dns_ipc_failure"))?;
        if data.is_empty() || data.len() > limit.min(MAX_FRAME) {
            return Err(Fail::State("dns_ipc_failure"));
        }
        self.sink
            .write_all(&(data.len() as u32).to_be_bytes())
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        self.sink
            .write_all(&data)
            .map_err(|_| Fail::State("dns_ipc_failure"))?;
        self.frames += 1;
        Ok(())
    }
}
// All three accepted messages are closed, non-recursive structs. Their grammar
// has at most three container levels and 21 object entries; unknown/duplicate
// keys reject at the typed parser. Collection visitors stop before growth past
// four answers/aliases or eight names. No arbitrary JSON Value enters this API.

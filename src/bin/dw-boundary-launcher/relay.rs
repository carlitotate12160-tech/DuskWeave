//! Raw-facing TCB: Docker capture stdout reaches only the isolated exporter input.
use super::attempt::Session;
use super::docker::Attach;
use duskweave::m1_dns::{
    Control, DnsRequest, DnsResult, FrameReader, FrameWriter, MAX_CONTROL, MAX_FRAME,
};
use duskweave::{Fail, Res};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

fn ready_control(source: &mut impl Read, q: &DnsRequest) -> Res<()> {
    FrameReader::new(source).control()?.validate(q, "READY", 1)
}

fn prepare_resources(image: &str, q: &DnsRequest, end: Instant) -> Res<(Attach, Attach)> {
    #[cfg(test)]
    if let Some(resources) = unit_tests::resources(image, q, end) {
        return resources;
    }
    let capture = super::docker::create(image, q.attempt, false, end)?;
    let exporter = super::docker::create(image, q.attempt, true, end)?;
    super::network::create(q.attempt, capture.pid, &q.resolver_ipv4, end)?;
    // Both handshakes finish before either worker receives its request.
    Ok((
        Attach::connect(&capture.id, end)?,
        Attach::connect(&exporter.id, end)?,
    ))
}

pub(super) struct Pair {
    request: DnsRequest,
    capture: Attach,
    exporter: Attach,
    disposed: bool,
}
impl Pair {
    pub fn prepare(image: &str, session: &Session) -> Res<Self> {
        let end = session
            .preparation_deadline
            .ok_or(Fail::Config("live_host_unqualified"))?;
        let q = &session.request;
        let (capture, exporter) = prepare_resources(image, q, end)?;
        let mut pair = Self {
            request: (**q).clone(),
            capture,
            exporter,
            disposed: false,
        };
        pair.ready()?;
        Ok(pair)
    }
    fn ready(&mut self) -> Res<()> {
        FrameWriter::new(&mut self.exporter).write(&self.request, MAX_FRAME)?;
        FrameWriter::new(&mut self.capture).write(&self.request, MAX_FRAME)?;
        ready_control(&mut self.exporter, &self.request)?;
        ready_control(&mut self.capture, &self.request)
    }
    fn set_attempt_deadline(&mut self, deadline: i64) -> Res<()> {
        let left = deadline - super::qualification::now_ms()?;
        if left <= 0 {
            return Err(Fail::State("live_effect_deadline"));
        }
        self.capture.deadline(left as u64)?;
        self.exporter.deadline(left as u64)
    }
    pub fn start(&mut self, deadline: i64) -> Res<()> {
        self.set_attempt_deadline(deadline)?;
        self.capture.nonblocking(true)?;
        if let Err(error) = self.exporter.nonblocking(true) {
            let _ = self.capture.nonblocking(false);
            return Err(error);
        }
        let mut start = Control::new(&self.request, "START", 2);
        start.deadline_at_ms = deadline;
        // Exporter context must precede the capture's first possible raw byte.
        let outcome = FrameWriter::new(&mut self.exporter)
            .write(&start, MAX_CONTROL)
            .and_then(|_| FrameWriter::new(&mut self.capture).write(&start, MAX_CONTROL));
        let capture_reset = self.capture.nonblocking(false);
        let exporter_reset = self.exporter.nonblocking(false);
        capture_reset?;
        exporter_reset?;
        outcome
    }
    pub fn result(&mut self, deadline: i64) -> Res<DnsResult> {
        let left = deadline.saturating_add(1000) - super::qualification::now_ms()?;
        if left <= 0 {
            return Err(Fail::State("live_effect_deadline"));
        }
        self.capture.deadline(left as u64)?;
        self.exporter.deadline(left as u64)?;
        forward_raw(&mut self.capture, &mut self.exporter)?;
        FrameReader::new(&mut self.exporter).result()
    }
    pub fn acknowledge(&mut self) -> Res<()> {
        let ack = Control::new(&self.request, "ACK", 4);
        FrameWriter::new(&mut self.exporter).write(&ack, MAX_CONTROL)
    }
    pub fn dispose(&mut self) -> bool {
        let end = Instant::now() + Duration::from_millis(1000);
        let q = &self.request;
        let _ =
            super::attempt::stop_until(q.engagement_id, q.campaign_id, q.session_generation, end);
        // Pair custody is the two containers and attempt-local namespace.
        let disposed = super::attempt::dispose_until(q.attempt, end);
        self.disposed = true;
        disposed && super::attempt::stopped(q)
    }
}
impl Drop for Pair {
    fn drop(&mut self) {
        if !self.disposed {
            let _ = self.dispose();
        }
    }
}

/// Stage exactly one opaque frame and confirm capture EOF before any exporter write.
fn forward_raw(source: &mut impl Read, sink: &mut impl Write) -> Res<()> {
    let mut prefix = [0u8; 4];
    source
        .read_exact(&mut prefix)
        .map_err(|_| Fail::State("dns_ipc_failure"))?;
    let count = u32::from_be_bytes(prefix) as usize;
    if !(1..=MAX_FRAME).contains(&count) {
        return Err(Fail::State("dns_ipc_failure"));
    }
    let mut frame = Vec::with_capacity(count + prefix.len());
    frame.extend_from_slice(&prefix);
    frame.resize(count + prefix.len(), 0);
    source
        .read_exact(&mut frame[prefix.len()..])
        .map_err(|_| Fail::State("dns_ipc_failure"))?;
    let mut extra = [0u8; 1];
    if source
        .read(&mut extra)
        .map_err(|_| Fail::State("dns_ipc_failure"))?
        != 0
    {
        return Err(Fail::State("dns_ipc_failure"));
    }
    sink.write_all(&frame)
        .map_err(|_| Fail::State("dns_ipc_failure"))
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/stop.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/launcher_relay.rs"]
pub(crate) mod unit_tests;

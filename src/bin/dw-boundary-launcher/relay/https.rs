//! Phased HTTPS worker-pair relay with aggregate `<= 16` transmission counter and raw forwarding.
use super::super::attempt::https::HttpsSession;
use super::super::docker::Attach;
use duskweave::m1_https::codec::{decode_payload, encode_frame};
use duskweave::m1_https::ipc::{
    HTTPS_IPC_MAX_CONTROL, HTTPS_IPC_MAX_FRAME, HTTPS_IPC_MAX_FRAMES, HttpsAck, HttpsIpcRequest,
    HttpsPhase, HttpsPhaseReady, HttpsPhaseStart, HttpsSafeResult,
};
use duskweave::{Fail, Res};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, SyncSender};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(in super::super) enum PairCmd {
    StartPhase(HttpsPhase, i64),
}

pub(in super::super) enum PairEvt {
    Ready(HttpsPhase),
    Done(Res<HttpsSafeResult>),
}

pub(in super::super) struct HttpsPair {
    request: HttpsIpcRequest,
    capture: Attach,
    exporter: Attach,
    frames: u8,
    granted_phase: Option<HttpsPhase>,
    disposed: bool,
}

impl HttpsPair {
    pub fn prepare(image: &str, session: &HttpsSession) -> Res<Self> {
        let end = session
            .preparation_deadline
            .ok_or(Fail::Config("live_host_unqualified"))?;
        let q = &session.request;
        let (capture, exporter) = Self::prepare_resources(image, q, end)?;
        Self::prepare_with(q, capture, exporter)
    }

    fn prepare_resources(image: &str, q: &HttpsIpcRequest, end: Instant) -> Res<(Attach, Attach)> {
        #[cfg(test)]
        if let Some(resources) = tests::resources(image, q, end) {
            return resources;
        }
        let capture = super::super::docker::create(image, q.attempt, false, end)?;
        let exporter = super::super::docker::create(image, q.attempt, true, end)?;
        super::super::network::create_https(q.attempt, capture.pid, &q.ipv4, end)?;
        Ok((
            Attach::connect(&capture.id, end)?,
            Attach::connect(&exporter.id, end)?,
        ))
    }

    pub(in super::super) fn prepare_with(
        request: &HttpsIpcRequest,
        capture: Attach,
        exporter: Attach,
    ) -> Res<Self> {
        let mut pair = Self {
            request: request.clone(),
            capture,
            exporter,
            frames: 0,
            granted_phase: None,
            disposed: false,
        };
        pair.ready_connect()?;
        Ok(pair)
    }

    fn ready_connect(&mut self) -> Res<()> {
        write_frame(
            &mut self.exporter,
            &mut self.frames,
            &self.request,
            HTTPS_IPC_MAX_FRAME,
        )?;
        write_frame(
            &mut self.capture,
            &mut self.frames,
            &self.request,
            HTTPS_IPC_MAX_FRAME,
        )?;
        read_ready(
            &mut self.exporter,
            &mut self.frames,
            &self.request,
            HttpsPhase::Connect,
        )?;
        read_ready(
            &mut self.capture,
            &mut self.frames,
            &self.request,
            HttpsPhase::Connect,
        )
    }

    fn set_deadline(&mut self, deadline: i64) -> Res<()> {
        let left = deadline - super::super::qualification::now_ms()?;
        if left <= 0 {
            return Err(Fail::State("live_effect_deadline"));
        }
        self.capture.deadline(left as u64)?;
        self.exporter.deadline(left as u64)
    }

    pub fn start_connect(&mut self, deadline: i64) -> Res<()> {
        self.set_deadline(deadline)?;
        self.write_start(HttpsPhase::Connect, deadline)?;
        self.granted_phase = Some(HttpsPhase::Connect);
        Ok(())
    }

    fn write_start(&mut self, phase: HttpsPhase, deadline: i64) -> Res<()> {
        self.capture.nonblocking(true)?;
        if let Err(err) = self.exporter.nonblocking(true) {
            let _ = self.capture.nonblocking(false);
            return Err(err);
        }
        let start = HttpsPhaseStart::new(&self.request, phase, deadline);
        let outcome = write_frame(
            &mut self.exporter,
            &mut self.frames,
            &start,
            HTTPS_IPC_MAX_CONTROL,
        )
        .and_then(|_| {
            write_frame(
                &mut self.capture,
                &mut self.frames,
                &start,
                HTTPS_IPC_MAX_CONTROL,
            )
        });
        let r1 = self.capture.nonblocking(false);
        let r2 = self.exporter.nonblocking(false);
        r1?;
        r2?;
        outcome
    }

    fn arm_deadlines(&mut self, deadline: i64) -> Res<()> {
        let left = deadline.saturating_add(1000) - super::super::qualification::now_ms()?;
        if left <= 0 {
            return Err(Fail::State("live_effect_deadline"));
        }
        self.capture.deadline(left as u64)?;
        self.exporter.deadline(left as u64)
    }

    fn next_capture_frame(&mut self) -> Res<([u8; 4], Vec<u8>, String)> {
        let (prefix, payload) = read_raw_frame(&mut self.capture, &mut self.frames)?;
        let kind = frame_kind(&payload)?.to_owned();
        Ok((prefix, payload, kind))
    }

    fn step_drive(
        &mut self,
        deadline: i64,
        evt_tx: &SyncSender<PairEvt>,
        cmd_rx: &Receiver<PairCmd>,
        revoke: &dyn Fn(Uuid),
    ) -> Res<Option<HttpsSafeResult>> {
        let (prefix, payload, kind) = self.next_capture_frame()?;
        if kind == "READY" {
            self.handle_ready(&payload, deadline, evt_tx, cmd_rx)?;
            Ok(None)
        } else if kind == "RESULT" {
            self.forward_and_export(prefix, payload, revoke).map(Some)
        } else {
            Err(Fail::State("https_ipc_failure"))
        }
    }

    pub fn drive(
        &mut self,
        deadline: i64,
        evt_tx: &SyncSender<PairEvt>,
        cmd_rx: &Receiver<PairCmd>,
        revoke: &dyn Fn(Uuid),
    ) -> Res<HttpsSafeResult> {
        self.arm_deadlines(deadline)?;
        loop {
            if let Some(done) = self.step_drive(deadline, evt_tx, cmd_rx, revoke)? {
                return Ok(done);
            }
        }
    }

    fn expected_next_phase(&self, ready: &HttpsPhaseReady) -> Res<HttpsPhase> {
        let expected = match self.granted_phase {
            Some(HttpsPhase::Connect) => HttpsPhase::Tls,
            Some(HttpsPhase::Tls) => HttpsPhase::Head,
            _ => return Err(Fail::State("https_ipc_failure")),
        };
        if !ready.matches(&self.request, expected) {
            return Err(Fail::State("https_ipc_failure"));
        }
        Ok(expected)
    }

    fn await_phase_start(
        &mut self,
        expected: HttpsPhase,
        deadline: i64,
        cmd_rx: &Receiver<PairCmd>,
    ) -> Res<()> {
        match cmd_rx.recv() {
            Ok(PairCmd::StartPhase(p, dl)) if (p, dl) == (expected, deadline) => {
                self.write_start(p, dl)?;
                self.granted_phase = Some(p);
                Ok(())
            }
            _ => Err(Fail::State("live_effect_stopped")),
        }
    }

    fn handle_ready(
        &mut self,
        payload: &[u8],
        deadline: i64,
        evt_tx: &SyncSender<PairEvt>,
        cmd_rx: &Receiver<PairCmd>,
    ) -> Res<()> {
        let ready: HttpsPhaseReady = decode_payload(payload, HTTPS_IPC_MAX_CONTROL)?;
        let expected = self.expected_next_phase(&ready)?;
        evt_tx
            .send(PairEvt::Ready(expected))
            .map_err(|_| Fail::State("live_effect_stopped"))?;
        self.await_phase_start(expected, deadline, cmd_rx)
    }

    fn forward_raw_result(
        &mut self,
        prefix: &[u8; 4],
        payload: &[u8],
        revoke: &dyn Fn(Uuid),
    ) -> Res<()> {
        let mut extra = [0u8; 1];
        if self
            .capture
            .read(&mut extra)
            .map_err(|_| Fail::State("https_ipc_failure"))?
            != 0
        {
            return Err(Fail::State("https_ipc_failure"));
        }
        revoke(self.request.attempt);
        charge_frame(&mut self.frames)?;
        self.exporter
            .write_all(prefix)
            .and_then(|_| self.exporter.write_all(payload))
            .and_then(|_| self.exporter.flush())
            .map_err(|_| Fail::State("https_ipc_failure"))
    }

    fn validate_safe_result(&mut self, safe_bytes: &[u8]) -> Res<HttpsSafeResult> {
        let safe: HttpsSafeResult = decode_payload(safe_bytes, HTTPS_IPC_MAX_FRAME)?;
        let granted = self.granted_phase.ok_or(Fail::State("https_ipc_failure"))?;
        safe.validate_for(
            &self.request,
            granted,
            super::super::qualification::now_ms()?,
        )?;
        Ok(safe)
    }

    fn forward_and_export(
        &mut self,
        prefix: [u8; 4],
        payload: Vec<u8>,
        revoke: &dyn Fn(Uuid),
    ) -> Res<HttpsSafeResult> {
        self.forward_raw_result(&prefix, &payload, revoke)?;
        let (_, safe_bytes) = read_raw_frame(&mut self.exporter, &mut self.frames)?;
        self.validate_safe_result(&safe_bytes)
    }

    pub fn acknowledge(&mut self) -> Res<()> {
        let ack = HttpsAck::new(&self.request);
        write_frame(
            &mut self.exporter,
            &mut self.frames,
            &ack,
            HTTPS_IPC_MAX_CONTROL,
        )
    }

    pub fn dispose(&mut self) -> bool {
        let end = Instant::now() + Duration::from_millis(1000);
        let q = &self.request;
        let _ = super::super::attempt::stop_until(
            q.engagement_id,
            q.campaign_id,
            q.session_generation,
            end,
        );
        let disposed = super::super::attempt::dispose_until(q.attempt, end);
        self.disposed = true;
        disposed
            && super::super::attempt::stopped_ids(
                q.engagement_id,
                q.campaign_id,
                q.session_generation,
            )
    }
}

impl Drop for HttpsPair {
    fn drop(&mut self) {
        if !self.disposed {
            let _ = self.dispose();
        }
    }
}

fn charge_frame(frames: &mut u8) -> Res<()> {
    if *frames >= HTTPS_IPC_MAX_FRAMES {
        return Err(Fail::State("https_ipc_failure"));
    }
    *frames += 1;
    Ok(())
}

fn write_frame<T: Serialize>(
    sink: &mut impl Write,
    frames: &mut u8,
    value: &T,
    limit: usize,
) -> Res<()> {
    charge_frame(frames)?;
    let framed = encode_frame(value, limit)?;
    sink.write_all(&framed)
        .and_then(|_| sink.flush())
        .map_err(|_| Fail::State("https_ipc_failure"))
}

fn read_raw_frame(source: &mut impl Read, frames: &mut u8) -> Res<([u8; 4], Vec<u8>)> {
    charge_frame(frames)?;
    let mut prefix = [0u8; 4];
    source
        .read_exact(&mut prefix)
        .map_err(|_| Fail::State("https_ipc_failure"))?;
    let count = u32::from_be_bytes(prefix) as usize;
    if !(1..=HTTPS_IPC_MAX_FRAME).contains(&count) {
        return Err(Fail::State("https_ipc_failure"));
    }
    let mut payload = vec![0u8; count];
    source
        .read_exact(&mut payload)
        .map_err(|_| Fail::State("https_ipc_failure"))?;
    Ok((prefix, payload))
}

fn read_ready(
    source: &mut impl Read,
    frames: &mut u8,
    q: &HttpsIpcRequest,
    phase: HttpsPhase,
) -> Res<()> {
    let (_, payload) = read_raw_frame(source, frames)?;
    let ready: HttpsPhaseReady = decode_payload(&payload, HTTPS_IPC_MAX_CONTROL)?;
    ready
        .matches(q, phase)
        .then_some(())
        .ok_or(Fail::State("https_ipc_failure"))
}

#[derive(serde::Deserialize)]
struct KindPeek {
    kind: String,
}

fn frame_kind(payload: &[u8]) -> Res<&'static str> {
    let peek: KindPeek = decode_payload(payload, HTTPS_IPC_MAX_FRAME)?;
    match peek.kind.as_str() {
        "READY" => Ok("READY"),
        "RESULT" => Ok("RESULT"),
        _ => Err(Fail::State("https_ipc_failure")),
    }
}

#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/launcher_https_relay.rs"]
pub(crate) mod tests;

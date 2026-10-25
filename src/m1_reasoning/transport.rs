//! Synchronous in-process transport: no shell, child process, trace or raw receipt.
use super::inference::{Provider, Received};
use super::proposal::ProposalBasis;
use super::provider_diagnostic::ProviderDiagnostic;
use super::request::BoundCall;
use super::transport_control::{CallClock, Permit, ReportedGuard, TransportGuard};
use curl::easy::{Easy, HttpVersion, List};
use std::cell::{Cell, RefCell};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};
type Res<T> = Result<T, &'static str>;
#[derive(Default)]
pub struct DeepSeekProvider {
    prepared: Option<(Easy, String, String)>,
}

struct Deadline(Sender<()>);
impl Deadline {
    fn arm(until: Instant) -> Res<Self> {
        let duration = until
            .checked_duration_since(Instant::now())
            .ok_or("inference_deadline")?;
        let (send, recv) = channel();
        std::thread::Builder::new()
            .name("inference_deadline".into())
            .spawn(move || {
                if matches!(
                    recv.recv_timeout(duration),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ) {
                    std::process::exit(124);
                }
            })
            .map_err(|_| "inference_transport_unavailable")?;
        Ok(Self(send))
    }
}
impl Drop for Deadline {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}
fn add(raw: &RefCell<Vec<u8>>, chunk: &[u8]) -> bool {
    let mut bytes = raw.borrow_mut();
    if chunk.len() > 65536usize.saturating_sub(bytes.len()) {
        return false;
    }
    bytes.extend_from_slice(chunk);
    true
}
fn authorization_headers(key: &str) -> Result<List, curl::Error> {
    let mut headers = List::new();
    headers.append("Content-Type: application/json")?;
    headers.append("Accept-Encoding: identity")?;
    headers.append(&format!("Authorization: Bearer {key}"))?;
    Ok(headers)
}
fn provider_key() -> Res<String> {
    let key = std::env::var("DEEPSEEK_API_KEY").map_err(|_| "live_key_missing")?;
    if key.is_empty() || key.chars().any(|c| c.is_control()) {
        return Err("live_key_invalid");
    }
    Ok(key)
}
fn tls_policy(h: &mut Easy) -> Result<(), curl::Error> {
    h.ssl_verify_peer(true)?;
    h.ssl_verify_host(true)?;
    h.http_content_decoding(false)
}
fn routing_policy(h: &mut Easy) -> Result<(), curl::Error> {
    h.http_version(HttpVersion::V11)?;
    h.proxy("")?;
    h.noproxy("*")?;
    h.follow_location(false)
}
fn configure(b: &BoundCall, key: &str, left: Duration) -> Result<Easy, curl::Error> {
    let mut h = Easy::new();
    post(&mut h, b, key)?;
    tls_policy(&mut h)?;
    routing_policy(&mut h)?;
    timeouts(&mut h, left)?;
    Ok(h)
}
fn timeouts(h: &mut Easy, left: Duration) -> Result<(), curl::Error> {
    h.timeout(left)?;
    h.connect_timeout(left.min(Duration::from_secs(5)))?;
    h.progress(true)
}

impl Provider for DeepSeekProvider {
    fn prepare(&mut self, b: &BoundCall, until: Instant) -> Res<()> {
        let key = provider_key()?;
        let left = until
            .checked_duration_since(Instant::now())
            .ok_or("inference_deadline")?;
        let h = configure(b, &key, left).map_err(|_| "inference_transport_unavailable")?;
        self.prepared = Some((h, key, b.digest.clone()));
        Ok(())
    }
    fn call(
        &mut self,
        b: &BoundCall,
        until: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Received> {
        let _deadline = Deadline::arm(until)?;
        let (mut h, key, digest) = self
            .prepared
            .take()
            .ok_or("inference_transport_unavailable")?;
        if digest != b.digest {
            return Err("inference_transport_unavailable");
        }
        let left = until
            .checked_duration_since(Instant::now())
            .ok_or("inference_deadline")?;
        h.timeout(left)
            .map_err(|_| "inference_transport_unavailable")?;
        h.connect_timeout(left.min(Duration::from_secs(5)))
            .map_err(|_| "inference_transport_unavailable")?;
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            // Launch is the release linearization point, ordered with Mission writers.
            // This worker owns the prepared request; it cannot outlive its process.
            let stop = &cancelled;
            let worker = std::thread::Builder::new()
                .name("inference_transport".into())
                .spawn_scoped(scope, move || attempt(h, key, until, basis, stop))
                .map_err(|_| "inference_transport_unavailable")?;
            let opened = release();
            if opened.is_err() {
                cancelled.store(true, std::sync::atomic::Ordering::Release);
            }
            // Always collect the owned worker; a saved release error leads join failure.
            let joined = worker.join();
            opened?;
            joined.map_err(|_| "provider_transport_unknown")?
        })
    }
}
fn attempt(
    mut h: Easy,
    key: String,
    until: Instant,
    basis: &mut dyn ProposalBasis,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Res<Received> {
    if !transport_current(until, cancelled, basis) {
        return Err("provider_cancelled_uncertain");
    }
    let (result, raw, oversized, elapsed) =
        exchange(&mut h, &mut || transport_current(until, cancelled, basis))?;
    finish_transfer(result, raw, oversized, &key, elapsed, basis)
}
type Exchange = (Result<(), curl::Error>, Vec<u8>, bool, Duration);
fn exchange(h: &mut Easy, current: &mut dyn FnMut() -> bool) -> Res<Exchange> {
    let raw = RefCell::new(Vec::new());
    let oversized = Cell::new(false);
    let start = Instant::now();
    let result = {
        let mut t = h.transfer();
        t.header_function(|chunk| {
            let ok = add(&raw, chunk);
            oversized.set(oversized.get() || !ok);
            ok
        })
        .map_err(|_| "inference_transport_unavailable")?;
        t.write_function(|chunk| {
            if add(&raw, chunk) {
                Ok(chunk.len())
            } else {
                oversized.set(true);
                Ok(0)
            }
        })
        .map_err(|_| "inference_transport_unavailable")?;
        t.progress_function(|_, _, _, _| current())
            .map_err(|_| "inference_transport_unavailable")?;
        t.perform()
    };
    Ok((result, raw.into_inner(), oversized.get(), start.elapsed()))
}
fn finish_transfer(
    result: Result<(), curl::Error>,
    raw: Vec<u8>,
    oversized: bool,
    key: &str,
    elapsed: Duration,
    basis: &mut dyn ProposalBasis,
) -> Res<Received> {
    if oversized {
        return Err("provider_response_limit");
    }
    if result.is_err() {
        return Err("provider_transport_unknown");
    }
    if raw.windows(key.len()).any(|v| v == key.as_bytes()) {
        return Err("provider_unsafe_response");
    }
    admit_without_key(&raw, elapsed, key, basis)
}
struct KeyExclusion<'a> {
    owner: &'a mut dyn ProposalBasis,
    key: &'a str,
    reflected: Cell<bool>,
}
impl KeyExclusion<'_> {
    fn clear(&self, text: &str) -> bool {
        if text.contains(self.key) {
            self.reflected.set(true);
            return false;
        }
        true
    }
}
impl ProposalBasis for KeyExclusion<'_> {
    fn reference_eligible(&self, r: &str) -> bool {
        self.clear(r) && self.owner.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        self.clear(r) && self.owner.manifest_eligible(r)
    }
    fn safe_explanation(&self, text: &str) -> bool {
        self.clear(text) && self.owner.safe_explanation(text)
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        self.owner.current_authority_and_premises()
    }
    fn current_authority_for_references(&mut self, refs: &[String]) -> Res<()> {
        if !refs.iter().all(|r| self.clear(r)) {
            return Err("provider_unsafe_response");
        }
        self.owner.current_authority_for_references(refs)
    }
}
fn admit_without_key(
    raw: &[u8],
    elapsed: Duration,
    key: &str,
    basis: &mut dyn ProposalBasis,
) -> Res<Received> {
    let mut diagnostic = ProviderDiagnostic::default();
    let mut screened = KeyExclusion {
        owner: basis,
        key,
        reflected: Cell::new(false),
    };
    // Screen decoded text before calling owners; raw matching misses both JSON escape layers.
    let received = super::response::admit_http(raw, elapsed, &mut screened, &mut diagnostic)?;
    if screened.reflected.get()
        || received
            .usage
            .reported_model
            .as_deref()
            .is_some_and(|s| s.contains(key))
    {
        return Err("provider_unsafe_response");
    }
    Ok(received)
}

fn transport_current(
    until: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
    basis: &mut dyn ProposalBasis,
) -> bool {
    until > Instant::now()
        && !cancelled.load(std::sync::atomic::Ordering::Acquire)
        && basis.current_authority_and_premises().is_ok()
}
fn post(h: &mut Easy, b: &BoundCall, key: &str) -> Result<(), curl::Error> {
    h.url("https://api.deepseek.com/responses")?;
    h.post_fields_copy(&b.body)?;
    h.http_headers(authorization_headers(key)?)
}

/// The autonomous lane uses the same wire/TLS/admission boundary, but no SQL in curl callbacks.
pub struct AutonomousProvider {
    prepared: Option<(Easy, String, String)>,
    permit: Permit,
    pre_send: Box<dyn Fn() -> Res<()> + Send + Sync>,
    clock: Option<CallClock>,
    admission_guard: Option<TransportGuard>,
    reporter: Option<std::sync::mpsc::SyncSender<ReportedGuard>>,
}
impl AutonomousProvider {
    pub fn new(permit: Permit, pre_send: impl Fn() -> Res<()> + Send + Sync + 'static) -> Self {
        Self {
            prepared: None,
            permit,
            pre_send: Box::new(pre_send),
            clock: None,
            admission_guard: None,
            reporter: None,
        }
    }
    pub fn with_reporter(
        mut self,
        reporter: Option<std::sync::mpsc::SyncSender<ReportedGuard>>,
    ) -> Self {
        self.reporter = reporter;
        self
    }
}
fn autonomous_binding(b: &BoundCall) -> Res<()> {
    let p: serde_json::Value = serde_json::from_slice(&b.pack).map_err(|_| "request_encode")?;
    let c: serde_json::Value =
        serde_json::from_slice(&b.configuration).map_err(|_| "request_encode")?;
    let profile = transport_profile(&p)?;
    let valid = [
        p["execution_mode"] == "controlled_fixture",
        c["run_profile"] == p["run_profile"],
        c["execution_mode"] == p["execution_mode"],
        c["profile"] == profile,
        c["recipient"] == "https://api.deepseek.com/responses",
    ];
    if !valid.into_iter().all(|v| v) {
        return Err("inference_transport_unavailable");
    }
    verify_wire_binding(b)
}
fn transport_profile(p: &serde_json::Value) -> Res<&'static str> {
    match (
        p["version"].as_str(),
        p["run_profile"].as_str(),
        p["inference_profile"].as_str(),
    ) {
        (
            Some("m1_context_pack_autonomous_v1"),
            Some("autonomous-dns-provider-fixture-v1"),
            None,
        ) => Ok("deepseek-responses-autonomous-v1"),
        (
            Some("m1_context_pack_autonomous_v2"),
            Some("autonomous-dns-fixture-v2"),
            Some("deepseek-responses-reasoning-v1"),
        ) => Ok("deepseek-responses-reasoning-v1"),
        _ => Err("inference_transport_unavailable"),
    }
}
fn verify_wire_binding(b: &BoundCall) -> Res<()> {
    let body: serde_json::Value = serde_json::from_slice(&b.body).map_err(|_| "request_encode")?;
    super::request::verify_binding(
        std::str::from_utf8(&b.pack).map_err(|_| "request_encode")?,
        std::str::from_utf8(&b.configuration).map_err(|_| "request_encode")?,
        std::str::from_utf8(&b.body).map_err(|_| "request_encode")?,
        &b.digest,
        body["input"].as_array().is_some_and(|v| v.len() == 2),
    )
}
impl Provider for AutonomousProvider {
    fn prepare(&mut self, b: &BoundCall, until: Instant) -> Res<()> {
        autonomous_binding(b)?;
        if self.prepared.is_some() {
            return Err("inference_transport_unavailable");
        }
        let clock = self.prepare_clock(b, until)?;
        if !self.permit.current() {
            return Err("current_authority_unavailable");
        }
        let key = provider_key()?;
        let handle = configure_prepared(b, &key, until, clock.as_ref())?;
        self.prepared = Some((handle, key, b.digest.clone()));
        self.clock = clock;
        // Replace the prior admission guard only after the next call guard is armed.
        self.admission_guard = None;
        Ok(())
    }
    fn call(
        &mut self,
        b: &BoundCall,
        until: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Received> {
        if self.clock.is_some() {
            return self.call_successor(b, until, basis, release);
        }
        let (handle, key, call_end) = self.take_transfer(b, until)?;
        let _guard = TransportGuard::arm(self.permit.clone(), call_end)
            .map_err(|_| "inference_transport_unavailable")?;
        let received = self.exchange_owned(handle, call_end, release)?;
        self.admit_received(received, &key, call_end, basis)
    }
}
fn configure_prepared(
    b: &BoundCall,
    key: &str,
    until: Instant,
    clock: Option<&CallClock>,
) -> Res<Easy> {
    let now = Instant::now();
    let left = until
        .checked_duration_since(now)
        .ok_or("inference_deadline")?
        .min(clock.map_or(Duration::from_millis(20000), |c| {
            c.soft.saturating_duration_since(now)
        }));
    configure(b, key, left).map_err(|_| "inference_transport_unavailable")
}
impl AutonomousProvider {
    fn prepare_clock(&self, b: &BoundCall, until: Instant) -> Res<Option<CallClock>> {
        let config: serde_json::Value =
            serde_json::from_slice(&b.configuration).map_err(|_| "request_encode")?;
        if config["profile"] != "deepseek-responses-reasoning-v1" {
            return Ok(None);
        }
        CallClock::arm(self.permit.clone(), until, self.reporter.clone()).map(Some)
    }
    fn call_successor(
        &mut self,
        b: &BoundCall,
        until: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Received> {
        let (handle, key, clock) = self.take_successor_transfer(b, until)?;
        let transfer = self.exchange_owned(handle, clock.soft, release)?;
        if Instant::now() >= clock.soft {
            return Err("provider_cancelled_uncertain");
        }
        let elapsed = clock.origin.elapsed();
        // A complete pre-soft body may screen/account under the original decision clock.
        self.admission_guard = Some(TransportGuard::arm_admission(
            self.permit.clone(),
            until,
            self.reporter.clone(),
        )?);
        drop(clock);
        let mut received = self.admit_received(transfer, &key, until, basis)?;
        received.usage.wall_time_ms = elapsed.as_millis() as u64;
        Ok(received)
    }
    fn take_successor_transfer(
        &mut self,
        b: &BoundCall,
        until: Instant,
    ) -> Res<(Easy, String, CallClock)> {
        let clock = self.clock.take().ok_or("inference_transport_unavailable")?;
        let (mut handle, key, digest) = self
            .prepared
            .take()
            .ok_or("inference_transport_unavailable")?;
        if digest != b.digest {
            return Err("inference_decision_integrity");
        }
        let left = clock.remaining(&self.permit, until)?;
        timeouts(&mut handle, left).map_err(|_| "inference_transport_unavailable")?;
        Ok((handle, key, clock))
    }

    fn take_transfer(&mut self, b: &BoundCall, until: Instant) -> Res<(Easy, String, Instant)> {
        let (mut handle, key, digest) = self
            .prepared
            .take()
            .ok_or("inference_transport_unavailable")?;
        if digest != b.digest || !self.permit.current() {
            return Err("current_authority_unavailable");
        }
        let now = Instant::now();
        let left = until
            .checked_duration_since(now)
            .ok_or("inference_deadline")?
            .min(Duration::from_millis(20000));
        timeouts(&mut handle, left).map_err(|_| "inference_transport_unavailable")?;
        Ok((handle, key, now + left))
    }
    fn exchange_owned(
        &mut self,
        mut handle: Easy,
        call_end: Instant,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Exchange> {
        std::thread::scope(|scope| {
            let permit = self.permit.clone();
            let pre_send = &self.pre_send;
            let worker = std::thread::Builder::new()
                .name("autonomous_inference_transport".into())
                .spawn_scoped(scope, move || {
                    // Marker-only I/O happens once outside curl callbacks, with
                    // the independent guard already armed. A late successful
                    // read cannot revive a stopped/expired permit or call.
                    if pre_send().is_err() {
                        permit.stop();
                        return Err("provider_cancelled_uncertain");
                    }
                    if !permit.current() || Instant::now() >= call_end {
                        return Err("provider_cancelled_uncertain");
                    }
                    exchange(&mut handle, &mut || {
                        permit.current() && Instant::now() < call_end
                    })
                })
                .map_err(|_| "inference_transport_unavailable")?;
            let opened = release();
            if opened.is_err() {
                self.permit.stop();
            }
            // Always collect the owned worker; a saved release error leads join failure.
            let joined = worker.join();
            opened?;
            joined.map_err(|_| "provider_transport_unknown")?
        })
    }
    fn admit_received(
        &self,
        received: Exchange,
        key: &str,
        call_end: Instant,
        basis: &mut dyn ProposalBasis,
    ) -> Res<Received> {
        if !self.permit.current() || Instant::now() >= call_end {
            return Err("provider_cancelled_uncertain");
        }
        // Fresh owner/reference checks remain in proposal admission, outside the curl worker.
        let (result, raw, oversized, elapsed) = received;
        let received = finish_transfer(result, raw, oversized, key, elapsed, basis)?;
        if !self.permit.current() {
            return Err("provider_cancelled_uncertain");
        }
        Ok(received)
    }
}

#[cfg(test)]
#[path = "../../tests/support/m1_native_transport.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/support/m1_autonomous_transport.rs"]
mod autonomous_tests;

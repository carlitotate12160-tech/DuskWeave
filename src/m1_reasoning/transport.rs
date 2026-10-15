//! Synchronous in-process transport: no shell, child process, trace or raw receipt.
use super::inference::{Provider, Received};
use super::proposal::ProposalBasis;
use super::provider_diagnostic::ProviderDiagnostic;
use super::request::BoundCall;
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
        let key = std::env::var("DEEPSEEK_API_KEY").map_err(|_| "live_key_missing")?;
        if key.is_empty() || key.chars().any(|c| c.is_control()) {
            return Err("live_key_invalid");
        }
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
            let result = worker.join().map_err(|_| "provider_transport_unknown")?;
            opened?;
            result
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
        t.progress_function(|_, _, _, _| {
            until > Instant::now()
                && !cancelled.load(std::sync::atomic::Ordering::Acquire)
                && basis.current_authority_and_premises().is_ok()
        })
        .map_err(|_| "inference_transport_unavailable")?;
        t.perform()
    };
    if oversized.get() {
        return Err("provider_response_limit");
    }
    if result.is_err() {
        return Err("provider_transport_unknown");
    }
    let raw = raw.into_inner();
    if raw.windows(key.len()).any(|v| v == key.as_bytes()) {
        return Err("provider_unsafe_response");
    }
    let mut diagnostic = ProviderDiagnostic::default();
    super::response::admit_http(&raw, start.elapsed(), basis, &mut diagnostic)
}

fn post(h: &mut Easy, b: &BoundCall, key: &str) -> Result<(), curl::Error> {
    h.url("https://api.deepseek.com/responses")?;
    h.post_fields_copy(&b.body)?;
    h.http_headers(authorization_headers(key)?)
}

#[cfg(test)]
#[path = "../../tests/support/m1_native_transport.rs"]
mod tests;

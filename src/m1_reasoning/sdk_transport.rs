//! Trusted non-streaming SDK adapter. No future is polled before owned release.
use super::inference::{Provider, ProviderReport, Received};
use super::inference_intent;
use super::proposal::ProposalBasis;
use super::provider_diagnostic::ProviderDiagnostic;
use super::request::BoundCall;
use super::transport_control::{CallClock, Permit, ReportedGuard, TransportGuard};
use crate::m1_inference_profile::{self as profiles, Backend};
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatResponseFormat, JsonSpec, ReasoningEffort,
};
use genai::resolver::{AuthData, Endpoint};
use genai::{Client, ModelIden, ModelSpec, ObservedChatResponse, ServiceTarget};
use serde_json::Value;
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};
type Res<T> = Result<T, &'static str>;
const FIXTURE_KEY: &str = "duskweave-synthetic-sdk-fixture-key";

struct Prepared {
    client: Client,
    target: ServiceTarget,
    request: ChatRequest,
    options: ChatOptions,
    key: String,
    digest: String,
}
pub struct SdkProvider {
    prepared: Option<Prepared>,
    permit: Permit,
    pre_send: Box<dyn Fn() -> Res<()> + Send + Sync>,
    clock: Option<CallClock>,
    admission_guard: Option<TransportGuard>,
    reporter: Option<SyncSender<ReportedGuard>>,
    diagnostic: ProviderDiagnostic,
    request_entity_bytes: Option<usize>,
}
impl SdkProvider {
    pub fn new(permit: Permit, pre_send: impl Fn() -> Res<()> + Send + Sync + 'static) -> Self {
        Self {
            prepared: None,
            permit,
            pre_send: Box::new(pre_send),
            clock: None,
            admission_guard: None,
            reporter: None,
            diagnostic: ProviderDiagnostic::default(),
            request_entity_bytes: None,
        }
    }
    pub fn with_reporter(mut self, reporter: Option<SyncSender<ReportedGuard>>) -> Self {
        self.reporter = reporter;
        self
    }
}
fn http_builder(left: Duration) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(left.min(Duration::from_secs(5)))
        .timeout(left)
}
fn sdk_client(left: Duration) -> Res<Client> {
    let http = http_builder(left)
        .build()
        .map_err(|_| "inference_transport_unavailable")?;
    Client::builder()
        .with_reqwest(http)
        .build()
        .map_err(|_| "inference_transport_unavailable")
}
fn infrastructure_key(profile: profiles::Profile) -> Res<String> {
    let Some(variable) = profile.connection.credential_environment else {
        return (profile.name == "fixture-genai-responses-v1")
            .then(|| FIXTURE_KEY.to_owned())
            .ok_or("inference_unsupported_profile");
    };
    let key = std::env::var(variable).map_err(|_| "live_key_missing")?;
    if key.is_empty() || key.chars().any(|c| c.is_control()) {
        return Err("live_key_invalid");
    }
    Ok(key)
}
fn service_target(intent: &Value) -> Res<(ServiceTarget, String)> {
    let profile = profiles::lookup(intent["profile"].as_str().ok_or("request_encode")?)
        .map_err(|_| "request_encode")?;
    if profile.backend != Backend::Genai {
        return Err("request_encode");
    }
    let key = infrastructure_key(profile)?;
    let endpoint = profile
        .connection
        .recipient
        .strip_suffix("responses")
        .ok_or("inference_unsupported_profile")?;
    let target = ServiceTarget {
        endpoint: Endpoint::from_static(endpoint),
        auth: AuthData::from_single(key.clone()),
        model: ModelIden::new(AdapterKind::OpenAIResp, "deepseek-flash"),
    };
    Ok((target, key))
}
fn chat_input(bound: &BoundCall, intent: &Value) -> Res<(ChatRequest, ChatOptions)> {
    let context = std::str::from_utf8(&bound.pack).map_err(|_| "pack_utf8")?;
    let mut request = ChatRequest::from_user(context)
        .with_system(intent["instructions"].as_str().ok_or("request_encode")?);
    request.store = Some(false);
    if intent["call_role"] == "repair" {
        let repair = serde_json::json!({"repair_error_codes":intent["repair_error_codes"]});
        request.messages.push(ChatMessage::user(
            serde_json::to_string(&repair).map_err(|_| "repair_encode")?,
        ));
    }
    let options = ChatOptions {
        reasoning_effort: Some(ReasoningEffort::Low),
        max_tokens: Some(4096),
        response_format: Some(ChatResponseFormat::JsonSpec(JsonSpec::new(
            "m1_inference_proposal_v1",
            intent["schema"].clone(),
        ))),
        capture_raw_body: Some(false),
        ..Default::default()
    };
    Ok((request, options))
}
fn prepare_call(bound: &BoundCall, intent: &Value, left: Duration) -> Res<Prepared> {
    let (target, key) = service_target(intent)?;
    let (request, options) = chat_input(bound, intent)?;
    Ok(Prepared {
        client: sdk_client(left)?,
        target,
        request,
        options,
        key,
        digest: bound.digest.clone(),
    })
}
impl Provider for SdkProvider {
    fn terminal_report(&self, digest: &str) -> Option<&ProviderReport> {
        (self.diagnostic.binding_digest == digest)
            .then_some(self.diagnostic.report.as_ref())
            .flatten()
    }
    fn prepare(&mut self, bound: &BoundCall, until: Instant) -> Res<()> {
        if self.prepared.is_some() {
            return Err("inference_transport_unavailable");
        }
        let intent = inference_intent::verify(bound)?;
        let clock = CallClock::arm(self.permit.clone(), until, self.reporter.clone())?;
        let left = clock.remaining(&self.permit, until)?;
        self.prepared = Some(prepare_call(bound, &intent, left)?);
        self.clock = Some(clock);
        self.admission_guard = None;
        Ok(())
    }
    fn call(
        &mut self,
        bound: &BoundCall,
        until: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Received> {
        self.diagnostic = ProviderDiagnostic::new(bound.digest.clone());
        self.request_entity_bytes = None;
        let (prepared, clock) = self.release_ready(bound, until)?;
        let (observed, key) = self.exchange_owned(prepared, clock.soft, release)?;
        if !self.permit.current() || Instant::now() >= clock.soft {
            return Err("provider_cancelled_uncertain");
        }
        self.admission_guard = Some(TransportGuard::arm_admission(
            self.permit.clone(),
            until,
            self.reporter.clone(),
        )?);
        let elapsed = clock.origin.elapsed();
        drop(clock);
        self.admit(observed, &key, elapsed, basis)
    }
}
impl SdkProvider {
    fn release_ready(&mut self, bound: &BoundCall, until: Instant) -> Res<(Prepared, CallClock)> {
        let prepared = self
            .prepared
            .take()
            .ok_or("inference_transport_unavailable")?;
        let clock = self.clock.take().ok_or("inference_transport_unavailable")?;
        clock.remaining(&self.permit, until)?;
        if prepared.digest != bound.digest {
            return Err("inference_decision_integrity");
        }
        // PgBroker still owns the campaign session fence during this final local
        // current check and launch. Its release closure only unlocks, never waits.
        (self.pre_send)()?;
        if !self.permit.current() {
            return Err("provider_cancelled_uncertain");
        }
        Ok((prepared, clock))
    }
    fn exchange_owned(
        &self,
        prepared: Prepared,
        end: Instant,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<(ObservedChatResponse, String)> {
        std::thread::scope(|scope| {
            let permit = self.permit.clone();
            let worker = std::thread::Builder::new()
                .name("genai_inference_transport".into())
                .spawn_scoped(scope, move || exchange(prepared, permit, end))
                .map_err(|_| "inference_transport_unavailable")?;
            let opened = release();
            if opened.is_err() {
                self.permit.stop();
            }
            let joined = worker.join();
            opened?;
            joined.map_err(|_| "provider_transport_unknown")?
        })
    }
    fn project_evidence(&mut self, evidence: &genai::ChatCallEvidence) {
        self.request_entity_bytes = evidence.request_entity_bytes;
        self.diagnostic.final_http_status = evidence.http_status;
        if let Some(bytes) = evidence.response_entity_bytes {
            self.diagnostic.response_bytes = bytes;
        }
    }
    fn admit(
        &mut self,
        observed: ObservedChatResponse,
        key: &str,
        elapsed: Duration,
        basis: &mut dyn ProposalBasis,
    ) -> Res<Received> {
        self.project_evidence(&observed.evidence);
        let raw = observed
            .evidence
            .raw_response
            .as_deref()
            .unwrap_or_default();
        if raw.windows(key.len()).any(|v| v == key.as_bytes()) {
            return Err("provider_unsafe_response");
        }
        if observed
            .evidence
            .http_status
            .is_some_and(|status| status != 200)
        {
            return Err("provider_protocol");
        }
        let content = observed.result.as_ref().ok().and_then(|r| r.first_text());
        let decoded = observed.result.is_ok();
        let error = observed.result.as_ref().err().map(classify_error);
        if let Some(code) = error
            && (code != "provider_protocol" || raw.is_empty())
        {
            return Err(code);
        }
        if !self
            .request_entity_bytes
            .is_some_and(|n| (1..=16384).contains(&n))
        {
            return Err("request_size");
        }
        super::response::screened(key, basis, &mut self.diagnostic, |screened, diagnostic| {
            super::response::admit_sdk(raw, content, decoded, elapsed, screened, diagnostic)
        })
    }
}
fn classify_error(error: &genai::Error) -> &'static str {
    match error {
        genai::Error::WebModelCall {
            webc_error: genai::webc::Error::ResponseEntityLimit { .. },
            ..
        } => "provider_response_limit",
        genai::Error::WebModelCall {
            webc_error: genai::webc::Error::RequestEntityLimit { .. },
            ..
        } => "request_size",
        genai::Error::WebModelCall {
            webc_error: genai::webc::Error::Reqwest(_),
            ..
        } => "provider_transport_unknown",
        _ => "provider_protocol",
    }
}
async fn cancelled(permit: &Permit, end: Instant) {
    while permit.current() && Instant::now() < end {
        tokio::time::sleep(
            Duration::from_millis(5).min(end.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}
fn exchange(
    prepared: Prepared,
    permit: Permit,
    end: Instant,
) -> Res<(ObservedChatResponse, String)> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "inference_transport_unavailable")?;
    let result = runtime.block_on(async {
        tokio::select! { biased;
            _ = cancelled(&permit, end) => None,
            result = prepared.client.exec_chat_observed(ModelSpec::Target(prepared.target), prepared.request, Some(&prepared.options)) => Some(result),
        }
    });
    result
        .map(|result| (result, prepared.key))
        .ok_or("provider_cancelled_uncertain")
}

#[cfg(test)]
#[path = "../../tests/support/m1_genai_transport.rs"]
mod tests;

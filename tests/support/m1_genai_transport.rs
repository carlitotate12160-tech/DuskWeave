//! Private tests exercise the same SDK client builder and per-call evidence.
use super::*;
#[path = "m1_genai_fixture.rs"]
mod fixture;
use fixture::{Reply, Server};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

fn target(address: std::net::SocketAddr) -> ServiceTarget {
    ServiceTarget {
        endpoint: Endpoint::from_owned(format!("http://{address}/")),
        auth: AuthData::from_single("synthetic-key-private-571"),
        model: ModelIden::new(AdapterKind::OpenAIResp, "deepseek-flash"),
    }
}
fn observed(server: &Server, request: &str, capture: bool) -> ObservedChatResponse {
    observed_with(
        sdk_client(Duration::from_secs(3)).unwrap(),
        target(server.address),
        request,
        capture,
    )
}
fn observed_with(
    client: Client,
    target: ServiceTarget,
    request: &str,
    capture: bool,
) -> ObservedChatResponse {
    let options = ChatOptions {
        capture_raw_body: Some(capture),
        ..Default::default()
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(client.exec_chat_observed(
            ModelSpec::Target(target),
            ChatRequest::from_user(request),
            Some(&options),
        ))
}
fn success_body() -> Vec<u8> {
    serde_json::to_vec(&fixture::envelope(
        &json!({"synthetic":"response-entity-571"}),
    ))
    .unwrap()
}
#[test]
fn observed_counts_are_actual_utf8_entities_and_not_shared_between_calls() {
    let body = success_body();
    let expected = body.len();
    let server = Server::start("127.0.0.1:0", move |_| Reply::Chunked(body.clone(), 7));
    for request in ["ASCII", "\u{65e5}\u{672c}\u{8a9e}\u{1f642}"] {
        let result = observed(&server, request, false);
        assert!(result.result.is_ok());
        assert_eq!(result.evidence.http_status, Some(200));
        assert_eq!(result.evidence.response_entity_bytes, Some(expected));
        let requests = server.requests.lock().unwrap();
        assert_eq!(
            result.evidence.request_entity_bytes,
            Some(requests.last().unwrap().body.len())
        );
        let wire: Value = serde_json::from_slice(&requests.last().unwrap().body).unwrap();
        assert!(wire.to_string().contains(request));
    }
    assert_eq!(server.count(), 2);
    let evidence = genai::ChatCallEvidence::default();
    assert!(evidence.request_entity_bytes.is_none() && evidence.response_entity_bytes.is_none());
}
#[test]
fn request_cap_is_utf8_bytes_and_prevents_send() {
    let server = Server::start("127.0.0.1:0", |_| Reply::Length(200, success_body()));
    let request = "\u{754c}".repeat(6000);
    let result = observed(&server, &request, false);
    assert_eq!(
        classify_error(result.result.as_ref().err().unwrap()),
        "request_size"
    );
    assert!(
        result
            .evidence
            .request_entity_bytes
            .is_some_and(|n| n > 16384)
    );
    assert!(
        result.evidence.http_status.is_none() && result.evidence.response_entity_bytes.is_none()
    );
    assert_eq!(server.count(), 0);
}
struct Basis;
impl ProposalBasis for Basis {
    fn reference_eligible(&self, _: &str) -> bool {
        true
    }
    fn manifest_eligible(&self, _: &str) -> bool {
        true
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        Ok(())
    }
    fn safe_explanation(&self, _: &str) -> bool {
        true
    }
}
fn provider() -> SdkProvider {
    SdkProvider::new(
        Permit::new(Instant::now() + Duration::from_secs(10)),
        || Ok(()),
    )
}
#[test]
fn admission_preserves_incomplete_unknown_and_zero_and_rejects_decoded_reflection() {
    let pack = json!({"references":[],"predecessor":{},"candidates":[{"reference":"asset_0"}]});
    let proposal = fixture::proposal(&pack);
    for zero in [false, true] {
        let mut v = fixture::envelope(&proposal);
        if !zero {
            v.as_object_mut().unwrap().remove("usage");
        }
        let bytes = serde_json::to_vec(&v).unwrap();
        let server = Server::start("127.0.0.1:0", move |_| Reply::Length(200, bytes.clone()));
        let mut adapter = provider();
        let observation = observed(&server, "context", false);
        let predicates = match observation.result.as_ref().err() {
            Some(genai::Error::WebModelCall {
                webc_error: genai::webc::Error::Reqwest(error),
                ..
            }) => Some((
                error.is_connect(),
                error.is_timeout(),
                error.is_request(),
                error.is_body(),
                error.is_decode(),
            )),
            _ => None,
        };
        eprintln!(
            "safe_local_observation zero={zero} sends={} status={:?} request={:?} response={:?} reqwest_connect_timeout_request_body_decode={predicates:?} wire={:?}",
            server.count(),
            observation.evidence.http_status,
            observation.evidence.request_entity_bytes,
            observation.evidence.response_entity_bytes,
            server.wire.lock().unwrap()
        );
        let received = adapter
            .admit(
                observation,
                "synthetic-key-private-571",
                Duration::ZERO,
                &mut Basis,
            )
            .unwrap();
        assert!(matches!(
            received.admission,
            crate::m1_reasoning::proposal::Admission::Accepted(_)
        ));
        assert_eq!(received.usage.input_tokens, zero.then_some(0));
        assert_eq!(received.usage.output_tokens, zero.then_some(0));
    }
    for (key, field) in [
        ("synthetic-key-private-571", "hypothesis"),
        ("synthetic-key-private-571", "rationale"),
    ] {
        let mut p = proposal.clone();
        p[field] = json!(key);
        let body = serde_json::to_string(&fixture::envelope(&p))
            .unwrap()
            .replace('s', "\\u0073");
        let server = Server::start("127.0.0.1:0", move |_| {
            Reply::Length(200, body.as_bytes().to_vec())
        });
        let result = provider().admit(
            observed(&server, "context", false),
            key,
            Duration::ZERO,
            &mut Basis,
        );
        assert!(matches!(result, Err("provider_unsafe_response")));
    }
    let mut v = fixture::envelope(&proposal);
    v["status"] = json!("incomplete");
    let body = serde_json::to_vec(&v).unwrap();
    let server = Server::start("127.0.0.1:0", move |_| Reply::Length(200, body.clone()));
    let mut p = provider();
    let result = p.admit(
        observed(&server, "context", false),
        "synthetic-key-private-571",
        Duration::ZERO,
        &mut Basis,
    );
    assert!(matches!(result, Err("provider_incomplete")));
    assert!(p.diagnostic.report.is_some());
}
#[test]
fn cancellation_drops_inflight_sdk_future_without_retry_and_preserves_no_send_before_poll() {
    let held = Arc::new(AtomicBool::new(false));
    let waiting = held.clone();
    let server = Server::start("127.0.0.1:0", move |_| Reply::Hold(waiting.clone()));
    let until = Instant::now() + Duration::from_secs(5);
    let permit = Permit::new(until);
    assert!(permit.publish(Instant::now(), until));
    let prepared = Prepared {
        client: sdk_client(Duration::from_secs(4)).unwrap(),
        target: target(server.address),
        request: ChatRequest::from_user("cancel-synthetic"),
        options: ChatOptions::default(),
        key: "synthetic".into(),
        digest: String::new(),
    };
    assert_eq!(server.count(), 0, "preparation must not poll network");
    std::thread::scope(|scope| {
        let owned = permit.clone();
        let worker = scope.spawn(move || exchange(prepared, owned, until));
        server.await_count(1);
        let start = Instant::now();
        permit.stop();
        assert!(matches!(
            worker.join().unwrap(),
            Err("provider_cancelled_uncertain")
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
    });
    held.store(true, Ordering::SeqCst);
    assert_eq!(server.count(), 1);
}

#[test]
fn stopped_permit_prevents_sdk_polling_and_send() {
    let server = Server::start("127.0.0.1:0", |_| Reply::Reset);
    let end = Instant::now() + Duration::from_secs(3);
    let permit = Permit::new(end);
    permit.stop();
    let prepared = Prepared {
        client: sdk_client(Duration::from_secs(2)).unwrap(),
        target: target(server.address),
        request: ChatRequest::from_user("synthetic stopped request"),
        options: ChatOptions::default(),
        key: "synthetic".into(),
        digest: String::new(),
    };
    assert!(matches!(
        exchange(prepared, permit, end),
        Err("provider_cancelled_uncertain")
    ));
    assert_eq!(server.count(), 0);
}
#[test]
fn stalled_current_thread_runtime_keeps_independent_clock_termination() {
    if std::env::var_os("DW_TEST_SDK_STALLED_RUNTIME").is_some() {
        let end = Instant::now() + Duration::from_secs(124);
        let permit = Permit::new(end);
        assert!(permit.publish(Instant::now(), end));
        let _clock = CallClock::arm(permit, end, None).unwrap();
        println!("sdk_stall_clock_armed");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                std::thread::park();
            });
        panic!("independent clock must terminate a noncooperative runtime");
    }
    let start = Instant::now();
    let child=std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact","m1_reasoning::sdk_transport::tests::stalled_current_thread_runtime_keeps_independent_clock_termination","--nocapture"])
        .env("DW_TEST_SDK_STALLED_RUNTIME","1").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let mut child = fixture::FixtureChild::new(child);
    let end = start + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    let terminated = child.try_wait().unwrap();
    if terminated.is_none() {
        let _ = child.kill();
    }
    let output = child.output();
    assert!(terminated.is_some() && start.elapsed() < Duration::from_secs(5));
    assert_eq!(output.status.code(), Some(124));
    assert!(String::from_utf8_lossy(&output.stdout).contains("sdk_stall_clock_armed"));
    // Exit plus the armed-clock marker prove bounded termination. No exact
    // guard-winner claim is made without a delivered closed guard report.
}

#[path = "m1_genai_transport/entities.rs"]
mod entities;
#[path = "m1_genai_transport/errors.rs"]
mod errors;
#[path = "m1_genai_transport/tls.rs"]
mod tls;

// Keep externally selected test identities and libtest ordering stable.
#[test]
fn typed_conversion_constructor_discards_request_and_rejected_values_for_both_capture_modes() {
    errors::typed_conversion_constructor_discards_request_and_rejected_values_for_both_capture_modes();
}

#[test]
fn trace_capture_independently_covers_success_http_json_and_typed_failure() {
    errors::trace_capture_independently_covers_success_http_json_and_typed_failure();
}

#[test]
fn same_http_builder_verifies_trusted_certificate_hostname_and_untrusted_roots() {
    tls::same_http_builder_verifies_trusted_certificate_hostname_and_untrusted_roots();
}

#[test]
fn tls_fixture_assertion_cleanup_reaps_server_and_removes_private_material() {
    tls::tls_fixture_assertion_cleanup_reaps_server_and_removes_private_material();
}

#[test]
fn response_caps_cover_success_error_chunked_and_close_delimited_entities() {
    entities::response_caps_cover_success_error_chunked_and_close_delimited_entities();
}

#[test]
fn exact_cap_status_invalid_json_redirect_and_reset_preserve_safe_evidence() {
    entities::exact_cap_status_invalid_json_redirect_and_reset_preserve_safe_evidence();
}

#[test]
fn fragmented_request_waits_for_remaining_bytes_without_closing_the_connection() {
    entities::fragmented_request_waits_for_remaining_bytes_without_closing_the_connection();
}

#[test]
fn exact_request_cap_and_first_illegal_byte_use_real_multibyte_serialization() {
    entities::exact_request_cap_and_first_illegal_byte_use_real_multibyte_serialization();
}

#[test]
fn maximum_pack_and_eight_repair_codes_use_observed_sdk_entity_bytes() {
    entities::maximum_pack_and_eight_repair_codes_use_observed_sdk_entity_bytes();
}

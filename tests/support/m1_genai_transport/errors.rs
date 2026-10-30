//! Private SDK errors controls with the same client and fixture.
use super::*;

fn no_sensitive(value: &str, secrets: &[&str]) {
    assert!(
        secrets.iter().all(|secret| !value.contains(secret)),
        "formatted output must exclude synthetic sensitive material"
    );
}
pub(super) fn assert_safe_error(result: ObservedChatResponse, secrets: &[&str], constructor: bool) {
    let error = match result.result {
        Err(error) => error,
        Ok(_) => panic!("protocol failure must remain an error"),
    };
    // Unsafe formatting exists only in memory; assertion diagnostics never emit it.
    no_sensitive(&format!("{error}"), secrets);
    no_sensitive(&format!("{error:?}"), secrets);
    no_sensitive(&format!("{:?}", result.evidence), secrets);
    if constructor {
        match error {
            genai::Error::ChatResponseGeneration {
                cause,
                request_payload,
                response_body,
                ..
            } => {
                assert!(
                    cause == "Response decoding failed",
                    "static bounded cause required"
                );
                assert!(
                    request_payload.is_null() && response_body.is_null(),
                    "constructor must discard raw values"
                );
                no_sensitive(&cause, secrets);
            }
            _ => panic!("typed envelope must preserve conversion-error category"),
        }
    }
}
pub(super) fn typed_conversion_constructor_discards_request_and_rejected_values_for_both_capture_modes()
 {
    let request = "synthetic-request-context-571";
    let secret = "synthetic-rejected-value-571";
    for capture in [false, true] {
        for field in ["input_tokens", "output_tokens"] {
            let mut envelope: Value = serde_json::from_slice(&success_body()).unwrap();
            envelope["usage"][field] = json!(secret);
            let bytes = serde_json::to_vec(&envelope).unwrap();
            let server = Server::start("127.0.0.1:0", move |_| Reply::Length(200, bytes.clone()));
            let result = observed(&server, request, capture);
            assert_eq!(
                result.evidence.response_entity_bytes,
                result.evidence.raw_response.as_ref().map(Vec::len)
            );
            assert_safe_error(result, &[request, secret], true);
            assert_eq!(server.count(), 1);
        }
    }
}
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}
pub(super) fn trace_capture_independently_covers_success_http_json_and_typed_failure() {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .without_time()
        .with_ansi(false)
        .with_writer(capture.clone())
        .finish();
    let mut typed: Value = serde_json::from_slice(&success_body()).unwrap();
    typed["usage"]["input_tokens"] = json!("synthetic-typed-private-571");
    let cases = [
        (200, success_body()),
        (503, b"synthetic-http-private-571".to_vec()),
        (200, b"{synthetic-json-private-571".to_vec()),
        (200, serde_json::to_vec(&typed).unwrap()),
    ];
    tracing::subscriber::with_default(subscriber, || {
        tracing::trace!("local_trace_capture_active");
        for (status, bytes) in cases {
            let server =
                Server::start("127.0.0.1:0", move |_| Reply::Length(status, bytes.clone()));
            let _ = observed(&server, "synthetic-context-private-571", true);
            assert_eq!(server.count(), 1);
        }
    });
    let bytes = capture.0.lock().unwrap();
    let logs = String::from_utf8_lossy(&bytes);
    assert!(logs.contains("local_trace_capture_active"));
    no_sensitive(
        &logs,
        &[
            "synthetic-context-private-571",
            "synthetic-typed-private-571",
            "synthetic-http-private-571",
            "synthetic-json-private-571",
            "synthetic-key-private-571",
            "response-entity-571",
        ],
    );
}

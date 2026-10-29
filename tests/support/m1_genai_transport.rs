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
fn no_sensitive(value: &str, secrets: &[&str]) {
    assert!(
        secrets.iter().all(|secret| !value.contains(secret)),
        "formatted output must exclude synthetic sensitive material"
    );
}
fn assert_safe_error(result: ObservedChatResponse, secrets: &[&str], constructor: bool) {
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
#[test]
fn typed_conversion_constructor_discards_request_and_rejected_values_for_both_capture_modes() {
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
#[test]
fn response_caps_cover_success_error_chunked_and_close_delimited_entities() {
    for (status, framing) in [(200, 0), (503, 0), (200, 1), (200, 2)] {
        let body = vec![b' '; 65537];
        let server = Server::start("127.0.0.1:0", move |_| match framing {
            1 => Reply::Chunked(body.clone(), 257),
            2 => Reply::Close(body.clone()),
            _ => Reply::Length(status, body.clone()),
        });
        let result = observed(&server, "bounded", false);
        assert_eq!(result.evidence.http_status, Some(status));
        assert!(
            result
                .evidence
                .response_entity_bytes
                .is_some_and(|n| n > 65536)
        );
        assert!(result.evidence.raw_response.is_none());
        assert_eq!(
            classify_error(result.result.as_ref().err().unwrap()),
            "provider_response_limit"
        );
        assert_eq!(server.count(), 1);
    }
}
#[test]
fn exact_cap_status_invalid_json_redirect_and_reset_preserve_safe_evidence() {
    let mut exact = success_body();
    exact.resize(65536, b' ');
    let server = Server::start("127.0.0.1:0", move |_| Reply::Length(200, exact.clone()));
    let result = observed(&server, "exact", false);
    assert!(result.result.is_ok());
    assert_eq!(result.evidence.response_entity_bytes, Some(65536));
    drop(server);
    for status in [400, 429, 503, 302] {
        let server = Server::start("127.0.0.1:0", move |_| {
            Reply::Length(status, b"synthetic-provider-error-571".to_vec())
        });
        let result = observed(&server, "context", true);
        assert_eq!(result.evidence.http_status, Some(status));
        assert_safe_error(result, &["synthetic-provider-error-571"], false);
        assert_eq!(server.count(), 1);
    }
    for reply in [
        Reply::Length(200, b"{synthetic-malformed-571".to_vec()),
        Reply::Misleading(b"{}".to_vec(), 50),
        Reply::Reset,
    ] {
        let mut once = Some(reply);
        let server = Server::start("127.0.0.1:0", move |_| once.take().expect("no retry"));
        assert_safe_error(
            observed(&server, "context", false),
            &["synthetic-malformed-571"],
            false,
        );
        assert_eq!(server.count(), 1);
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
#[test]
fn trace_capture_independently_covers_success_http_json_and_typed_failure() {
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
fn fragmented_request_waits_for_remaining_bytes_without_closing_the_connection() {
    use std::io::{Read, Write};
    let server = Server::start("127.0.0.1:0", |_| Reply::Length(200, b"{}".to_vec()));
    let mut client = std::net::TcpStream::connect(server.address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    client
        .write_all(b"POST /responses HTTP/1.1\r\nContent-Length: 2\r\n")
        .unwrap();
    let mut byte = [0];
    let read = client.read(&mut byte);
    assert!(
        matches!(read,Err(ref e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)),
        "fixture must wait for remaining request bytes: wire={:?}",
        server.wire.lock().unwrap()
    );
    client.write_all(b"\r\n{}").unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reply = Vec::new();
    client.read_to_end(&mut reply).unwrap();
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    assert_eq!(server.count(), 1);
    let wire = server.wire.lock().unwrap();
    assert_eq!(wire.read_complete, 1);
    assert_eq!(wire.write_complete, 1);
}

#[test]
fn exact_request_cap_and_first_illegal_byte_use_real_multibyte_serialization() {
    let server = Server::start("127.0.0.1:0", |_| Reply::Length(200, success_body()));
    let baseline = observed(&server, "base", false);
    let overhead = baseline.evidence.request_entity_bytes.unwrap() - 4;
    let content_bytes = 16384 - overhead;
    let exact = "\u{754c}".repeat(content_bytes / 3) + &"x".repeat(content_bytes % 3);
    let result = observed(&server, &exact, false);
    assert!(result.result.is_ok());
    assert_eq!(result.evidence.request_entity_bytes, Some(16384));
    assert_eq!(
        server.requests.lock().unwrap().last().unwrap().body.len(),
        16384
    );
    let illegal = observed(&server, &(exact + "x"), false);
    assert_eq!(illegal.evidence.request_entity_bytes, Some(16385));
    assert_eq!(
        classify_error(illegal.result.as_ref().err().unwrap()),
        "request_size"
    );
    assert_eq!(server.count(), 2, "first illegal byte must prevent send");
}

struct TlsFixture {
    dir: std::path::PathBuf,
    server: Option<fixture::FixtureChild>,
    events: Option<std::sync::mpsc::Receiver<String>>,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl TlsFixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dw-sdk-tls-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let fixture = Self {
            dir,
            server: None,
            events: None,
            reader: None,
        };
        fixture.openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "ca-key.pem",
            "-out",
            "ca.pem",
            "-subj",
            "/CN=DuskWeave synthetic fixture CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE,pathlen:0",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
            "-days",
            "1",
        ]);
        fixture.openssl(&[
            "req",
            "-new",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "key.pem",
            "-out",
            "leaf.csr",
            "-subj",
            "/CN=localhost",
        ]);
        std::fs::write(fixture.dir.join("leaf.ext"), "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\nauthorityKeyIdentifier=keyid,issuer\nsubjectKeyIdentifier=hash\n").unwrap();
        fixture.openssl(&[
            "x509",
            "-req",
            "-in",
            "leaf.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca-key.pem",
            "-CAcreateserial",
            "-out",
            "certificate.pem",
            "-extfile",
            "leaf.ext",
            "-days",
            "1",
            "-sha256",
        ]);
        let mut chain = std::fs::read(fixture.dir.join("certificate.pem")).unwrap();
        chain.extend(std::fs::read(fixture.dir.join("ca.pem")).unwrap());
        std::fs::write(fixture.dir.join("chain.pem"), chain).unwrap();
        fixture
    }
    fn openssl(&self, args: &[&str]) {
        let openssl = std::env::var_os("DW_TEST_OPENSSL").unwrap_or_else(|| "openssl".into());
        let mut child = fixture::FixtureChild::new(
            std::process::Command::new(openssl)
                .current_dir(&self.dir)
                .args(args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        assert!(
            child
                .wait_until(Instant::now() + Duration::from_secs(10))
                .is_some_and(|s| s.success()),
            "bounded synthetic certificate generation"
        );
    }
    fn start(&mut self) -> u16 {
        use std::io::BufRead;
        let script = r#"import socket,ssl,sys,pathlib
context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(sys.argv[1],sys.argv[2])
with socket.socket() as listener:
 listener.bind(('127.0.0.1',0));listener.listen(1);listener.settimeout(4)
 print(listener.getsockname()[1],flush=True)
 try:
  raw,_=listener.accept();raw.settimeout(3)
  with context.wrap_socket(raw,server_side=True) as stream:
   print('handshake',flush=True);data=b''
   while b'\r\n\r\n' not in data:
    part=stream.recv(4096)
    if not part: break
    data+=part
   if b'\r\n\r\n' in data:
    header,payload=data.split(b'\r\n\r\n',1)
    count=next((int(x.split(b':',1)[1]) for x in header.split(b'\r\n') if x.lower().startswith(b'content-length:')),0)
    while len(payload)<count:
     part=stream.recv(4096)
     if not part: break
     payload+=part
    if len(payload)!=count: raise ConnectionError()
    print('request:'+str(len(payload)),flush=True);response=pathlib.Path(sys.argv[3]).read_bytes()
    stream.sendall(b'HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: '+str(len(response)).encode()+b'\r\nConnection: close\r\n\r\n'+response)
    print('response',flush=True)
 except ssl.SSLError as e:
  print('tls_certificate_alert' if e.reason in ('SSLV3_ALERT_BAD_CERTIFICATE','TLSV1_ALERT_UNKNOWN_CA','TLSV1_ALERT_CERTIFICATE_UNKNOWN') else 'tls_handshake_refused',flush=True)
 except TimeoutError: print('timeout',flush=True)
 except (ConnectionError,OSError): print('connection',flush=True)
"#;
        std::fs::write(self.dir.join("response.json"), success_body()).unwrap();
        let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| {
            if cfg!(windows) {
                "python".into()
            } else {
                "python3".into()
            }
        });
        let mut child = fixture::FixtureChild::new(
            std::process::Command::new(python)
                .args(["-u", "-c", script])
                .arg(self.dir.join("chain.pem"))
                .arg(self.dir.join("key.pem"))
                .arg(self.dir.join("response.json"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let stdout = child.stdout.take().unwrap();
        self.server = Some(child);
        let (tx, rx) = std::sync::mpsc::channel();
        self.events = Some(rx);
        self.reader = Some(std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                if let Ok(line) = line {
                    let _ = tx.send(line);
                } else {
                    break;
                }
            }
        }));
        self.events
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .expect("TLS fixture readiness")
            .parse()
            .expect("TLS fixture port")
    }
    fn finish(&mut self) -> Vec<String> {
        assert!(
            self.server
                .as_mut()
                .unwrap()
                .wait_until(Instant::now() + Duration::from_secs(4))
                .is_some_and(|s| s.success()),
            "bounded TLS fixture exit"
        );
        self.server.take();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
        self.events.take().unwrap().try_iter().collect()
    }
}
impl Drop for TlsFixture {
    fn drop(&mut self) {
        self.server.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
fn observed_error_kind(result: &ObservedChatResponse) -> &'static str {
    match result.result.as_ref().err() {
        None => "success",
        Some(genai::Error::WebModelCall {
            webc_error: genai::webc::Error::Reqwest(e),
            ..
        }) => {
            if e.is_timeout() {
                "timeout"
            } else if e.is_connect() {
                "connect"
            } else if e.is_body() {
                "body"
            } else if e.is_decode() {
                "decode"
            } else {
                "request"
            }
        }
        _ => "protocol",
    }
}
#[test]
fn same_http_builder_verifies_trusted_certificate_hostname_and_untrusted_roots() {
    let mut fixture = TlsFixture::new();
    for (case, trusted, host, accepted) in [
        ("trusted_localhost", true, "localhost", true),
        ("name_mismatch", true, "127.0.0.1", false),
        ("untrusted", false, "localhost", false),
    ] {
        let port = fixture.start();
        let mut builder = http_builder(Duration::from_secs(3));
        if trusted {
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(&std::fs::read(fixture.dir.join("ca.pem")).unwrap())
                    .unwrap(),
            );
        }
        let client = Client::builder()
            .with_reqwest(builder.build().unwrap())
            .build()
            .unwrap();
        let mut service = target(([127, 0, 0, 1], port).into());
        service.endpoint = Endpoint::from_owned(format!("https://{host}:{port}/"));
        let result = observed_with(client, service, "synthetic TLS context", false);
        let events = fixture.finish();
        let requests = events.iter().filter(|e| e.starts_with("request:")).count();
        let handshake = events.iter().any(|e| e == "handshake");
        let certificate_alert = events.iter().any(|e| e == "tls_certificate_alert");
        let handshake_refused = events.iter().any(|e| e == "tls_handshake_refused");
        eprintln!(
            "safe_tls case={case} sdk={} handshake={handshake} certificate_alert={certificate_alert} handshake_refused={handshake_refused} requests={requests} status={:?} request_entity={:?} response_entity={:?}",
            observed_error_kind(&result),
            result.evidence.http_status,
            result.evidence.request_entity_bytes,
            result.evidence.response_entity_bytes
        );
        assert_eq!(
            result.result.is_ok(),
            accepted,
            "normal certificate and hostname verification: {case}"
        );
        assert_eq!(requests, usize::from(accepted));
        if accepted {
            assert_eq!(result.evidence.http_status, Some(200));
            let sent = events
                .iter()
                .find_map(|e| e.strip_prefix("request:"))
                .and_then(|n| n.parse::<usize>().ok());
            assert_eq!(sent, result.evidence.request_entity_bytes);
            assert_eq!(
                result.evidence.response_entity_bytes,
                Some(success_body().len())
            );
            assert!(result.evidence.response_entity_bytes.is_some_and(|n| n > 0));
        }
    }
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

#[test]
fn maximum_pack_and_eight_repair_codes_use_observed_sdk_entity_bytes() {
    use crate::m1_reasoning::inference::{Reservation, Scope};
    use uuid::Uuid;
    // Synthetic identities qualify the serializer only. Actual reserved identity
    // attachment and restricted authority are exercised by the PG target.
    let scope = Scope {
        engagement: Uuid::from_u128(1),
        campaign: Uuid::from_u128(2),
        decision: Uuid::from_u128(3),
        permission: Uuid::from_u128(4),
        mission_revision: 1,
    };
    let end = Instant::now() + Duration::from_secs(4);
    let r = Reservation {
        generation: i64::MAX as u64,
        generation_call: Uuid::from_u128(5),
        repair_call: Uuid::from_u128(6),
        shared_deadline: end,
    };
    let p = json!({"version":"m1_context_pack_autonomous_v3","run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
        "inference_profile":"fixture-genai-responses-v1","engagement_id":scope.engagement,"campaign_id":scope.campaign,"decision_id":scope.decision,
        "permission_id":scope.permission,"mission_revision":1,"references":[{"id":"asset_0"},{"id":"mission"}],"manifests":[{"reference":"dns_udp"}],
        "question":"\u{754c} \"synthetic\" request"});
    let mut pack = serde_json::to_vec(&p).unwrap();
    pack.resize(8192, b' ');
    let upper = inference_intent::preflight(&pack, 8192, 16384).unwrap();
    let server = Server::start("127.0.0.1:0", |_| Reply::Length(200, success_body()));
    for (call, codes) in [
        (r.generation_call, None),
        (r.repair_call, Some(&["unexpected_view_request"; 8][..])),
    ] {
        let bound = inference_intent::bind(&scope, &r, call, &pack, codes).unwrap();
        assert!(bound.configuration.len() <= upper);
        let intent = inference_intent::verify(&bound).unwrap();
        let mut prepared = prepare_call(&bound, &intent, Duration::from_secs(3)).unwrap();
        // Endpoint injection is private to this test module, never production API.
        prepared.target.endpoint = Endpoint::from_owned(format!("http://{}/", server.address));
        let observed = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(prepared.client.exec_chat_observed(
                ModelSpec::Target(prepared.target),
                prepared.request,
                Some(&prepared.options),
            ));
        assert!(observed.result.is_ok());
        let bytes = observed.evidence.request_entity_bytes.unwrap();
        assert!(bytes <= 16384);
        assert_eq!(
            server.requests.lock().unwrap().last().unwrap().body.len(),
            bytes
        );
        let role = if codes.is_some() {
            "repair"
        } else {
            "generation"
        };
        let observation = format!(
            "safe_sdk_boundary role={} pack={} intent={} upper={} repair_codes={} request_entity={} response_entity={:?}",
            role,
            pack.len(),
            bound.configuration.len(),
            upper,
            codes.map_or(0, |c| c.len()),
            bytes,
            observed.evidence.response_entity_bytes
        );
        assert!(observation.starts_with(&format!("safe_sdk_boundary role={role} pack=")));
        for id in [
            scope.engagement,
            scope.campaign,
            scope.decision,
            scope.permission,
            r.generation_call,
            r.repair_call,
        ] {
            assert!(
                !observation.contains(&id.to_string()),
                "closed boundary observation excludes identities"
            );
        }
        eprintln!("{observation}");
    }
    assert_eq!(server.count(), 2);
}

#[test]
fn tls_fixture_assertion_cleanup_reaps_server_and_removes_private_material() {
    let mut fixture = TlsFixture::new();
    let port = fixture.start();
    let dir = fixture.dir.clone();
    let verdict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _fixture = fixture;
        panic!("synthetic TLS barrier failure");
    }));
    assert!(verdict.is_err());
    assert!(!dir.exists());
    assert!(
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "TLS custody must release its port before next case"
    );
}

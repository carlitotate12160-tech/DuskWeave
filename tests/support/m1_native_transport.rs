//! Local-only construction stays inside the concrete adapter's test module.
use super::*;
use crate::m1_reasoning::{proposal::Admission, request};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Basis(bool);
impl ProposalBasis for Basis {
    fn reference_eligible(&self, r: &str) -> bool {
        r == "ref-1"
    }
    fn manifest_eligible(&self, _: &str) -> bool {
        false
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        if self.0 {
            Ok(())
        } else {
            Err("current_authority_unavailable")
        }
    }
    fn safe_explanation(&self, _: &str) -> bool {
        true
    }
}

fn envelope() -> Vec<u8> {
    let proposal = json!({"version":"m1_inference_proposal_v1",
        "hypothesis":"bounded hypothesis","considered_alternative":"alternative",
        "references":["ref-1"],"expected_observation":"expected",
        "falsifying_observation":"falsifying","uncertainty":"unknown",
        "rationale":"safe explanation","next_intent":{"category":"observe_more",
        "references":["ref-1"],"manifest_reference":null,"view_request":null}});
    serde_json::to_vec(&json!({"status":"completed","model":"deepseek-flash",
        "output":[{"type":"message","role":"assistant","status":"completed",
        "content":[{"type":"output_text","text":proposal.to_string()}]}],
        "usage":{"input_tokens":12,"output_tokens":23}}))
    .unwrap()
}

pub(super) struct Server {
    pub(super) url: String,
    pub(super) calls: Arc<AtomicUsize>,
    pub(super) received_body: Arc<std::sync::Mutex<Vec<u8>>>,
    pub(super) arrived: std::sync::mpsc::Receiver<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub(super) fn start(status: &str, headers: &str, body: Vec<u8>) -> Self {
        Self::start_with_preparation(status, headers, body, Duration::from_secs(3))
    }
    pub(super) fn start_with_preparation(
        status: &str,
        headers: &str,
        body: Vec<u8>,
        preparation: Duration,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/responses", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
            body.len()
        )
        .into_bytes();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let received_body = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = received_body.clone();
        let (delivered, arrived) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let end = Instant::now() + preparation;
            while Instant::now() < end {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        observed.fetch_add(1, Ordering::SeqCst);
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut bytes = Vec::new();
                        let mut chunk = [0; 4096];
                        while let Ok(n) = stream.read(&mut chunk) {
                            if n == 0 {
                                break;
                            }
                            bytes.extend_from_slice(&chunk[..n]);
                            if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                                let header = String::from_utf8_lossy(&bytes[..end]);
                                let len = header
                                    .lines()
                                    .find_map(|line| {
                                        let (k, v) = line.split_once(':')?;
                                        k.eq_ignore_ascii_case("content-length")
                                            .then(|| v.trim().parse::<usize>().unwrap())
                                    })
                                    .unwrap_or(0);
                                if bytes.len() >= end + 4 + len {
                                    break;
                                }
                            }
                        }
                        assert!(bytes.starts_with(b"POST /responses HTTP/1.1\r\n"));
                        let offset = bytes.windows(4).position(|v| v == b"\r\n\r\n").unwrap() + 4;
                        *captured.lock().unwrap() = bytes[offset..].to_vec();
                        let _ = delivered.send(());
                        let _ = stream.write_all(&response);
                        let _ = stream.write_all(&body);
                        return;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => return,
                }
            }
        });
        Self {
            url,
            calls,
            received_body,
            arrived,
            worker: Some(worker),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn provider(server: &Server, call: &BoundCall) -> DeepSeekProvider {
    let mut easy = configure(call, "synthetic-fixture-token", Duration::from_secs(2)).unwrap();
    // Private test construction; the production factory has no endpoint override.
    easy.url(&server.url).unwrap();
    DeepSeekProvider {
        prepared: Some((easy, "synthetic-fixture-token".into(), call.digest.clone())),
    }
}

#[test]
fn concrete_transport_posts_bound_body_and_admits_response() {
    let server = Server::start("200 OK", "Content-Type: application/json\r\n", envelope());
    let call = request::bind(br#"{"references":[{"id":"ref-1"}]}"#, None).unwrap();
    let mut p = provider(&server, &call);
    let mut released = 0;
    let result = p
        .call(
            &call,
            Instant::now() + Duration::from_secs(2),
            &mut Basis(true),
            &mut || {
                released += 1;
                Ok(())
            },
        )
        .unwrap();
    assert!(matches!(result.admission, Admission::Accepted(_)));
    assert_eq!(result.usage.output_tokens, Some(23));
    assert_eq!(released, 1);
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    assert_eq!(*server.received_body.lock().unwrap(), call.body);
    assert!(
        p.call(
            &call,
            Instant::now() + Duration::from_secs(2),
            &mut Basis(true),
            &mut || Ok(())
        )
        .is_err()
    );
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn concrete_transport_refuses_redirect_encoding_size_and_reflection() {
    for (status, headers, body, expected) in [
        (
            "302 Found",
            "Location: http://127.0.0.1:1/forbidden\r\n",
            vec![],
            "provider_protocol",
        ),
        (
            "200 OK",
            "Content-Encoding: gzip\r\n",
            envelope(),
            "provider_protocol",
        ),
        ("200 OK", "", vec![b'x'; 65537], "provider_response_limit"),
        (
            "200 OK",
            "",
            b"synthetic-fixture-token".to_vec(),
            "provider_unsafe_response",
        ),
    ] {
        let server = Server::start(status, headers, body);
        let call = request::bind(br#"{"references":[{"id":"ref-1"}]}"#, None).unwrap();
        let result = provider(&server, &call).call(
            &call,
            Instant::now() + Duration::from_secs(2),
            &mut Basis(true),
            &mut || Ok(()),
        );
        assert_eq!(result.err(), Some(expected), "status={status}");
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn concrete_transport_refuses_unprepared_or_changed_binding_before_release() {
    let call = request::bind(br#"{"references":[{"id":"ref-1"}]}"#, None).unwrap();
    let mut easy = configure(&call, "synthetic-fixture-token", Duration::from_secs(1)).unwrap();
    easy.url("http://127.0.0.1:1/forbidden").unwrap();
    let mut p = DeepSeekProvider {
        prepared: Some((easy, "synthetic-fixture-token".into(), "changed".into())),
    };
    for candidate in [&mut p, &mut DeepSeekProvider::default()] {
        let result = candidate.call(
            &call,
            Instant::now() + Duration::from_secs(2),
            &mut Basis(true),
            &mut || panic!("release must not run"),
        );
        assert!(matches!(result, Err("inference_transport_unavailable")));
    }
}

#[test]
fn release_failure_cancels_worker_and_preserves_uncertainty() {
    let call = request::bind(br#"{"references":[{"id":"ref-1"}]}"#, None).unwrap();
    let mut easy = configure(&call, "synthetic-fixture-token", Duration::from_secs(1)).unwrap();
    easy.url("http://127.0.0.1:1/closed").unwrap();
    let mut p = DeepSeekProvider {
        prepared: Some((easy, "synthetic-fixture-token".into(), call.digest.clone())),
    };
    let result = p.call(
        &call,
        Instant::now() + Duration::from_secs(2),
        &mut Basis(true),
        &mut || Err("inference_release_unknown"),
    );
    assert!(matches!(result, Err("inference_release_unknown")));
}

#[test]
fn aggregate_limit_includes_headers_and_never_extends_on_overflow() {
    let bytes = RefCell::new(vec![0; 65530]);
    assert!(add(&bytes, b"header"));
    assert!(!add(&bytes, b"body"));
    assert_eq!(bytes.borrow().len(), 65536);
}

#[test]
fn unlock_failure_after_owned_launch_can_follow_one_real_http_send() {
    let server = Server::start("200 OK", "", envelope());
    let call = request::bind(br#"{"references":[{"id":"ref-1"}]}"#, None).unwrap();
    let result = provider(&server, &call).call(
        &call,
        Instant::now() + Duration::from_secs(3),
        &mut Basis(true),
        &mut || {
            server
                .arrived
                .recv_timeout(Duration::from_secs(2))
                .expect("actual received request before unlock failure");
            Err("inference_release_unknown")
        },
    );
    assert!(matches!(result, Err("inference_release_unknown")));
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    assert_eq!(*server.received_body.lock().unwrap(), call.body);
}

#[test]
fn concrete_transport_worker_child() {
    let Some(url) = std::env::var_os("DW_NATIVE_TRANSPORT_TEST_URL") else {
        return;
    };
    let call = request::bind(br#"{"references":[{"id":"ref-1"}]}"#, None).unwrap();
    let mut h = configure(&call, "synthetic-fixture-token", Duration::from_secs(2)).unwrap();
    h.url(url.to_str().unwrap()).unwrap();
    let mut p = DeepSeekProvider {
        prepared: Some((h, "synthetic-fixture-token".into(), call.digest.clone())),
    };
    let result = p.call(
        &call,
        Instant::now() + Duration::from_secs(2),
        &mut Basis(true),
        &mut || Ok(()),
    );
    assert!(
        result.is_err(),
        "stalled fixture never completes a proposal"
    );
}

#[test]
fn parent_loss_and_tls_stall_leave_no_owned_transport_worker() {
    for terminate in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let (arrived, received) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < end =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    _ => panic!("owned transport connection missing"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            stream.set_nonblocking(false).unwrap();
            let mut bytes = [0; 4096];
            assert!(stream.read(&mut bytes).unwrap() > 0);
            arrived.send(()).unwrap();
            loop {
                match stream.read(&mut bytes) {
                    Ok(0) => return,
                    Ok(_) => {}
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                                | std::io::ErrorKind::BrokenPipe
                                | std::io::ErrorKind::UnexpectedEof
                        ) =>
                    {
                        return;
                    }
                    Err(e) => panic!(
                        "owned transport close: kind={:?} os={:?} terminate={terminate}",
                        e.kind(),
                        e.raw_os_error()
                    ),
                }
            }
        });
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "m1_reasoning::transport::tests::concrete_transport_worker_child",
                "--nocapture",
            ])
            .env(
                "DW_NATIVE_TRANSPORT_TEST_URL",
                format!(
                    "{}://{address}/responses",
                    if terminate { "http" } else { "https" }
                ),
            )
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        received
            .recv_timeout(Duration::from_secs(3))
            .expect("actual HTTP/TLS bytes from child worker");
        let start = Instant::now();
        if terminate {
            child.kill().unwrap();
        }
        let status = child.wait().unwrap();
        assert!(terminate || status.success() || status.code() == Some(124));
        assert!(start.elapsed() < Duration::from_secs(4));
        server.join().unwrap();
    }
}

#[test]
fn concrete_tls_verifies_certificate_and_hostname() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let dir = std::env::temp_dir().join(format!(
        "dw-native-tls-{}",
        uuid::Uuid::from_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    ));
    std::fs::create_dir(&dir).unwrap();
    let cert = dir.join("certificate.pem");
    let key = dir.join("key.pem");
    let body = dir.join("response.json");
    std::fs::write(&body, envelope()).unwrap();
    let openssl = std::env::var_os("DW_TEST_OPENSSL").unwrap_or_else(|| "openssl".into());
    let output = Command::new(openssl)
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .args([
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost",
            "-days",
            "1",
        ])
        .output()
        .expect("fixture openssl");
    assert!(
        output.status.success(),
        "synthetic TLS fixture generation failed"
    );
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    });
    let server = r#"
import socket,ssl,sys,pathlib
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(sys.argv[1],sys.argv[2])
with socket.socket() as listener:
 listener.bind(('127.0.0.1',0));listener.listen(1);listener.settimeout(4)
 print(listener.getsockname()[1],flush=True)
 try:
  raw,_=listener.accept();raw.settimeout(3)
  with ctx.wrap_socket(raw,server_side=True) as stream:
   data=b''
   while b'\r\n\r\n' not in data:
    chunk=stream.recv(4096)
    if not chunk: break
    data+=chunk
   if b'\r\n\r\n' in data:
    header,body=data.split(b'\r\n\r\n',1)
    n=next((int(x.split(b':',1)[1]) for x in header.split(b'\r\n') if x.lower().startswith(b'content-length:')),0)
    while len(body)<n:
     chunk=stream.recv(4096)
     if not chunk: break
     body+=chunk
    response=pathlib.Path(sys.argv[3]).read_bytes()
    stream.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: '+str(len(response)).encode()+b'\r\nConnection: close\r\n\r\n'+response)
 except (ssl.SSLError,TimeoutError,ConnectionError,OSError): pass
"#;
    for (trusted, host, accepted) in [
        (true, "localhost", true),
        (false, "localhost", false),
        (true, "127.0.0.1", false),
    ] {
        let mut child = Command::new(&python)
            .arg("-u")
            .arg("-c")
            .arg(server)
            .arg(&cert)
            .arg(&key)
            .arg(&body)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("local TLS server");
        let mut line = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port: u16 = line.trim().parse().expect("TLS fixture readiness port");
        let call = request::bind(br#"{"references":[{"id":"ref-1"}]}"#, None).unwrap();
        let mut h = configure(&call, "synthetic-fixture-token", Duration::from_secs(2)).unwrap();
        h.url(&format!("https://{host}:{port}/responses")).unwrap();
        if trusted {
            h.cainfo(&cert).unwrap();
        }
        let mut p = DeepSeekProvider {
            prepared: Some((h, "synthetic-fixture-token".into(), call.digest.clone())),
        };
        let result = p.call(
            &call,
            Instant::now() + Duration::from_secs(3),
            &mut Basis(true),
            &mut || Ok(()),
        );
        assert_eq!(result.is_ok(), accepted, "certificate and hostname policy");
        assert!(child.wait().unwrap().success());
    }
    for path in [cert, key, body] {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

#[path = "m1_deadline_diagnostic.rs"]
mod m1_deadline_diagnostic;

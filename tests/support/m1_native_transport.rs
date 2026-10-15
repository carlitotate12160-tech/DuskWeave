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

struct Server {
    url: String,
    calls: Arc<AtomicUsize>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn start(status: &str, headers: &str, body: Vec<u8>) -> Self {
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
        let worker = std::thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(3);
            while Instant::now() < end {
                match listener.accept() {
                    Ok((mut stream, _)) => {
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
                        assert!(
                            bytes.starts_with(b"POST /responses HTTP/1.1\r\n"),
                            "bytes was: {:?}",
                            String::from_utf8_lossy(&bytes)
                        );
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
    let openssl = if cfg!(windows) {
        "C:/Program Files/Git/usr/bin/openssl.exe"
    } else {
        "openssl"
    };
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
    let python = if cfg!(windows) {
        "C:/Users/ekoku/AppData/Local/Programs/Python/Python312/python.exe"
    } else {
        "python3"
    };
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
        let mut child = Command::new(python)
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

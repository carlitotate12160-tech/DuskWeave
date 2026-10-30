//! Private SDK tls controls with the same client and fixture.
use super::*;

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
pub(super) fn same_http_builder_verifies_trusted_certificate_hostname_and_untrusted_roots() {
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

pub(super) fn tls_fixture_assertion_cleanup_reaps_server_and_removes_private_material() {
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

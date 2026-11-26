//! Resource-dependent Linux Docker executable-pair qualification for public HTTPS worker boundary.
use super::{sample_public_request, uid};
use duskweave::m1_https::codec::{decode_payload, encode_frame};
use duskweave::m1_https::ipc::{
    HTTPS_IPC_MAX_CONTROL, HTTPS_IPC_MAX_FRAME, HttpsAck, HttpsIpcRequest, HttpsOutcome,
    HttpsPhase, HttpsPhaseReady, HttpsPhaseStart, HttpsSafeResult,
};
use duskweave::m1_https::result::{LAB_SCOPE_LIMITATION, PUBLIC_SCOPE_LIMITATION};
use duskweave::m1_inference_profile::{
    HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER, HTTPS_LAB_RUN_PROFILE,
};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LABEL_PKT: &str = "org.duskweave.packet=DW-M1-PUBLIC-HTTPS-WORKER-R1";
const GO_BIN: &str = "/home/ekoku/duskweave-build/go-1.27.3/toolchain/bin/go";
const GO_CACHE: &str = "/home/ekoku/duskweave-build/go-1.27.3/public-https-worker-r1/gocache";
const GO_MOD: &str = "/home/ekoku/duskweave-build/go-1.27.3/public-https-worker-r1/gomodcache";
const DOCKER_WRAP: &str =
    "/home/ekoku/duskweave-reports/https-provider-feedback-r1/tools/bin/docker";
const CLEAN_PATH: &str = "/home/ekoku/duskweave-build/go-1.27.3/toolchain/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
const ROOT_SHA: &str = "427c064216761a1e72a8b30f0d954d08cb6e4435a82091fb865afd20c8497285";
const CLEAN_WAIT: Duration = Duration::from_secs(20);

fn now_ms() -> i64 {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(d.as_millis()).unwrap()
}

fn dec<T: DeserializeOwned>(b: &[u8], lim: usize) -> T {
    decode_payload(b, lim).unwrap()
}

fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn docker_bin() -> &'static str {
    if Path::new(DOCKER_WRAP).exists() {
        DOCKER_WRAP
    } else {
        "/usr/bin/docker"
    }
}

fn clean_cmd(bin: &str) -> Command {
    let mut cmd = Command::new(bin);
    cmd.env_clear().env("PATH", CLEAN_PATH);
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    }
    cmd
}

fn go_cmd(dir: &Path) -> Command {
    let mut cmd = clean_cmd(GO_BIN);
    cmd.current_dir(dir);
    for kv in "CGO_ENABLED=0 GOOS=linux GOARCH=amd64 GOTOOLCHAIN=local GOPROXY=off GOSUMDB=off"
        .split_whitespace()
    {
        let (k, v) = kv.split_once('=').unwrap();
        cmd.env(k, v);
    }
    cmd.env("GOCACHE", GO_CACHE).env("GOMODCACHE", GO_MOD);
    cmd
}

fn wait_child(
    child: &mut Child,
    lbl: &str,
    lim: Duration,
) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + lim;
    loop {
        if let Some(st) = child.try_wait().map_err(|e| format!("{lbl}: {e}"))? {
            return Ok(st);
        }
        if Instant::now() >= deadline {
            let _ = (child.kill(), child.wait());
            return Err(format!("{lbl} timed out"));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn run_bounded(cmd: &mut Command, stdin: Option<&[u8]>, lbl: &str) -> Result<String, String> {
    let si = if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    cmd.stdin(si).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("{lbl}: {e}"))?;
    if let Some(bytes) = stdin {
        let mut sin = child.stdin.take().ok_or_else(|| format!("{lbl} stdin"))?;
        sin.write_all(bytes)
            .map_err(|e| format!("{lbl} write: {e}"))?;
    }
    let st = wait_child(&mut child, lbl, Duration::from_secs(60))?;
    let (mut out, mut err) = (String::new(), String::new());
    let _ = child.stdout.take().map(|mut s| s.read_to_string(&mut out));
    let _ = child.stderr.take().map(|mut s| s.read_to_string(&mut err));
    st.success()
        .then_some(out)
        .ok_or_else(|| format!("{lbl} ({st:?}): {err}"))
}

fn docker_run(args: &[&str], stdin: Option<&[u8]>, lbl: &str) -> String {
    run_bounded(clean_cmd(docker_bin()).args(args), stdin, lbl).unwrap()
}

fn append_tar(tar: &mut Vec<u8>, name: &str, mode: u32, data: &[u8]) {
    let mut hdr = [0u8; 512];
    hdr[..name.len()].copy_from_slice(name.as_bytes());
    hdr[100..108].copy_from_slice(format!("{mode:07o}\0").as_bytes());
    hdr[108..124].copy_from_slice(b"0000000\x000000000\0");
    hdr[124..136].copy_from_slice(format!("{:011o}\0", data.len()).as_bytes());
    hdr[136..157].copy_from_slice(b"00000000000\0        0");
    hdr[257..265].copy_from_slice(b"ustar\x0000");
    let sum: u32 = hdr.iter().map(|&b| u32::from(b)).sum();
    hdr[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    tar.extend_from_slice(&hdr);
    tar.extend_from_slice(data);
    tar.extend(std::iter::repeat_n(0u8, (512 - data.len() % 512) % 512));
}

fn build_tar_image(
    lbl: &str,
    tag: &str,
    df: (&str, &[u8]),
    bins: (&[u8], &[u8]),
    pem: Option<&[u8]>,
) -> String {
    let mut tar = Vec::new();
    for (n, m, b) in [
        (df.0, 0o644, df.1),
        ("dw-dns-capture", 0o755, bins.0),
        ("dw-dns-exporter", 0o755, bins.1),
    ] {
        append_tar(&mut tar, n, m, b);
    }
    if let Some(bytes) = pem {
        append_tar(&mut tar, "cacert-2026-11-05.pem", 0o644, bytes);
    }
    tar.extend(std::iter::repeat_n(0u8, 1024));
    let mut cmd = clean_cmd(docker_bin());
    cmd.args([
        "build",
        "--pull=false",
        "--no-cache",
        "-f",
        df.0,
        "--label",
        LABEL_PKT,
        "--label",
        lbl,
        "-t",
        tag,
        "-",
    ]);
    run_bounded(&mut cmd, Some(&tar), tag).unwrap();
    docker_run(
        &["image", "inspect", "--format", "{{.Id}}", tag],
        None,
        "inspect",
    )
    .trim()
    .into()
}

fn list_owned(lbl: &str, what: &[&str]) -> Result<String, String> {
    let f = format!("label={lbl}");
    let mut c = clean_cmd(docker_bin());
    c.args(what).args(["-q", "--filter", &f]);
    run_bounded(&mut c, None, "list")
}

struct OwnedResources {
    run_label: String,
    temp_dir: PathBuf,
}

impl OwnedResources {
    fn sweep(&self, what: &[&str], rm: &str) -> Result<(), String> {
        for id in list_owned(&self.run_label, what)?.split_whitespace() {
            run_bounded(clean_cmd(docker_bin()).args([rm, "-f", id]), None, rm)?;
        }
        list_owned(&self.run_label, what)?
            .trim()
            .is_empty()
            .then_some(())
            .ok_or_else(|| rm.into())
    }
    fn cleanup_strict(&self) -> Result<(), String> {
        let mut errs: Vec<_> = [(&["ps", "-a"][..], "rm"), (&["images"][..], "rmi")]
            .into_iter()
            .filter_map(|(w, rm)| self.sweep(w, rm).err())
            .collect();
        if self.temp_dir.exists() && std::fs::remove_dir_all(&self.temp_dir).is_err() {
            errs.push("temp dir cleanup failed".into());
        }
        errs.is_empty().then_some(()).ok_or_else(|| errs.join("; "))
    }
}
impl Drop for OwnedResources {
    fn drop(&mut self) {
        let _ = self.cleanup_strict();
    }
}

fn write_frame<T: Serialize>(sink: &mut impl Write, val: &T, limit: usize) {
    sink.write_all(&encode_frame(val, limit).unwrap()).unwrap();
    sink.flush().unwrap();
}

fn read_bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static, lbl: &str) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(CLEAN_WAIT)
        .unwrap_or_else(|e| panic!("{lbl} timeout: {e}"))
}

fn read_frame_raw<R: Read + Send + 'static>(mut src: R, lim: usize) -> (R, [u8; 4], Vec<u8>) {
    read_bounded(
        move || {
            let mut hdr = [0u8; 4];
            src.read_exact(&mut hdr).unwrap();
            let len = u32::from_be_bytes(hdr) as usize;
            assert!((1..=lim).contains(&len), "invalid frame length {len}");
            let mut buf = vec![0u8; len];
            src.read_exact(&mut buf).unwrap();
            (src, hdr, buf)
        },
        "read_frame_raw",
    )
}

fn read_eof<R: Read + Send + 'static>(mut src: R, lbl: &'static str) -> Vec<u8> {
    read_bounded(
        move || {
            let mut v = Vec::new();
            src.read_to_end(&mut v).unwrap();
            v
        },
        lbl,
    )
}

fn spawn_pair_container(run_label: &str, name: &str, image: &str, exporter: bool) -> Child {
    let (entry, user) = if exporter {
        ("/dw-dns-exporter", "65533:65533")
    } else {
        ("/dw-dns-capture", "65532:65532")
    };
    let flags = "--network none --ipc none --read-only --cap-drop ALL --security-opt no-new-privileges --pids-limit 32 --memory 128m --memory-swap 128m --cpus 0.5 --ulimit nofile=64:64 --ulimit core=0:0 --log-driver none --interactive";
    let mut create = clean_cmd(docker_bin());
    create.args([
        "create", "--name", name, "--label", LABEL_PKT, "--label", run_label,
    ]);
    create
        .args(flags.split_whitespace())
        .args(["--user", user, "--entrypoint", entry, image]);
    run_bounded(&mut create, None, name).unwrap();
    let mut cmd = clean_cmd(docker_bin());
    cmd.args(["start", "-a", "-i", name])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap_or_else(|e| panic!("start {name}: {e}"));
    let deadline = Instant::now() + CLEAN_WAIT;
    loop {
        let mut insp = clean_cmd(docker_bin());
        insp.args([
            "inspect",
            "--format",
            "{{.State.Running}} {{.Config.User}}",
            name,
        ]);
        match run_bounded(&mut insp, None, "warm") {
            Ok(out) if out.trim() == format!("true {user}") => return child,
            Ok(out) if Instant::now() >= deadline => {
                let _ = (child.kill(), child.wait());
                panic!("{name} warm: {out}");
            }
            Err(e) => {
                let _ = (child.kill(), child.wait());
                panic!("{name} inspect: {e}");
            }
            Ok(_) => thread::sleep(Duration::from_millis(25)),
        }
    }
}

fn wait_exit(mut child: Child, label: &str) -> std::process::ExitStatus {
    wait_child(&mut child, label, CLEAN_WAIT).unwrap()
}

fn run_full_pair(run_label: &str, tag: &str, image: &str) -> (HttpsIpcRequest, HttpsSafeResult) {
    let (cap_n, exp_n) = (format!("dwpubcap-{tag}"), format!("dwpubexp-{tag}"));
    let mut cap = spawn_pair_container(run_label, &cap_n, image, false);
    let mut exp = spawn_pair_container(run_label, &exp_n, image, true);
    let (mut cap_in, cap_out) = (cap.stdin.take().unwrap(), cap.stdout.take().unwrap());
    let (mut exp_in, exp_out) = (exp.stdin.take().unwrap(), exp.stdout.take().unwrap());

    // Construct fresh request clock ONLY after both containers are warm-started.
    let req = sample_public_request(now_ms());
    write_frame(&mut exp_in, &req, HTTPS_IPC_MAX_FRAME);
    write_frame(&mut cap_in, &req, HTTPS_IPC_MAX_FRAME);

    let (exp_out, _, exp_b) = read_frame_raw(exp_out, HTTPS_IPC_MAX_CONTROL);
    let (cap_out, _, cap_b) = read_frame_raw(cap_out, HTTPS_IPC_MAX_CONTROL);
    let p = HttpsPhase::Connect;
    let exp_rdy: HttpsPhaseReady = dec(&exp_b, HTTPS_IPC_MAX_CONTROL);
    let cap_rdy: HttpsPhaseReady = dec(&cap_b, HTTPS_IPC_MAX_CONTROL);
    assert!(exp_rdy.matches(&req, p) && cap_rdy.matches(&req, p));

    let start = HttpsPhaseStart::new(&req, p, req.deadline_at_ms);
    write_frame(&mut exp_in, &start, HTTPS_IPC_MAX_CONTROL);
    write_frame(&mut cap_in, &start, HTTPS_IPC_MAX_CONTROL);

    // Relay bounded raw frame opaquely without parsing private capture content.
    let (cap_out, pfx, raw) = read_frame_raw(cap_out, HTTPS_IPC_MAX_FRAME);
    assert!(read_eof(cap_out, "cap eof").is_empty());
    exp_in.write_all(&[&pfx[..], &raw[..]].concat()).unwrap();
    exp_in.flush().unwrap();

    let (_, _, safe_b) = read_frame_raw(exp_out, HTTPS_IPC_MAX_FRAME);
    let safe: HttpsSafeResult = dec(&safe_b, HTTPS_IPC_MAX_FRAME);
    safe.validate_for(&req, p, now_ms()).unwrap();
    let res = safe.into_head_result(&req).unwrap();
    assert_eq!(res.limitations[0], PUBLIC_SCOPE_LIMITATION);

    write_frame(&mut exp_in, &HttpsAck::new(&req), HTTPS_IPC_MAX_CONTROL);
    drop((cap_in, exp_in));
    assert!(wait_exit(cap, &cap_n).success(), "{cap_n} failed");
    assert!(wait_exit(exp, &exp_n).success(), "{exp_n} failed");
    (req, safe)
}

fn assert_capture_control_fails(run_label: &str, tag: &str, image: &str, wrong_attempt: bool) {
    let cap_n = format!("dwpubneg-{tag}");
    let mut cap = spawn_pair_container(run_label, &cap_n, image, false);
    let (mut cap_in, cap_out) = (cap.stdin.take().unwrap(), cap.stdout.take().unwrap());
    let req = sample_public_request(now_ms());
    write_frame(&mut cap_in, &req, HTTPS_IPC_MAX_FRAME);
    let (cap_out, _, rdy_b) = read_frame_raw(cap_out, HTTPS_IPC_MAX_CONTROL);
    let rdy: HttpsPhaseReady = dec(&rdy_b, HTTPS_IPC_MAX_CONTROL);
    assert!(rdy.matches(&req, HttpsPhase::Connect));
    if wrong_attempt {
        let mut bad = HttpsPhaseStart::new(&req, HttpsPhase::Connect, req.deadline_at_ms);
        bad.attempt = uid(0xdead);
        write_frame(&mut cap_in, &bad, HTTPS_IPC_MAX_CONTROL);
    }
    drop(cap_in);
    assert!(read_eof(cap_out, "neg eof").is_empty());
    assert!(!wait_exit(cap, &cap_n).success());
}

fn run_neg_root(lbl: &str, tag_p: &str, bins: (&[u8], &[u8]), pem: &[u8], lab: &[u8], df: &[u8]) {
    let df_miss = b"FROM scratch\nCOPY --chmod=0555 dw-dns-capture /dw-dns-capture\nCOPY --chmod=0555 dw-dns-exporter /dw-dns-exporter\n";
    let mut bad = pem.to_vec();
    let idx = bad.len() - 2;
    bad[idx] ^= 0x01;
    let lim = [
        PUBLIC_SCOPE_LIMITATION,
        "single_endpoint_execution_refused_not_whole_host",
    ];
    let cases = [
        ("missing", ("Dockerfile.missing", df_miss.as_slice()), None),
        (
            "tampered",
            ("Dockerfile.https-public", df),
            Some(bad.as_slice()),
        ),
        ("lab", ("Dockerfile.https-public", df), Some(lab)),
    ];
    for (k, d, r) in cases {
        let tag = format!("dw-public-https-{k}:{tag_p}");
        let _ = build_tar_image(lbl, &tag, d, bins, r);
        let (_, res) = run_full_pair(lbl, &format!("{tag_p}-{k}"), &tag);
        assert_eq!(res.outcome, HttpsOutcome::ExecutionRefused, "{k}");
        assert_eq!(res.limitations, lim, "{k}");
    }
}

#[ignore = "resource-dependent: requires qualified Linux Docker engine and pinned Go 1.27.3 toolchain"]
#[test]
fn public_https_worker_pair_uses_pinned_roots_and_safe_projection() {
    assert_eq!(
        (std::env::consts::OS, std::env::consts::ARCH),
        ("linux", "x86_64")
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        run_bounded(go_cmd(&root.join("boundary")).args(["version"]), None, "go")
            .unwrap()
            .trim(),
        "go version go1.27.3 linux/amd64"
    );
    let dinfo = docker_run(
        &["info", "--format", "{{.OSType}} {{.Architecture}}"],
        None,
        "info",
    );
    assert!(dinfo.contains("linux") && (dinfo.contains("x86_64") || dinfo.contains("amd64")));

    let pem = std::fs::read(root.join("boundary/trust/cacert-2026-11-05.pem")).unwrap();
    assert_eq!((pem.len(), sha256_hex(&pem).as_str()), (200_116, ROOT_SHA));
    let lab_pem = std::fs::read(root.join("boundary/fixtures/https-lab-root-v1.pem")).unwrap();
    let df_pub = std::fs::read(root.join("boundary/Dockerfile.https-public")).unwrap();

    let run_tag = format!("dwpub-{}", now_ms());
    let run_label = format!("org.duskweave.run={run_tag}");
    let temp_dir = PathBuf::from(format!(
        "/home/ekoku/duskweave-build/go-1.27.3/public-https-worker-r1/pair-{run_tag}"
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let guard = OwnedResources {
        run_label: run_label.clone(),
        temp_dir: temp_dir.clone(),
    };

    let [cap_b, exp_b] = ["capture", "exporter"].map(|n| {
        let p = temp_dir.join(format!("dw-dns-{n}"));
        let mut c = go_cmd(&root.join("boundary"));
        c.args(["build", "-trimpath", "-buildvcs=false", "-o"])
            .arg(&p)
            .arg(format!("./cmd/dw-dns-{n}"));
        run_bounded(&mut c, None, n).unwrap();
        std::fs::read(p).unwrap()
    });
    let (cap_sha, exp_sha) = (sha256_hex(&cap_b), sha256_hex(&exp_b));

    let img_prod = format!("dw-public-https-pair:{run_tag}");
    let img_id = build_tar_image(
        &run_label,
        &img_prod,
        ("Dockerfile.https-public", &df_pub),
        (&cap_b, &exp_b),
        Some(&pem),
    );
    let (req_ok, safe_ok) = run_full_pair(&run_label, &format!("{run_tag}-ok"), &img_prod);
    assert_eq!(safe_ok.outcome, HttpsOutcome::ConnectionFailed);
    assert_eq!(
        safe_ok.limitations,
        [
            PUBLIC_SCOPE_LIMITATION,
            "single_endpoint_connection_failed_not_whole_host"
        ]
    );

    run_neg_root(
        &run_label,
        &run_tag,
        (&cap_b, &exp_b),
        &pem,
        &lab_pem,
        &df_pub,
    );
    for wrong in [true, false] {
        assert_capture_control_fails(
            &run_label,
            &format!("{run_tag}-neg-{wrong}"),
            &img_prod,
            wrong,
        );
    }

    let lab_req = HttpsIpcRequest {
        run_profile: HTTPS_LAB_RUN_PROFILE.into(),
        hostname: HTTPS_LAB_HOSTNAME.into(),
        ipv4: HTTPS_LAB_IPV4.into(),
        resolver_ipv4: HTTPS_LAB_RESOLVER.into(),
        ..req_ok.clone()
    };
    let mut lab_safe = safe_ok.clone();
    lab_safe.limitations[0] = LAB_SCOPE_LIMITATION.into();
    for (s, q) in [(&safe_ok, &lab_req), (&lab_safe, &req_ok)] {
        assert!(s.validate_for(q, HttpsPhase::Connect, now_ms()).is_err());
        assert!(s.into_head_result(q).is_err());
    }

    eprintln!(
        "PAIR_QUALIFIED image={img_id} capture_sha256={cap_sha} exporter_sha256={exp_sha} root_sha256={ROOT_SHA} outcome={:?}",
        safe_ok.outcome
    );
    guard.cleanup_strict().unwrap();
}

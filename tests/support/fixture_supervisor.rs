use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const GUARD: Duration = Duration::from_secs(45);

pub(super) fn supervisor() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dw-fixture-supervisor"))
}

pub(super) fn python() -> String {
    std::env::var("DW_TEST_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    })
}

pub(super) fn unique_root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dw-fs-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&root).expect("unique report root");
    root
}

pub(super) fn request_for(
    root: &Path,
    label: &str,
    script: &str,
    budget_ms: u64,
    redactions: &[&str],
) -> String {
    json!({
        "version": 1,
        "argv": [python(), "-c", script],
        "cwd": env!("CARGO_MANIFEST_DIR"),
        "label": label,
        "report_root": root.to_string_lossy(),
        "action_budget_ms": budget_ms,
        "redactions": redactions,
    })
    .to_string()
}

pub(super) fn invoke(arguments: &[&str], request: Option<&str>) -> (i32, Value, String) {
    let mut command = Command::new(supervisor());
    let output = bounded(command.args(arguments), request, GUARD);
    let protocol = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    (
        status_code(&output.status),
        protocol,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

pub(super) fn status_code(status: &std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

/// Independent bounded guard: the supervisor is killed if it ever exceeds the
/// test's own deadline, so a supervision defect cannot hang the suite.
pub(super) fn bounded(command: &mut Command, request: Option<&str>, guard: Duration) -> Output {
    command
        .stdin_if(request)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("supervisor spawn");
    if let Some(request) = request {
        let mut stdin = child.stdin.take().expect("supervisor stdin");
        stdin.write_all(request.as_bytes()).expect("request write");
    }
    let stdout = reader(child.stdout.take().expect("supervisor stdout"));
    let stderr = reader(child.stderr.take().expect("supervisor stderr"));
    let status = wait_or_panic(&mut child, guard);
    Output {
        status,
        stdout: stdout.join().expect("stdout"),
        stderr: stderr.join().expect("stderr"),
    }
}

trait StdinIf {
    fn stdin_if(&mut self, request: Option<&str>) -> &mut Self;
}

impl StdinIf for Command {
    fn stdin_if(&mut self, request: Option<&str>) -> &mut Self {
        if request.is_some() {
            self.stdin(Stdio::piped());
        } else {
            self.stdin(Stdio::null());
        }
        self
    }
}

pub(super) fn reader(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).expect("pipe read");
        bytes
    })
}

pub(super) fn wait_or_panic(child: &mut Child, guard: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + guard;
    loop {
        if let Some(status) = child.try_wait().expect("supervisor poll") {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("supervisor exceeded the independent test guard");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub(super) fn metadata(root: &Path, label: &str) -> Value {
    let path = root.join(format!("{label}.result.json"));
    serde_json::from_str(&std::fs::read_to_string(path).expect("metadata")).expect("json")
}

pub(super) fn log_text(root: &Path, label: &str) -> String {
    std::fs::read_to_string(root.join(format!("{label}.log"))).expect("log")
}

pub(super) fn service_arguments<'a>(container: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut arguments = vec!["service-check", "--container", container];
    arguments.extend_from_slice(extra);
    arguments
}

pub(super) fn process_alive(pid: u32) -> bool {
    if cfg!(windows) {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .expect("tasklist");
        String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
    } else {
        Path::new(&format!("/proc/{pid}/stat")).exists()
    }
}

pub(super) fn direct_child(script: &str) -> Child {
    Command::new(python())
        .arg("-c")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("sentinel spawn")
}

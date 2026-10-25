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
        proc_stat_alive(Path::new(&format!("/proc/{pid}/stat")))
    }
}

/// A zombie has terminated even while its proc entry awaits reaping.
/// Missing entries are dead; failed observations must never prove death.
fn proc_stat_alive(path: &Path) -> bool {
    let stat = match std::fs::read_to_string(path) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        Err(error) => panic!("proc observation unavailable: {:?}", error.kind()),
    };
    // comm is parenthesized and can itself contain spaces and parentheses.
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .expect("proc observation has no process state");
    let known = ["R", "S", "D", "Z", "T", "t", "W", "X", "x", "K", "P", "I"];
    assert!(
        known.contains(&state),
        "proc observation has an invalid state"
    );
    state != "Z"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_stat_zombies_are_terminated_and_live_states_remain_alive() {
        let root = unique_root("proc-states");
        let path = root.join("stat");
        let cases = [
            ("123 (worker) Z 1 0 0", false),
            ("123 (worker) R 1 0 0", true),
            ("123 (escaped sleeper) S 1 0 0", true),
            ("123 (worker) D 1 0 0", true),
            ("123 (worker) T 1 0 0", true),
            ("123 (name ) Z marker) S 1 0 0", true),
            ("123 (name with spaces and )) Z 1 0 0", false),
        ];
        for (stat, expected) in cases {
            std::fs::write(&path, stat).expect("synthetic proc stat");
            assert_eq!(proc_stat_alive(&path), expected, "{stat}");
        }
        std::fs::remove_file(path).expect("owned stat cleanup");
        std::fs::remove_dir(root).expect("owned root cleanup");
    }

    #[test]
    fn proc_stat_missing_is_dead_but_read_errors_are_not_death_evidence() {
        let root = unique_root("proc-errors");
        assert!(!proc_stat_alive(&root.join("missing")));
        assert!(
            std::panic::catch_unwind(|| proc_stat_alive(&root)).is_err(),
            "a directory read error must fail the observation"
        );
        let path = root.join("stat");
        std::fs::write(&path, [0xff]).expect("invalid UTF-8 fixture");
        assert!(
            std::panic::catch_unwind(|| proc_stat_alive(&path)).is_err(),
            "an undecodable stat must fail the observation"
        );
        std::fs::remove_file(path).expect("owned stat cleanup");
        std::fs::remove_dir(root).expect("owned root cleanup");
    }

    #[test]
    fn proc_stat_malformed_state_is_rejected() {
        let root = unique_root("proc-malformed");
        let path = root.join("stat");
        for stat in [
            "",
            "123 worker S",
            "123 (worker)",
            "123 (worker) ?",
            "123 (worker) ZZ",
        ] {
            std::fs::write(&path, stat).expect("malformed stat fixture");
            assert!(
                std::panic::catch_unwind(|| proc_stat_alive(&path)).is_err(),
                "malformed process state must fail the observation: {stat}"
            );
        }
        std::fs::remove_file(path).expect("owned stat cleanup");
        std::fs::remove_dir(root).expect("owned root cleanup");
    }
}

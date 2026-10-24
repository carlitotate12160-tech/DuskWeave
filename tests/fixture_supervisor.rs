//! Integration coverage for the development fixture supervisor binary.
//!
//! Every scenario uses real OS processes behind the built executable with an
//! independent bounded guard per invocation. No database, provider or target
//! is contacted; the sentinel scenarios prove unrelated processes are never
//! signalled.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const GUARD: Duration = Duration::from_secs(45);

fn supervisor() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dw-fixture-supervisor"))
}

fn python() -> String {
    std::env::var("DW_TEST_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    })
}

fn unique_root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dw-fs-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&root).expect("unique report root");
    root
}

fn request_for(
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

fn invoke(arguments: &[&str], request: Option<&str>) -> (i32, Value, String) {
    let mut command = Command::new(supervisor());
    let output = bounded(command.args(arguments), request, GUARD);
    let protocol = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    (
        status_code(&output.status),
        protocol,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn status_code(status: &std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

/// Independent bounded guard: the supervisor is killed if it ever exceeds the
/// test's own deadline, so a supervision defect cannot hang the suite.
fn bounded(command: &mut Command, request: Option<&str>, guard: Duration) -> Output {
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

fn reader(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).expect("pipe read");
        bytes
    })
}

fn wait_or_panic(child: &mut Child, guard: Duration) -> std::process::ExitStatus {
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

fn metadata(root: &Path, label: &str) -> Value {
    let path = root.join(format!("{label}.result.json"));
    serde_json::from_str(&std::fs::read_to_string(path).expect("metadata")).expect("json")
}

fn log_text(root: &Path, label: &str) -> String {
    std::fs::read_to_string(root.join(format!("{label}.log"))).expect("log")
}

fn service_arguments<'a>(container: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut arguments = vec!["service-check", "--container", container];
    arguments.extend_from_slice(extra);
    arguments
}

fn process_alive(pid: u32) -> bool {
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

fn direct_child(script: &str) -> Child {
    Command::new(python())
        .arg("-c")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("sentinel spawn")
}

#[test]
fn successful_action_publishes_redacted_log_and_closed_metadata() {
    let root = unique_root("success");
    let script = "import sys; print('visible-output'); print('synthetic-secret'); sys.exit(0)";
    let request = request_for(&root, "success", script, 30_000, &["synthetic-secret"]);
    let (code, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(code, 0);
    assert_eq!(protocol["ok"], true);
    assert_eq!(protocol["status"], "completed");
    assert_eq!(protocol["exit"], 0);
    assert_eq!(protocol["child_exit"], 0);
    assert_eq!(protocol["capture"], "complete");
    assert_eq!(protocol["cleanup"], "completed");
    assert_eq!(protocol["timing_origin"], "request_admission");
    let log = log_text(&root, "success");
    assert!(log.contains("visible-output"));
    assert!(!log.contains("synthetic-secret"));
    assert!(log.contains("[REDACTED]"));
    assert_eq!(metadata(&root, "success")["status"], "completed");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn failing_child_keeps_its_actual_exit_code() {
    let root = unique_root("failure");
    let request = request_for(&root, "failure", "import sys; sys.exit(7)", 30_000, &[]);
    let (code, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(code, 0);
    assert_eq!(
        (protocol["status"].as_str(), protocol["exit"].as_i64()),
        (Some("action_failed"), Some(7))
    );
    assert_eq!(metadata(&root, "failure")["exit"], 7);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn child_exit_124_is_action_failure_not_harness_timeout() {
    let root = unique_root("exit124");
    let request = request_for(&root, "exit124", "import sys; sys.exit(124)", 30_000, &[]);
    let (_, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(protocol["status"], "action_failed");
    assert_eq!(protocol["exit"], 124);
    assert_eq!(metadata(&root, "exit124")["status"], "action_failed");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn live_parent_timeout_terminates_owned_tree_within_the_budget() {
    let root = unique_root("timeout");
    let script = format!(
        "import subprocess, sys, time; from pathlib import Path; \
         child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); \
         Path({root:?}).joinpath('descendant.pid').write_text(str(child.pid)); \
         time.sleep(60)"
    );
    let request = request_for(&root, "timeout", &script, 700, &[]);
    let started = Instant::now();
    let (_, protocol, _) = invoke(&["run-action"], Some(&request));
    assert!(started.elapsed() < Duration::from_secs(6));
    assert_eq!(protocol["status"], "harness_timeout");
    assert_eq!(protocol["exit"], 124);
    assert_eq!(protocol["timing_origin"], "request_admission");
    let pid: u32 = std::fs::read_to_string(root.join("descendant.pid"))
        .expect("pid record")
        .trim()
        .parse()
        .expect("pid");
    assert!(!process_alive(pid), "owned descendant must be terminated");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn exited_parent_with_pipe_holding_descendant_settles_within_bound() {
    let root = unique_root("exited");
    let script = format!(
        "import subprocess, sys; from pathlib import Path; \
         child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(8)']); \
         Path({root:?}).joinpath('descendant.pid').write_text(str(child.pid))"
    );
    let request = request_for(&root, "exited", &script, 5_000, &[]);
    let started = Instant::now();
    let (_, protocol, _) = invoke(&["run-action"], Some(&request));
    assert!(started.elapsed() < Duration::from_secs(6));
    assert_eq!(protocol["status"], "completed");
    assert_eq!(protocol["exit"], 0);
    assert_eq!(protocol["cleanup"], "completed");
    let pid: u32 = std::fs::read_to_string(root.join("descendant.pid"))
        .expect("pid record")
        .trim()
        .parse()
        .expect("pid");
    assert!(
        !process_alive(pid),
        "leftover owned descendant must be stopped"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn group_escape_leaves_an_honest_incomplete_disposition() {
    let root = unique_root("escape");
    let script = format!(
        "import subprocess, sys, time; from pathlib import Path; \
         child = subprocess.Popen([sys.executable, '-c', \
         'import os, time; os.setsid(); time.sleep(8)']); \
         Path({root:?}).joinpath('escape.pid').write_text(str(child.pid)); time.sleep(8)"
    );
    let request = request_for(&root, "escape", &script, 900, &[]);
    let (_, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(protocol["status"], "harness_timeout");
    assert_eq!(protocol["capture"], "incomplete");
    assert_eq!(protocol["cleanup"], "unknown");
    let pid: u32 = std::fs::read_to_string(root.join("escape.pid"))
        .expect("pid record")
        .trim()
        .parse()
        .expect("pid");
    // The escaped descendant is outside the owned group; its termination is
    // not claimed, and the incomplete cleanup is reported honestly. The test
    // cleans the escaped process up by its recorded pid.
    assert!(process_alive(pid));
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("{pid}")])
        .output();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn output_overflow_suppresses_the_stream_with_a_closed_reason() {
    let root = unique_root("overflow");
    let script = "import sys; sys.stdout.write('synthetic-secret' + 'x' * (17 * 1024 * 1024)); \
                  sys.stderr.write('visible-error'); sys.exit(0)";
    let request = request_for(&root, "overflow", script, 60_000, &["synthetic-secret"]);
    let (code, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(code, 0);
    assert_eq!(protocol["capture"], "incomplete");
    assert_eq!(protocol["omissions"], json!(["stdout"]));
    assert_eq!(protocol["status"], "action_failed");
    let log = log_text(&root, "overflow");
    assert!(log.contains("[stdout omitted: capture_overflow]"));
    assert!(!log.contains("synthetic-secret"));
    assert!(log.contains("visible-error"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn secret_split_across_writes_is_redacted_after_assembly() {
    let root = unique_root("split");
    let script = "import sys; sys.stdout.write('AAAAsynth'); sys.stdout.flush(); \
                  sys.stdout.write('etic-secretBBBB'); sys.exit(0)";
    let request = request_for(&root, "split", script, 30_000, &["synthetic-secret"]);
    let (_, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(protocol["capture"], "complete");
    let log = log_text(&root, "split");
    assert!(log.contains("AAAA[REDACTED]BBBB"));
    assert!(!log.contains("synthetic-secret"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn malformed_or_oversized_requests_are_refused_without_reflection() {
    let root = unique_root("refusal");
    let marker = "DW_REFLECTION_MARKER";
    let oversized = format!(
        "{{\"pad\": \"{}\"}}",
        marker.repeat(128 * 1024 / marker.len() + 1)
    );
    let cases: Vec<Option<String>> = vec![
        Some(oversized),
        Some("{\"version\": 1, \"unknown\": true}".into()),
        Some(request_for(&root, "Bad Label", "pass", 30_000, &[])),
        Some(
            json!({"version": 1, "argv": [], "cwd": env!("CARGO_MANIFEST_DIR"), "label": "x",
                     "report_root": root.to_string_lossy(), "action_budget_ms": 30_000,
                     "redactions": []})
            .to_string(),
        ),
        Some(
            json!({"version": 1, "argv": [python()], "cwd": "relative", "label": "x",
                     "report_root": root.to_string_lossy(), "action_budget_ms": 30_000,
                     "redactions": []})
            .to_string(),
        ),
        Some("not-json".into()),
    ];
    for request in cases {
        let (code, protocol, _) = invoke(&["run-action"], request.as_deref());
        assert_eq!(code, 1, "{request:?}");
        assert_eq!(protocol["ok"], false);
        assert_eq!(protocol["category"], "native_supervisor_request_refused");
        assert!(!protocol.to_string().contains(marker));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unknown_command_or_missing_request_bounds_are_usage_refusals() {
    let (code, protocol, _) = invoke(&["not-a-command"], None);
    assert_eq!(
        (code, protocol["category"].as_str()),
        (1, Some("native_supervisor_usage_refused"))
    );
    let (code, protocol, _) = invoke(&[], None);
    assert_eq!(
        (code, protocol["category"].as_str()),
        (1, Some("native_supervisor_usage_refused"))
    );
    let oversized_budget = json!({
        "version": 1, "argv": [python()], "cwd": env!("CARGO_MANIFEST_DIR"), "label": "x",
        "report_root": unique_root("budget").to_string_lossy(),
        "action_budget_ms": 3_600_001, "redactions": [],
    });
    let (code, protocol, _) = invoke(&["run-action"], Some(&oversized_budget.to_string()));
    assert_eq!(
        (code, protocol["category"].as_str()),
        (1, Some("native_supervisor_request_refused"))
    );
}

#[test]
fn report_collision_is_refused_before_the_action_runs() {
    let root = unique_root("collision");
    std::fs::create_dir_all(&root).expect("root");
    std::fs::write(root.join("collision.log"), b"retained").expect("log");
    let script = format!("from pathlib import Path; Path({root:?}).joinpath('marker').touch()");
    let request = request_for(&root, "collision", &script, 30_000, &[]);
    let (code, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(
        (code, protocol["category"].as_str()),
        (1, Some("native_report_collision"))
    );
    assert!(
        !root.join("marker").exists(),
        "the refused action must never run"
    );
    assert_eq!(
        std::fs::read(root.join("collision.log")).expect("log"),
        b"retained"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn report_write_failure_is_non_success() {
    let file_root = std::env::temp_dir().join(format!("dw-fs-blocked-file-{}", std::process::id()));
    std::fs::write(&file_root, b"blocked").expect("file in the way");
    let request = json!({
        "version": 1, "argv": [python()], "cwd": env!("CARGO_MANIFEST_DIR"), "label": "blocked",
        "report_root": file_root.to_string_lossy(), "action_budget_ms": 30_000, "redactions": [],
    });
    let (code, protocol, _) = invoke(&["run-action"], Some(&request.to_string()));
    assert_eq!(
        (code, protocol["category"].as_str()),
        (1, Some("native_report_write_failed"))
    );
    let _ = std::fs::remove_file(&file_root);
}

#[test]
fn unrelated_sentinel_survives_owned_tree_cleanup() {
    let mut sentinel = direct_child("import time; time.sleep(30)");
    let root = unique_root("sentinel");
    let request = request_for(&root, "sentinel", "import time; time.sleep(30)", 700, &[]);
    let (_, protocol, _) = invoke(&["run-action"], Some(&request));
    assert_eq!(protocol["status"], "harness_timeout");
    assert!(
        process_alive(sentinel.id()),
        "unrelated processes must never be signalled"
    );
    let _ = sentinel.kill();
    let _ = sentinel.wait();
    assert!(!process_alive(sentinel.id()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn service_check_refuses_unowned_containers_and_open_flags() {
    let cases = vec![
        (
            service_arguments("other", &["--version-prefix", "17."]),
            "native_service_unqualified",
        ),
        (
            service_arguments("dwpg-r3", &["--ci", "--version-prefix", "17."]),
            "native_service_unqualified",
        ),
        (
            service_arguments(
                "dwpg-r3",
                &["--version-prefix", "17.", "--version-exact", "17.11"],
            ),
            "native_supervisor_usage_refused",
        ),
        (vec!["service-check"], "native_supervisor_usage_refused"),
        // Deterministic with or without the named container present: an absent
        // container fails the inspect probe, a present one fails the version.
        (
            service_arguments("dwpg-r3", &["--version-exact", "0.0"]),
            "native_service_unqualified",
        ),
        (
            service_arguments("dwpg", &["--version-exact", "0.0"]),
            "native_service_unqualified",
        ),
    ];
    for (arguments, category) in &cases {
        let (code, protocol, _) = invoke(arguments, None);
        assert_eq!((code, protocol["category"].as_str()), (1, Some(*category)));
    }
}

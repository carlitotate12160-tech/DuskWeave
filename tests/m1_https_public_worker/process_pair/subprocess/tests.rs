use super::{MAX_STREAM_BYTES, run_bounded, run_bounded_with_timeout, run_captured, wait_child};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_temp_dir(name: &str) -> TempDir {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("dw_subproc_{name}_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

fn read_ready_pid(ready: &Path) -> u32 {
    std::fs::read_to_string(ready)
        .expect("child should have signaled readiness before returning")
        .trim()
        .parse()
        .expect("valid pid in ready file")
}

fn assert_reaped(pid: u32) {
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "child {pid} must be terminated and reaped"
    );
}

fn sh_ready_cmd(script: &str, ready: &Path) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", script, "_"]).arg(ready);
    cmd
}

#[test]
fn unconsumed_large_stdin_times_out_and_reaps_child() {
    let dir = make_temp_dir("stall");
    let ready = dir.0.join("ready.pid");
    let mut cmd = sh_ready_cmd(
        "printf '%s\\n' \"$$\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\" && exec /bin/sleep 60",
        &ready,
    );
    let input = vec![b'x'; 512 * 1024];
    let start = Instant::now();
    let res = run_bounded_with_timeout(&mut cmd, Some(&input), "stall", Duration::from_millis(500));
    let elapsed = start.elapsed();
    assert!(res.is_err(), "expected timeout, got {res:?}");
    assert!(
        elapsed < Duration::from_secs(5),
        "timeout took too long: {elapsed:?}"
    );
    let pid = read_ready_pid(&ready);
    assert_reaped(pid);
}

#[test]
fn concurrent_large_stdout_stderr_and_stdin_completes() {
    let dir = make_temp_dir("concurrent");
    let out_file = dir.0.join("out.txt");
    let err_file = dir.0.join("err.txt");
    let out_body = format!("OUT_HEAD:{}:OUT_TAIL", "o".repeat(128 * 1024));
    let err_body = format!("ERR_HEAD:{}:ERR_TAIL", "e".repeat(128 * 1024));
    std::fs::write(&out_file, &out_body).unwrap();
    std::fs::write(&err_file, &err_body).unwrap();

    let mut cmd = Command::new("/bin/sh");
    cmd.args([
        "-c",
        "cat \"$1\" && cat \"$2\" >&2 && exec /usr/bin/wc -c",
        "_",
    ])
    .arg(&out_file)
    .arg(&err_file);
    let input = vec![b'i'; 256 * 1024];
    let (status, out, err) = run_captured(
        &mut cmd,
        Some(&input),
        "concurrent",
        Duration::from_secs(10),
    )
    .expect("concurrent stdin/stdout/stderr should complete without deadlock");
    assert!(status.success());
    assert!(out.starts_with("OUT_HEAD:"));
    assert!(out.contains(":OUT_TAIL"));
    assert!(out.trim_end().ends_with("262144"), "unexpected stdout tail");
    assert!(err.starts_with("ERR_HEAD:"));
    assert!(err.ends_with(":ERR_TAIL"));
    assert_eq!(err.len(), err_body.len());

    let mut cmd_nz = Command::new("/bin/sh");
    cmd_nz
        .args(["-c", "cat \"$1\" && cat \"$2\" >&2 && exit 4", "_"])
        .arg(&out_file)
        .arg(&err_file);
    let err_msg =
        run_bounded_with_timeout(&mut cmd_nz, None, "concurrent-nz", Duration::from_secs(10))
            .unwrap_err();
    assert!(err_msg.contains("ERR_HEAD:") && err_msg.contains(":ERR_TAIL"));
}

#[test]
fn early_exit_broken_input_pipe_and_nonzero_exit_fail_and_reap() {
    let dir = make_temp_dir("early_exit");
    let input = vec![b'x'; 512 * 1024];

    let ready_nz = dir.0.join("nz.pid");
    let mut cmd_nz = sh_ready_cmd(
        "printf '%s\\n' \"$$\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\" && printf 'early-err\\n' >&2 && exit 7",
        &ready_nz,
    );
    let start = Instant::now();
    let res_nz = run_bounded_with_timeout(
        &mut cmd_nz,
        Some(&input),
        "early-nz",
        Duration::from_secs(10),
    );
    assert!(res_nz.is_err());
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_reaped(read_ready_pid(&ready_nz));

    let ready_ok = dir.0.join("ok.pid");
    let mut cmd_ok = sh_ready_cmd(
        "printf '%s\\n' \"$$\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\" && exit 0",
        &ready_ok,
    );
    let res_ok = run_bounded_with_timeout(
        &mut cmd_ok,
        Some(&input),
        "early-ok",
        Duration::from_secs(10),
    );
    assert!(
        res_ok.is_err(),
        "unconsumed input on early exit 0 must fail"
    );
    assert_reaped(read_ready_pid(&ready_ok));

    let mut cmd_err = Command::new("/bin/sh");
    cmd_err.args(["-c", "printf 'boom-stderr-tail' >&2; exit 3"]);
    let err =
        run_bounded_with_timeout(&mut cmd_err, None, "nz", Duration::from_secs(5)).unwrap_err();
    assert!(err.contains("boom-stderr-tail"), "unexpected err: {err}");
}

#[test]
fn output_overflow_refused_and_child_reaped_while_exact_limit_succeeds() {
    let dir = make_temp_dir("overflow");
    for (idx, redir) in ["", " >&2"].into_iter().enumerate() {
        let ready = dir.0.join(format!("ov_{idx}.pid"));
        let script = format!(
            "printf '%s\\n' \"$$\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\" && exec /usr/bin/head -c 10485760 /dev/zero{redir}"
        );
        let mut cmd = sh_ready_cmd(&script, &ready);
        let start = Instant::now();
        let res = run_bounded_with_timeout(&mut cmd, None, "overflow", Duration::from_secs(10));
        assert!(res.is_err(), "overflow stream {idx} must fail");
        assert!(start.elapsed() < Duration::from_secs(5));
        assert_reaped(read_ready_pid(&ready));
    }

    let mut exact = Command::new("/usr/bin/head");
    exact.args(["-c", &MAX_STREAM_BYTES.to_string(), "/dev/zero"]);
    let out = run_bounded_with_timeout(&mut exact, None, "exact", Duration::from_secs(5))
        .expect("exact 1 MiB output must succeed");
    assert_eq!(out.len(), MAX_STREAM_BYTES);
}

#[test]
fn closed_stdio_hanging_child_and_wait_child_timeout_reap() {
    let dir = make_temp_dir("closed_stdio");
    let ready = dir.0.join("closed.pid");
    let mut cmd = sh_ready_cmd(
        "printf '%s\\n' \"$$\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\" && exec >&- 2>&- && exec /bin/sleep 60",
        &ready,
    );
    let res = run_bounded_with_timeout(&mut cmd, None, "closed", Duration::from_millis(500));
    assert!(res.is_err());
    assert_reaped(read_ready_pid(&ready));

    let mut child = Command::new("/bin/sleep")
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    let w = wait_child(&mut child, "direct-wait", Duration::from_millis(150));
    assert!(w.is_err());
    assert_reaped(pid);

    let mut cat = Command::new("/bin/cat");
    let echoed = run_bounded(&mut cat, Some(b"stdin-eof-closed"), "cat").unwrap();
    assert_eq!(echoed, "stdin-eof-closed");
}

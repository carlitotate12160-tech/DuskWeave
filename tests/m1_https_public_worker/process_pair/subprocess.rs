//! Supervised tool-subprocess execution for the Linux Docker executable-pair harness.
use std::io::{ErrorKind, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_STREAM_BYTES: usize = 1024 * 1024;
const READ_CHUNK_BYTES: usize = 8192;

struct ChildPipes {
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
}

struct IoWorkers {
    stdin: Option<JoinHandle<Result<(), String>>>,
    stdout: JoinHandle<Result<Vec<u8>, String>>,
    stderr: JoinHandle<Result<Vec<u8>, String>>,
}

fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn wait_child(
    child: &mut Child,
    lbl: &str,
    lim: Duration,
) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + lim;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return Ok(st),
            Ok(None) => {}
            Err(e) => {
                kill_and_reap(child);
                return Err(format!("{lbl}: {e}"));
            }
        }
        let Some(rem) = deadline.checked_duration_since(Instant::now()) else {
            kill_and_reap(child);
            return Err(format!("{lbl} timed out"));
        };
        if rem.is_zero() {
            kill_and_reap(child);
            return Err(format!("{lbl} timed out"));
        }
        thread::sleep(rem.min(Duration::from_millis(10)));
    }
}

fn take_pipes(child: &mut Child, need_stdin: bool, lbl: &str) -> Result<ChildPipes, String> {
    let stdin = if need_stdin {
        Some(child.stdin.take().ok_or_else(|| format!("{lbl} stdin"))?)
    } else {
        None
    };
    let stdout = child.stdout.take().ok_or_else(|| format!("{lbl} stdout"))?;
    let stderr = child.stderr.take().ok_or_else(|| format!("{lbl} stderr"))?;
    Ok(ChildPipes {
        stdin,
        stdout,
        stderr,
    })
}

fn spawn_stdin_writer(
    mut pipe: ChildStdin,
    bytes: Vec<u8>,
    lbl: String,
    tx: mpsc::Sender<Result<(), String>>,
) -> JoinHandle<Result<(), String>> {
    thread::spawn(move || {
        let res = pipe
            .write_all(&bytes)
            .map_err(|e| format!("{lbl} write: {e}"));
        drop(pipe);
        let _ = tx.send(res.clone());
        res
    })
}

fn read_stream_limited<R: Read>(
    mut pipe: R,
    stream: &'static str,
    lbl: &str,
) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; READ_CHUNK_BYTES];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => return Ok(buf),
            Ok(n) => {
                if buf.len() + n > MAX_STREAM_BYTES {
                    return Err(format!("{lbl} {stream} exceeded {MAX_STREAM_BYTES} bytes"));
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("{lbl} {stream} read: {e}")),
        }
    }
}

fn spawn_stream_reader<R: Read + Send + 'static>(
    pipe: R,
    stream: &'static str,
    lbl: String,
    tx: mpsc::Sender<Result<(), String>>,
) -> JoinHandle<Result<Vec<u8>, String>> {
    thread::spawn(move || {
        let res = read_stream_limited(pipe, stream, &lbl);
        let _ = tx.send(res.as_ref().map(|_| ()).map_err(Clone::clone));
        res
    })
}

struct JoinedIo {
    stdin: Result<(), String>,
    stdout: Result<Vec<u8>, String>,
    stderr: Result<Vec<u8>, String>,
}

impl IoWorkers {
    fn spawn(
        pipes: ChildPipes,
        stdin_bytes: Option<Vec<u8>>,
        lbl: &str,
        tx: &mpsc::Sender<Result<(), String>>,
    ) -> Self {
        let stdin = pipes
            .stdin
            .zip(stdin_bytes)
            .map(|(pipe, bytes)| spawn_stdin_writer(pipe, bytes, lbl.to_owned(), tx.clone()));
        let stdout = spawn_stream_reader(pipes.stdout, "stdout", lbl.to_owned(), tx.clone());
        let stderr = spawn_stream_reader(pipes.stderr, "stderr", lbl.to_owned(), tx.clone());
        Self {
            stdin,
            stdout,
            stderr,
        }
    }

    fn count(&self) -> usize {
        2 + usize::from(self.stdin.is_some())
    }

    fn join_raw(self, lbl: &str) -> Result<JoinedIo, String> {
        let stdin = match self.stdin {
            Some(h) => h
                .join()
                .map_err(|_| format!("{lbl} stdin worker panicked"))?,
            None => Ok(()),
        };
        let stdout = self
            .stdout
            .join()
            .map_err(|_| format!("{lbl} stdout worker panicked"))?;
        let stderr = self
            .stderr
            .join()
            .map_err(|_| format!("{lbl} stderr worker panicked"))?;
        Ok(JoinedIo {
            stdin,
            stdout,
            stderr,
        })
    }

    fn join_all(self, lbl: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
        let joined = self.join_raw(lbl)?;
        joined.stdin?;
        Ok((joined.stdout?, joined.stderr?))
    }

    fn join_after_abort(self, lbl: &str) -> Result<Option<String>, String> {
        let joined = self.join_raw(lbl)?;
        let _ = (joined.stdin, joined.stdout);
        Ok(joined
            .stderr
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .filter(|s| !s.trim().is_empty()))
    }
}

fn await_io(
    rx: &mpsc::Receiver<Result<(), String>>,
    expected: usize,
    deadline: Instant,
    lbl: &str,
) -> Result<(), String> {
    for _ in 0..expected {
        let Some(rem) = deadline.checked_duration_since(Instant::now()) else {
            return Err(format!("{lbl} timed out"));
        };
        match rx.recv_timeout(rem) {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => return Err(msg),
            Err(mpsc::RecvTimeoutError::Timeout) => return Err(format!("{lbl} timed out")),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(format!("{lbl} io worker disconnected"));
            }
        }
    }
    Ok(())
}

fn run_captured(
    cmd: &mut Command,
    stdin: Option<&[u8]>,
    lbl: &str,
    lim: Duration,
) -> Result<(ExitStatus, String, String), String> {
    let deadline = Instant::now() + lim;
    let si = if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    cmd.stdin(si).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("{lbl}: {e}"))?;
    let pipes = match take_pipes(&mut child, stdin.is_some(), lbl) {
        Ok(p) => p,
        Err(e) => {
            kill_and_reap(&mut child);
            return Err(e);
        }
    };
    let (tx, rx) = mpsc::channel();
    let workers = IoWorkers::spawn(pipes, stdin.map(ToOwned::to_owned), lbl, &tx);
    let expected = workers.count();
    let io_res = await_io(&rx, expected, deadline, lbl);
    drop(rx);
    if let Err(err) = io_res {
        kill_and_reap(&mut child);
        let stderr_diag = workers.join_after_abort(lbl)?;
        return match stderr_diag {
            Some(diag) if !err.contains("timed out") && !err.contains("exceeded") => {
                Err(format!("{err}: {diag}"))
            }
            _ => Err(err),
        };
    }
    let rem = deadline.saturating_duration_since(Instant::now());
    let status = match wait_child(&mut child, lbl, rem) {
        Ok(st) => st,
        Err(err) => {
            let _ = workers.join_after_abort(lbl)?;
            return Err(err);
        }
    };
    let (out_bytes, err_bytes) = workers.join_all(lbl)?;
    let out = String::from_utf8(out_bytes).map_err(|e| format!("{lbl} stdout utf8: {e}"))?;
    let err = String::from_utf8(err_bytes).map_err(|e| format!("{lbl} stderr utf8: {e}"))?;
    Ok((status, out, err))
}

fn run_bounded_with_timeout(
    cmd: &mut Command,
    stdin: Option<&[u8]>,
    lbl: &str,
    lim: Duration,
) -> Result<String, String> {
    let (status, out, err) = run_captured(cmd, stdin, lbl, lim)?;
    status
        .success()
        .then_some(out)
        .ok_or_else(|| format!("{lbl} ({status:?}): {err}"))
}

pub(super) fn run_bounded(
    cmd: &mut Command,
    stdin: Option<&[u8]>,
    lbl: &str,
) -> Result<String, String> {
    run_bounded_with_timeout(cmd, stdin, lbl, DEFAULT_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_STREAM_BYTES, run_bounded, run_bounded_with_timeout, run_captured, wait_child,
    };
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
        let res =
            run_bounded_with_timeout(&mut cmd, Some(&input), "stall", Duration::from_millis(500));
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
}

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

struct BoundedOutput {
    bytes: Vec<u8>,
    stream: &'static str,
}

impl BoundedOutput {
    fn new(stream: &'static str) -> Self {
        Self {
            bytes: Vec::new(),
            stream,
        }
    }

    fn append(&mut self, chunk: &[u8], lbl: &str) -> Result<(), String> {
        if self.bytes.len() + chunk.len() > MAX_STREAM_BYTES {
            return Err(format!(
                "{lbl} {} exceeded {MAX_STREAM_BYTES} bytes",
                self.stream
            ));
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }
}

fn read_next_chunk<R: Read>(
    pipe: &mut R,
    chunk: &mut [u8],
    stream: &'static str,
    lbl: &str,
) -> Result<usize, String> {
    loop {
        match pipe.read(chunk) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("{lbl} {stream} read: {e}")),
        }
    }
}

fn read_stream_limited<R: Read>(
    mut pipe: R,
    stream: &'static str,
    lbl: &str,
) -> Result<Vec<u8>, String> {
    let mut out = BoundedOutput::new(stream);
    let mut chunk = [0u8; READ_CHUNK_BYTES];
    loop {
        let n = read_next_chunk(&mut pipe, &mut chunk, stream, lbl)?;
        if n == 0 {
            return Ok(out.bytes);
        }
        out.append(&chunk[..n], lbl)?;
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

struct SupervisedChild {
    child: Child,
    workers: IoWorkers,
}

impl SupervisedChild {
    fn spawn(
        cmd: &mut Command,
        stdin: Option<&[u8]>,
        lbl: &str,
    ) -> Result<(Self, mpsc::Receiver<Result<(), String>>), String> {
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
        Ok((Self { child, workers }, rx))
    }

    fn abort_after_io_error(mut self, err: String, lbl: &str) -> String {
        kill_and_reap(&mut self.child);
        match self.workers.join_after_abort(lbl) {
            Ok(diag) => format_io_abort_error(err, diag),
            Err(panic_err) => panic_err,
        }
    }

    fn wait_and_collect(
        mut self,
        deadline: Instant,
        lbl: &str,
    ) -> Result<(ExitStatus, Vec<u8>, Vec<u8>), String> {
        let rem = deadline.saturating_duration_since(Instant::now());
        let status = match wait_child(&mut self.child, lbl, rem) {
            Ok(st) => st,
            Err(err) => {
                let _ = self.workers.join_after_abort(lbl)?;
                return Err(err);
            }
        };
        let (out_bytes, err_bytes) = self.workers.join_all(lbl)?;
        Ok((status, out_bytes, err_bytes))
    }
}

fn format_io_abort_error(err: String, stderr_diag: Option<String>) -> String {
    match stderr_diag {
        Some(diag) if !err.contains("timed out") && !err.contains("exceeded") => {
            format!("{err}: {diag}")
        }
        _ => err,
    }
}

fn decode_captured_output(
    out_bytes: Vec<u8>,
    err_bytes: Vec<u8>,
    lbl: &str,
) -> Result<(String, String), String> {
    let out = String::from_utf8(out_bytes).map_err(|e| format!("{lbl} stdout utf8: {e}"))?;
    let err = String::from_utf8(err_bytes).map_err(|e| format!("{lbl} stderr utf8: {e}"))?;
    Ok((out, err))
}

fn run_captured(
    cmd: &mut Command,
    stdin: Option<&[u8]>,
    lbl: &str,
    lim: Duration,
) -> Result<(ExitStatus, String, String), String> {
    let deadline = Instant::now() + lim;
    let (supervised, rx) = SupervisedChild::spawn(cmd, stdin, lbl)?;
    let io_res = await_io(&rx, supervised.workers.count(), deadline, lbl);
    drop(rx);
    if let Err(err) = io_res {
        return Err(supervised.abort_after_io_error(err, lbl));
    }
    let (status, out_bytes, err_bytes) = supervised.wait_and_collect(deadline, lbl)?;
    let (out, err) = decode_captured_output(out_bytes, err_bytes, lbl)?;
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
#[path = "subprocess/tests.rs"]
mod tests;

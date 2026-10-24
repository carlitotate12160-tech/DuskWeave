//! Owned child/group lifetime, bounded capture and closed supervision.
//!
//! The direct child is assigned to an OS-owned boundary (a Windows Job Object
//! established before initial execution, or a Unix process group) so the owned
//! tree stays reachable after the direct parent exits. Only the direct
//! child's disposition is treated as the action result; wrapper group state
//! is never taken as whole-tree completion proof, and termination of the
//! owned tree is signalled through the wrapper rather than helper processes.

use crate::report::{self, CAPTURE_COMPLETE, CLEANUP_COMPLETED, CLEANUP_UNKNOWN, CaptureStream};
#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};

/// Bounded transient memory per captured output stream.
pub const CAPTURE_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const PROGRESS_INTERVAL_SECS: u64 = 15;
const COLLECTION_WINDOW: Duration = Duration::from_millis(2000);
const REAP_WINDOW: Duration = Duration::from_secs(1);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const TIMEOUT_EXIT: i32 = 124;
const CHUNK_BYTES: usize = 64 * 1024;

/// A validated action request admitted from the private stdin protocol.
pub struct ActionRequest {
    pub argv: Vec<String>,
    pub cwd: String,
    pub label: String,
    pub report_root: String,
    pub action_budget_ms: u64,
    pub redactions: Vec<String>,
}

/// Supervision facts for one finished action attempt.
pub struct SupervisedAction {
    pub status: &'static str,
    pub exit: i32,
    pub child_exit: Option<i32>,
    pub elapsed_ms: u64,
    pub capture: &'static str,
    pub cleanup: &'static str,
    pub omissions: Vec<&'static str>,
    pub stdout: Option<CaptureStream>,
    pub stderr: Option<CaptureStream>,
}

impl SupervisedAction {
    /// The single closed result line emitted on stdout after publication.
    pub fn protocol(&self) -> Value {
        json!({
            "ok": true,
            "status": self.status,
            "exit": self.exit,
            "child_exit": self.child_exit,
            "elapsed_ms": self.elapsed_ms,
            "capture": self.capture,
            "cleanup": self.cleanup,
            "timing_origin": "request_admission",
            "omissions": self.omissions,
        })
    }
}

enum ParentEvent {
    Exited(ExitStatus),
    TimedOut,
    Lost,
}

struct Readers {
    stdout: Option<JoinHandle<CaptureStream>>,
    stderr: Option<JoinHandle<CaptureStream>>,
}

struct Collection {
    stdout: Option<CaptureStream>,
    stderr: Option<CaptureStream>,
}

/// Supervise one action and publish its redacted log and closed metadata.
pub fn run_action(request: &ActionRequest) -> Result<SupervisedAction, &'static str> {
    let report_root = Path::new(&request.report_root);
    if report::collision(report_root, &request.label) {
        return Err("native_report_collision");
    }
    let admitted = Instant::now();
    let mut child = spawn_owned(request)?;
    let readers = Readers::start(child.as_mut());
    let deadline = admitted + Duration::from_millis(request.action_budget_ms);
    let event = wait_for_parent(child.as_mut(), &request.label, admitted, deadline);
    let supervised = settle(child.as_mut(), readers, event, admitted);
    publish(report_root, request, &supervised)?;
    Ok(supervised)
}

fn spawn_owned(request: &ActionRequest) -> Result<Box<dyn ChildWrapper>, &'static str> {
    let mut command = Command::new(&request.argv[0]);
    command
        .args(&request.argv[1..])
        .current_dir(&request.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut wrapped = CommandWrap::from(command);
    #[cfg(windows)]
    wrapped.wrap(JobObject);
    #[cfg(unix)]
    wrapped.wrap(ProcessGroup::leader());
    wrapped.spawn().map_err(|_| "native_action_spawn_failed")
}

/// Poll only the direct child for the parent disposition; group or job state
/// is never consumed as completion evidence on this path.
fn wait_for_parent(
    child: &mut dyn ChildWrapper,
    label: &str,
    admitted: Instant,
    deadline: Instant,
) -> ParentEvent {
    let mut progress_step = 0;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return ParentEvent::Exited(status),
            Ok(None) => {}
            Err(_) => return ParentEvent::Lost,
        }
        if Instant::now() >= deadline {
            return ParentEvent::TimedOut;
        }
        emit_progress(label, admitted.elapsed(), &mut progress_step);
        sleep(POLL_INTERVAL);
    }
}

/// Progress lines stay closed — label, running status and elapsed seconds —
/// and are emitted once per completed interval, never per poll.
fn progress_line(label: &str, elapsed: Duration, last_step: &mut u64) -> Option<String> {
    let step = elapsed.as_secs() / PROGRESS_INTERVAL_SECS;
    if step <= *last_step {
        return None;
    }
    *last_step = step;
    Some(
        json!({ "action": label, "status": "running", "elapsed_seconds": elapsed.as_secs() })
            .to_string(),
    )
}

fn emit_progress(label: &str, elapsed: Duration, last_step: &mut u64) {
    if let Some(line) = progress_line(label, elapsed, last_step) {
        let _ = writeln!(std::io::stderr(), "{line}");
    }
}

impl Readers {
    fn start(child: &mut dyn ChildWrapper) -> Readers {
        Readers {
            stdout: child.stdout().take().map(spawn_reader),
            stderr: child.stderr().take().map(spawn_reader),
        }
    }
}

fn spawn_reader(mut pipe: impl Read + Send + 'static) -> JoinHandle<CaptureStream> {
    std::thread::spawn(move || {
        let mut stream = CaptureStream {
            bytes: Vec::new(),
            finished: false,
            overflowed: false,
        };
        let mut chunk = [0u8; CHUNK_BYTES];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => append_bounded(&mut stream, &chunk[..read]),
            }
        }
        stream.finished = true;
        stream
    })
}

fn append_bounded(stream: &mut CaptureStream, chunk: &[u8]) {
    if stream.bytes.len() + chunk.len() > CAPTURE_LIMIT_BYTES {
        // Keep draining so the action never blocks on a full pipe, but retain
        // nothing further: partial content could not be published safely.
        stream.overflowed = true;
        return;
    }
    stream.bytes.extend_from_slice(chunk);
}

fn settle(
    child: &mut dyn ChildWrapper,
    readers: Readers,
    event: ParentEvent,
    admitted: Instant,
) -> SupervisedAction {
    let parent_disposed = matches!(event, ParentEvent::Exited(_));
    let (mut status, exit, child_exit, collection) = match event {
        ParentEvent::TimedOut => timeout_settlement(child, readers),
        ParentEvent::Exited(status) => exit_settlement(child, readers, status),
        ParentEvent::Lost => lost_settlement(child, readers),
    };
    let (capture, omissions) =
        report::capture_state(collection.stdout.as_ref(), collection.stderr.as_ref());
    let cleanup = cleanup_state(&collection, parent_disposed || child_exit.is_some());
    demote_incomplete(&mut status, capture, cleanup);
    SupervisedAction {
        status,
        exit,
        child_exit,
        elapsed_ms: admitted.elapsed().as_millis() as u64,
        capture,
        cleanup,
        omissions,
        stdout: collection.stdout,
        stderr: collection.stderr,
    }
}

/// A successful direct child with unknown cleanup or incomplete capture is
/// non-success and can never present as PASS.
fn demote_incomplete(status: &mut &'static str, capture: &str, cleanup: &str) {
    let incomplete = capture != CAPTURE_COMPLETE || cleanup != CLEANUP_COMPLETED;
    if *status == "completed" && incomplete {
        *status = "action_failed";
    }
}

type Settlement = (&'static str, i32, Option<i32>, Collection);

fn timeout_settlement(child: &mut dyn ChildWrapper, readers: Readers) -> Settlement {
    let collection = collect_after_kill(child, readers);
    let reaped = reap_within(child);
    ("harness_timeout", TIMEOUT_EXIT, reaped, collection)
}

fn exit_settlement(
    child: &mut dyn ChildWrapper,
    readers: Readers,
    status: ExitStatus,
) -> Settlement {
    let code = status.code();
    let action_status = if code == Some(0) {
        "completed"
    } else {
        "action_failed"
    };
    (
        action_status,
        code.unwrap_or(1),
        code,
        collect_after_exit(child, readers),
    )
}

fn lost_settlement(child: &mut dyn ChildWrapper, readers: Readers) -> Settlement {
    let collection = collect_after_kill(child, readers);
    reap_within(child);
    ("action_failed", 1, None, collection)
}

fn cleanup_state(collection: &Collection, parent_disposed: bool) -> &'static str {
    let collected = collection.stdout.is_some() && collection.stderr.is_some();
    if collected && parent_disposed {
        CLEANUP_COMPLETED
    } else {
        CLEANUP_UNKNOWN
    }
}

/// Join one reader if it finishes before the deadline; otherwise leave its
/// handle behind so the worker is abandoned instead of blocking the tool.
fn join_within(
    slot: &mut Option<JoinHandle<CaptureStream>>,
    deadline: Instant,
) -> Option<CaptureStream> {
    while Instant::now() < deadline {
        if let Some(handle) = slot.take() {
            if handle.is_finished() {
                return handle.join().ok();
            }
            *slot = Some(handle);
        }
        sleep(POLL_INTERVAL);
    }
    None
}

fn collect_after_exit(child: &mut dyn ChildWrapper, mut readers: Readers) -> Collection {
    let deadline = Instant::now() + COLLECTION_WINDOW;
    let grace = Instant::now() + COLLECTION_WINDOW / 2;
    let mut stdout = join_within(&mut readers.stdout, grace);
    let mut stderr = join_within(&mut readers.stderr, grace);
    if stdout.is_none() || stderr.is_none() {
        let _ = child.start_kill();
        stdout = stdout.or_else(|| join_within(&mut readers.stdout, deadline));
        stderr = stderr.or_else(|| join_within(&mut readers.stderr, deadline));
    }
    Collection { stdout, stderr }
}

fn collect_after_kill(child: &mut dyn ChildWrapper, mut readers: Readers) -> Collection {
    let _ = child.start_kill();
    let deadline = Instant::now() + COLLECTION_WINDOW;
    Collection {
        stdout: join_within(&mut readers.stdout, deadline),
        stderr: join_within(&mut readers.stderr, deadline),
    }
}

fn reap_within(child: &mut dyn ChildWrapper) -> Option<i32> {
    let deadline = Instant::now() + REAP_WINDOW;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) => {}
            Err(_) => return None,
        }
        if Instant::now() >= deadline {
            return None;
        }
        sleep(POLL_INTERVAL);
    }
}

fn publish(
    report_root: &Path,
    request: &ActionRequest,
    supervised: &SupervisedAction,
) -> Result<(), &'static str> {
    let stderr =
        report::stream_publication(supervised.stderr.as_ref(), &request.redactions, "stderr");
    let stdout =
        report::stream_publication(supervised.stdout.as_ref(), &request.redactions, "stdout");
    let mut log = stderr;
    log.push(b'\n');
    log.extend_from_slice(&stdout);
    let fields = report::MetadataFields {
        status: supervised.status,
        exit: supervised.exit,
        child_exit: supervised.child_exit,
        elapsed_ms: supervised.elapsed_ms,
        action_budget_ms: request.action_budget_ms,
        capture: supervised.capture,
        cleanup: supervised.cleanup,
        omissions: &supervised.omissions,
    };
    report::publish(
        report_root,
        &request.label,
        &log,
        &report::metadata_json(&fields),
    )
    .map_err(|_| "native_report_write_failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_emit_once_per_completed_interval() {
        let mut step = 0;
        assert!(progress_line("act", Duration::from_secs(14), &mut step).is_none());
        let line = progress_line("act", Duration::from_secs(15), &mut step).expect("line");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).expect("json"),
            json!({ "action": "act", "status": "running", "elapsed_seconds": 15 })
        );
        assert!(progress_line("act", Duration::from_secs(29), &mut step).is_none());
        let next = progress_line("act", Duration::from_secs(31), &mut step).expect("line");
        assert!(next.contains("\"elapsed_seconds\":31"));
    }
}

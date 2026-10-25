//! Owned child/group lifetime, bounded capture and closed supervision.
//!
//! The direct child is assigned to an OS-owned boundary (a Windows Job Object
//! established before initial execution, or a Unix process group) so the owned
//! tree stays reachable after the direct parent exits. Only the direct
//! child's disposition is treated as the action result; wrapper group state
//! is never taken as whole-tree completion proof, and termination of the
//! owned tree is signalled through the wrapper rather than helper processes.

use crate::report::{
    self, CAPTURE_COMPLETE, CLEANUP_COMPLETED, CLEANUP_UNKNOWN, CaptureStream, SupervisedAction,
};
#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};

/// Bounded transient memory per captured output stream.
pub const CAPTURE_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const COLLECTION_WINDOW: Duration = Duration::from_millis(2000);
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

#[derive(Clone, Copy)]
enum ParentEvent {
    Exited(ExitStatus),
    TimedOut,
    Lost,
}

struct Readers {
    stdout: Option<JoinHandle<CaptureStream>>,
    stderr: Option<JoinHandle<CaptureStream>>,
}

#[derive(Default)]
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
    let mut command = Command::new(&request.argv[0]);
    command.args(&request.argv[1..]).current_dir(&request.cwd);
    let supervised = capture_process(
        command,
        &request.label,
        Duration::from_millis(request.action_budget_ms),
    )?;
    report::publish_action(
        report_root,
        &request.label,
        &supervised,
        &request.redactions,
        request.action_budget_ms,
    )?;
    Ok(supervised)
}

/// Shared report-free operation for actions and the two fixed service probes.
pub(super) fn capture_process(
    command: Command,
    label: &str,
    budget: Duration,
) -> Result<SupervisedAction, &'static str> {
    let admitted = Instant::now();
    let mut child = spawn_owned(command)?;
    let readers = Readers::start(child.as_mut());
    let event = wait_for_parent(child.as_mut(), label, admitted, admitted + budget);
    Ok(settle(child, readers, event, admitted))
}

fn spawn_owned(mut command: Command) -> Result<Box<dyn ChildWrapper>, &'static str> {
    command
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
        // spawn_owned installs exactly one wrapper over the native Child.
        // Retain that wrapper for boundary termination; never poll its job port.
        match child.inner_mut().try_wait() {
            Ok(Some(status)) => return ParentEvent::Exited(status),
            Ok(None) => {}
            Err(_) => return ParentEvent::Lost,
        }
        if Instant::now() >= deadline {
            return ParentEvent::TimedOut;
        }
        report::emit_progress(label, admitted.elapsed(), &mut progress_step);
        sleep(POLL_INTERVAL);
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
                Ok(0) => {
                    stream.finished = true;
                    break;
                }
                Err(_) => break,
                Ok(read) => append_bounded(&mut stream, &chunk[..read]),
            }
        }
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

struct Settlement {
    termination_ok: bool,
    parent_disposed: bool,
    child_exit: Option<i32>,
    collection: Collection,
}

fn settle(
    mut child: Box<dyn ChildWrapper>,
    readers: Readers,
    event: ParentEvent,
    admitted: Instant,
) -> SupervisedAction {
    // One deadline owns termination acknowledgment, parent disposal and readers.
    // Even native cleanup/drop stays in this unscoped worker. Never join it.
    let deadline = Instant::now() + COLLECTION_WINDOW;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let termination_ok = termination_acknowledged(child.start_kill());
        let parent = dispose_parent(child.as_mut(), &event, deadline);
        let mut readers = readers;
        let collection = Collection {
            stdout: join_within(&mut readers.stdout, deadline),
            stderr: join_within(&mut readers.stderr, deadline),
        };
        drop(child);
        let _ = sender.send(Settlement {
            termination_ok,
            parent_disposed: parent.is_some(),
            child_exit: parent.and_then(|status| status.code()),
            collection,
        });
    });
    let settled = receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok();
    result_from_settlement(&event, settled, admitted)
}

fn termination_acknowledged(result: std::io::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        // ProcessGroup::start_kill signals the owned pgid; ESRCH specifically
        // establishes that this group is already absent. No other error does.
        #[cfg(unix)]
        Err(error) => error.raw_os_error() == Some(3),
        #[cfg(not(unix))]
        Err(_) => false,
    }
}

fn dispose_parent(
    child: &mut dyn ChildWrapper,
    event: &ParentEvent,
    deadline: Instant,
) -> Option<ExitStatus> {
    if let ParentEvent::Exited(status) = event {
        return Some(*status);
    }
    while Instant::now() < deadline {
        match child.inner_mut().try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => sleep(POLL_INTERVAL),
            Err(_) => return None,
        }
    }
    None
}

fn result_from_settlement(
    event: &ParentEvent,
    settled: Option<Settlement>,
    admitted: Instant,
) -> SupervisedAction {
    let cleanup_ok = settled.as_ref().is_some_and(|s| {
        s.termination_ok
            && s.parent_disposed
            && s.collection.stdout.as_ref().is_some_and(|s| s.finished)
            && s.collection.stderr.as_ref().is_some_and(|s| s.finished)
    });
    let cleanup = if cleanup_ok {
        CLEANUP_COMPLETED
    } else {
        CLEANUP_UNKNOWN
    };
    let settled_exit = settled.as_ref().and_then(|s| s.child_exit);
    let collection = settled.map(|s| s.collection).unwrap_or_default();
    let (capture, omissions) =
        report::capture_state(collection.stdout.as_ref(), collection.stderr.as_ref());
    let child_exit = match event {
        ParentEvent::Exited(status) => status.code(),
        _ => settled_exit,
    };
    let (status, exit) = disposition(event, child_exit, capture, cleanup);
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

fn disposition(
    event: &ParentEvent,
    child_exit: Option<i32>,
    capture: &str,
    cleanup: &str,
) -> (&'static str, i32) {
    if matches!(event, ParentEvent::TimedOut) {
        return ("harness_timeout", TIMEOUT_EXIT);
    }
    let success = matches!(event, ParentEvent::Exited(_))
        && child_exit == Some(0)
        && capture == CAPTURE_COMPLETE
        && cleanup == CLEANUP_COMPLETED;
    if success {
        ("completed", 0)
    } else {
        (
            "action_failed",
            child_exit.filter(|code| *code != 0).unwrap_or(1),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct FailedTermination(std::process::Child, bool);

    impl ChildWrapper for FailedTermination {
        fn inner(&self) -> &dyn ChildWrapper {
            &self.0
        }
        fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
            &mut self.0
        }
        fn into_inner(self: Box<Self>) -> Box<dyn ChildWrapper> {
            Box::new(self.0)
        }
        fn start_kill(&mut self) -> std::io::Result<()> {
            if self.1 {
                sleep(Duration::from_secs(6));
            }
            Err(std::io::Error::other("synthetic termination failure"))
        }
    }

    #[test]
    fn failed_termination_cannot_be_completed_cleanup() {
        let python = std::env::var("DW_TEST_PYTHON")
            .unwrap_or_else(|_| String::from(if cfg!(windows) { "python" } else { "python3" }));
        // Failure and a pending worker must both preserve the original cause.
        for (delayed, timed_out) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut native = Command::new(&python)
                .args(["-c", "pass"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("child");
            let status = native.wait().expect("parent status");
            let mut child = FailedTermination(native, delayed);
            let readers = Readers::start(&mut child);
            let event = if timed_out {
                ParentEvent::TimedOut
            } else {
                ParentEvent::Exited(status)
            };
            let started = Instant::now();
            let result = settle(Box::new(child), readers, event, started);
            assert_eq!(result.cleanup, CLEANUP_UNKNOWN);
            let expected = if timed_out {
                ("harness_timeout", 124)
            } else {
                ("action_failed", 1)
            };
            assert_eq!((result.status, result.exit), expected);
            assert!(started.elapsed() < COLLECTION_WINDOW + Duration::from_millis(300));
            if !timed_out {
                assert_eq!(result.child_exit, Some(0));
            }
        }
    }

    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("synthetic read failure"))
        }
    }

    #[test]
    fn reader_error_is_incomplete_and_suppressed() {
        let stream = spawn_reader(BrokenReader).join().expect("reader");
        assert!(!stream.finished);
        assert_eq!(
            report::stream_publication(Some(&stream), &[], "stdout"),
            b"[stdout omitted: capture_incomplete]"
        );
    }
}

//! Subprocess dialogue helper for the CLI fresh-authority confirmation.
//! Spawns the real compiled binary with piped IO, reads the bounded
//! challenge line from stderr under a deadline, writes one response line,
//! and collects the final result. Retains unexpected stderr diagnostics for
//! assertions; never echoes response bytes.
#![allow(dead_code)]

use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Exact statement the trusted operator affirms; hardcoded here so tests
/// verify the wire value independently of the production constant.
pub const STATEMENT: &str =
    "current_authority_within_original_bounds_and_no_unreconciled_withdrawal";

const CHALLENGE_DEADLINE: Duration = Duration::from_secs(15);
const EXIT_DEADLINE: Duration = Duration::from_secs(30);

/// One spawned CLI invocation waiting on (or finished with) the authority
/// confirmation interaction on stdin/stderr.
pub struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    challenge_rx: Receiver<Vec<u8>>,
    tail: Option<JoinHandle<Vec<u8>>>,
    out_tail: Option<JoinHandle<Vec<u8>>>,
    first_line: Option<Vec<u8>>,
}

/// Terminal result of an interactive invocation.
pub struct Finished {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    /// Raw first stderr line (the challenge), empty when none was emitted.
    pub challenge_line: Vec<u8>,
    /// Anything else the child wrote to stderr; expected empty.
    pub stderr_tail: Vec<u8>,
}

fn command(args: &[&str], dsn: Option<&str>) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    if let Some(dsn) = dsn {
        cmd.env("DW_DATABASE_URL", dsn);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args);
    cmd
}

fn spawn(args: &[&str], dsn: Option<&str>) -> Session {
    let mut child = command(args, dsn).spawn().expect("spawn CLI");
    let stdin = child.stdin.take();
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = BufReader::new(child.stderr.take().expect("piped stderr"));
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let tail = std::thread::spawn(move || {
        let mut first = Vec::new();
        let _ = stderr.read_until(b'\n', &mut first);
        let _ = tx.send(first);
        let mut rest = Vec::new();
        let _ = stderr.read_to_end(&mut rest);
        rest
    });
    let out_tail = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    Session {
        child,
        stdin,
        challenge_rx: rx,
        tail: Some(tail),
        out_tail: Some(out_tail),
        first_line: None,
    }
}

/// Spawn `assess --operation .. --input .. --recover ..` with piped IO.
pub fn assess_session(operation: &str, input: &str, recover: bool, dsn: &str) -> Session {
    spawn(
        &[
            "assess",
            "--operation",
            operation,
            "--input",
            input,
            "--recover",
            if recover { "true" } else { "false" },
        ],
        Some(dsn),
    )
}

impl Session {
    /// Raw first stderr line, read under a bounded deadline.
    pub fn challenge_line(&mut self) -> &[u8] {
        if self.first_line.is_none() {
            match self.challenge_rx.recv_timeout(CHALLENGE_DEADLINE) {
                Ok(line) if line.ends_with(b"\n") => self.first_line = Some(line),
                _ => {
                    let _ = self.child.kill();
                    panic!("challenge line missing or unterminated within deadline");
                }
            }
        }
        self.first_line.as_ref().expect("challenge recorded")
    }

    /// The challenge parsed as one JSON object.
    pub fn challenge(&mut self) -> Value {
        let line = self.challenge_line().to_vec();
        serde_json::from_slice(&line).expect("challenge line is JSON")
    }

    /// Assert the live challenge matches the exact scoped contract fields
    /// and answer it with the fixed statement and its own challenge id.
    pub fn affirm(
        &mut self,
        engagement: &str,
        campaign: &str,
        operation: &str,
        revision: u64,
    ) -> Value {
        let challenge = self.challenge();
        let object = challenge.as_object().expect("challenge object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "action",
                "campaign_id",
                "challenge_id",
                "engagement_id",
                "expected_mission_revision",
                "operation_id",
                "statement",
            ]
        );
        assert_eq!(challenge["action"], "confirm_current_authority");
        assert_eq!(challenge["engagement_id"], engagement);
        assert_eq!(challenge["campaign_id"], campaign);
        assert_eq!(challenge["operation_id"], operation);
        assert_eq!(challenge["expected_mission_revision"], revision);
        assert_eq!(challenge["statement"], STATEMENT);
        let challenge_id = challenge["challenge_id"].as_str().expect("id string");
        assert_ne!(Uuid::parse_str(challenge_id).unwrap(), Uuid::nil());
        let response = serde_json::json!({
            "challenge_id": challenge_id,
            "statement": STATEMENT,
        });
        self.respond(response.to_string().as_bytes());
        challenge
    }

    /// Write response bytes verbatim followed by LF, then flush. Callers
    /// that need an unterminated line pass bytes and use `write_raw`.
    pub fn respond(&mut self, bytes: &[u8]) {
        self.write_raw(bytes);
        self.write_raw(b"\n");
    }

    pub fn write_raw(&mut self, bytes: &[u8]) {
        self.stdin
            .as_mut()
            .expect("stdin open")
            .write_all(bytes)
            .and_then(|()| self.stdin.as_mut().unwrap().flush())
            .expect("write response");
    }

    /// Close stdin so the child sees EOF.
    pub fn close_input(&mut self) {
        drop(self.stdin.take());
    }

    fn wait_exit(&mut self, context: &str) -> ExitStatus {
        let deadline = Instant::now() + EXIT_DEADLINE;
        loop {
            match self.child.try_wait().expect("try_wait") {
                Some(status) => return status,
                None if Instant::now() >= deadline => {
                    let _ = self.child.kill();
                    panic!("{context}");
                }
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    fn collect(mut self, status: ExitStatus) -> Finished {
        let stdout = self
            .out_tail
            .take()
            .expect("stdout drain")
            .join()
            .unwrap_or_default();
        let stderr_tail = self
            .tail
            .take()
            .expect("tail thread")
            .join()
            .unwrap_or_default();
        let challenge_line = match self.first_line.take() {
            Some(line) => line,
            None => self.challenge_rx.try_recv().unwrap_or_default(),
        };
        Finished {
            status,
            stdout,
            challenge_line,
            stderr_tail,
        }
    }

    /// Close stdin, wait under a bounded deadline, and collect output plus
    /// all stderr diagnostics (challenge line + any tail).
    pub fn finish(mut self) -> Finished {
        self.close_input();
        let status = self.wait_exit("CLI did not exit within deadline");
        self.collect(status)
    }

    /// Wait for child exit under the same bounded deadline while stdin
    /// stays open, proving the CLI acts on a complete response line without
    /// waiting for EOF. A child blocked on EOF fails here instead of being
    /// rescued by a close.
    pub fn finish_with_open_input(mut self) -> Finished {
        let status = self.wait_exit("CLI did not exit while stdin stayed open");
        self.close_input();
        self.collect(status)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

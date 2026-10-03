//! In-module fault controls for the private confirmation boundary. Injected
//! port and I/O failures must deny a new assessment before any mutation and
//! without consuming or echoing input. These doubles prove the observable
//! ordering and error contract of `confirm_if_new`; they do not claim
//! physical SQL failures or PostgreSQL rollback behavior.

use super::*;
use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef};
use std::io::Error;

fn request() -> PlanningRequest {
    PlanningRequest {
        engagement_id: EngagementId(uuid::Uuid::from_u128(0xf101)),
        campaign_id: CampaignId(uuid::Uuid::from_u128(0xf102)),
        purpose_ref: GoalRef(uuid::Uuid::from_u128(0xf103)),
        asset_ref: AssetRef(uuid::Uuid::from_u128(0xf104)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}

fn operation() -> OperationId {
    OperationId(uuid::Uuid::from_u128(0xf105))
}

/// Recovery lookups return no durable decision; mutation attempts are
/// counted separately so a denied exchange is provably side-effect free.
struct CountingStore {
    lookups: usize,
    mutations: usize,
}

impl CountingStore {
    fn new() -> Self {
        Self {
            lookups: 0,
            mutations: 0,
        }
    }
}

impl PlanningStore for CountingStore {
    fn assess(
        &mut self,
        _request: &PlanningRequest,
        _operation_id: OperationId,
        recover: bool,
        _allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<PlanningAssessed>> {
        if recover {
            self.lookups += 1;
        } else {
            self.mutations += 1;
        }
        Ok(None)
    }
}

struct ErrAllocator;

impl OperationAllocator for ErrAllocator {
    fn allocate(&mut self) -> Res<uuid::Uuid> {
        Err(Fail::Store("allocation_fault"))
    }
}

struct NilAllocator;

impl OperationAllocator for NilAllocator {
    fn allocate(&mut self) -> Res<uuid::Uuid> {
        Ok(uuid::Uuid::nil())
    }
}

struct FixedAllocator;

impl OperationAllocator for FixedAllocator {
    fn allocate(&mut self) -> Res<uuid::Uuid> {
        Ok(uuid::Uuid::from_u128(0xf106))
    }
}

/// Records accepted bytes; write and flush outcomes are injected.
/// `budget` bytes are accepted before writes fail; `usize::MAX` never fails.
struct ProbeWrite {
    written: Vec<u8>,
    budget: usize,
    flush_fails: bool,
}

impl ProbeWrite {
    fn ok() -> Self {
        Self {
            written: Vec::new(),
            budget: usize::MAX,
            flush_fails: false,
        }
    }
    fn writing(budget: usize, flush_fails: bool) -> Self {
        Self {
            written: Vec::new(),
            budget,
            flush_fails,
        }
    }
}

impl Write for ProbeWrite {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.budget == 0 {
            return Err(Error::other("write_fault"));
        }
        let n = buf.len().min(self.budget);
        self.written.extend_from_slice(&buf[..n]);
        self.budget -= n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.flush_fails {
            Err(Error::other("flush_fault"))
        } else {
            Ok(())
        }
    }
}

/// Counts fill_buf calls so an aborted exchange provably never read input.
/// Bytes after the first newline are retained, proving a complete line is
/// consumed without waiting for further input or EOF.
struct OpenReader<'a> {
    rest: &'a [u8],
    reads: usize,
}

impl<'a> OpenReader<'a> {
    fn new(rest: &'a [u8]) -> Self {
        Self { rest, reads: 0 }
    }
}

impl Read for OpenReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reads += 1;
        let n = buf.len().min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

impl BufRead for OpenReader<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.reads += 1;
        Ok(self.rest)
    }
    fn consume(&mut self, amt: usize) {
        self.rest = &self.rest[amt..];
    }
}

struct FailReader;

impl Read for FailReader {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(Error::other("read_fault"))
    }
}

impl BufRead for FailReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Err(Error::other("read_fault"))
    }
    fn consume(&mut self, _amt: usize) {}
}

const DENIED: Fail = Fail::State("authority_confirmation_failed");

#[test]
fn allocator_error_propagates_before_any_exchange() {
    let mut store = CountingStore::new();
    let mut out = ProbeWrite::ok();
    let mut input = OpenReader::new(b"");
    let res = confirm_if_new(
        &mut ErrAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(res.err(), Some(Fail::Store("allocation_fault")));
    assert_eq!(store.lookups, 1);
    assert_eq!(store.mutations, 0);
    assert!(out.written.is_empty(), "no challenge after alloc failure");
    assert_eq!(input.reads, 0);
}

#[test]
fn nil_challenge_denied_before_any_exchange() {
    let mut store = CountingStore::new();
    let mut out = ProbeWrite::ok();
    let mut input = OpenReader::new(b"");
    let res = confirm_if_new(
        &mut NilAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(res.err(), Some(DENIED));
    assert_eq!(store.mutations, 0);
    assert!(out.written.is_empty());
    assert_eq!(input.reads, 0);
}

#[test]
fn challenge_write_failure_denies_before_reading() {
    let mut store = CountingStore::new();
    let mut out = ProbeWrite::writing(0, false);
    let mut input = OpenReader::new(b"{\"challenge_id\":\"x\"}\n");
    let res = confirm_if_new(
        &mut FixedAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(res.err(), Some(DENIED));
    assert_eq!(store.mutations, 0);
    assert_eq!(input.reads, 0, "response must not be consumed");
}

#[test]
fn partial_challenge_write_denies_without_echo() {
    let mut store = CountingStore::new();
    let mut out = ProbeWrite::writing(16, false);
    let mut input = OpenReader::new(b"READ_FAULT_CANARY\n");
    let res = confirm_if_new(
        &mut FixedAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(res.err(), Some(DENIED));
    assert_eq!(store.mutations, 0);
    assert_eq!(input.reads, 0);
    let partial = String::from_utf8_lossy(&out.written);
    assert_eq!(partial, "{\"action\":\"confi");
    assert!(!partial.contains("write_fault"));
    assert!(!partial.contains("READ_FAULT_CANARY"));
}

#[test]
fn challenge_flush_failure_denies_before_reading() {
    let mut store = CountingStore::new();
    let mut out = ProbeWrite::writing(usize::MAX, true);
    let mut input = OpenReader::new(b"");
    let res = confirm_if_new(
        &mut FixedAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(res.err(), Some(DENIED));
    assert_eq!(store.mutations, 0);
    assert_eq!(input.reads, 0);
    let line: serde_json::Value =
        serde_json::from_slice(out.written.trim_ascii_end()).expect("one JSON challenge line");
    assert_eq!(line["action"], "confirm_current_authority");
    assert_eq!(
        line["challenge_id"],
        uuid::Uuid::from_u128(0xf106).to_string()
    );
    assert!(!line.to_string().contains("flush_fault"));
}

#[test]
fn response_read_failure_denies_without_mutation() {
    let mut store = CountingStore::new();
    let mut out = ProbeWrite::ok();
    let mut input = FailReader;
    let res = confirm_if_new(
        &mut FixedAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(res.err(), Some(DENIED));
    assert_eq!(store.mutations, 0);
    let emitted = String::from_utf8_lossy(&out.written);
    assert!(emitted.contains("\"action\":\"confirm_current_authority\""));
    assert!(!emitted.contains("read_fault"), "no diagnostic echo");
}

#[test]
fn complete_line_reaches_assessment_without_eof() {
    let response = format!(
        "{{\"challenge_id\":\"{}\",\"statement\":\"{STATEMENT}\"}}\n",
        uuid::Uuid::from_u128(0xf106)
    );
    let wire = format!("{response}UNCONSUMED");
    let mut store = CountingStore::new();
    let mut out = ProbeWrite::ok();
    let mut input = OpenReader::new(wire.as_bytes());
    let res = confirm_if_new(
        &mut FixedAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(res.unwrap(), None);
    assert_eq!(store.lookups, 1);
    assert_eq!(store.mutations, 1);
    assert_eq!(
        input.rest, b"UNCONSUMED",
        "a complete line ends the exchange without waiting for EOF"
    );
}

//! `dw-fixture-supervisor`: development-only fixture action supervision.
//!
//! Owns OS process-tree lifetime, bounded capture, redacted publication and
//! the fixed local Docker/PostgreSQL service check for DuskWeave fixtures.
//! This executable is selected explicitly through the `fixture-supervisor`
//! feature and the `DW_TEST_FIXTURE_SUPERVISOR` path; it is never exposed to
//! reasoning workers or campaign execution. Credentials stay in the inherited
//! process environment and the private stdin request — never in argv,
//! ordinary output or report payloads.

#![forbid(unsafe_code)]
mod action;
mod report;
mod service;

use action::ActionRequest;
use serde::Deserialize;
use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;

/// Private stdin request bounds; oversized or malformed input is refused.
const MAX_REQUEST_BYTES: usize = 128 * 1024;
const MAX_ARGV_ITEMS: usize = 128;
const MAX_REDACTIONS: usize = 256;
const MAX_STRING_BYTES: usize = 8192;
const MAX_BUDGET_MS: u64 = 3_600_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRequest {
    version: u8,
    argv: Vec<String>,
    cwd: String,
    label: String,
    report_root: String,
    action_budget_ms: u64,
    redactions: Vec<String>,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let dispatched = dispatch(&arguments);
    let payload = match dispatched {
        Ok(value) => value,
        Err(category) => {
            report_line(&report::refusal_line(category));
            return ExitCode::from(1);
        }
    };
    report_line(&payload);
    ExitCode::SUCCESS
}

fn dispatch(arguments: &[String]) -> Result<Value, &'static str> {
    match arguments.first().map(String::as_str) {
        Some("run-action") if arguments.len() == 1 => run_action_command(),
        Some("service-check") => service::command(&arguments[1..]),
        _ => Err("native_supervisor_usage_refused"),
    }
}

fn run_action_command() -> Result<Value, &'static str> {
    let raw = read_request()?;
    let request: RunRequest =
        serde_json::from_str(&raw).map_err(|_| "native_supervisor_request_refused")?;
    let validated = validate(request)?;
    let supervised = action::run_action(&validated)?;
    Ok(supervised.protocol())
}

fn read_request() -> Result<String, &'static str> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut stdin = std::io::stdin();
    loop {
        let read = stdin
            .read(&mut chunk)
            .map_err(|_| "native_supervisor_request_refused")?;
        if read == 0 {
            return String::from_utf8(buffer).map_err(|_| "native_supervisor_request_refused");
        }
        if buffer.len() + read > MAX_REQUEST_BYTES {
            return Err("native_supervisor_request_refused");
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

fn validate(request: RunRequest) -> Result<ActionRequest, &'static str> {
    let refused = Err("native_supervisor_request_refused");
    if request.version != 1 || !valid_strings(&request.argv, &request.redactions) {
        return refused;
    }
    if !valid_label(&request.label) || !valid_budget(request.action_budget_ms) {
        return refused;
    }
    if !Path::new(&request.cwd).is_absolute() || !Path::new(&request.report_root).is_absolute() {
        return refused;
    }
    Ok(ActionRequest {
        argv: request.argv,
        cwd: request.cwd,
        label: request.label,
        report_root: request.report_root,
        action_budget_ms: request.action_budget_ms,
        redactions: request.redactions,
    })
}

fn valid_strings(argv: &[String], redactions: &[String]) -> bool {
    let argv_ok = !argv.is_empty() && each_bounded(argv, MAX_ARGV_ITEMS);
    let redactions_ok =
        each_bounded(redactions, MAX_REDACTIONS) && !redactions.iter().any(String::is_empty);
    argv_ok && redactions_ok
}

fn each_bounded(values: &[String], max_items: usize) -> bool {
    values.len() <= max_items && values.iter().all(|value| value.len() <= MAX_STRING_BYTES)
}

fn valid_label(label: &str) -> bool {
    let bounded = (1..=64).contains(&label.len());
    bounded
        && label.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

fn valid_budget(budget_ms: u64) -> bool {
    budget_ms > 0 && budget_ms <= MAX_BUDGET_MS
}

fn report_line(value: &Value) {
    let _ = writeln!(std::io::stdout(), "{value}");
}

//! `dw-boundary-launcher`: privileged fixed host launcher for the ADR-015
//! qualified M1 acquisition boundary. It creates the attempt-local
//! capture/exporter container pair, the private pipe routes, the host nftables
//! egress lease and the monotonic STOP latch handling. Linux only.
//!
//! Subcommands: `preflight`, `run-attempt` (private stdin request),
//! `stop --control-root <dir> --session <id>`, `cleanup --session <id>`.
//! Credentials and raw candidate bytes never appear in argv or output;
//! receipts carry safe fixed fields only.
#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod attempt;
#[cfg(target_os = "linux")]
mod docker;
#[cfg(target_os = "linux")]
mod msgs;
#[cfg(target_os = "linux")]
mod nft;
#[cfg(target_os = "linux")]
mod stop;
#[cfg(target_os = "linux")]
mod wire;

#[cfg(target_os = "linux")]
mod real {
    use super::{attempt, msgs, stop};
    use serde_json::json;
    use std::io::Write;
    use std::process::ExitCode;

    pub fn run() -> ExitCode {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match dispatch(&args) {
            Ok(v) => {
                let _ = writeln!(std::io::stdout(), "{v}");
                ExitCode::SUCCESS
            }
            Err(code) => {
                let _ = writeln!(
                    std::io::stdout(),
                    "{}",
                    json!({"version": "dw_boundary_receipt_v1", "error": code})
                );
                ExitCode::from(match code {
                    "stopped" => 3,
                    "unknown" => 4,
                    _ => 1,
                })
            }
        }
    }

    fn dispatch(args: &[String]) -> Result<serde_json::Value, &'static str> {
        match args.first().map(String::as_str) {
            Some("preflight") if args.len() == 1 => attempt::preflight(),
            Some("run-attempt") if args.len() == 1 => attempt::run(&read_stdin()?),
            Some("stop") => stop_command(&args[1..]),
            Some("cleanup") => cleanup_command(&args[1..]),
            _ => Err("usage_refused"),
        }
    }

    /// The request arrives as one length-prefixed frame; stdin then stays
    /// open as the exclusive control channel for the attempt (ADR-017).
    fn read_stdin() -> Result<msgs::AttemptRequest, &'static str> {
        let mut raw = Vec::new();
        let mut stdin = std::io::stdin();
        match super::wire::read_frame(&mut stdin, &mut raw) {
            Ok(_) => {}
            Err(super::wire::WireErr::Oversize) => return Err("request_refused"),
            Err(_) => return Err("request_unreadable"),
        }
        serde_json::from_slice(&raw).map_err(|_| "request_invalid")
    }

    fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str, &'static str> {
        let pos = args.iter().position(|a| a == name).ok_or("usage_refused")?;
        args.get(pos + 1).map(String::as_str).ok_or("usage_refused")
    }

    fn stop_command(args: &[String]) -> Result<serde_json::Value, &'static str> {
        if args.len() != 4 {
            return Err("usage_refused");
        }
        let root = flag(args, "--control-root")?;
        let session = flag(args, "--session")?;
        stop::latch(root, session)?;
        Ok(json!({"version":"dw_boundary_receipt_v1","stop":"latched","session":session}))
    }

    fn cleanup_command(args: &[String]) -> Result<serde_json::Value, &'static str> {
        if args.len() != 2 {
            return Err("usage_refused");
        }
        let session = flag(args, "--session")?;
        attempt::cleanup(session)
    }
}

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    real::run()
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("dw-boundary-launcher: unsupported_platform");
    std::process::ExitCode::from(1)
}

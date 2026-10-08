//! Synthetic loopback startup cases for the CLI wait bounds. A peer that
//! accepts but never answers negotiation and one that consumes the startup
//! packet but never completes authentication must terminate independently at
//! the fixed startup bound with exit status 124, no receipt and no secret
//! disclosure; an eventual server close can never be the event that ends the
//! CLI. A closed loopback endpoint fails safely with the existing category.
//! These are CLI-process bounds: no firewall, routing or listener-backlog
//! manipulation is used and no SYN-blackhole behavior is claimed.

use std::io::Read;
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

#[path = "support/database_wait_db.rs"]
mod wait_db;

const SECRET: &str = "dw_synthetic_wait_secret";
const STARTUP_FLOOR: Duration = Duration::from_millis(4500);

fn command(port: u16) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    cmd.env("DW_DATABASE_CONFIG_MODE", "env-local");
    cmd.env(
        "DW_DATABASE_URL",
        format!(
            "host=127.0.0.1 port={port} user=dw_synthetic password={SECRET} dbname=dw_synthetic"
        ),
    );
    cmd.args([
        "inspect",
        "--engagement",
        "00000000-0000-0000-0000-000000000001",
        "--campaign",
        "00000000-0000-0000-0000-000000000002",
    ]);
    cmd
}

/// Held loopback peer. `consume_startup` selects the fault: accept without
/// answering negotiation, or read the startup packet without completing
/// authentication. The returned sender keeps the socket held open until the
/// test releases it after the CLI exit, so a server close can never be the
/// event that ends the CLI; the engaged barrier proves the fault fired.
fn stall_peer(consume_startup: bool) -> (u16, Receiver<()>, Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (engaged_tx, engaged_rx) = channel();
    let (release_tx, release_rx) = channel::<()>();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        if consume_startup {
            let mut packet = vec![0u8; 8192];
            let read = stream.read(&mut packet).unwrap();
            assert!(read > 0, "startup packet must be observed before the bound");
        }
        engaged_tx.send(()).unwrap();
        let _ = release_rx.recv();
    });
    (port, engaged_rx, release_tx)
}

fn expect_watchdog_stop(output: &std::process::Output, elapsed: Duration) {
    wait_db::expect_watchdog_stop(output, elapsed, STARTUP_FLOOR, wait_db::STARTUP_ENVELOPE);
}

#[test]
fn silent_negotiation_peer_is_bounded_independently() {
    let (port, engaged, _hold) = stall_peer(false);
    let (output, elapsed) = wait_db::run_bounded(&mut command(port), Duration::from_secs(20));
    match engaged.recv_timeout(Duration::from_secs(2)) {
        Ok(()) => {}
        Err(RecvTimeoutError::Timeout) => panic!("peer never accepted the CLI connection"),
        Err(RecvTimeoutError::Disconnected) => panic!("stall peer failed before engaging"),
    }
    expect_watchdog_stop(&output, elapsed);
}

#[test]
fn consumed_startup_packet_without_authentication_is_bounded() {
    let (port, engaged, _hold) = stall_peer(true);
    let (output, elapsed) = wait_db::run_bounded(&mut command(port), Duration::from_secs(20));
    match engaged.recv_timeout(Duration::from_secs(2)) {
        Ok(()) => {}
        Err(RecvTimeoutError::Timeout) => panic!("peer never accepted the CLI connection"),
        Err(RecvTimeoutError::Disconnected) => panic!("stall peer failed before engaging"),
    }
    expect_watchdog_stop(&output, elapsed);
}

#[test]
fn closed_loopback_endpoint_fails_safely() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let (output, elapsed) = wait_db::run_bounded(&mut command(port), Duration::from_secs(10));
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"error=connect_failed\n");
    assert!(output.stderr.is_empty());
    assert!(
        elapsed <= wait_db::STARTUP_ENVELOPE,
        "closed-endpoint refusal must stay bounded: {elapsed:?}"
    );
}

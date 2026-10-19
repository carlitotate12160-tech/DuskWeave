//! Isolated command-watchdog evidence (DW-DIAGNOSE-M1-SCHEMA-DEADLINE):
//! `database_wait` monitor/CommandGuard behavior in a sacrificial child with
//! NO transport watchdog armed. Demonstrates that exit code 124 alone cannot
//! attribute a process exit to the inference transport deadline. No C0
//! pause/reset is used; blocking is a controlled sleep only.
use super::*;
use std::process::{Command, Stdio};

/// Sacrificial-child dispatch: only runs under DW_CMD_DEADLINE_SCENARIO.
#[test]
fn m1_command_deadline_child() {
    if std::env::var("DW_CMD_DEADLINE_SCENARIO").as_deref() != Ok("expire") {
        return;
    }
    let _monitor = spawn_monitor(Budget::start(Duration::from_millis(150))).unwrap();
    // Controlled blocking: a plain sleep; only the armed monitor can end it.
    std::thread::sleep(Duration::from_secs(30));
}

fn spawn_deadline_child() -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "database_wait::m1_command_deadline::m1_command_deadline_child",
            "--nocapture",
        ])
        .env("DW_CMD_DEADLINE_SCENARIO", "expire")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn wait_bounded(child: &mut std::process::Child, bound: Duration) -> Option<i32> {
    let start = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return status.code();
        }
        if start.elapsed() > bound {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn expired_command_monitor_exits_124_without_transport() {
    // The child arms only the database_wait command monitor and blocks. Exit
    // code 124 results with zero inference transport involvement, so the same
    // code that the transport watchdog produces is ambiguous on its own.
    let mut child = spawn_deadline_child();
    let code = wait_bounded(&mut child, Duration::from_secs(10));
    assert_eq!(code, Some(124));
}

#[test]
fn done_response_disarms_monitor_in_process() {
    // Control: a Done response before expiry disarms the monitor. Completing
    // this test in-process past the armed bound proves no exit occurred.
    let tx = spawn_monitor(Budget::start(Duration::from_millis(100))).unwrap();
    tx.send(Control::Done).unwrap();
    std::thread::sleep(Duration::from_millis(250));
}

#[test]
fn command_guard_installs_fixed_budget_and_drops_cleanly() {
    // Guard lifecycle without expiry: Drop sends Done, disarming the monitor.
    let _guard = CommandGuard::start();
}

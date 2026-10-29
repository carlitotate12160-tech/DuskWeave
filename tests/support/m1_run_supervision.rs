//! Owner-local regression tests.
use super::*;
#[test]
fn successor_phase_selection_keeps_acquisition_and_original_run_bounds() {
    let now = Instant::now();
    let run = now + Duration::from_millis(400000);
    let successor = selected_phase_deadline(run, RunPhase::DecisionOne, true);
    assert!(successor >= now + Duration::from_millis(134000));
    assert!(successor <= Instant::now() + Duration::from_millis(134000));
    let acquisition = selected_phase_deadline(run, RunPhase::Acquisition, true);
    assert!(acquisition <= Instant::now() + Duration::from_millis(60000));
    let legacy = selected_phase_deadline(run, RunPhase::DecisionTwo, false);
    assert!(legacy <= Instant::now() + Duration::from_millis(50000));
    let remainder = Instant::now() + Duration::from_millis(10);
    assert_eq!(
        selected_phase_deadline(remainder, RunPhase::DecisionTwo, true),
        remainder
    );
}
fn parsed(bytes: &[u8]) -> Observation {
    let (tx, _rx) = mpsc::channel();
    read_ready(std::io::Cursor::new(bytes), "expected", &tx)
}
#[test]
fn guard_reports_never_change_parent_cause_and_competing_or_malformed_reports_are_unknown() {
    let one = parsed(b"expected\nm1_run_guard_v1 transport_hard_deadline\n");
    assert_eq!(
        one.reported_guard,
        Some(ReportedGuard::TransportHardDeadline)
    );
    assert_eq!(one.cause, None);
    assert!(one.ready);
    let competing = parsed(
        b"expected\nm1_run_guard_v1 transport_hard_deadline\nm1_run_guard_v1 permit_expiry\n",
    );
    assert_eq!(competing.reported_guard, None);
    assert_eq!(competing.cause, None);
    for bad in ["timeout", "permit_expiry\r", "synthetic-sensitive-sentinel"] {
        let malformed = parsed(format!("expected\nm1_run_guard_v1 {bad}\n").as_bytes());
        assert_eq!(malformed.reported_guard, None);
        assert_eq!(malformed.cause, Some(DiagnosticCause::ReadinessRefused));
    }
    let missing = parsed(b"expected\nerror=child_failed\n");
    assert_eq!(missing.reported_guard, None);
    assert_eq!(missing.error, Some("child_failed"));
}
#[test]
fn terminal_projection_is_closed_before_and_after_ready_and_rejects_extra_frames() {
    for prefix in ["", "expected\n"] {
        for code in ["missing_env", "run_child_refused", "run_inference_pending"] {
            let observed = parsed(format!("{prefix}error={code}\n").as_bytes());
            assert_eq!(observed.error, Some(code));
            assert_eq!(observed.ready, !prefix.is_empty());
        }
        let state = parsed(format!("{prefix}error=synthetic-sensitive-sentinel\n").as_bytes());
        assert_eq!(state.error, Some("child_failed"));
        for bad in [
            "error=missing_env\r\n",
            "error=missing_env",
            "error=missing_env \n",
            "error=\n",
            "{}\n",
            "error=missing_env\nextra\n",
        ] {
            let state = parsed(format!("{prefix}{bad}").as_bytes());
            assert_eq!(state.cause, Some(DiagnosticCause::ReadinessRefused));
        }
        let mut bad = format!("{prefix}error=").into_bytes();
        bad.extend_from_slice(&[0xff, b'\n']);
        assert_eq!(parsed(&bad).cause, Some(DiagnosticCause::ReadinessRefused));
        let oversize = format!("{prefix}error={}\n", "x".repeat(128));
        assert_eq!(
            parsed(oversize.as_bytes()).cause,
            Some(DiagnosticCause::ReadinessRefused)
        );
    }
    assert!(parsed(b"expected\n").ready);
    assert_eq!(parsed(b"expected\n").error, None);
}
fn spawn_terminal(frame: &str, exit: i32) -> Child {
    #[cfg(windows)]
    let mut command = Command::new(
        std::path::Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    );
    #[cfg(windows)]
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &format!("[Console]::Write('{frame}'); exit {exit}"),
    ]);
    #[cfg(not(windows))]
    let mut command = Command::new("/bin/sh");
    #[cfg(not(windows))]
    command.args(["-c", &format!("printf '{frame}'; exit {exit}")]);
    command.env_remove("DEEPSEEK_API_KEY");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}
fn terminal_child(frame: &str, exit: i32) -> PhaseChild {
    let mut spawned = spawn_terminal(frame, exit);
    let pipe = spawned.stdout.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut child = PhaseChild::supervise(spawned, deadline).unwrap();
    child
        .arm_guards(None, Some(("expected".into(), pipe)), deadline, deadline)
        .unwrap();
    while child.finished().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    child
}
#[test]
fn immediate_exit_reader_race_retains_code_and_exit_124_does_not_identify_a_watchdog() {
    for _ in 0..12 {
        let mut child = terminal_child("error=missing_env\n", 1);
        let d = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
        assert_eq!(d.child_error.as_deref(), Some("missing_env"));
        assert_eq!(d.exit_code, Some(1));
        assert_eq!(d.cause_source, CauseSource::ChildReport);
        assert!(!child.protocol_valid());
    }
    let mut child = terminal_child("", 124);
    let d = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
    assert_eq!(d.exit_code, Some(124));
    assert_eq!(d.child_error, None);
    assert_ne!(d.cause, DiagnosticCause::ReadinessDeadline);
    assert_ne!(d.cause, DiagnosticCause::PhaseDeadline);
    #[cfg(windows)]
    {
        let mut child = terminal_child("", -1073741819);
        assert_eq!(
            child
                .diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne)
                .exit_code,
            Some(-1073741819)
        );
    }
}
#[test]
fn reader_completion_without_a_watchdog_completion_cannot_finalize_success() {
    let mut child = terminal_child("expected\n", 0);
    // Synthetic completion ordering: reader has finished, watchdog is still pending.
    // Keep its sender alive so absence is a wait, not a disconnect.
    let (finished, rx) = mpsc::channel();
    finished.send(()).unwrap();
    child.terminal = Some(rx);
    child.deadline = Instant::now() + Duration::from_millis(30);
    let diagnostic = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
    assert_eq!(diagnostic.cause, DiagnosticCause::Unavailable);
    assert!(!child.protocol_valid());
    drop(finished);
}
#[test]
fn gated_late_watchdog_rejects_exact_ready_and_successful_exit_before_snapshot() {
    let mut spawned = spawn_terminal("expected\n", 0);
    let pipe = spawned.stdout.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut child = PhaseChild::supervise(spawned, deadline).unwrap();
    child.readiness_required = true;
    let (verdict, rx) = mpsc::channel();
    let (finished, terminal) = mpsc::channel();
    child.terminal = Some(terminal);
    let result = read_ready(pipe, "expected", &verdict);
    assert!(result.ready);
    child.observation.lock().unwrap().merge(result);
    finished.send(()).unwrap();
    while child.finished().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(child.finished().unwrap().unwrap().success());
    assert!(
        !child.protocol_valid(),
        "reader alone cannot authorize acceptance"
    );
    let (gate, held) = mpsc::channel();
    let observed = Arc::clone(&child.observation);
    let process = Arc::clone(&child.child);
    let prepared_by = Instant::now() - Duration::from_millis(1);
    let watchdog = std::thread::spawn(move || {
        held.recv().unwrap();
        readiness_watch(&process, &rx, prepared_by, &observed);
        finished.send(()).unwrap();
    });
    // The queued exact frame is now processed after its original readiness bound.
    gate.send(()).unwrap();
    let diagnostic = child.diagnostic(uuid::Uuid::from_u128(1), RunPhase::DecisionOne);
    watchdog.join().unwrap();
    assert_eq!(diagnostic.exit_code, Some(0));
    assert_eq!(diagnostic.cause, DiagnosticCause::ReadinessDeadline);
    assert!(!diagnostic.readiness_observed);
    assert!(!child.protocol_valid());
}
#[test]
fn child_terminal_error_before_ready_preserves_the_safe_code() {
    let (tx, rx) = mpsc::channel();
    read_ready(
        std::io::Cursor::new(b"error=missing_env\n"),
        "expected",
        &tx,
    );
    assert!(matches!(
        rx.recv().unwrap(),
        Err(Fail::Config("missing_env"))
    ));
}
#[test]
fn readiness_does_not_refuse_before_the_original_deadline() {
    let (tx, rx) = mpsc::channel();
    let prepared_by = Instant::now() + Duration::from_millis(40);
    let observed = readiness_result(&rx, prepared_by);
    assert!(
        Instant::now() >= prepared_by,
        "a pending READY must retain the final interval of its original deadline"
    );
    assert!(!observed.ready);
    assert_eq!(observed.cause, Some(DiagnosticCause::ReadinessDeadline));
    drop(tx);
}
#[test]
fn queued_late_ready_eof_and_refusal_cannot_disarm_the_original_guard() {
    let (tx, rx) = mpsc::channel();
    tx.send(Ok(())).unwrap();
    assert!(!readiness_result(&rx, Instant::now() - Duration::from_millis(1)).ready);
    let end = Instant::now() + Duration::from_secs(5);
    tx.send(Err(Fail::Config("run_readiness_refused"))).unwrap();
    assert!(!readiness_result(&rx, end).ready);
    tx.send(Ok(())).unwrap();
    assert!(readiness_result(&rx, end).ready);
    drop(tx);
    assert!(!readiness_result(&rx, end).ready);
}
fn parked_child() -> Child {
    #[cfg(windows)]
    let mut command = Command::new(
        std::path::Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    );
    #[cfg(windows)]
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Threading.Thread]::Sleep(60000)",
    ]);
    #[cfg(not(windows))]
    let mut command = Command::new("/bin/sleep");
    #[cfg(not(windows))]
    command.arg("60");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}
#[test]
fn independent_deadline_reaps_blocked_child_without_parent_polling_or_database() {
    let owned =
        PhaseChild::supervise(parked_child(), Instant::now() + Duration::from_millis(30)).unwrap();
    // No finished()/SQL calls while the independent reaper enforces the bound.
    std::thread::sleep(Duration::from_millis(200));
    assert!(owned.child.lock().unwrap().try_wait().unwrap().is_some());
}
#[test]
fn parent_scope_exit_reaps_child_and_phase_ceiling_clamps_to_run_remainder() {
    let owned =
        PhaseChild::supervise(parked_child(), Instant::now() + Duration::from_secs(60)).unwrap();
    let child = Arc::clone(&owned.child);
    drop(owned);
    assert!(child.lock().unwrap().try_wait().unwrap().is_some());
    let run = Instant::now() + Duration::from_millis(5);
    assert_eq!(phase_deadline(run, RunPhase::Acquisition), run);
    assert_eq!(phase_deadline(run, RunPhase::DecisionOne), run);
}
#[test]
fn readiness_frame_contract_accepts_exact_and_refuses_everything_else() {
    let run = uuid::Uuid::from_u128(0x5150);
    let expected = super::super::m1_run_control::ready_frame(run, "decision_one");
    let verdict = |bytes: &[u8]| {
        let (tx, rx) = mpsc::channel();
        read_ready(std::io::Cursor::new(bytes.to_vec()), &expected, &tx);
        rx.recv().unwrap()
    };
    // Exact frame only; a prelude must never be skipped.
    assert!(verdict(format!("{expected}\n").as_bytes()).is_ok());
    assert!(verdict(format!("{{\"receipt\":1}}\n{expected}\nerror=x\n").as_bytes()).is_err());
    for suffix in [" ", "  ", "\t", "\r", &" ".repeat(200)] {
        assert!(
            verdict(format!("{expected}{suffix}\n").as_bytes()).is_err(),
            "suffix accepted"
        );
    }
    let mut invalid = expected.as_bytes().to_vec();
    invalid.extend_from_slice(&[0xff, b'\n']);
    assert!(verdict(&invalid).is_err());
    assert!(verdict(expected.as_bytes()).is_err());
    for frame in [
        super::super::m1_run_control::ready_frame(uuid::Uuid::from_u128(0x5151), "decision_one"),
        super::super::m1_run_control::ready_frame(run, "decision_two"),
        expected.to_uppercase(),
    ] {
        assert!(verdict(format!("{frame}\n").as_bytes()).is_err());
    }
    for n in [128, 129] {
        let mut padded = expected.as_bytes().to_vec();
        padded.resize(n - 1, b' ');
        padded.push(b'\n');
        assert!(verdict(&padded).is_err(), "{n}-byte padding accepted");
        let mut cursor = std::io::Cursor::new(padded);
        assert!(await_ready(&mut cursor, &expected).is_err());
        assert_eq!(
            cursor.position(),
            128,
            "raw framing cannot consume a 129th byte"
        );
    }
    // Missing, mismatched and prefix-shaped-but-wrong content refuse.
    assert!(verdict(b"").is_err());
    assert!(verdict(b"hello\n").is_err());
    assert!(verdict(format!("{expected} two\n").as_bytes()).is_err());
    assert!(
        verdict(format!("{} bad\n", super::super::m1_run_control::READY_PREFIX).as_bytes())
            .is_err()
    );
    // Oversized content without a complete frame refuses, never grows.
    assert!(verdict(&vec![b'x'; 20000]).is_err());
}
#[test]
fn pending_readiness_expires_and_reapes_parked_child_without_parent_polling() {
    #[cfg(windows)]
    let mut command = Command::new(
        std::path::Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    );
    #[cfg(not(windows))]
    let mut command = Command::new("/bin/sleep");
    #[cfg(windows)]
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Threading.Thread]::Sleep(60000)",
    ]);
    #[cfg(not(windows))]
    command.arg("60");
    let mut spawned = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pipe = spawned.stdout.take().unwrap();
    let child = Arc::new(Mutex::new(spawned));
    // Absolute remaining deadline: pending readiness expires on its own.
    monitor_readiness(
        Arc::clone(&child),
        pipe,
        "never sent".to_string(),
        Instant::now() + Duration::from_millis(150),
        Instant::now() + Duration::from_secs(60),
        Arc::new(Mutex::new(Observation::default())),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert!(child.lock().unwrap().try_wait().unwrap().is_some());
}
fn credential_record(profile: Option<&str>) -> RunRecord {
    let id = uuid::Uuid::from_u128(0x77);
    serde_json::from_value(serde_json::json!({
            "request":{"version":"m1_autonomous_run_request_v2","run_id":id,
                "run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
                "inference_profile":profile,"deadline_at_ms":1700004000000i64,
                "context":{"version":"m1_context_prepare_request_v1","mission":{
                    "version":"m1_inference_reservation_request_v1","engagement_id":id,"campaign_id":id,
                    "operator_ref":id,"expected_mission_revision":1,"permission_id":id,"goal_ref":id,
                    "exercise_mode":"blind"},"candidates":[{"asset_ref":id,"priority":"high"}]},
                "seed_names":[{"asset_ref":id,"name":"seed.example.invalid"}]},
            "decision_one":id,"decision_two":id,"attempt":id,"result":id,"terrain_operation":id,
            "release_sha256":"ab","qualification_ref":id,"status":"running","deadline_at_ms":1700004000000i64
        }))
        .unwrap()
}
#[test]
fn credential_forwarding_follows_the_selected_profile_credential() {
    const CASE: &str = "DW_TEST_CREDENTIAL_FORWARDING_CASE";
    if let Ok(case) = std::env::var(CASE) {
        assert_credential_forwarding(case == "present");
        return;
    }
    for case in ["absent", "present"] {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args([
                "--exact",
                "m1_run_supervision::tests::credential_forwarding_follows_the_selected_profile_credential",
                "--test-threads=1",
            ]).env(CASE, case).env_remove("DEEPSEEK_API_KEY");
        if case == "present" {
            child.env("DEEPSEEK_API_KEY", "synthetic-forwarding-sentinel");
        }
        let output = child.output().unwrap();
        // Child output is private even on failure; never log the sentinel.
        assert!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "credential forwarding child failed: {case}"
        );
    }
}
fn assert_credential_forwarding(key_present: bool) {
    for (profile, forwarded) in [
        (Some("deepseek-responses-reasoning-v1"), true),
        (Some("deepseek-responses-low-v1"), true),
        (Some("fixture-responses-v1"), false),
        (Some("fixture-chat-completions-v1"), false),
        (Some("synthetic-unknown-profile"), false),
    ] {
        let run = credential_record(profile);
        for phase in [
            RunPhase::DecisionOne,
            RunPhase::DecisionTwo,
            RunPhase::Acquisition,
        ] {
            let mut c = Command::new("unused");
            c.env_clear();
            install_provider_environment(&mut c, phase, &run);
            let entry = c
                .get_envs()
                .find(|(k, _)| *k == "DEEPSEEK_API_KEY")
                .map(|(_, v)| v.map(|v| v.to_os_string()));
            let carries_key = matches!(entry, Some(Some(_)));
            assert_eq!(
                carries_key,
                forwarded && phase != RunPhase::Acquisition && key_present,
                "{profile:?}/{phase:?}: key must reach only a network decision child with a synthetic key"
            );
            assert!(
                entry
                    .flatten()
                    .is_none_or(|value| value == "synthetic-forwarding-sentinel")
            );
        }
    }
}

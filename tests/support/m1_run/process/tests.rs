use super::*;

/// A cached identity under another phase label must never satisfy a named
/// phase lookup: the label, not capture order, selects custody and proof.
#[test]
fn named_phase_lookup_never_reuses_another_phase_identity() {
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| "python".into());
    let parent = Command::new(python)
        .args(["-c", "import time;time.sleep(30)"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let run = Uuid::from_u128(0xda07);
    let mut owned = OwnedRun::new(parent, run);
    owned.phases.push(PhaseIdentity {
        pid: u32::MAX,
        birth: 1,
        started: std::time::Instant::now(),
        run,
        phase: Some(RunPhase::Acquisition.name()),
    });
    owned.focus = Some(0);
    // The decision-two lookup must not accept the acquisition-labelled
    // identity, and real discovery of a foreign cmdline captures nothing.
    assert!(owned.phase_for(RunPhase::DecisionTwo).is_none());
    assert_eq!(owned.phases.len(), 1);
    assert_eq!(owned.focused().unwrap().0, u32::MAX);
    assert_eq!(owned.phase().unwrap().0, u32::MAX);
}

fn control_parent(code: &str) -> std::process::Child {
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| "python".into());
    Command::new(python)
        .args(["-c", code])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

fn exited_control_parent() -> std::process::Child {
    let mut parent = control_parent("import sys;sys.exit(3)");
    parent.wait().unwrap();
    parent
}

fn exit_wait_result(
    parent: &mut std::process::Child,
    end: std::time::Instant,
) -> std::thread::Result<std::time::Instant> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wait_parent_exit(parent, end, "control")
    }))
}

/// An exit already complete is qualified by its observation instant: seen
/// before the bound, conservative evidence still proves on-time exit.
#[test]
fn already_exited_parent_first_seen_before_end_qualifies() {
    let mut parent = exited_control_parent();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let done = exit_wait_result(&mut parent, end).unwrap();
    assert!(done <= end);
}

/// An exit first observed after the bound cannot be qualified on time:
/// the observation instant is the only evidence and it lies past end.
#[test]
fn already_exited_parent_first_seen_after_end_cannot_qualify() {
    let mut parent = exited_control_parent();
    let end = std::time::Instant::now() - std::time::Duration::from_millis(1);
    assert!(
        exit_wait_result(&mut parent, end).is_err(),
        "an already-exited parent first observed after end must fail qualification"
    );
}

/// A still-running parent must fail at its supplied bound; observation
/// never grants an open-ended grace.
#[test]
fn still_running_parent_fails_at_its_bound() {
    let mut parent = control_parent("import time;time.sleep(30)");
    let end = std::time::Instant::now() + std::time::Duration::from_millis(120);
    assert!(exit_wait_result(&mut parent, end).is_err());
    let _ = parent.kill();
    let _ = parent.wait();
}

#[test]
fn parse_proc_stat_handles_parentheses_in_command_name() {
    let fields: Vec<String> = (0..22).map(|i| (100 + i).to_string()).collect();
    let stat = format!("4242 (weird (proc) name) S 900 {}", fields[2..].join(" "));
    assert_eq!(parse_proc_stat(&stat), Some((900, 119)));
}

#[test]
fn parse_proc_stat_rejects_missing_or_malformed_fields() {
    assert_eq!(parse_proc_stat("123 no_parens S 1 2 3"), None);
    assert_eq!(parse_proc_stat("123 (cmd) S"), None);
    assert_eq!(parse_proc_stat("123 (cmd) S bad_ppid 2 3"), None);
    let short = format!(
        "123 (cmd) S 42 {}",
        (2..19).map(|_| "1").collect::<Vec<_>>().join(" ")
    );
    assert_eq!(parse_proc_stat(&short), None);
    let bad_birth = format!(
        "123 (cmd) S 42 {} not_u64",
        (2..19).map(|_| "1").collect::<Vec<_>>().join(" ")
    );
    assert_eq!(parse_proc_stat(&bad_birth), None);
}

#[test]
fn proc_candidate_requires_exact_parent_run_and_optional_phase_args() {
    let run = Uuid::from_u128(0xda08).to_string();
    let rest = (2..19).map(|_| "1").collect::<Vec<_>>().join(" ");
    let stat = format!("77 (worker) S 500 {rest} 98765");
    let cmd = format!("duskweave\0m1-run-phase\0--run\0{run}\0--phase\0acquisition\0");
    assert_eq!(
        match_proc_candidate(&stat, cmd.as_bytes(), 500, &run, None),
        Some(98765)
    );
    assert_eq!(
        match_proc_candidate(&stat, cmd.as_bytes(), 500, &run, Some("acquisition")),
        Some(98765)
    );
    assert_eq!(
        match_proc_candidate(&stat, cmd.as_bytes(), 501, &run, Some("acquisition")),
        None
    );
    assert_eq!(
        match_proc_candidate(&stat, cmd.as_bytes(), 500, &run, Some("decision_two")),
        None
    );
    let partial_run = format!("duskweave\0--run={run}\0acquisition\0");
    assert_eq!(
        match_proc_candidate(
            &stat,
            partial_run.as_bytes(),
            500,
            &run,
            Some("acquisition")
        ),
        None
    );
    let partial_phase = format!("duskweave\0{run}\0acquisition_extra\0");
    assert_eq!(
        match_proc_candidate(
            &stat,
            partial_phase.as_bytes(),
            500,
            &run,
            Some("acquisition")
        ),
        None
    );
}

#[test]
fn select_unique_candidate_refuses_empty_and_multiple_matches() {
    assert_eq!(select_unique_candidate::<(u32, u64)>(vec![]), None);
    assert_eq!(select_unique_candidate(vec![(10, 20)]), Some((10, 20)));
    assert_eq!(select_unique_candidate(vec![(10, 20), (11, 21)]), None);
}

#[test]
fn project_exit_instant_handles_before_equal_and_after_wall_sample() {
    let base = std::time::Instant::now();
    let uncertainty = std::time::Duration::from_millis(5);
    assert_eq!(
        project_exit_instant(base, 1_000, 990, uncertainty),
        base.checked_sub(std::time::Duration::from_millis(5))
    );
    assert_eq!(
        project_exit_instant(base, 1_000, 1_000, uncertainty),
        Some(base + uncertainty)
    );
    assert_eq!(
        project_exit_instant(base, 1_000, 1_025, uncertainty),
        Some(base + std::time::Duration::from_millis(30))
    );
}

#[test]
fn project_exit_instant_fails_closed_on_conversion_and_arithmetic_overflow() {
    let base = std::time::Instant::now();
    let uncertainty = std::time::Duration::from_millis(2);
    let too_big = u128::from(u64::MAX) + 1;
    assert_eq!(project_exit_instant(base, 0, too_big, uncertainty), None);
    assert_eq!(project_exit_instant(base, too_big, 0, uncertainty), None);
    assert_eq!(
        project_exit_instant(base, 1_000, 1_000, std::time::Duration::MAX),
        None
    );
    let edge = std::time::Duration::from_secs(i64::MAX as u64 - 1_000_000_000);
    if let Some(near_max) = base.checked_add(edge) {
        assert_eq!(
            project_exit_instant(near_max, 0, u128::from(u64::MAX), uncertainty),
            None
        );
    }
    if let Some(near_min) = base.checked_sub(edge) {
        assert_eq!(
            project_exit_instant(near_min, u128::from(u64::MAX), 0, uncertainty),
            None
        );
    }
}

#[test]
fn project_exit_instant_keeps_late_and_uncertain_exit_past_deadline() {
    let sampled = std::time::Instant::now();
    let end = sampled + std::time::Duration::from_millis(100);
    let uncertainty = std::time::Duration::from_millis(2);
    let late = project_exit_instant(sampled, 5_000, 5_101, uncertainty).unwrap();
    assert!(late > end);
    let boundary_uncertain = project_exit_instant(sampled, 5_000, 5_100, uncertainty).unwrap();
    assert!(boundary_uncertain > end);
    let timely = project_exit_instant(sampled, 5_000, 5_095, uncertainty).unwrap();
    assert!(timely <= end);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_phase_identity_discovers_owned_synthetic_phase_child() {
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let run = Uuid::from_u128(0xda09);
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| "python".into());
    let child = Command::new(python)
        .args([
            "-c",
            "import time; time.sleep(30)",
            "m1-run-phase",
            "--run",
            &run.to_string(),
            "--phase",
            RunPhase::Acquisition.name(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let guard = ChildGuard(child);
    let child_pid = guard.0.id();
    let parent_pid = std::process::id();

    let stat = std::fs::read_to_string(format!("/proc/{child_pid}/stat")).unwrap();
    let (stat_ppid, stat_birth) = parse_proc_stat(&stat).unwrap();
    assert_eq!(stat_ppid, parent_pid);

    let found = phase_identity(parent_pid, run, Some(RunPhase::Acquisition)).unwrap();
    assert_eq!(found.pid, child_pid);
    assert_eq!(found.birth, stat_birth);
    assert!(found.birth > 0);
    assert_eq!(found.run, run);
    assert_eq!(found.phase, Some("acquisition"));
    assert!(phase_identity(parent_pid, run, Some(RunPhase::DecisionTwo)).is_none());

    let wildcard = phase_identity(parent_pid, run, None).unwrap();
    assert_eq!(wildcard.pid, child_pid);
    assert_eq!(wildcard.birth, stat_birth);
}

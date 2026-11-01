use super::*;

#[test]
fn an_unready_boundary_never_sends_host_commands() {
    let mut boundary = LiveBoundary::new(Uuid::new_v4());
    let error = boundary
        .command(&HostCommand::Ack {
            attempt: Uuid::new_v4(),
            generation: 1,
        })
        .unwrap_err();
    assert_eq!(error, Fail::State("live_host_unavailable"));
}

#[test]
fn exited_reports_clean_exit_and_reaps_killed_children() {
    let mut child = std::process::Command::new("/bin/true").spawn().unwrap();
    assert!(exited(&mut child, 5000).unwrap(), "clean exit reports success");
    let mut child = std::process::Command::new("/bin/sleep")
        .arg("5")
        .spawn()
        .unwrap();
    // A stalled child is terminated inside the shared grace and reports
    // not-clean rather than pretending the deadline was met.
    assert!(!exited(&mut child, 50).unwrap());
    assert!(
        child.try_wait().unwrap().is_some(),
        "the killed child is reaped, not leaked"
    );
}

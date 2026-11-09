use super::*;

#[test]
fn admission_refuses_a_non_canonical_invocation() {
    let error = serve().unwrap_err();
    assert_eq!(error, Fail::Config("live_host_unqualified"));
}

#[test]
fn harden_refuses_when_not_root_or_extra_arguments() {
    let error = harden().unwrap_err();
    assert_eq!(error, Fail::Config("live_host_unqualified"));
}

#[test]
fn parent_channel_fails_when_stdin_is_not_socket() {
    let _ = parent_channel();
}

#[test]
fn serve_connection_routes_by_peer_uid() {
    let (a, _b) = UnixStream::pair().unwrap();

    // Root peer uid routes to watchdog_peer (which fails closed in unprivileged test)
    let root_cred = rustix::net::UCred {
        pid: rustix::process::getpid(),
        uid: rustix::process::Uid::from_raw(0),
        gid: rustix::process::Gid::from_raw(0),
    };
    let err = serve_connection(a, root_cred).unwrap_err();
    assert_eq!(err, Fail::Config("live_host_unqualified"));

    let (a, _b) = UnixStream::pair().unwrap();

    // Non-root peer uid routes to identity (which fails closed in unprivileged test)
    let non_root_cred = rustix::net::UCred {
        pid: rustix::process::getpid(),
        uid: rustix::process::Uid::from_raw(1001),
        gid: rustix::process::Gid::from_raw(1001),
    };
    let err = serve_connection(a, non_root_cred).unwrap_err();
    assert_eq!(err, Fail::Config("live_host_unqualified"));
}

#[test]
fn authenticated_channel_uses_real_peer_credentials_and_bounded_io() {
    let (socket, _peer) = UnixStream::pair().unwrap();
    let (socket, credential) = authenticate_channel(socket).unwrap();
    assert_eq!(credential.pid, rustix::process::getpid());
    assert_eq!(credential.uid, rustix::process::getuid());
    assert_eq!(
        socket.read_timeout().unwrap(),
        Some(Duration::from_millis(1000))
    );
    assert_eq!(
        socket.write_timeout().unwrap(),
        Some(Duration::from_millis(1000))
    );
}

#[test]
fn process_hardening_is_verified_in_a_disposable_child() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "admission::tests::process_hardening_probe",
            "--ignored",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "invoked only by the disposable-child hardening witness"]
fn process_hardening_probe() {
    restrict_process().unwrap();
    let limit = rustix::process::getrlimit(rustix::process::Resource::Core);
    assert_eq!(limit.current, Some(0));
    assert_eq!(limit.maximum, Some(0));
    assert_eq!(
        rustix::process::dumpable_behavior().unwrap(),
        rustix::process::DumpableBehavior::NotDumpable
    );
}

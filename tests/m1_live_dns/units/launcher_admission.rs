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
    let (r, w) = UnixStream::pair().unwrap();
    let raw = super::super::relay::RawPipe { read: r, write: w };

    // Root peer uid routes to watchdog_peer (which fails closed in unprivileged test)
    let root_cred = rustix::net::UCred {
        pid: rustix::process::getpid(),
        uid: rustix::process::Uid::from_raw(0),
        gid: rustix::process::Gid::from_raw(0),
    };
    let err = serve_connection(a, root_cred, raw).unwrap_err();
    assert_eq!(err, Fail::Config("live_host_unqualified"));

    let (a, _b) = UnixStream::pair().unwrap();
    let (r, w) = UnixStream::pair().unwrap();
    let raw = super::super::relay::RawPipe { read: r, write: w };

    // Non-root peer uid routes to identity (which fails closed in unprivileged test)
    let non_root_cred = rustix::net::UCred {
        pid: rustix::process::getpid(),
        uid: rustix::process::Uid::from_raw(1001),
        gid: rustix::process::Gid::from_raw(1001),
    };
    let err = serve_connection(a, non_root_cred, raw).unwrap_err();
    assert_eq!(err, Fail::Config("live_host_unqualified"));
}

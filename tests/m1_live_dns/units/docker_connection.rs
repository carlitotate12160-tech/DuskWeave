use super::*;
use std::fs::DirBuilder;
use std::io::Read;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct PrivateSocket(PathBuf);
impl PrivateSocket {
    fn new() -> Self {
        let unique = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let dir = std::env::temp_dir().join(format!("dwconn-{}", unique.trim()));
        DirBuilder::new().mode(0o700).create(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> PathBuf {
        self.0.join("socket")
    }
}
impl Drop for PrivateSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path());
        let _ = std::fs::remove_file(self.0.join("ready"));
        let _ = std::fs::remove_dir(&self.0);
    }
}
struct Probe(Child);
impl Probe {
    fn start(test: &str, path: &Path, non_root: bool) -> Self {
        let executable = std::env::current_exe().unwrap();
        let mut command = if non_root && rustix::process::geteuid().as_raw() == 0 {
            let mut command = Command::new("/usr/bin/setpriv");
            command
                .args(["--reuid=65534", "--regid=65534", "--clear-groups"])
                .arg(executable);
            command
        } else {
            Command::new(executable)
        };
        Self(
            command
                .args(["--exact", test, "--test-threads=1"])
                .env_clear()
                .env("DW_TEST_CONNECTION_PATH", path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
    fn ready_by(&mut self, marker: &Path, end: Instant) -> bool {
        while Instant::now() < end {
            if marker.is_file() {
                return true;
            }
            if self.0.try_wait().unwrap().is_some() {
                return false;
            }
            std::thread::yield_now();
        }
        false
    }
    fn finished_by(&mut self, end: Instant) -> bool {
        while Instant::now() < end {
            if self.0.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::yield_now();
        }
        false
    }
    fn succeeded(&mut self) -> bool {
        self.0
            .try_wait()
            .unwrap()
            .is_some_and(|status| status.success())
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        assert!(super::super::reap_child(
            &mut self.0,
            Instant::now() + Duration::from_millis(1000)
        ));
    }
}

#[test]
fn full_backlog_refuses_without_waiting_for_accept() {
    if let Some(path) = std::env::var_os("DW_TEST_CONNECTION_PATH") {
        let before = descriptor_count();
        assert!(
            acquire_at(
                Path::new(&path),
                Instant::now() + Duration::from_millis(100),
                Instant::now
            )
            .is_err()
        );
        assert_eq!(
            descriptor_count(),
            before,
            "backlog refusal must close its descriptor"
        );
        return;
    }
    let private = PrivateSocket::new();
    let listener = UnixListener::bind(private.path()).unwrap();
    rustix::net::listen(&listener, 0).unwrap();
    listener.set_nonblocking(true).unwrap();
    let _queued = UnixStream::connect(private.path()).unwrap();
    let mut probe = Probe::start(
        "docker::connection::tests::full_backlog_refuses_without_waiting_for_accept",
        &private.path(),
        false,
    );
    let refused_before_accept = probe.finished_by(Instant::now() + Duration::from_secs(2));
    if !refused_before_accept {
        // The outer observer frees the RED case, then always reaps its child.
        let _accepted = listener.accept().unwrap();
        assert!(probe.finished_by(Instant::now() + Duration::from_secs(1)));
    }
    assert!(
        refused_before_accept,
        "acquisition waited for accept despite the original deadline"
    );
    assert!(probe.succeeded());
}
fn descriptor_count() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}
fn no_payload(peer: &mut UnixStream) {
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(
        peer.read(&mut [0]).unwrap(),
        0,
        "acquisition/authentication must send no attach bytes"
    );
}

#[test]
fn available_listener_authenticates_root_and_restores_blocking_mode() {
    let private = PrivateSocket::new();
    let listener = UnixListener::bind(private.path()).unwrap();
    let result = acquire_at(
        &private.path(),
        Instant::now() + Duration::from_secs(1),
        Instant::now,
    );
    assert_eq!(result.is_ok(), rustix::process::geteuid().as_raw() == 0);
    if let Ok(io) = result {
        assert!(
            !rustix::fs::fcntl_getfl(&io)
                .unwrap()
                .contains(rustix::fs::OFlags::NONBLOCK)
        );
        assert!(
            rustix::io::fcntl_getfd(&io)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        drop(io);
    }
    no_payload(&mut listener.accept().unwrap().0);
}

#[test]
fn expired_original_deadline_refuses_before_connect() {
    let private = PrivateSocket::new();
    let listener = UnixListener::bind(private.path()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let end = Instant::now();
    assert!(acquire_at(&private.path(), end, || end).is_err());
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

#[test]
fn expiry_after_authentication_closes_without_attach_bytes() {
    let private = PrivateSocket::new();
    let listener = UnixListener::bind(private.path()).unwrap();
    let start = Instant::now();
    let end = start + Duration::from_millis(100);
    let mut clocks = 0;
    let result = acquire_at(&private.path(), end, || {
        clocks += 1;
        if clocks == 1 { start } else { end }
    });
    assert!(result.is_err());
    if rustix::process::geteuid().as_raw() == 0 {
        assert_eq!(clocks, 2);
    }
    no_payload(&mut listener.accept().unwrap().0);
}

#[test]
fn absent_and_refused_listeners_leave_no_descriptors() {
    if let Some(path) = std::env::var_os("DW_TEST_CONNECTION_PATH") {
        let path = Path::new(&path);
        let before = descriptor_count();
        for _ in 0..32 {
            for candidate in [path.to_path_buf(), path.with_file_name("absent")] {
                assert!(
                    acquire_at(
                        &candidate,
                        Instant::now() + Duration::from_secs(1),
                        Instant::now
                    )
                    .is_err()
                );
            }
        }
        assert_eq!(descriptor_count(), before);
        return;
    }
    let private = PrivateSocket::new();
    drop(UnixListener::bind(private.path()).unwrap());
    let mut probe = Probe::start(
        "docker::connection::tests::absent_and_refused_listeners_leave_no_descriptors",
        &private.path(),
        false,
    );
    assert!(probe.finished_by(Instant::now() + Duration::from_secs(2)));
    assert!(probe.succeeded());
}

#[test]
fn non_root_listener_is_rejected_without_attach_bytes() {
    if let Some(path) = std::env::var_os("DW_TEST_CONNECTION_PATH") {
        assert_ne!(rustix::process::geteuid().as_raw(), 0);
        let path = Path::new(&path);
        let listener = UnixListener::bind(path).unwrap();
        std::fs::write(path.with_file_name("ready"), b"ready").unwrap();
        no_payload(&mut listener.accept().unwrap().0);
        return;
    }
    let private = PrivateSocket::new();
    if rustix::process::geteuid().as_raw() == 0 {
        std::os::unix::fs::chown(&private.0, Some(65534), Some(65534)).unwrap();
    }
    let mut probe = Probe::start(
        "docker::connection::tests::non_root_listener_is_rejected_without_attach_bytes",
        &private.path(),
        true,
    );
    assert!(probe.ready_by(
        &private.0.join("ready"),
        Instant::now() + Duration::from_secs(2)
    ));
    assert!(
        acquire_at(
            &private.path(),
            Instant::now() + Duration::from_secs(1),
            Instant::now
        )
        .is_err()
    );
    assert!(probe.finished_by(Instant::now() + Duration::from_secs(1)));
    assert!(probe.succeeded());
}

#[test]
fn acquire_entrypoint_rejects_a_past_deadline() {
    assert!(acquire(Instant::now() - Duration::from_secs(1)).is_err());
}

#[test]
fn acquire_at_refuses_an_absent_private_socket() {
    let private = PrivateSocket::new();
    assert!(
        acquire_at(
            &private.path(),
            Instant::now() + Duration::from_secs(1),
            Instant::now
        )
        .is_err()
    );
}

#[test]
fn root_peer_authentication_is_exercised_in_an_isolated_child() {
    let executable = std::env::current_exe().unwrap();
    for name in [
        "docker::connection::tests::available_listener_authenticates_root_and_restores_blocking_mode",
        "docker::connection::tests::expiry_after_authentication_closes_without_attach_bytes",
    ] {
        let mut command = Command::new("/usr/bin/sudo");
        command.args(["-n", "--", "/usr/bin/env"]);
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            let mut assignment = std::ffi::OsString::from("LLVM_PROFILE_FILE=");
            assignment.push(profile);
            command.arg(assignment);
        }
        command
            .arg(&executable)
            .args(["--exact", name, "--test-threads=1"])
            .env_clear()
            .stdin(Stdio::null());
        assert!(
            command.status().unwrap().success(),
            "root peer witness: {name}"
        );
    }
}

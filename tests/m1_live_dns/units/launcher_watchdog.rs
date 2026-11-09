use super::*;

#[test]
fn watchdog_serve_refuses_non_ack_commands_and_unknown_sessions() {
    use duskweave::m1_effect_control::{HostChannel, HostCommand};
    // A non-Ack first frame is refused at initialize, before any loop.
    let (a, b) = UnixStream::pair().unwrap();
    let mut peer = HostChannel::new(b);
    peer.send_command(&HostCommand::StopRun {
        run_id: uuid::Uuid::new_v4(),
    })
    .unwrap();
    let error = serve(a).unwrap_err();
    assert_eq!(error, Fail::Config("live_host_unqualified"));
    // An Ack that names no committed session cannot become a watchdog lease:
    // attempt::load fails before the socket is ever watched.
    let (a, b) = UnixStream::pair().unwrap();
    let mut peer = HostChannel::new(b);
    peer.send_command(&HostCommand::Ack {
        attempt: uuid::Uuid::new_v4(),
        generation: 9,
    })
    .unwrap();
    assert!(
        serve(a).is_err(),
        "unknown session refuses the watchdog lane"
    );
}

#[test]
fn watchdog_current_monitors_child_process_status() {
    let (parent, _child) = UnixStream::pair().unwrap();
    let process = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    let mut watchdog = Watchdog {
        process,
        channel: parent,
    };
    assert!(watchdog.current().is_ok());

    // Drop cleanly kills and reaps
    drop(watchdog);

    // Watchdog on already exited child reports unavailable
    let (parent, _child) = UnixStream::pair().unwrap();
    let mut process = Command::new("/bin/true").spawn().unwrap();
    let _ = process.wait();
    let mut watchdog = Watchdog {
        process,
        channel: parent,
    };
    assert_eq!(
        watchdog.current().unwrap_err(),
        Fail::State("live_watchdog_unavailable")
    );
}

thread_local! {
    static TEST_CHILD: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}
pub(crate) struct ChildScope(Option<bool>);
impl Drop for ChildScope {
    fn drop(&mut self) {
        TEST_CHILD.with(|slot| slot.set(self.0));
    }
}
pub(crate) fn child_scope(bad: bool) -> ChildScope {
    ChildScope(TEST_CHILD.with(|slot| slot.replace(Some(bad))))
}
pub(super) fn child_command() -> Option<Command> {
    TEST_CHILD.with(|slot| {
        slot.get().map(|bad| {
            let mut command = Command::new(std::env::current_exe().unwrap());
            let test = if bad {
                "control::watchdog::tests::wrong_ready_probe"
            } else {
                "control::watchdog::tests::ready_probe"
            };
            command.args(["--exact", test, "--ignored", "--test-threads=1"]);
            command
        })
    })
}
fn probe(bad: bool) {
    let socket = UnixStream::from(rustix::io::dup(std::io::stdin()).unwrap());
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut channel = HostChannel::new(socket.try_clone().unwrap());
    let HostCommand::Ack {
        attempt,
        generation,
    } = channel.command().unwrap()
    else {
        panic!("watchdog handshake must bind an attempt");
    };
    channel
        .send_reply(&HostReply::Ready {
            attempt,
            generation: generation + u64::from(bad),
        })
        .unwrap();
    let _ = (&socket).read(&mut [0]);
}
#[test]
#[ignore = "owned protocol child, reached only through the scoped command factory"]
fn ready_probe() {
    probe(false);
}
#[test]
#[ignore = "owned protocol child returning the wrong generation"]
fn wrong_ready_probe() {
    probe(true);
}

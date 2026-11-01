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
    assert!(serve(a).is_err(), "unknown session refuses the watchdog lane");
}

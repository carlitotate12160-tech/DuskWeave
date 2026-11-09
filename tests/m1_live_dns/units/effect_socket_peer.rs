//! Process transport fixture; canonical host reply validation stays in socket.rs.
use super::*;
use std::cell::Cell;
use std::process::Command;
thread_local! {
    static MODE: Cell<Option<&'static str>> = const { Cell::new(None) };
}
pub(super) struct Scope(Option<&'static str>);
impl Scope {
    pub(super) fn new(mode: &'static str) -> Self {
        Self(MODE.with(|slot| slot.replace(Some(mode))))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        MODE.with(|slot| slot.set(self.0));
    }
}
pub(in crate::m1_effect_control::socket) fn command() -> Option<Command> {
    MODE.with(|slot| {
        slot.get().map(|mode| {
            let mut command = Command::new(std::env::current_exe().unwrap());
            let test = format!("m1_effect_control::socket::tests::peer::{mode}");
            command.args(["--exact", &test, "--ignored", "--test-threads=1"]);
            command
        })
    })
}
fn run(mode: &str) {
    use std::os::unix::net::UnixStream;
    let socket = UnixStream::from(rustix::io::dup(std::io::stdin()).unwrap());
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut channel = HostChannel::new(socket);
    let command = channel.command().unwrap();
    if mode == "refuse" {
        channel.send_reply(&HostReply::Refused).unwrap();
        return;
    }
    let reply = match command {
        HostCommand::Qualify { binding, .. } => HostReply::Qualified {
            qualification_ref: if mode == "wrong_identity" {
                Uuid::new_v4()
            } else {
                binding.qualification_ref
            },
            manifest_sha256: "a".repeat(64),
            boot_id: Uuid::from_u128(1),
        },
        HostCommand::Stop { .. } => HostReply::Stopped {
            disposal_confirmed: true,
        },
        HostCommand::StopRun { run_id } => HostReply::RunStopped {
            run_id: if mode == "wrong_identity" {
                Uuid::new_v4()
            } else {
                run_id
            },
            stop_latched: true,
            disposal_confirmed: true,
        },
        HostCommand::StopCampaign {
            engagement_id,
            campaign_id,
        } => HostReply::CampaignStopped {
            engagement_id,
            campaign_id,
            stop_latched: true,
            disposal_confirmed: false,
        },
        HostCommand::Open { request, .. } => {
            channel
                .send_reply(&HostReply::Ready {
                    attempt: request.attempt,
                    generation: request.session_generation + u64::from(mode == "wrong_identity"),
                })
                .unwrap();
            let Ok(HostCommand::Start {
                attempt,
                generation,
                ..
            }) = channel.command()
            else {
                return;
            };
            assert_eq!(
                (attempt, generation),
                (request.attempt, request.session_generation)
            );
            let result: DnsResult = serde_json::from_value(serde_json::json!({
                "schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":request.attempt,
                "session_generation":request.session_generation,"sequence":3,"status":"completed",
                "observed_at_ms":1000,"rcode":0,"truncated":false,"complete":true,
                "withheld_aliases":0,"answers":[{"ipv4":"198.51.100.7","ttl_seconds":30}],"aliases":[]
            })).unwrap();
            channel.send_reply(&HostReply::Result { result }).unwrap();
            match channel.command() {
                Ok(HostCommand::Ack {
                    attempt,
                    generation,
                }) => {
                    assert_eq!(
                        (attempt, generation),
                        (request.attempt, request.session_generation)
                    );
                    HostReply::Stopped {
                        disposal_confirmed: true,
                    }
                }
                _ => return,
            }
        }
        _ => panic!("unexpected fixture command"),
    };
    channel.send_reply(&reply).unwrap();
}
#[test]
#[ignore = "owned fixed-protocol child"]
fn good() {
    run("good");
}
#[test]
#[ignore = "owned fixed-protocol refusal child"]
fn refuse() {
    run("refuse");
}
#[test]
#[ignore = "owned fixed-protocol child with the wrong identity"]
fn wrong_identity() {
    run("wrong_identity");
}

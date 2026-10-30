//! Owner-local regression tests.
use super::*;
use std::time::Duration;
struct BlockedBasis(bool);
impl ProposalBasis for BlockedBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        matches!(r, "mission" | "asset_0")
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
    }
    fn safe_explanation(&self, _: &str) -> bool {
        true
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        if !self.0 {
            return Ok(());
        }
        println!("screening_blocked");
        loop {
            std::thread::park();
        }
    }
}
#[test]
fn controlled_fixture_worker() {
    let Ok(name) = std::env::var("DW_CONTROLLED_PROTOCOL_WORKER") else {
        return;
    };
    let end = Instant::now() + Duration::from_millis(124000);
    let permit = Permit::new(end);
    assert!(permit.publish(Instant::now(), end));
    let (reporter, full_queue) = std::sync::mpsc::sync_channel(1);
    reporter.try_send(ReportedGuard::CommandWatchdog).unwrap();
    let _full_queue = full_queue;
    let profile = crate::m1_inference_profile::lookup(&name).unwrap();
    let mut provider = ProtocolFixture::controlled(profile, permit, Some(reporter)).unwrap();
    let pack=json!({"version":"m1_context_pack_autonomous_v2","run_profile":"autonomous-dns-fixture-v2",
            "execution_mode":"controlled_fixture","inference_profile":name,"references":[{"id":"mission"},{"id":"asset_0"}],
            "candidates":[{"reference":"asset_0"}],"manifests":[{"reference":"dns_udp"}]}).to_string();
    let b = super::super::request::bind(pack.as_bytes(), None).unwrap();
    provider.prepare(&b, end).unwrap();
    let after_call = std::env::var_os("DW_CONTROLLED_PROTOCOL_AFTER_CALL").is_some();
    let mut basis = BlockedBasis(!after_call);
    let received = provider.call(&b, end, &mut basis, &mut || {
        println!("fixture_released");
        Ok(())
    });
    if after_call {
        assert!(matches!(
            received.unwrap().admission,
            super::super::proposal::Admission::Accepted(_)
        ));
        basis.0 = true;
        let _ = basis.recording_premises();
    }
    panic!("blocked screening returned without the independent guard");
}
#[test]
fn both_controlled_protocols_terminate_blocked_screening_when_lease_and_reporter_are_lost() {
    controlled_screening_stall(false);
}
#[test]
fn both_controlled_protocols_keep_the_lease_guard_for_post_call_recording() {
    controlled_screening_stall(true);
}
fn controlled_screening_stall(after_call: bool) {
    use std::process::{Command, Stdio};
    for name in ["fixture-responses-v1", "fixture-chat-completions-v1"] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.env_clear();
        for key in ["SystemRoot", "LLVM_PROFILE_FILE"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        if after_call {
            command.env("DW_CONTROLLED_PROTOCOL_AFTER_CALL", "1");
        }
        let started = Instant::now();
        let mut child = command
            .args([
                "--exact",
                "m1_reasoning::fixture::tests::controlled_fixture_worker",
                "--nocapture",
            ])
            .env("DW_CONTROLLED_PROTOCOL_WORKER", name)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        while child.try_wait().unwrap().is_none() {
            if started.elapsed() > Duration::from_secs(5) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("custodian cleanup is not product termination");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(124));
        assert!(String::from_utf8_lossy(&output.stdout).contains("screening_blocked"));
        assert!(started.elapsed() < Duration::from_millis(2300));
        println!(
            "controlled {name} stopped blocked screening after {}ms",
            started.elapsed().as_millis()
        );
    }
}

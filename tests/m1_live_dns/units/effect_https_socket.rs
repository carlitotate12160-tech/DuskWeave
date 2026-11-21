use super::super::{HostChannel, HostCommand, HostReply};
use super::*;
use crate::m1_https::ipc::{HttpsLimits, HttpsOutcome, HttpsSafeResult};
use crate::m1_https::result::LAB_SCOPE_LIMITATION;
use crate::m1_https::{RedirectDisposition, VERSION_HTTPS_IPC};
use crate::m1_inference_profile::{
    HTTPS_HEAD_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER,
    HTTPS_LAB_RUN_PROFILE,
};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn sample_request() -> HttpsIpcRequest {
    HttpsIpcRequest {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "REQUEST".into(),
        attempt: Uuid::from_u128(1),
        session_generation: 1,
        sequence: 0,
        engagement_id: Uuid::from_u128(2),
        campaign_id: Uuid::from_u128(3),
        decision_id: Uuid::from_u128(4),
        run_id: Uuid::from_u128(5),
        step_index: 2,
        attempt_kind: "head".into(),
        result_id: Uuid::from_u128(6),
        profile: HTTPS_HEAD_PROFILE.into(),
        execution_mode: "live".into(),
        run_profile: HTTPS_LAB_RUN_PROFILE.into(),
        release_sha256: "a".repeat(64),
        qualification_ref: Uuid::from_u128(7),
        asset_ref: Uuid::from_u128(8),
        endpoint_ref: "endpoint_1".into(),
        hostname: HTTPS_LAB_HOSTNAME.into(),
        ipv4: HTTPS_LAB_IPV4.into(),
        port: 443,
        approved_path: "/".into(),
        method: "HEAD".into(),
        parent_claim_id: Uuid::from_u128(9),
        parent_claim_revision: 1,
        resolver_ipv4: HTTPS_LAB_RESOLVER.into(),
        vantage_ref: Uuid::from_u128(10),
        permission_id: Uuid::from_u128(11),
        expected_mission_revision: 1,
        authority_evaluated_at_ms: 1000,
        claim_valid_until_s: 60,
        episode_deadline_at_ms: 5000,
        deadline_at_ms: 4000,
        max_duration_ms: 3000,
        limits: HttpsLimits::fixed(),
    }
}

fn sample_safe_result(q: &HttpsIpcRequest) -> HttpsSafeResult {
    HttpsSafeResult {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "RESULT".into(),
        attempt: q.attempt,
        session_generation: q.session_generation,
        sequence: 7,
        outcome: HttpsOutcome::HeadResponse,
        observed_at_ms: 3500,
        network_bytes: 128,
        header_bytes: 64,
        status_code: 200,
        tls_category: "none".into(),
        redirect: RedirectDisposition::None,
        limitations: vec![
            LAB_SCOPE_LIMITATION.into(),
            "head_response_does_not_prove_access_or_objective_completion".into(),
        ],
    }
}

struct ControlRootGuard(std::path::PathBuf);
impl ControlRootGuard {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("dw-https-sock-{}", Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        super::super::session::set_test_control_root(Some(root.clone()));
        Self(root)
    }
}
impl Drop for ControlRootGuard {
    fn drop(&mut self) {
        super::super::session::set_test_control_root(None);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cat_child() -> (std::process::Child, std::process::ChildStdin) {
    let mut child = std::process::Command::new("/bin/cat")
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let gate = child.stdin.take().unwrap();
    (child, gate)
}

#[test]
fn unready_https_boundary_refuses_commands_and_uncharged_stop_succeeds() {
    let req = sample_request();
    let mut boundary = LiveHttpsBoundary::new(req.run_id);
    assert_eq!(
        boundary
            .start_phase(&req, HttpsPhase::Connect, 3000)
            .unwrap_err(),
        Fail::State("live_host_unavailable")
    );
    assert_eq!(
        boundary.poll().unwrap_err(),
        Fail::State("live_host_unavailable")
    );
    assert_eq!(
        boundary.renew(&req, 3500).unwrap_err(),
        Fail::State("live_host_unavailable")
    );
    assert_eq!(
        boundary.acknowledge(&req).unwrap_err(),
        Fail::State("live_host_unavailable")
    );
    assert!(boundary.stop().unwrap());

    let _root = ControlRootGuard::new();
    assert!(boundary.ready_connect(&req).is_err());
    assert_eq!(
        boundary.ready_connect(&req).unwrap_err(),
        Fail::State("live_effect_already_charged")
    );
}

#[test]
fn open_with_drives_phases_renew_poll_and_acknowledges_cleanly() {
    let req = sample_request();
    let (client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let (child, gate) = cat_child();

    let req_srv = req.clone();
    let server_h = std::thread::spawn(move || {
        let _gate = gate;
        let mut ch = HostChannel::new(server);
        assert!(matches!(
            ch.command().unwrap(),
            HostCommand::OpenHttps { .. }
        ));
        ch.send_reply(&HostReply::ReadyPhase {
            attempt: req_srv.attempt,
            generation: req_srv.session_generation,
            phase: HttpsPhase::Connect,
        })
        .unwrap();
        for phase in [HttpsPhase::Connect, HttpsPhase::Tls, HttpsPhase::Head] {
            assert!(
                matches!(ch.command().unwrap(), HostCommand::StartPhase { phase: p, .. } if p == phase)
            );
            let next = match phase {
                HttpsPhase::Connect => Some(HttpsPhase::Tls),
                HttpsPhase::Tls => Some(HttpsPhase::Head),
                HttpsPhase::Head => None,
            };
            if let Some(p) = next {
                ch.send_reply(&HostReply::ReadyPhase {
                    attempt: req_srv.attempt,
                    generation: req_srv.session_generation,
                    phase: p,
                })
                .unwrap();
            }
        }
        assert!(matches!(
            ch.command().unwrap(),
            HostCommand::RenewHttps {
                deadline_at_ms: 3900,
                ..
            }
        ));
        ch.send_reply(&HostReply::ResultHttps {
            result: sample_safe_result(&req_srv),
        })
        .unwrap();
        assert!(matches!(
            ch.command().unwrap(),
            HostCommand::AckHttps { .. }
        ));
        drop(_gate);
        ch.send_reply(&HostReply::Stopped {
            disposal_confirmed: true,
        })
        .unwrap();
    });

    let mut boundary = LiveHttpsBoundary::new(req.run_id);
    boundary.request = Some(req.clone());
    boundary
        .open_with(&req, |_| Ok((HostChannel::new(client), child)))
        .expect("open_with");
    assert!(boundary.poll().unwrap().is_none());

    boundary
        .start_phase(&req, HttpsPhase::Connect, 3500)
        .unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        boundary.poll().unwrap(),
        Some(HttpsEvent::Ready(HttpsPhase::Tls))
    );
    boundary.start_phase(&req, HttpsPhase::Tls, 3500).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        boundary.poll().unwrap(),
        Some(HttpsEvent::Ready(HttpsPhase::Head))
    );
    boundary.start_phase(&req, HttpsPhase::Head, 3500).unwrap();
    boundary.renew(&req, 3900).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    assert!(matches!(
        boundary.poll().unwrap(),
        Some(HttpsEvent::Result(_))
    ));
    assert!(boundary.acknowledge(&req).unwrap());
    assert!(boundary.closed);
    server_h.join().unwrap();
}

#[test]
fn protocol_validators_reject_drift_and_failed_open_reaps_child() {
    let req = sample_request();
    let expected = (req.attempt, req.session_generation);
    assert!(!valid_phase_reply(
        req.attempt,
        req.session_generation,
        HttpsPhase::Connect,
        expected
    ));
    assert!(!valid_phase_reply(
        req.attempt,
        req.session_generation + 1,
        HttpsPhase::Tls,
        expected
    ));
    assert_eq!(
        decode_polled_reply(HostReply::Refused, expected).unwrap_err(),
        Fail::State("live_host_protocol_refused")
    );

    let (client, server) = UnixStream::pair().unwrap();
    let (child, _gate) = cat_child();
    let pid = child.id();
    std::thread::spawn(move || {
        let mut ch = HostChannel::new(server);
        let _ = ch.command();
        let _ = ch.send_reply(&HostReply::Refused);
    });
    let mut boundary = LiveHttpsBoundary::new(req.run_id);
    let err = boundary
        .open_with(&req, |_| Ok((HostChannel::new(client), child)))
        .unwrap_err();
    assert_eq!(err, Fail::State("live_host_protocol_refused"));
    assert!(boundary.peer.is_none());
    assert!(!std::path::PathBuf::from(format!("/proc/{pid}")).exists());

    let (tx, rx) = std::sync::mpsc::sync_channel(4);
    let (c, s) = UnixStream::pair().unwrap();
    HostChannel::new(s)
        .send_reply(&HostReply::ReadyPhase {
            attempt: req.attempt,
            generation: req.session_generation,
            phase: HttpsPhase::Connect,
        })
        .unwrap();
    let mut chan = HostChannel::new(c);
    assert!(!step_reply(&mut chan, expected, &tx));
    assert!(matches!(
        rx.recv().unwrap(),
        Err(Fail::State("live_host_protocol_refused"))
    ));

    let (c2, s2) = UnixStream::pair().unwrap();
    let t = std::thread::spawn(move || {
        let mut s_ch = HostChannel::new(s2);
        let _ = s_ch.command();
        let _ = s_ch.send_reply(&HostReply::Refused);
    });
    let mut c2_ch = HostChannel::new(c2);
    assert!(matches!(
        exchange_ack(&mut c2_ch, &req),
        Err(Fail::State("live_host_protocol_refused"))
    ));
    t.join().unwrap();
}

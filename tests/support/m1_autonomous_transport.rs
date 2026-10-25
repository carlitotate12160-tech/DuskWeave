//! Private loopback construction; production exposes no endpoint/CA override.
use super::tests::Server;
use super::*;
use crate as control_domain;
use crate::m1_reasoning::{proposal::Admission, request};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "../../src/m1_run_control.rs"]
mod run_control;

struct Basis {
    checks: AtomicUsize,
    eligible: bool,
}
impl ProposalBasis for Basis {
    fn reference_eligible(&self, r: &str) -> bool {
        r == "asset_0"
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        self.eligible
            .then_some(())
            .ok_or("current_authority_unavailable")
    }
    fn safe_explanation(&self, text: &str) -> bool {
        assert!(
            !text.contains("synthetic-fixture-token"),
            "known key reached an owner callback"
        );
        true
    }
}
fn bound(repair: bool) -> BoundCall {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../docs/contracts/fixtures/m1-autonomous-wire-examples-v1.json"
    ))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c["pack"]
                .as_str()
                .unwrap()
                .contains("autonomous-dns-provider-fixture-v1")
        })
        .unwrap();
    request::bind(
        case["pack"].as_str().unwrap().as_bytes(),
        repair.then_some(&["proposal_shape"][..]),
    )
    .unwrap()
}
fn envelope() -> Vec<u8> {
    let proposal = json!({"version":"m1_inference_proposal_v1",
        "hypothesis":"A bounded fixture premise remains uncertain.",
        "considered_alternative":"The premise may be unavailable.","references":["asset_0"],
        "expected_observation":"Fixture routing metadata.","falsifying_observation":"No eligible premise.",
        "uncertainty":"Synthetic evidence only.","rationale":"Retain uncertainty, not a live fact.",
        "next_intent":{"category":"retain","references":["asset_0"],"manifest_reference":null}});
    serde_json::to_vec(&json!({"status":"completed","model":"deepseek-flash",
        "output":[{"type":"message","role":"assistant","status":"completed",
        "content":[{"type":"output_text","text":proposal.to_string()}]}]}))
    .unwrap()
}
fn permit() -> Permit {
    let end = Instant::now() + Duration::from_secs(40);
    let p = Permit::new(end);
    assert!(p.publish(Instant::now(), end));
    p
}
fn provider(server: &Server, b: &BoundCall, permit: Permit) -> AutonomousProvider {
    autonomous_binding(b).unwrap();
    let mut h = configure(b, "synthetic-fixture-token", Duration::from_secs(20)).unwrap();
    h.url(&server.url).unwrap();
    AutonomousProvider {
        clock: None,
        admission_guard: None,
        reporter: None,
        prepared: Some((h, "synthetic-fixture-token".into(), b.digest.clone())),
        permit,
        pre_send: Box::new(|| Ok(())),
    }
}

#[test]
fn generation_and_repair_post_exact_autonomous_bytes_with_no_progress_owner_calls() {
    let linked = curl::Version::get();
    println!(
        "linked libcurl={} TLS={:?} c_ares={:?} features={linked:?}",
        linked.version(),
        linked.ssl_version(),
        linked.ares_version()
    );
    for repair in [false, true] {
        let b = bound(repair);
        let server = Server::start("200 OK", "", envelope());
        let mut basis = Basis {
            checks: AtomicUsize::new(0),
            eligible: true,
        };
        let hooks = Arc::new(AtomicUsize::new(0));
        let called = hooks.clone();
        let mut provider = provider(&server, &b, permit());
        provider.pre_send = Box::new(move || {
            called.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        let result = provider
            .call(
                &b,
                Instant::now() + Duration::from_secs(40),
                &mut basis,
                &mut || Ok(()),
            )
            .unwrap();
        assert!(matches!(result.admission, Admission::Accepted(_)));
        assert_eq!(
            basis.checks.load(Ordering::SeqCst),
            1,
            "only fresh response admission checks owners"
        );
        assert_eq!(*server.received_body.lock().unwrap(), b.body);
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            hooks.load(Ordering::SeqCst),
            1,
            "pre-send hook must never run in progress callbacks"
        );
    }
}

#[test]
fn stopped_before_release_has_zero_requests_and_an_actual_send_survives_unlock_failure_as_unknown()
{
    for repair in [false, true] {
        let b = bound(repair);
        let server = Server::start("200 OK", "", envelope());
        let permit = permit();
        let mut p = provider(&server, &b, permit.clone());
        permit.stop();
        assert_eq!(
            p.call(
                &b,
                Instant::now() + Duration::from_secs(40),
                &mut Basis {
                    checks: AtomicUsize::new(0),
                    eligible: true
                },
                &mut || panic!("no release from a stopped permit")
            )
            .err(),
            Some("current_authority_unavailable")
        );
        assert_eq!(server.calls.load(Ordering::SeqCst), 0);
        let mut p = provider(&server, &b, self::permit());
        let result = p.call(
            &b,
            Instant::now() + Duration::from_secs(40),
            &mut Basis {
                checks: AtomicUsize::new(0),
                eligible: true,
            },
            &mut || {
                server.arrived.recv_timeout(Duration::from_secs(1)).unwrap();
                Err("inference_release_unknown")
            },
        );
        assert_eq!(result.err(), Some("inference_release_unknown"));
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn worker_panic_prioritizes_release_error_then_reports_unknown_transport() {
    for release_ok in [false, true] {
        let b = bound(false);
        let server = Server::start("200 OK", "", envelope());
        let mut provider = provider(&server, &b, permit());
        let (entered_send, entered) = channel();
        let (gate_send, gate) = channel();
        let gate = std::sync::Mutex::new(gate);
        provider.pre_send = Box::new(move || {
            let _ = entered_send.send(());
            let _ = gate.lock().unwrap().recv_timeout(Duration::from_secs(3));
            panic!("synthetic worker panic");
        });
        let result = provider.call(
            &b,
            Instant::now() + Duration::from_secs(40),
            &mut Basis {
                checks: AtomicUsize::new(0),
                eligible: true,
            },
            &mut || {
                entered
                    .recv_timeout(Duration::from_secs(3))
                    .expect("worker reached the pre-send barrier");
                gate_send.send(()).unwrap();
                if release_ok {
                    Ok(())
                } else {
                    Err("inference_release_unknown")
                }
            },
        );
        let expected = if release_ok {
            "provider_transport_unknown"
        } else {
            "inference_release_unknown"
        };
        assert_eq!(result.err(), Some(expected), "release_ok={release_ok}");
        assert_eq!(
            server.calls.load(Ordering::SeqCst),
            0,
            "panic before exchange sends nothing"
        );
    }
}

#[test]
fn aggregate_bounds_protocol_reflection_and_late_reference_loss_never_publish_a_proposal() {
    for (status, headers, body, error) in [
        (
            "302 Found",
            "Location: http://127.0.0.1:1/never\r\n",
            vec![],
            "provider_protocol",
        ),
        (
            "200 OK",
            "Content-Encoding: gzip\r\n",
            envelope(),
            "provider_protocol",
        ),
        ("200 OK", "", vec![b'x'; 65537], "provider_response_limit"),
        (
            "200 OK",
            "",
            b"synthetic-fixture-token".to_vec(),
            "provider_unsafe_response",
        ),
        ("200 OK", "", b"not-json".to_vec(), "provider_protocol"),
    ] {
        let b = bound(false);
        let server = Server::start(status, headers, body);
        let result = provider(&server, &b, permit()).call(
            &b,
            Instant::now() + Duration::from_secs(40),
            &mut Basis {
                checks: AtomicUsize::new(0),
                eligible: true,
            },
            &mut || Ok(()),
        );
        assert_eq!(result.err(), Some(error));
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    }
    let b = bound(false);
    let server = Server::start("200 OK", "", envelope());
    let result = provider(&server, &b, permit())
        .call(
            &b,
            Instant::now() + Duration::from_secs(40),
            &mut Basis {
                checks: AtomicUsize::new(0),
                eligible: false,
            },
            &mut || Ok(()),
        )
        .unwrap();
    assert!(matches!(
        result.admission,
        Admission::Rejected("current_authority_unavailable")
    ));
}

#[test]
fn encoded_key_reflection_is_rejected_after_both_json_layers_are_decoded() {
    let key = "synthetic-fixture-token";
    let mut envelope: Value = serde_json::from_slice(&envelope()).unwrap();
    let mut proposal: Value = serde_json::from_str(
        envelope["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    proposal["hypothesis"] = json!(key);
    let escaped: String = key
        .chars()
        .map(|c| format!("\\u{:04x}", u32::from(c)))
        .collect();
    envelope["output"][0]["content"][0]["text"] =
        json!(proposal.to_string().replace(key, &escaped));
    let body = serde_json::to_vec(&envelope).unwrap();
    assert!(
        !body.windows(key.len()).any(|v| v == key.as_bytes()),
        "negative control bypasses raw-byte matching"
    );
    let b = bound(false);
    let server = Server::start("200 OK", "", body);
    let result = provider(&server, &b, permit()).call(
        &b,
        Instant::now() + Duration::from_secs(40),
        &mut Basis {
            checks: AtomicUsize::new(0),
            eligible: true,
        },
        &mut || Ok(()),
    );
    assert_eq!(result.err(), Some("provider_unsafe_response"));
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
}

fn control_request() -> crate::m1_run::RunRequest {
    let id = "00000000-0000-0000-0000-000000000021";
    serde_json::from_value(json!({"version":"m1_autonomous_run_request_v1","run_id":id,
        "run_profile":"autonomous-dns-provider-fixture-v1","execution_mode":"controlled_fixture",
        "deadline_at_ms":1,"context":{"version":"m1_context_prepare_request_v1","mission":{
            "version":"m1_inference_reservation_request_v1","engagement_id":id,"campaign_id":id,
            "operator_ref":id,"expected_mission_revision":1,"permission_id":id,"goal_ref":id,
            "exercise_mode":"blind"},"candidates":[{"asset_ref":id,"priority":"high"}]},
        "seed_names":[{"asset_ref":id,"name":"seed.example.invalid"}]}))
    .unwrap()
}

#[test]
fn autonomous_control_worker() {
    let Ok(mode) = std::env::var("DW_AUTONOMOUS_CONTROL_TEST") else {
        return;
    };
    let request = control_request();
    if mode == "reject_root" {
        assert!(run_control::Binding::qualify(&request).is_err());
        return;
    }
    let preparation = run_control::Preparation::start().unwrap();
    let binding = run_control::Binding::qualify(&request).unwrap();
    drop(preparation);
    if mode.starts_with("successor_") {
        return successor_clock_case(&mode, binding);
    }
    if mode == "stalled_pre_send" {
        return stalled_pre_send_case();
    }
    if mode.starts_with("pre_send_") {
        return pre_send_control_case(&mode, binding);
    }
    if mode == "signal_run" && std::env::var_os("DW_AUTONOMOUS_SIGNAL_ONLY").is_some() {
        run_control::signal_run(request.run_id).unwrap();
        assert!(binding.current().is_err());
        return;
    }
    if mode == "signal_campaign" && std::env::var_os("DW_AUTONOMOUS_SIGNAL_ONLY").is_some() {
        run_control::signal_campaign(
            request.context.mission.engagement_id.0,
            request.context.mission.campaign_id.0,
        )
        .unwrap();
        assert!(binding.current().is_err());
        return;
    }
    let b = bound(mode == "repair_remaining");
    let p = permit();
    run_control::read_parent(p.clone()).unwrap();
    let marker = binding.clone();
    if mode != "coordinator_loss" {
        let renew = p.clone();
        std::thread::spawn(move || {
            while !renew.stopped() {
                let start = Instant::now();
                if binding.current().is_err() {
                    renew.stop();
                    return;
                }
                if !renew.publish(start, start + Duration::from_secs(40)) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(400));
            }
        });
    }
    let mut h = configure(&b, "synthetic-fixture-token", Duration::from_secs(40)).unwrap();
    h.url(&std::env::var("DW_AUTONOMOUS_TEST_URL").unwrap())
        .unwrap();
    let mut provider = AutonomousProvider {
        clock: None,
        admission_guard: None,
        reporter: None,
        prepared: Some((h, "synthetic-fixture-token".into(), b.digest.clone())),
        permit: p,
        pre_send: Box::new(move || {
            marker
                .current()
                .map_err(|_| "current_authority_unavailable")
        }),
    };
    let remaining = if mode == "repair_remaining" {
        Duration::from_millis(500)
    } else {
        Duration::from_secs(40)
    };
    assert!(
        provider
            .call(
                &b,
                Instant::now() + remaining,
                &mut Basis {
                    checks: AtomicUsize::new(0),
                    eligible: true
                },
                &mut || Ok(())
            )
            .is_err()
    );
}

fn pre_send_control_case(mode: &str, binding: run_control::Binding) {
    let late_success = mode == "pre_send_late_success";
    let b = bound(mode.ends_with("repair"));
    let server = Server::start("200 OK", "", envelope());
    let p = permit();
    let mut provider = provider(&server, &b, p.clone());
    let (entered, gate) = channel();
    let (resume, parked) = channel();
    let parked = std::sync::Mutex::new(parked);
    provider.pre_send = Box::new(move || {
        entered.send(()).unwrap();
        parked.lock().unwrap().recv().unwrap();
        if late_success {
            Ok(())
        } else {
            binding
                .current()
                .map_err(|_| "current_authority_unavailable")
        }
    });
    let mut basis = Basis {
        checks: AtomicUsize::new(0),
        eligible: true,
    };
    basis.before_provider_release().unwrap();
    let result = provider.call(
        &b,
        Instant::now() + Duration::from_secs(20),
        &mut basis,
        &mut || {
            gate.recv_timeout(Duration::from_secs(1)).unwrap();
            if late_success {
                p.stop();
            } else {
                let q = control_request();
                run_control::signal_campaign(
                    q.context.mission.engagement_id.0,
                    q.context.mission.campaign_id.0,
                )
                .unwrap();
            }
            resume.send(()).unwrap();
            Ok(())
        },
    );
    assert_eq!(result.err(), Some("provider_cancelled_uncertain"));
    assert!(p.stopped());
    assert_eq!(server.calls.load(Ordering::SeqCst), 0);
}

fn stalled_pre_send_case() {
    let b = bound(false);
    let p = permit();
    run_control::read_parent(p.clone()).unwrap();
    let mut h = configure(&b, "synthetic-fixture-token", Duration::from_secs(20)).unwrap();
    h.url(&std::env::var("DW_AUTONOMOUS_TEST_URL").unwrap())
        .unwrap();
    let root = std::env::var_os("DW_M1_RUN_CONTROL_ROOT").unwrap();
    let mut provider = AutonomousProvider {
        clock: None,
        admission_guard: None,
        reporter: None,
        prepared: Some((h, "synthetic-fixture-token".into(), b.digest.clone())),
        permit: p,
        pre_send: Box::new(move || {
            std::fs::write(
                std::path::Path::new(&root).join("pre-send-entered"),
                b"ready",
            )
            .unwrap();
            loop {
                std::thread::park();
            }
        }),
    };
    let _ = provider.call(
        &b,
        Instant::now() + Duration::from_secs(20),
        &mut Basis {
            checks: AtomicUsize::new(0),
            eligible: true,
        },
        &mut || Ok(()),
    );
    panic!("stalled hook returned without independent enforcement");
}

#[test]
fn blocked_pre_send_cannot_block_independent_eof_or_lease_guard() {
    use std::process::Stdio;
    for eof in [false, true] {
        let root = private_root();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut worker = child("stalled_pre_send", &root)
            .env(
                "DW_AUTONOMOUS_TEST_URL",
                format!("http://{}/responses", listener.local_addr().unwrap()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        while !root.join("pre-send-entered").exists() {
            assert!(
                Instant::now() < end && worker.try_wait().unwrap().is_none(),
                "hook barrier missing"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let started = Instant::now();
        if eof {
            drop(worker.stdin.take());
        }
        let end = started + Duration::from_millis(2300);
        while worker.try_wait().unwrap().is_none() {
            assert!(Instant::now() < end, "blocked marker hook blocked guard");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(worker.wait().unwrap().code(), Some(124));
        assert!(
            listener.accept().is_err(),
            "pre-send wait disclosed a request"
        );
    }
}

#[test]
fn generation_and_repair_stop_after_admission_before_send_refuses_actual_marker() {
    for mode in [
        "pre_send_generation",
        "pre_send_repair",
        "pre_send_late_success",
    ] {
        let root = private_root();
        let status = child(mode, &root).status().unwrap();
        assert!(status.success(), "{mode}");
    }
}

fn successor_bound(repair: bool) -> BoundCall {
    let mut pack: Value = serde_json::from_slice(&bound(false).pack).unwrap();
    pack["version"] = json!("m1_context_pack_autonomous_v2");
    pack["run_profile"] = json!("autonomous-dns-fixture-v2");
    pack["inference_profile"] = json!("deepseek-responses-reasoning-v1");
    request::bind(
        &serde_json::to_vec(&pack).unwrap(),
        repair.then_some(&["proposal_shape"][..]),
    )
    .unwrap()
}

fn successor_clock_case(mode: &str, binding: run_control::Binding) {
    let b = successor_bound(false);
    autonomous_binding(&b).unwrap();
    let end = Instant::now() + Duration::from_millis(124000);
    let permit = Permit::new(end);
    assert!(permit.publish(Instant::now(), end));
    run_control::read_parent(permit.clone()).unwrap();
    let owned = permit.clone();
    let checks = Arc::new(AtomicUsize::new(0));
    let observed = checks.clone();
    let screening = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let screening_started = screening.clone();
    let stop_at_screening = matches!(mode, "successor_admission" | "successor_recording");
    std::thread::spawn(move || {
        while !owned.stopped() {
            if stop_at_screening && screening_started.load(Ordering::Acquire) {
                return;
            }
            let started = Instant::now();
            if binding.current().is_err() || !owned.publish(started, end) {
                owned.stop();
                return;
            }
            observed.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(400));
        }
    });
    let mut handle =
        configure(&b, "synthetic-fixture-token", Duration::from_millis(60000)).unwrap();
    handle
        .url(&std::env::var("DW_AUTONOMOUS_TEST_URL").unwrap())
        .unwrap();
    let (report, blocked_reader) = std::sync::mpsc::sync_channel(1);
    report.try_send(ReportedGuard::CommandWatchdog).unwrap();
    let _blocked_reader = blocked_reader;
    let clock = CallClock::arm(permit.clone(), end, Some(report)).unwrap();
    let mut provider = AutonomousProvider {
        prepared: Some((handle, "synthetic-fixture-token".into(), b.digest.clone())),
        permit,
        pre_send: Box::new(|| Ok(())),
        clock: Some(clock),
        admission_guard: None,
        reporter: None,
    };
    let mut basis = Basis {
        checks: AtomicUsize::new(0),
        eligible: true,
    };
    let mut blocked = BlockedScreening(screening);
    let basis: &mut dyn ProposalBasis = if mode == "successor_admission" {
        &mut blocked
    } else {
        &mut basis
    };
    let entered = Instant::now();
    let result = provider.call(&b, end, basis, &mut || {
        if mode == "successor_blocked" {
            loop {
                std::thread::park();
            }
        }
        Ok(())
    });
    if mode == "successor_recording" {
        assert!(matches!(result.unwrap().admission, Admission::Accepted(_)));
        let _ = blocked.recording_premises();
        panic!("post-call recording returned without independent termination");
    }
    assert_eq!(mode, "successor_response");
    assert!(matches!(result.unwrap().admission, Admission::Accepted(_)));
    assert!(
        checks.load(Ordering::SeqCst) >= 40,
        "real renewal loop did not run"
    );
    assert!(entered.elapsed() > Duration::from_secs(20));
    assert!(entered.elapsed() < Duration::from_secs(60));
    println!(
        "successor response complete after {}ms renewals={}",
        entered.elapsed().as_millis(),
        checks.load(Ordering::SeqCst)
    );
}

struct BlockedScreening(Arc<std::sync::atomic::AtomicBool>);
impl ProposalBasis for BlockedScreening {
    fn reference_eligible(&self, r: &str) -> bool {
        r == "asset_0"
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
    }
    fn safe_explanation(&self, _: &str) -> bool {
        true
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        self.0.store(true, Ordering::Release);
        println!("network_admission_blocked");
        loop {
            std::thread::park();
        }
    }
}

#[test]
fn completed_network_body_keeps_a_lease_guard_while_owner_screening_is_blocked() {
    network_admission_stall("successor_admission");
}

#[test]
fn completed_network_call_keeps_a_lease_guard_for_post_call_recording() {
    network_admission_stall("successor_recording");
}

fn network_admission_stall(mode: &str) {
    use std::process::Stdio;
    let root = private_root();
    let timing = &crate::m1_inference_profile::REASONING_BOUNDED;
    let server = Server::start_with_preparation(
        "200 OK",
        "Content-Type: application/json\r\n",
        envelope(),
        Duration::from_millis(timing.phase_ms - timing.decision_ms),
    );
    let mut worker = child(mode, &root)
        .env("DW_AUTONOMOUS_TEST_URL", &server.url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Err(error) = server.arrived.recv_timeout(Duration::from_secs(5)) {
        let _ = worker.kill();
        let output = worker.wait_with_output().unwrap();
        panic!(
            "network fixture did not receive the bound body: {error}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let started = Instant::now();
    let mut custodian_killed = false;
    while worker.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(4) {
            worker.kill().unwrap();
            custodian_killed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let elapsed = started.elapsed();
    let output = worker.wait_with_output().unwrap();
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("network_admission_blocked"),
        "the actual completed network body did not enter owner screening"
    );
    assert!(
        !custodian_killed,
        "lease loss left network admission unguarded"
    );
    assert_eq!(output.status.code(), Some(124));
    assert!(elapsed <= Duration::from_millis(2300));
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        *server.received_body.lock().unwrap(),
        successor_bound(false).body
    );
    println!(
        "{mode} product exit={}ms physical_requests=1",
        elapsed.as_millis()
    );
}

#[test]
fn successor_real_duration_response_and_blocked_callback_keep_original_soft_hard_bounds() {
    use std::io::{Read, Write};
    use std::process::Stdio;
    for mode in ["successor_response", "successor_blocked"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut worker = child(mode, &private_root())
            .env(
                "DW_AUTONOMOUS_TEST_URL",
                format!("http://{}/responses", listener.local_addr().unwrap()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let launch = Instant::now();
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        launch.elapsed() < Duration::from_secs(5),
                        "successor failed to release"
                    );
                    if let Some(status) = worker.try_wait().unwrap() {
                        let output = worker.wait_with_output().unwrap();
                        panic!(
                            "successor refused before its physical request: {mode} {status}: {} {}",
                            String::from_utf8_lossy(&output.stdout),
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        loop {
            let n = socket.read(&mut chunk).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&chunk[..n]);
            if let Some(header_end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..header_end]);
                let len: usize = header
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                if bytes.len() >= header_end + 4 + len {
                    break;
                }
            }
        }
        let sent = Instant::now();
        if mode == "successor_response" {
            std::thread::sleep(Duration::from_secs(22));
            let body = envelope();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            socket.write_all(&body).unwrap();
        }
        while worker.try_wait().unwrap().is_none() {
            if sent.elapsed() > Duration::from_secs(64) {
                worker.kill().unwrap();
                worker.wait().unwrap();
                panic!("custodian cleanup is not a product termination: {mode}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let elapsed = sent.elapsed();
        let output = worker.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(if mode == "successor_response" { 0 } else { 124 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if mode == "successor_blocked" {
            assert!(
                elapsed >= Duration::from_millis(60000),
                "old outer guard terminated the call"
            );
            assert!(
                elapsed <= Duration::from_millis(62200),
                "original hard bound was reset"
            );
        }
        assert!(listener.accept().is_err(), "one physical request, no retry");
        println!(
            "mode={mode} actual stop/complete={}ms physical_requests=1 {}",
            elapsed.as_millis(),
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

fn child(mode: &str, root: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new(std::env::current_exe().unwrap());
    c.env_clear().args([
        "--exact",
        "m1_reasoning::transport::autonomous_tests::autonomous_control_worker",
        "--nocapture",
    ]);
    for key in ["SystemRoot", "LLVM_PROFILE_FILE"] {
        if let Some(value) = std::env::var_os(key) {
            c.env(key, value);
        }
    }
    c.env("DW_AUTONOMOUS_CONTROL_TEST", mode)
        .env("DW_M1_RUN_CONTROL_ROOT", root);
    c
}

fn private_root() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dw-transport-control-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(windows)]
    {
        let system = std::env::var_os("SystemRoot").unwrap();
        let script = "$ErrorActionPreference='Stop'; $s=[Security.Principal.WindowsIdentity]::GetCurrent().User; \
            $a=New-Object Security.AccessControl.DirectorySecurity; $a.SetOwner($s); $a.SetAccessRuleProtection($true,$false); \
            $r=New-Object Security.AccessControl.FileSystemAccessRule($s,'FullControl','ContainerInherit,ObjectInherit','None','Allow'); \
            $a.AddAccessRule($r); Set-Acl -LiteralPath $env:DW_M1_RUN_CONTROL_ROOT -AclObject $a";
        assert!(
            std::process::Command::new(
                std::path::Path::new(&system)
                    .join("System32/WindowsPowerShell/v1.0/powershell.exe")
            )
            .env_clear()
            .env("SystemRoot", system)
            .env("DW_M1_RUN_CONTROL_ROOT", &root)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
        );
    }
    root
}

#[test]
fn actual_control_eof_marker_and_coordinator_loss_close_owned_idle_io_and_reap() {
    use std::io::{Read, Write};
    use std::process::Stdio;
    for mode in [
        "parent_loss",
        "tls_parent_loss",
        "signal_run",
        "signal_campaign",
        "coordinator_loss",
        "call_limit",
        "repair_remaining",
    ] {
        let root = private_root();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut worker = child(mode, &root)
            .env(
                "DW_AUTONOMOUS_TEST_URL",
                format!(
                    "{}://{}/responses",
                    if mode == "tls_parent_loss" {
                        "https"
                    } else {
                        "http"
                    },
                    listener.local_addr().unwrap()
                ),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let end = Instant::now() + Duration::from_secs(6);
        let mut socket = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < end,
                        "owned transport did not start: {mode}"
                    );
                    assert!(
                        worker.try_wait().unwrap().is_none(),
                        "worker refused before positive control"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("{e}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 16384];
        while !bytes.windows(4).any(|v| v == b"\r\n\r\n") {
            let n = socket.read(&mut chunk).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&chunk[..n]);
            if mode == "tls_parent_loss" {
                break;
            }
        }
        if mode == "tls_parent_loss" {
            assert_eq!(bytes[0], 0x16, "stalled TLS handshake positive control");
        } else {
            assert!(bytes.starts_with(b"POST /responses HTTP/1.1\r\n"));
        }
        let started = Instant::now();
        if mode == "parent_loss" || mode == "tls_parent_loss" {
            drop(worker.stdin.take());
        }
        if mode.starts_with("signal_") {
            assert!(
                child(mode, &root)
                    .env("DW_AUTONOMOUS_SIGNAL_ONLY", "1")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let limit = if mode == "call_limit" {
            Duration::from_millis(20500)
        } else if mode == "repair_remaining" {
            Duration::from_millis(1000)
        } else if mode.starts_with("signal_") {
            Duration::from_secs(4)
        } else {
            Duration::from_millis(2300)
        };
        while worker.try_wait().unwrap().is_none() {
            if started.elapsed() > limit {
                worker.kill().unwrap();
                worker.wait().unwrap();
                panic!("local cancellation failed: {mode}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        worker.wait().unwrap();
        if mode == "call_limit" {
            assert!(
                started.elapsed() >= Duration::from_secs(19),
                "did not exercise the 20s call within a 40s decision"
            );
        }
        // Drain already-sent body, then require socket closure rather than observing just a latch.
        loop {
            match socket.read(&mut chunk) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => break,
                Err(e) => panic!("owned I/O remained open: {mode}: {e}"),
            }
        }
        assert!(
            listener.accept().is_err(),
            "no retry or late second request"
        );
        let _ = socket.write_all(b"late");
        println!(
            "actual control mode={mode} closed/reaped after {}ms",
            started.elapsed().as_millis()
        );
    }
}

#[test]
fn private_root_and_write_once_markers_refuse_traversal_and_cannot_resume() {
    use std::process::Stdio;
    let root = private_root();
    assert!(
        child("reject_root", &root.join(".."))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
    assert!(
        child("signal_run", &root)
            .env("DW_AUTONOMOUS_SIGNAL_ONLY", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
    assert!(
        child("reject_root", &root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
    for marker in std::fs::read_dir(&root).unwrap() {
        assert_eq!(std::fs::read(marker.unwrap().path()).unwrap(), b"STOP\n");
    }
}

#[cfg(windows)]
#[test]
fn operator_owned_but_nonwritable_control_root_is_not_qualified() {
    use std::process::Stdio;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dw-transport-control-ro-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let system = std::env::var_os("SystemRoot").unwrap();
    let script = "$ErrorActionPreference='Stop'; $s=[Security.Principal.WindowsIdentity]::GetCurrent().User; \
        $a=New-Object Security.AccessControl.DirectorySecurity; $a.SetOwner($s); $a.SetAccessRuleProtection($true,$false); \
        $r=New-Object Security.AccessControl.FileSystemAccessRule($s,'ReadAndExecute','ContainerInherit,ObjectInherit','None','Allow'); \
        $a.AddAccessRule($r); Set-Acl -LiteralPath $env:DW_M1_RUN_CONTROL_ROOT -AclObject $a";
    let setup = std::process::Command::new(
        std::path::Path::new(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    )
    .env_clear()
    .env("SystemRoot", system)
    .env("DW_M1_RUN_CONTROL_ROOT", &root)
    .args(["-NoProfile", "-NonInteractive", "-Command", script])
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .output()
    .unwrap();
    assert!(
        setup.status.success(),
        "read-only ACL fixture prerequisite: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    assert!(
        child("reject_root", &root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}

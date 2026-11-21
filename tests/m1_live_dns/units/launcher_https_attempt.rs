use super::*;
use crate::attempt::dir;
use crate::attempt::tests::TestRootGuard;
use crate::custody::tests::{install_fixture_map, simulate_root_directory, simulate_root_file};
use crate::network::tests::fixture::Network;
use crate::qualification::now_ms;
use duskweave::m1_https::VERSION_HTTPS_IPC;
use duskweave::m1_https::ipc::{HttpsIpcRequest, HttpsLimits, HttpsPhase};
use duskweave::m1_inference_profile::{
    HTTPS_HEAD_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER,
    HTTPS_LAB_RUN_PROFILE,
};
use std::cell::Cell;
use std::time::{Duration, Instant};

fn register_journal(attempt: Uuid) {
    simulate_root_file(&dir(attempt).join("session.json"));
}

fn prepare_attempt(attempt: Uuid, ipv4: &str) {
    crate::docker::tests::simulate_absent_attempt(attempt);
    crate::network::create_https(attempt, 42, ipv4, Instant::now() + Duration::from_secs(2))
        .expect("create_https");
}

pub(crate) fn sample_request(attempt: Uuid, now: i64) -> HttpsIpcRequest {
    HttpsIpcRequest {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "REQUEST".into(),
        attempt,
        session_generation: 1,
        sequence: 0,
        engagement_id: Uuid::new_v4(),
        campaign_id: Uuid::new_v4(),
        decision_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        step_index: 2,
        attempt_kind: "head".into(),
        result_id: Uuid::new_v4(),
        profile: HTTPS_HEAD_PROFILE.into(),
        execution_mode: "live".into(),
        run_profile: HTTPS_LAB_RUN_PROFILE.into(),
        release_sha256: "a".repeat(64),
        qualification_ref: Uuid::new_v4(),
        asset_ref: Uuid::new_v4(),
        endpoint_ref: "endpoint_1".into(),
        hostname: HTTPS_LAB_HOSTNAME.into(),
        ipv4: HTTPS_LAB_IPV4.into(),
        port: 443,
        approved_path: "/".into(),
        method: "HEAD".into(),
        parent_claim_id: Uuid::new_v4(),
        parent_claim_revision: 1,
        resolver_ipv4: HTTPS_LAB_RESOLVER.into(),
        vantage_ref: Uuid::new_v4(),
        permission_id: Uuid::new_v4(),
        expected_mission_revision: 1,
        authority_evaluated_at_ms: now,
        claim_valid_until_s: (now / 1000) + 120,
        episode_deadline_at_ms: now + 5_000,
        deadline_at_ms: now + 4_000,
        max_duration_ms: 4_000,
        limits: HttpsLimits::fixed(),
    }
}

#[test]
fn open_validates_inputs_and_stop_ids_and_load_checks() {
    let root = TestRootGuard::new();
    let _net = Network::new();
    let _custody = install_fixture_map();
    simulate_root_directory(root.path());

    let now = now_ms().expect("now");
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, now);
    assert!(matches!(
        open(Uuid::nil(), &req),
        Err(Fail::Config("live_host_unqualified"))
    ));

    let run_id = Uuid::new_v4();
    prepare_attempt(attempt, &req.ipv4);
    let opened = open(run_id, &req).expect("open");
    assert_eq!(opened.max_watchdog_ms(), 13_000);
    assert_eq!(opened.run_id, run_id);

    register_journal(attempt);
    assert_eq!(record(attempt).expect("record").request.attempt, attempt);
    assert!(load(attempt, 2).is_err());
    assert_eq!(
        stop_ids(attempt),
        Some((run_id, req.engagement_id, req.campaign_id, 1))
    );

    register_journal(attempt);
    assert!(crate::attempt::stop(req.engagement_id, req.campaign_id, 1));
    assert!(matches!(
        open(run_id, &req),
        Err(Fail::State("live_effect_stopped"))
    ));
}

#[test]
fn ready_and_start_connect_enforce_one_use_and_rollback_on_release_error() {
    let root = TestRootGuard::new();
    let net = Network::new();
    let _custody = install_fixture_map();
    simulate_root_directory(root.path());

    let now = now_ms().expect("now");
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, now);
    prepare_attempt(attempt, &req.ipv4);
    open(Uuid::new_v4(), &req).expect("open");

    register_journal(attempt);
    assert!(matches!(
        start_connect(attempt, 1, req.deadline_at_ms, || Ok(())),
        Err(Fail::State("live_effect_start_refused"))
    ));

    register_journal(attempt);
    ready_connect(attempt, 1).expect("ready_connect");
    register_journal(attempt);
    assert_eq!(
        ready_connect(attempt, 1).unwrap_err(),
        Fail::State("live_effect_stopped")
    );

    register_journal(attempt);
    let err = start_connect(attempt, 1, req.deadline_at_ms, || {
        Err(Fail::State("injected_release_fail"))
    })
    .unwrap_err();
    assert_eq!(err, Fail::State("injected_release_fail"));
    assert!(
        net.events
            .borrow()
            .iter()
            .any(|e| std::fs::read_to_string(&e.input).is_ok_and(|s| s.contains("flush set")))
    );
}

#[test]
fn advance_phase_and_renew_enforce_monotonic_bounds_and_rollback() {
    let root = TestRootGuard::new();
    let net = Network::new();
    let _custody = install_fixture_map();
    simulate_root_directory(root.path());

    let now = now_ms().expect("now");
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, now);
    prepare_attempt(attempt, &req.ipv4);
    open(Uuid::new_v4(), &req).expect("open");
    register_journal(attempt);
    ready_connect(attempt, 1).expect("ready");

    register_journal(attempt);
    assert_eq!(
        start_connect(attempt, 1, req.deadline_at_ms + 1, || Ok(())).unwrap_err(),
        Fail::State("live_effect_start_refused")
    );

    register_journal(attempt);
    let released = Cell::new(false);
    start_connect(attempt, 1, req.deadline_at_ms, || {
        released.set(true);
        Ok(())
    })
    .expect("start_connect");
    assert!(released.get());

    register_journal(attempt);
    assert_eq!(
        advance_phase(attempt, 1, HttpsPhase::Head, req.deadline_at_ms, || Ok(())).unwrap_err(),
        Fail::State("live_effect_start_refused")
    );

    register_journal(attempt);
    advance_phase(attempt, 1, HttpsPhase::Tls, req.deadline_at_ms, || Ok(())).expect("tls");

    register_journal(attempt);
    assert_eq!(
        renew(attempt, 1, req.deadline_at_ms + 50).unwrap_err(),
        Fail::State("live_effect_stopped")
    );
    register_journal(attempt);
    renew(attempt, 1, req.deadline_at_ms).expect("renew");

    register_journal(attempt);
    let err = advance_phase(attempt, 1, HttpsPhase::Head, req.deadline_at_ms, || {
        Err(Fail::State("head_release_err"))
    })
    .unwrap_err();
    assert_eq!(err, Fail::State("head_release_err"));
    assert!(
        net.events
            .borrow()
            .iter()
            .filter(|e| std::fs::read_to_string(&e.input).is_ok_and(|s| s.contains("flush set")))
            .count()
            >= 1
    );
}

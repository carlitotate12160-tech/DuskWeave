use super::*;
use duskweave::m1_dns::DnsRequest;

struct TestRootGuard(tempfile::TempDir);
impl TestRootGuard {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("create tempdir");
        set_test_root(Some(dir.path().to_path_buf()));
        Self(dir)
    }
}
impl Drop for TestRootGuard {
    fn drop(&mut self) {
        set_test_root(None);
    }
}

fn sample_request(attempt: Uuid, now: i64) -> DnsRequest {
    DnsRequest {
        attempt,
        engagement_id: Uuid::new_v4(),
        campaign_id: Uuid::new_v4(),
        session_generation: 1,
        asset_id: Uuid::new_v4(),
        host_name: "test.duskweave.test".to_string(),
        resolver_ip: "10.203.0.1".to_string(),
        deadline_at_ms: now + 4000,
    }
}

#[test]
fn attempt_paths_stay_scoped_under_the_control_root() {
    let _g = TestRootGuard::new();
    let (a, e, c, g) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), 3);
    for path in [dir(a), stop_path(e, c, g), campaign_path(e, c), run_path(a)] {
        let text = path.to_str().unwrap();
        assert!(!text.contains(".."), "{text}");
    }
    assert!(
        stop_path(e, c, g)
            .to_str()
            .unwrap()
            .contains(&g.to_string())
    );
    assert!(stop_path(e, c, g) != stop_path(e, c, g + 1));
}

#[test]
fn test_session_lifecycle_open_ready_charge_start_renew() {
    let _g = TestRootGuard::new();
    let now = super::super::qualification::now_ms().unwrap();
    let run_id = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, now);

    // 1. Open session
    let session = open(run_id, &req).expect("open session");
    assert_eq!(session.run_id, run_id);
    assert_eq!(session.request.attempt, attempt);
    assert_eq!(session.opened_at_ms, now);
    assert!(session.ready_at_ms.is_none());

    // 2. Load session
    let loaded = load(attempt, 1).expect("load session");
    assert_eq!(loaded.request.attempt, attempt);

    // 3. Load with wrong generation fails
    assert_eq!(
        load(attempt, 2).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 4. Transition to ready
    ready(attempt, 1).expect("ready session");
    let ready_session = load(attempt, 1).expect("load ready session");
    assert!(ready_session.ready_at_ms.is_some());

    // 5. Ready twice fails
    assert_eq!(
        ready(attempt, 1).unwrap_err(),
        Fail::State("live_effect_stopped")
    );

    // 6. Start session (charge + grant)
    let deadline = now + 1500;
    let released = Cell::new(false);
    start(attempt, 1, deadline, || {
        released.set(true);
        Ok(())
    })
    .expect("start session");
    assert!(released.get(), "release hook must be called");

    // 7. Start twice fails with already charged
    assert_eq!(
        start(attempt, 1, deadline, || Ok(())).unwrap_err(),
        Fail::State("live_effect_already_charged")
    );

    // 8. Renew session
    renew(attempt, 1, deadline).expect("renew session within grant");

    // 9. Renew with deadline > original fails
    assert_eq!(
        renew(attempt, 1, deadline + 500).unwrap_err(),
        Fail::State("live_effect_stopped")
    );
}

#[test]
fn test_record_validation_and_corruption() {
    let _g = TestRootGuard::new();
    let now = super::super::qualification::now_ms().unwrap();
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, now);
    let session = open(Uuid::new_v4(), &req).expect("open session");

    // Attempt mismatch fails
    let other = Uuid::new_v4();
    std::fs::create_dir_all(dir(other)).unwrap();
    let bytes = serde_json::to_vec(&session).unwrap();
    std::fs::write(dir(other).join("session.json"), bytes).unwrap();
    assert_eq!(
        record(other).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // Corrupted file fails
    std::fs::write(dir(other).join("session.json"), b"invalid-json").unwrap();
    assert_eq!(
        record(other).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
}

#[test]
fn test_stop_generation_campaign_and_run_scopes() {
    let _g = TestRootGuard::new();
    let now = super::super::qualification::now_ms().unwrap();
    let attempt = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let req = sample_request(attempt, now);
    let session = open(run_id, &req).expect("open session");

    assert!(!stopped(&req));
    assert!(!blocked(&session));

    // Generation stop
    assert!(stop(
        req.engagement_id,
        req.campaign_id,
        req.session_generation
    ));
    assert!(stopped(&req));
    assert!(blocked(&session));

    // Campaign stop
    assert!(stop_campaign(req.engagement_id, req.campaign_id).0);

    // Run stop
    assert!(stop_run(run_id).0);

    // Scope matches
    let scope_gen =
        StopScope::Generation(req.engagement_id, req.campaign_id, req.session_generation);
    assert!(scope_gen.matches(&session));

    let scope_camp = StopScope::Campaign(req.engagement_id, req.campaign_id);
    assert!(scope_camp.matches(&session));

    let scope_run = StopScope::Run(run_id);
    assert!(scope_run.matches(&session));

    // Scope rejection on invalid inputs
    let invalid_gen = StopScope::Generation(Uuid::nil(), req.campaign_id, 1);
    assert_eq!(
        invalid_gen.path().unwrap_err(),
        Fail::Input("invalid_input")
    );
    let zero_gen = StopScope::Generation(req.engagement_id, req.campaign_id, 0);
    assert_eq!(zero_gen.path().unwrap_err(), Fail::Input("invalid_input"));
}

#[test]
fn test_guards_flock_and_mutual_exclusion() {
    let _g = TestRootGuard::new();
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let end = Instant::now() + Duration::from_secs(2);

    let guard1 = guard(e, c, end).expect("acquire first guard");

    // Second lock on same campaign file with immediate deadline must fail with WOULDBLOCK
    let immediate = Instant::now();
    let guard2_err = guard(e, c, immediate).unwrap_err();
    assert_eq!(guard2_err, Fail::State("live_host_guard_unavailable"));

    drop(guard1);

    // Now second lock can succeed
    let guard3 = guard(e, c, end).expect("acquire guard after release");
    drop(guard3);
}

#[test]
fn test_transport_markers() {
    let _g = TestRootGuard::new();
    let attempt = Uuid::new_v4();
    std::fs::create_dir_all(dir(attempt)).unwrap();

    assert!(transport_cleared(attempt), "initially clear");
    transport_started(attempt).expect("set pending marker");
    assert!(dir(attempt).join("transport.pending").exists());

    assert!(transport_reaped(attempt), "reaped clears marker");
    assert!(!dir(attempt).join("transport.pending").exists());
}

#[test]
fn test_session_lifecycle_open_ready_charge_start_renew() {
    let _g = TestRootGuard::new();
    let now = super::super::qualification::now_ms().unwrap();
    let run_id = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, now);

    // 1. Open session
    let session = open(run_id, &req).expect("open session");
    assert_eq!(session.run_id, run_id);
    assert_eq!(session.request.attempt, attempt);
    assert_eq!(session.opened_at_ms, now);
    assert!(session.ready_at_ms.is_none());

    // 2. Load session
    let loaded = load(attempt, 1).expect("load session");
    assert_eq!(loaded.request.attempt, attempt);

    // 3. Load with wrong generation fails
    assert_eq!(
        load(attempt, 2).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // 4. Transition to ready
    ready(attempt, 1).expect("ready session");
    let ready_session = load(attempt, 1).expect("load ready session");
    assert!(ready_session.ready_at_ms.is_some());

    // 5. Ready twice fails
    assert_eq!(
        ready(attempt, 1).unwrap_err(),
        Fail::State("live_effect_stopped")
    );

    // 6. Start session (charge + grant)
    let deadline = now + 1500;
    let released = Cell::new(false);
    start(attempt, 1, deadline, || {
        released.set(true);
        Ok(())
    })
    .expect("start session");
    assert!(released.get(), "release hook must be called");

    // 7. Start twice fails with already charged
    assert_eq!(
        start(attempt, 1, deadline, || Ok(())).unwrap_err(),
        Fail::State("live_effect_already_charged")
    );

    // 8. Renew session
    renew(attempt, 1, deadline).expect("renew session within grant");

    // 9. Renew with deadline > original fails
    assert_eq!(
        renew(attempt, 1, deadline + 500).unwrap_err(),
        Fail::State("live_effect_stopped")
    );
}

#[test]
fn test_record_validation_and_corruption() {
    let _g = TestRootGuard::new();
    let now = super::super::qualification::now_ms().unwrap();
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, now);
    let session = open(Uuid::new_v4(), &req).expect("open session");

    // Attempt mismatch fails
    let other = Uuid::new_v4();
    std::fs::create_dir_all(dir(other)).unwrap();
    let bytes = serde_json::to_vec(&session).unwrap();
    std::fs::write(dir(other).join("session.json"), bytes).unwrap();
    assert_eq!(
        record(other).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );

    // Corrupted file fails
    std::fs::write(dir(other).join("session.json"), b"invalid-json").unwrap();
    assert_eq!(
        record(other).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
}

#[test]
fn test_stop_generation_campaign_and_run_scopes() {
    let _g = TestRootGuard::new();
    let now = super::super::qualification::now_ms().unwrap();
    let attempt = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let req = sample_request(attempt, now);
    let session = open(run_id, &req).expect("open session");

    assert!(!stopped(&req));
    assert!(!blocked(&session));

    // Generation stop
    assert!(stop(
        req.engagement_id,
        req.campaign_id,
        req.session_generation
    ));
    assert!(stopped(&req));
    assert!(blocked(&session));

    // Campaign stop
    assert!(stop_campaign(req.engagement_id, req.campaign_id).0);

    // Run stop
    assert!(stop_run(run_id).0);

    // Scope matches
    let scope_gen =
        StopScope::Generation(req.engagement_id, req.campaign_id, req.session_generation);
    assert!(scope_gen.matches(&session));

    let scope_camp = StopScope::Campaign(req.engagement_id, req.campaign_id);
    assert!(scope_camp.matches(&session));

    let scope_run = StopScope::Run(run_id);
    assert!(scope_run.matches(&session));

    // Scope rejection on invalid inputs
    let invalid_gen = StopScope::Generation(Uuid::nil(), req.campaign_id, 1);
    assert_eq!(
        invalid_gen.path().unwrap_err(),
        Fail::Input("invalid_input")
    );
    let zero_gen = StopScope::Generation(req.engagement_id, req.campaign_id, 0);
    assert_eq!(zero_gen.path().unwrap_err(), Fail::Input("invalid_input"));
}

#[test]
fn test_guards_flock_and_mutual_exclusion() {
    let _g = TestRootGuard::new();
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let end = Instant::now() + Duration::from_secs(2);

    let guard1 = guard(e, c, end).expect("acquire first guard");

    // Second lock on same campaign file with immediate deadline must fail with WOULDBLOCK
    let immediate = Instant::now();
    let guard2_err = guard(e, c, immediate).unwrap_err();
    assert_eq!(guard2_err, Fail::State("live_host_guard_unavailable"));

    drop(guard1);

    // Now second lock can succeed
    let guard3 = guard(e, c, end).expect("acquire guard after release");
    drop(guard3);
}

#[test]
fn test_transport_markers() {
    let _g = TestRootGuard::new();
    let attempt = Uuid::new_v4();
    std::fs::create_dir_all(dir(attempt)).unwrap();

    assert!(transport_cleared(attempt), "initially clear");
    transport_started(attempt).expect("set pending marker");
    assert!(dir(attempt).join("transport.pending").exists());

    assert!(transport_reaped(attempt), "reaped clears marker");
    assert!(!dir(attempt).join("transport.pending").exists());
}

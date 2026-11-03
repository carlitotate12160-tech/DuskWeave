use super::*;
use duskweave::m1_dns::{DnsRequest, PROFILE, SCHEMA};
use std::cell::Cell;
use std::path::Path;

struct TestRootGuard(PathBuf);
impl TestRootGuard {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let id = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
        let path = std::env::temp_dir().join(format!("dw-attempt-{}", id.trim()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        set_test_root(Some(path.clone()));
        Self(path)
    }
}
impl Drop for TestRootGuard {
    fn drop(&mut self) {
        set_test_root(None);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_qualified_journal(path: &Path, bytes: &[u8]) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn sample_request(attempt: Uuid, now: i64) -> DnsRequest {
    DnsRequest {
        schema: SCHEMA.to_string(),
        kind: "REQUEST".to_string(),
        attempt,
        session_generation: 1,
        sequence: 0,
        engagement_id: Uuid::new_v4(),
        campaign_id: Uuid::new_v4(),
        decision_id: Uuid::new_v4(),
        profile: PROFILE.to_string(),
        execution_mode: "live".to_string(),
        run_profile: "autonomous-dns-genai-lab-v1".to_string(),
        release_sha256: "a".repeat(64),
        qualification_ref: Uuid::new_v4(),
        name: "seed.duskweave.test".to_string(),
        resolver_ipv4: "192.0.2.53".to_string(),
        transport: "udp".to_string(),
        query_id: 1,
        allowed_names: vec!["seed.duskweave.test".to_string()],
        deadline_at_ms: now + 4000,
        max_duration_ms: 2000,
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
    let before = super::super::qualification::now_ms().unwrap();
    let run_id = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let req = sample_request(attempt, before);

    // 1. Open session; opened_at is bounded by samples taken around open().
    let session = open(run_id, &req).expect("open session");
    let after = super::super::qualification::now_ms().unwrap();
    assert_eq!(session.run_id, run_id);
    assert_eq!(session.request.attempt, attempt);
    assert!(session.opened_at_ms >= before && session.opened_at_ms <= after);
    assert!(session.ready_at_ms.is_none());

    // 2. Load session
    let loaded = load(attempt, 1).expect("load session");
    assert_eq!(loaded.request.attempt, attempt);

    // 3. Load with wrong generation fails
    assert!(matches!(load(attempt, 2),
        Err(e) if e == Fail::Config("live_host_unqualified")));

    // 4. Transition to ready
    ready(attempt, 1).expect("ready session");
    let ready_session = load(attempt, 1).expect("load ready session");
    assert!(ready_session.ready_at_ms.is_some());

    // 5. Ready twice fails
    assert_eq!(
        ready(attempt, 1).unwrap_err(),
        Fail::State("live_effect_stopped")
    );

    // 6. Start validates owned state and charges the one-use journal, then
    //    fails closed at the unqualified network slot: no real egress path
    //    can run and the release hook must not fire. Real grant/release
    //    enforcement is owned by the physical suite.
    let now = super::super::qualification::now_ms().unwrap();
    let deadline = now + 1500;
    let released = Cell::new(false);
    assert!(matches!(start(attempt, 1, deadline, || {
        released.set(true);
        Ok(())
    }), Err(e) if e == Fail::Config("live_host_unqualified")));
    assert!(!released.get(), "release must not run without a grant");
    let charged = load(attempt, 1).expect("reload charged session");
    assert_eq!(charged.attempt_deadline_at_ms, Some(deadline));

    // 7. The one-use marker refuses a second charge
    let mut again = load(attempt, 1).expect("reload for second charge");
    assert_eq!(
        charge(&mut again, deadline).unwrap_err(),
        Fail::State("live_effect_already_charged")
    );

    // 8. A charged session refuses further start validation
    let now = super::super::qualification::now_ms().unwrap();
    assert_eq!(
        validate_start(&charged, now, now + 1500).unwrap_err(),
        Fail::State("live_effect_start_refused")
    );

    // 9. Renew refuses a later deadline before any egress grant runs
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

    // Attempt mismatch fails; the journal file is qualified first, so the
    // identity check itself is exercised.
    let other = Uuid::new_v4();
    std::fs::create_dir_all(dir(other)).unwrap();
    let journal = dir(other).join("session.json");
    let bytes = serde_json::to_vec(&session).unwrap();
    write_qualified_journal(&journal, &bytes);
    assert!(matches!(record(other),
        Err(e) if e == Fail::Config("live_host_unqualified")));

    // Corrupted file fails decoding after the same qualification
    write_qualified_journal(&journal, b"invalid-json");
    assert!(matches!(record(other),
        Err(e) if e == Fail::Config("live_host_unqualified")));
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

    // Latching is marker-file-only in unit coverage: it never sweeps or
    // disposes host resources; real stop enforcement is owned by the
    // physical suite.
    let end = Instant::now() + Duration::from_secs(2);
    scope_gen.latch(end).expect("latch generation stop");
    assert!(stopped(&req));
    assert!(blocked(&session));
    scope_camp.latch(end).expect("latch campaign stop");
    scope_run.latch(end).expect("latch run stop");
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
    assert!(matches!(guard(e, c, immediate),
        Err(err) if err == Fail::State("live_host_guard_unavailable")));

    drop(guard1);

    // Now second lock can succeed
    let guard3 = guard(e, c, end).expect("acquire guard after release");
    drop(guard3);
}

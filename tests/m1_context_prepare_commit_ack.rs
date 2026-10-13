use std::net::SocketAddr;
use std::time::Duration;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/m1_context_fault.rs"]
mod context_fault;
#[path = "support/m1_context_prepare.rs"]
mod context_support;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;

fn admin_db() -> postgres::Client {
    wait_db::admin_db_client(
        db_support::dsn("DW_TEST_ADMIN_DATABASE_URL"),
        &db_support::dsn("DW_TEST_DATABASE_URL"),
    )
}

fn db_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], session_support::db_port()))
}

#[test]
fn query_timeout_on_locked_outbox_yields_bounded_refusal_without_mutation() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9701);
    let req_val = context_support::request(&serde_json::to_value(&req).unwrap(), &[]);
    let broker_dsn = session_support::broker_dsn(session_support::db_port());

    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);

    // Hold AccessExclusiveLock on registration_outbox table
    let _lock = wait_db::AccessExclusiveLock::hold(admin_db(), "mission.registration_outbox");

    let input = wait_db::InputFile::new(&req_val, "outbox_timeout");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let (out, elapsed) = wait_db::run_bounded(
        &mut wait_db::cli(&args, &broker_dsn),
        wait_db::COMMAND_ENVELOPE,
    );
    assert!(!out.status.success(), "timeout must fail non-zero: {out:?}");

    // Lock was held, so CLI timed out within bounded envelope
    assert!(
        elapsed <= wait_db::COMMAND_ENVELOPE + Duration::from_secs(5),
        "execution remained bounded: {elapsed:?}"
    );

    drop(_lock);

    let after_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(
        before_state, after_state,
        "query timeout must not mutate scoped state"
    );
}

#[test]
fn connection_failure_after_allocation_emits_unavailable_receipt() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9702);
    let req_val = context_support::request(&serde_json::to_value(&req).unwrap(), &[]);

    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);

    // TwoConnectionRelay allows connection 1 (PgAllocator) and refuses connection 2 (PgContextReader)
    let relay = context_fault::TwoConnectionRelay::start(db_addr());
    let relay_dsn = session_support::broker_dsn(relay.port());

    let input = wait_db::InputFile::new(&req_val, "conn2_fail");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let (out, _) = wait_db::run_bounded(
        &mut wait_db::cli(&args, &relay_dsn),
        wait_db::COMMAND_ENVELOPE,
    );
    assert!(
        !out.status.success(),
        "second connection failure must exit non-zero"
    );

    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "unavailable");
    assert!(
        r["decision_id"].as_str().is_some_and(|s| !s.is_empty()),
        "non-nil allocated decision_id must be present in receipt"
    );
    assert!(r["pack_utf8"].is_null());
    context_support::assert_false_flags(&r);

    let after_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(
        before_state, after_state,
        "connection failure must not mutate scoped state"
    );
}

#[test]
fn physical_readonly_commit_ack_loss_withholds_caller_reply_and_fails_closed() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (req, _source) = budget::fixture(0x9703);
    let req_val = context_support::request(&serde_json::to_value(&req).unwrap(), &[]);

    let before_state = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);

    let relay = proxy::Relay::start(db_addr(), b"COMMIT");
    let relay_dsn = session_support::broker_dsn(relay.port());

    let input = wait_db::InputFile::new(&req_val, "commit_ack_loss");
    let args = vec![
        "m1-context-prepare".into(),
        "--input".into(),
        input.path().into(),
    ];

    let (mut child, _) = wait_db::spawn_piped(&mut wait_db::cli(&args, &relay_dsn));

    assert!(
        relay.observed(Duration::from_secs(30)),
        "COMMIT wire frame must be observed"
    );
    assert!(
        relay.consumed(Duration::from_secs(30)),
        "COMMIT CommandComplete must be consumed"
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "child must still be blocked while acknowledgment is withheld"
    );

    // State is confirmed unchanged before dropping relay
    let state_during = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(before_state, state_during);

    drop(relay);

    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success(), "dropped connection must fail child");

    let r = context_support::receipt(&out);
    assert_eq!(r["outcome"], "unavailable");
    assert!(r["pack_utf8"].is_null());
    context_support::assert_false_flags(&r);

    let state_after = context_support::scoped_state(req.engagement_id.0, req.campaign_id.0);
    assert_eq!(
        before_state, state_after,
        "read-only commit ack loss leaves ledger and history untouched"
    );

    // A fresh direct CLI may prepare a new decision with unchanged ledger/history
    let direct_dsn = session_support::broker_dsn(session_support::db_port());
    let fresh_out = context_support::run(&args, &direct_dsn);
    assert!(
        fresh_out.status.success(),
        "fresh direct CLI must succeed: {fresh_out:?}"
    );
    let fresh_r = context_support::receipt(&fresh_out);
    assert_eq!(fresh_r["outcome"], "prepared");
    assert_eq!(
        context_support::scoped_state(req.engagement_id.0, req.campaign_id.0),
        before_state
    );
}

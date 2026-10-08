//! Real query/lock/read-response bounds through the restricted-role CLI on
//! the owned disposable database: a held ACCESS EXCLUSIVE lock bounds the
//! real inspect read to a bounded refusal, an injected pg_sleep insert
//! trigger is canceled at the fixed server statement bound with zero
//! durable effects, and a backend that stops answering after a real
//! business query is bounded by the active invocation watchdog at 124.
//! Faults are test-owned fixtures confirmed before launch and removed with
//! the guard; cluster roles and other databases are never altered.

use duskweave::mission::{CampaignId, EngagementId};
use duskweave::registration;
use std::net::SocketAddr;
use std::time::Duration;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::{accepted, count, db, dsn, ports, reg_json, scope};

#[path = "support/database_wait_db.rs"]
mod wait_db;

#[path = "support/database_wait_proxy.rs"]
mod proxy;

/// Loopback address of the owned test cluster, resolved from the
/// authorized runtime DSN rather than assumed.
fn admin_db() -> postgres::Client {
    wait_db::admin_db_client(
        dsn("DW_TEST_ADMIN_DATABASE_URL"),
        &dsn("DW_TEST_DATABASE_URL"),
    )
}

fn db_addr() -> SocketAddr {
    let cfg = dsn("DW_TEST_DATABASE_URL");
    let host = match cfg.get_hosts() {
        [postgres::config::Host::Tcp(host)] => host.clone(),
        _ => panic!("test runtime DSN must be one TCP host"),
    };
    let port = *cfg.get_ports().first().unwrap_or(&5432);
    SocketAddr::new(host.parse().expect("literal loopback IP"), port)
}

fn inspect_args(e: &EngagementId, c: &CampaignId) -> Vec<String> {
    vec![
        "inspect".into(),
        "--engagement".into(),
        e.to_string(),
        "--campaign".into(),
        c.to_string(),
    ]
}

#[test]
fn held_access_exclusive_lock_bounds_inspect_to_refusal() {
    let _guard = db();
    let (e, c) = scope(101);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let _lock = wait_db::AccessExclusiveLock::hold(admin_db(), "mission.missions");
    let dsn_text = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let (output, elapsed) = wait_db::run_bounded(
        &mut wait_db::cli(&inspect_args(&e, &c), &dsn_text),
        Duration::from_secs(15),
    );
    assert!(
        !output.status.success(),
        "a blocked read must refuse, not report an empty result"
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("error="), "categorical refusal: {text}");
    assert!(
        !text.contains("inspect result="),
        "no fabricated read: {text}"
    );
    assert!(output.stderr.is_empty());
    assert!(
        elapsed <= wait_db::STARTUP_ENVELOPE,
        "lock refusal stayed bounded: {elapsed:?}"
    );
}

#[test]
fn sleeping_insert_trigger_is_canceled_at_statement_bound() {
    let _guard = db();
    let (e, c) = scope(102);
    let fault = wait_db::SleepFault::install(admin_db(), &e.to_string(), 10);
    let (mut alloc, _store, _traj) = ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    let file = wait_db::InputFile::new(&reg_json(e, c), &op.to_string());
    let dsn_text = std::env::var("DW_TEST_DATABASE_URL").unwrap();
    let args = vec![
        "register".into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        file.path().to_string(),
    ];
    let (output, elapsed) =
        wait_db::run_bounded(&mut wait_db::cli(&args, &dsn_text), Duration::from_secs(15));
    drop(fault);
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("error="), "categorical refusal: {text}");
    assert!(
        !text.contains("result=accepted"),
        "no fabricated receipt: {text}"
    );
    assert!(output.stderr.is_empty());
    assert!(
        elapsed <= Duration::from_secs(15),
        "statement cancellation stayed bounded: {elapsed:?}"
    );
    assert_eq!(count("mission.missions", e, c), 0);
    assert_eq!(count("mission.registration_outbox", e, c), 0);
    assert_eq!(count("trajectory.registration_history", e, c), 0);
}

#[test]
fn withheld_business_reply_is_bounded_by_command_watchdog() {
    let _guard = db();
    let (e, c) = scope(103);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);
    let relay = proxy::Relay::start(db_addr(), b"mission.missions");
    let dsn_kw = wait_db::relay_dsn(&dsn("DW_TEST_DATABASE_URL"), relay.port());
    let mut cmd = wait_db::cli(&inspect_args(&e, &c), &dsn_kw);
    let (child, started) = wait_db::spawn_piped(&mut cmd);
    assert!(
        relay.observed(Duration::from_secs(10)),
        "relay never observed the real inspect query; fault never engaged"
    );
    let (output, _) = wait_db::wait_bounded(
        child,
        wait_db::COMMAND_ENVELOPE + Duration::from_secs(5),
        started,
    );
    wait_db::expect_watchdog_stop(
        &output,
        started.elapsed(),
        wait_db::COMMAND_FLOOR,
        wait_db::COMMAND_ENVELOPE,
    );
}

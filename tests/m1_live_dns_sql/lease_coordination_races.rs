//! M1 heartbeat lease coordination race assertions: monotonic concurrency,
//! STOP after precheck, and real expiry after precheck under observed row locks,
//! with bounded cleanup via scoped thread ownership.
use super::fixture::*;
use super::lease_coordination_support::*;
use super::run_support::*;
use duskweave::postgres_m1_run::PgRunStore;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InjectedFault {
    WhileBWaits,
    BeforeStart,
}

fn check_injected_fault<T>(
    result: &std::thread::Result<T>,
    expected: InjectedFault,
) -> Result<(), &'static str> {
    match result {
        Ok(_) => Err("expected_panic_but_succeeded"),
        Err(payload) => match payload.downcast_ref::<InjectedFault>() {
            Some(f) if *f == expected => Ok(()),
            Some(_) => Err("wrong_injected_fault_variant"),
            None => Err("unexpected_unrelated_panic_payload"),
        },
    }
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_lease_renewal_preserves_stop_and_expiry() {
    assert!(worker());

    // -------------------------------------------------------------------------
    // Predicate Discrimination Controls
    // -------------------------------------------------------------------------
    let (ctrl_ok, ctrl_wrong, ctrl_unrel, ctrl_match) = (
        Ok(()),
        std::panic::catch_unwind(|| std::panic::panic_any(InjectedFault::BeforeStart)),
        std::panic::catch_unwind(|| panic!("unrelated_prereq_failure")),
        std::panic::catch_unwind(|| std::panic::panic_any(InjectedFault::WhileBWaits)),
    );
    assert_eq!(
        check_injected_fault(&ctrl_ok, InjectedFault::WhileBWaits),
        Err("expected_panic_but_succeeded")
    );
    assert_eq!(
        check_injected_fault(&ctrl_wrong, InjectedFault::WhileBWaits),
        Err("wrong_injected_fault_variant")
    );
    assert_eq!(
        check_injected_fault(&ctrl_unrel, InjectedFault::WhileBWaits),
        Err("unexpected_unrelated_panic_payload")
    );
    assert_eq!(
        check_injected_fault(&ctrl_match, InjectedFault::WhileBWaits),
        Ok(())
    );

    // -------------------------------------------------------------------------
    // Scenario 1: Monotonic Concurrency under Observed Row Waiting
    // -------------------------------------------------------------------------
    {
        let (e, c, _) = register_live(0x7c11, false);
        let run = Uuid::new_v4();
        let publ = live_publication(e, c, run);
        let mut runs = PgRunStore::new(broker());
        runs.start(
            &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
            RELEASE,
        )
        .unwrap();

        let initial_control = admin()
            .query_one(
                "SELECT lease_until_ms, status FROM execution.autonomous_control WHERE run_id=$1",
                &[&run],
            )
            .unwrap();
        let init_lease: i64 = initial_control.get(0);
        let rec = runs.inspect(run).unwrap();

        let b_res = thread::scope(|s| {
            let mut tx_a = broker();
            let a_pid: i32 = tx_a
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);

            let (tx_ready, rx_ready) = mpsc::channel();
            let (tx_start, rx_start) = mpsc::channel();
            let b_worker = s.spawn(move || {
                let mut b_conn = broker();
                let b_pid: i32 = b_conn
                    .query_one("SELECT pg_backend_pid()", &[])
                    .unwrap()
                    .get(0);
                tx_ready.send(b_pid).unwrap();
                if rx_start.recv().is_err() {
                    return Err(None);
                }
                b_conn
                    .execute("SELECT execution.run_renew($1)", &[&run])
                    .map_err(Some)
            });

            let b_pid = rx_ready
                .recv_timeout(Duration::from_secs(5))
                .expect("thread B must announce backend pid");

            // Transaction A acquires row lock via actual run_renew.
            tx_a.batch_execute("BEGIN").unwrap();
            tx_a.execute("SELECT execution.run_renew($1)", &[&run])
                .unwrap();

            // Signal thread B to invoke run_renew (will block on row lock held by A).
            tx_start.send(()).unwrap();

            // From a separate observer, poll pg_blocking_pids(B) until it contains A.
            let mut obs = admin();
            wait_for_blocking_pid(&mut obs, b_pid, a_pid, Duration::from_secs(5))
                .expect("tx B must demonstrably block on row lock held by tx A");

            // Commit A only after observation is verified.
            tx_a.batch_execute("COMMIT").unwrap();

            b_worker.join().unwrap()
        });

        assert!(
            b_res.is_ok(),
            "tx B completes successfully after tx A commits: {b_res:?}"
        );

        let final_control = admin()
            .query_one(
                "SELECT lease_until_ms, status FROM execution.autonomous_control WHERE run_id=$1",
                &[&run],
            )
            .unwrap();
        let final_lease: i64 = final_control.get(0);
        let final_status: String = final_control.get(1);

        assert_eq!(final_status, "running");
        assert!(
            final_lease >= init_lease,
            "final lease must not regress: {final_lease} >= {init_lease}"
        );
        assert!(
            final_lease <= rec.deadline_at_ms,
            "final lease must remain within deadline horizon"
        );
    }

    // -------------------------------------------------------------------------
    // Scenario 2: STOP after B's Precheck
    // -------------------------------------------------------------------------
    {
        let (e, c, _) = register_live(0x7c12, false);
        let run = Uuid::new_v4();
        let publ = live_publication(e, c, run);
        let mut runs = PgRunStore::new(broker());
        runs.start(
            &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
            RELEASE,
        )
        .unwrap();

        let b_res = thread::scope(|s| {
            let mut tx_a = broker();
            let a_pid: i32 = tx_a
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);

            let (tx_ready, rx_ready) = mpsc::channel();
            let (tx_start, rx_start) = mpsc::channel();
            let b_worker = s.spawn(move || {
                let mut b_conn = broker();
                let b_pid: i32 = b_conn
                    .query_one("SELECT pg_backend_pid()", &[])
                    .unwrap()
                    .get(0);
                tx_ready.send(b_pid).unwrap();
                if rx_start.recv().is_err() {
                    return Err(None);
                }
                b_conn
                    .execute("SELECT execution.run_renew($1)", &[&run])
                    .map_err(Some)
            });

            let b_pid = rx_ready
                .recv_timeout(Duration::from_secs(5))
                .expect("thread B must announce backend pid");

            tx_a.batch_execute("BEGIN").unwrap();
            tx_a.execute("SELECT execution.run_renew($1)", &[&run])
                .unwrap();

            tx_start.send(()).unwrap();

            let mut obs = admin();
            wait_for_blocking_pid(&mut obs, b_pid, a_pid, Duration::from_secs(5))
                .expect("tx B must demonstrably block on row lock held by tx A");

            // While B waits on A, invoke actual execution.run_stop through run owner's A connection.
            tx_a.execute("SELECT execution.run_stop($1)", &[&run])
                .unwrap();
            tx_a.batch_execute("COMMIT").unwrap();

            b_worker.join().unwrap()
        });

        let err = b_res
            .expect_err("B must fail after run is stopped")
            .expect("must be database error");
        assert!(
            db_message(&err).contains("run_not_current"),
            "B must refuse with run_not_current: {err:?}"
        );

        let final_control = admin()
            .query_one(
                "SELECT status, lease_until_ms FROM execution.autonomous_control WHERE run_id=$1",
                &[&run],
            )
            .unwrap();
        let final_status: String = final_control.get(0);
        let final_lease: i64 = final_control.get(1);

        assert_eq!(final_status, "stopped", "status must remain stopped");
        assert_eq!(final_lease, 0, "lease must remain 0 on stopped run");
    }

    // -------------------------------------------------------------------------
    // Scenario 3: Expiry after B's Precheck
    // -------------------------------------------------------------------------
    {
        let (e, c, _) = register_live(0x7c13, false);
        let run = Uuid::new_v4();
        let publ = live_publication(e, c, run);
        let mut runs = PgRunStore::new(broker());
        runs.start(
            &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
            RELEASE,
        )
        .unwrap();

        let (b_res, captured_lease) = thread::scope(|s| {
            let mut tx_a = broker();
            let a_pid: i32 = tx_a
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);

            let (tx_ready, rx_ready) = mpsc::channel();
            let (tx_start, rx_start) = mpsc::channel();
            let b_worker = s.spawn(move || {
                let mut b_conn = broker();
                let b_pid: i32 = b_conn
                    .query_one("SELECT pg_backend_pid()", &[])
                    .unwrap()
                    .get(0);
                tx_ready.send(b_pid).unwrap();
                if rx_start.recv().is_err() {
                    return Err(None);
                }
                b_conn
                    .execute("SELECT execution.run_renew($1)", &[&run])
                    .map_err(Some)
            });

            let b_pid = rx_ready
                .recv_timeout(Duration::from_secs(5))
                .expect("thread B must announce backend pid");

            tx_a.batch_execute("BEGIN").unwrap();
            tx_a.execute("SELECT execution.run_renew($1)", &[&run])
                .unwrap();

            let captured_lease: i64 = tx_a
                .query_one(
                    "SELECT lease_until_ms FROM execution.autonomous_control WHERE run_id=$1",
                    &[&run],
                )
                .unwrap()
                .get(0);

            tx_start.send(()).unwrap();

            let mut obs = admin();
            wait_for_blocking_pid(&mut obs, b_pid, a_pid, Duration::from_secs(5))
                .expect("tx B must demonstrably block on row lock held by tx A");

            // Hold A's renewal transaction until captured lease has actually expired on DB clock.
            let start_wait = Instant::now();
            let mut expired = false;
            while start_wait.elapsed() < Duration::from_secs(6) {
                let is_past: bool = tx_a
                    .query_one(
                        "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint > $1",
                        &[&captured_lease],
                    )
                    .unwrap()
                    .get(0);
                if is_past {
                    expired = true;
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
            assert!(expired, "database clock must exceed captured lease");

            // Commit without fabricating an expired row.
            tx_a.batch_execute("COMMIT").unwrap();

            (b_worker.join().unwrap(), captured_lease)
        });

        let err = b_res
            .expect_err("B must refuse renewal on expired lease")
            .expect("must be database error");
        assert!(
            db_message(&err).contains("run_not_current"),
            "B must refuse expired lease with run_not_current: {err:?}"
        );

        let final_control = admin()
            .query_one(
                "SELECT status, lease_until_ms FROM execution.autonomous_control WHERE run_id=$1",
                &[&run],
            )
            .unwrap();
        let final_status: String = final_control.get(0);
        let final_lease: i64 = final_control.get(1);

        assert_eq!(final_status, "running", "status remains running");
        assert_eq!(
            final_lease, captured_lease,
            "expired lease remains unchanged at captured lease value"
        );
    }

    // -------------------------------------------------------------------------
    // Subcase 4: Unwind while B Waits on A Releases Row Lock Orderly
    // -------------------------------------------------------------------------
    {
        let (e, c, _) = register_live(0x7c14, false);
        let run = Uuid::new_v4();
        let publ = live_publication(e, c, run);
        let mut runs = PgRunStore::new(broker());
        runs.start(
            &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
            RELEASE,
        )
        .unwrap();

        let subcase4_start = Instant::now();
        let unwind_observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            thread::scope(|s| {
                let mut tx_a = broker();
                let a_pid: i32 = tx_a
                    .query_one("SELECT pg_backend_pid()", &[])
                    .unwrap()
                    .get(0);

                let (tx_ready, rx_ready) = mpsc::channel();
                let (tx_start, rx_start) = mpsc::channel();
                let _b_worker = s.spawn(move || {
                    let mut b_conn = broker();
                    let b_pid: i32 = b_conn
                        .query_one("SELECT pg_backend_pid()", &[])
                        .unwrap()
                        .get(0);
                    tx_ready.send(b_pid).unwrap();
                    if rx_start.recv().is_err() {
                        return;
                    }
                    // B blocks waiting on A's row lock; when A unwinds and drops,
                    // A's transaction rolls back, releasing the lock, allowing B to complete.
                    let _ = b_conn.execute("SELECT execution.run_renew($1)", &[&run]);
                });

                let b_pid = rx_ready
                    .recv_timeout(Duration::from_secs(5))
                    .expect("thread B must announce backend pid");

                tx_a.batch_execute("BEGIN").unwrap();
                tx_a.execute("SELECT execution.run_renew($1)", &[&run])
                    .unwrap();

                tx_start.send(()).unwrap();

                let mut obs = admin();
                wait_for_blocking_pid(&mut obs, b_pid, a_pid, Duration::from_secs(5))
                    .expect("tx B must demonstrably block on row lock held by tx A");

                // Inject simulated failure while B is waiting, before A commits.
                std::panic::panic_any(InjectedFault::WhileBWaits);
            });
        }));

        assert_eq!(
            check_injected_fault(&unwind_observed, InjectedFault::WhileBWaits),
            Ok(())
        );
        let sc4_elapsed = subcase4_start.elapsed();
        eprintln!("subcase 4 elapsed: {sc4_elapsed:?}");
        assert!(
            sc4_elapsed < Duration::from_secs(10),
            "subcase 4 observation bound"
        );

        // Verify that B has joined and A's row lock is fully released:
        // observer can acquire an exclusive row lock immediately without blocking.
        let check_lock = admin().query_opt(
            "SELECT lease_until_ms FROM execution.autonomous_control WHERE run_id=$1 FOR UPDATE NOWAIT",
            &[&run],
        );
        assert!(
            matches!(check_lock, Ok(Some(_))),
            "observer must immediately acquire row lock without contention after unwind"
        );
    }

    // -------------------------------------------------------------------------
    // Subcase 5: Pre-start Unwind Closes Channel and B Exits Promptly
    // -------------------------------------------------------------------------
    {
        let (e, c, _) = register_live(0x7c15, false);
        let run = Uuid::new_v4();
        let publ = live_publication(e, c, run);
        let mut runs = PgRunStore::new(broker());
        runs.start(
            &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
            RELEASE,
        )
        .unwrap();

        let subcase5_start = Instant::now();
        let pre_start_unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            thread::scope(|s| {
                let (tx_ready, rx_ready) = mpsc::channel();
                let (tx_start, rx_start) = mpsc::channel();
                let _b_worker = s.spawn(move || {
                    let mut b_conn = broker();
                    let b_pid: i32 = b_conn
                        .query_one("SELECT pg_backend_pid()", &[])
                        .unwrap()
                        .get(0);
                    tx_ready.send(b_pid).unwrap();
                    // If tx_start is dropped before sending, recv() unblocks immediately with Err.
                    let start = rx_start.recv();
                    if start.is_err() {
                        return; // exit cleanly without invoking SQL
                    }
                    let _ = b_conn.execute("SELECT execution.run_renew($1)", &[&run]);
                });

                let _b_pid = rx_ready
                    .recv_timeout(Duration::from_secs(5))
                    .expect("thread B must announce backend pid");

                // Inject failure before sending the start signal.
                if true {
                    std::panic::panic_any(InjectedFault::BeforeStart);
                }
                tx_start.send(()).unwrap();
            });
        }));

        assert_eq!(
            check_injected_fault(&pre_start_unwind, InjectedFault::BeforeStart),
            Ok(())
        );
        let sc5_elapsed = subcase5_start.elapsed();
        eprintln!("subcase 5 elapsed: {sc5_elapsed:?}");
        assert!(
            sc5_elapsed < Duration::from_secs(10),
            "subcase 5 observation bound"
        );

        // Verify observer can read the retained row without contention.
        let status: String = admin()
            .query_one(
                "SELECT status FROM execution.autonomous_control WHERE run_id=$1",
                &[&run],
            )
            .unwrap()
            .get(0);
        assert_eq!(status, "running");
    }
}

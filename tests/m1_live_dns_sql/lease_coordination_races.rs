//! M1 heartbeat lease coordination race assertions: monotonic concurrency,
//! STOP after precheck, and real expiry after precheck under observed row locks.
use super::fixture::*;
use super::lease_coordination_support::*;
use super::run_support::*;
use duskweave::postgres_m1_run::PgRunStore;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct ThreadJoiner<T>(Option<thread::JoinHandle<T>>);
impl<T> ThreadJoiner<T> {
    fn new(h: thread::JoinHandle<T>) -> Self {
        Self(Some(h))
    }
    fn join(mut self) -> std::thread::Result<T> {
        self.0.take().unwrap().join()
    }
}
impl<T> Drop for ThreadJoiner<T> {
    fn drop(&mut self) {
        if let Some(h) = self.0.take() {
            let _ = h.join();
        }
    }
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_lease_renewal_preserves_stop_and_expiry() {
    assert!(worker());

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

        // Prepare both restricted connections before starting short-lived authority.
        let mut tx_a = broker();
        let a_pid: i32 = tx_a
            .query_one("SELECT pg_backend_pid()", &[])
            .unwrap()
            .get(0);

        let (tx_ready, rx_ready) = mpsc::channel();
        let (tx_start, rx_start) = mpsc::channel();
        let b_worker = thread::spawn(move || {
            let mut b_conn = broker();
            let b_pid: i32 = b_conn
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);
            tx_ready.send(b_pid).unwrap();
            rx_start.recv().unwrap();
            b_conn.execute("SELECT execution.run_renew($1)", &[&run])
        });
        let joiner = ThreadJoiner::new(b_worker);
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

        let b_res = joiner.join().expect("join thread B");
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

        // Prepare both connections.
        let mut tx_a = broker();
        let a_pid: i32 = tx_a
            .query_one("SELECT pg_backend_pid()", &[])
            .unwrap()
            .get(0);

        let (tx_ready, rx_ready) = mpsc::channel();
        let (tx_start, rx_start) = mpsc::channel();
        let b_worker = thread::spawn(move || {
            let mut b_conn = broker();
            let b_pid: i32 = b_conn
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);
            tx_ready.send(b_pid).unwrap();
            rx_start.recv().unwrap();
            b_conn.execute("SELECT execution.run_renew($1)", &[&run])
        });
        let joiner = ThreadJoiner::new(b_worker);
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

        let b_res = joiner.join().expect("join thread B");
        let err = b_res.expect_err("B must fail after run is stopped");
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

        let mut tx_a = broker();
        let a_pid: i32 = tx_a
            .query_one("SELECT pg_backend_pid()", &[])
            .unwrap()
            .get(0);

        let (tx_ready, rx_ready) = mpsc::channel();
        let (tx_start, rx_start) = mpsc::channel();
        let b_worker = thread::spawn(move || {
            let mut b_conn = broker();
            let b_pid: i32 = b_conn
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);
            tx_ready.send(b_pid).unwrap();
            rx_start.recv().unwrap();
            b_conn.execute("SELECT execution.run_renew($1)", &[&run])
        });
        let joiner = ThreadJoiner::new(b_worker);
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

        let b_res = joiner.join().expect("join thread B");
        let err = b_res.expect_err("B must refuse renewal on expired lease");
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
}

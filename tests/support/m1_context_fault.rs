//! Bounded test-only barrier and connection fault fixtures for ContextPack tests.
#![allow(dead_code)]

use postgres::Client;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const BARRIER_CLASS_ID: u32 = 202;
const BARRIER_LOCK_ID: u32 = 888_777;

static BARRIER_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionSnapshot {
    pub def: String,
    pub owner: String,
    pub lang: String,
    pub volatile: String,
    pub secdef: bool,
    pub config: Option<Vec<String>>,
    pub acl: Option<String>,
}

pub fn capture_metadata(admin: &mut Client) -> Result<FunctionSnapshot, String> {
    let row = admin
        .query_one(
            "SELECT pg_get_functiondef(p.oid), p.proowner::regrole::text, l.lanname, \
                    p.provolatile::text, p.prosecdef, p.proconfig, p.proacl::text \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             JOIN pg_language l ON l.oid = p.prolang \
             WHERE p.oid = 'execution.inference_capacity_snapshot(uuid,uuid)'::regprocedure::oid",
            &[],
        )
        .map_err(|e| format!("failed to resolve inference_capacity_snapshot metadata: {e}"))?;

    let def: String = row.get(0);
    let owner: String = row.get(1);
    let lang: String = row.get(2);
    let volatile: String = row.get(3);
    let secdef: bool = row.get(4);
    let config: Option<Vec<String>> = row.get(5);
    let acl: Option<String> = row.get(6);

    if lang != "sql" {
        return Err(format!("unexpected language: expected 'sql', got '{lang}'"));
    }
    if volatile != "s" {
        return Err(format!(
            "unexpected volatility: expected 's', got '{volatile}'"
        ));
    }
    if secdef {
        return Err("unexpected security definer: expected invoker (prosecdef=false)".into());
    }

    Ok(FunctionSnapshot {
        def,
        owner,
        lang,
        volatile,
        secdef,
        config,
        acl,
    })
}

pub struct CapacityBarrierFault {
    _lock: std::sync::MutexGuard<'static, ()>,
    admin: Client,
    baseline: FunctionSnapshot,
    released: bool,
    finished: bool,
}

impl CapacityBarrierFault {
    pub fn install(mut admin: Client) -> Self {
        let lock = BARRIER_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let baseline = capture_metadata(&mut admin).expect("capture normal function baseline");

        // Verify fixture alias does not already exist
        let alias_exists: bool = admin
            .query_one(
                "SELECT to_regprocedure('execution.dw_context_capacity_fixture(uuid,uuid)') IS NOT NULL",
                &[],
            )
            .expect("check alias existence")
            .get(0);
        assert!(!alias_exists, "fixture alias must not already exist");
        let mut fault = Self {
            _lock: lock,
            admin,
            baseline,
            released: true,
            finished: false,
        };

        // Acquire advisory lock
        fault
            .admin
            .query_one(
                "SELECT pg_advisory_lock($1::int, $2::int)",
                &[&(BARRIER_CLASS_ID as i32), &(BARRIER_LOCK_ID as i32)],
            )
            .expect("acquire advisory barrier lock");
        fault.released = false;

        // Create alias with identical body and declaration identity
        let alias_def = fault.baseline.def.replacen(
            "execution.inference_capacity_snapshot",
            "execution.dw_context_capacity_fixture",
            1,
        );
        let mut tx = fault.admin.transaction().expect("install transaction");
        tx.batch_execute(&format!(
            "{alias_def};\n\
             REVOKE ALL ON FUNCTION execution.dw_context_capacity_fixture(uuid,uuid) FROM PUBLIC;\n\
             GRANT EXECUTE ON FUNCTION execution.dw_context_capacity_fixture(uuid,uuid) TO dw_m1_broker;"
        )).expect("install captured fixture alias");

        // Install hooked function delegating to captured alias
        let hooked = format!(
            r#"
            CREATE OR REPLACE FUNCTION execution.inference_capacity_snapshot(e uuid, c uuid)
            RETURNS jsonb LANGUAGE plpgsql STABLE SET search_path=pg_catalog AS $$
            DECLARE
                res jsonb;
                old_lock text;
            BEGIN
                old_lock := current_setting('lock_timeout', true);
                PERFORM set_config('lock_timeout', '30s', true);
                PERFORM pg_advisory_lock({class_id}, {lock_id});
                PERFORM pg_advisory_unlock({class_id}, {lock_id});
                PERFORM set_config('lock_timeout', coalesce(old_lock, '750ms'), true);
                SELECT execution.dw_context_capacity_fixture(e, c) INTO res;
                RETURN res;
            END;
            $$;
            "#,
            class_id = BARRIER_CLASS_ID,
            lock_id = BARRIER_LOCK_ID
        );
        tx.batch_execute(&hooked).expect("install barrier hook");
        tx.commit().expect("commit barrier installation");
        fault
    }

    /// Poll pg_locks until a backend is waiting on BARRIER_LOCK_ID
    pub fn wait_blocked(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            let waiting: i64 = self
                .admin
                .query_one(
                    "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' \
                     AND classid = $1 AND objid = $2 AND NOT granted",
                    &[&BARRIER_CLASS_ID, &BARRIER_LOCK_ID],
                )
                .unwrap()
                .get(0);
            if waiting > 0 {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    pub fn release(&mut self) -> Result<(), String> {
        if !self.released {
            let unlocked: bool = self
                .admin
                .query_one(
                    "SELECT pg_advisory_unlock($1::int, $2::int)",
                    &[&(BARRIER_CLASS_ID as i32), &(BARRIER_LOCK_ID as i32)],
                )
                .map_err(|_| "barrier_release_failed")?
                .get(0);
            if !unlocked {
                return Err("barrier_lock_not_owned".into());
            }
            self.released = true;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), String> {
        self.release()?;

        let mut tx = self
            .admin
            .transaction()
            .map_err(|_| "barrier_restore_transaction")?;
        tx.batch_execute(&format!(
            "{};\nDROP FUNCTION IF EXISTS execution.dw_context_capacity_fixture(uuid,uuid);",
            self.baseline.def
        ))
        .map_err(|_| "barrier_restore_definition")?;
        tx.commit().map_err(|_| "barrier_restore_commit")?;

        let alias_exists: bool = self
            .admin
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = 'execution' AND p.proname = 'dw_context_capacity_fixture')",
                &[],
            )
            .map_err(|e| format!("failed to check alias absence: {e}"))?
            .get(0);
        if alias_exists {
            return Err("alias execution.dw_context_capacity_fixture was not removed".into());
        }

        let current = capture_metadata(&mut self.admin)?;
        if current != self.baseline {
            return Err(format!(
                "restoration mismatch: expected {:?}, got {:?}",
                self.baseline, current
            ));
        }

        self.finished = true;
        Ok(())
    }
}

impl Drop for CapacityBarrierFault {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.release();
            let _ = self.admin.batch_execute(&format!(
                "{};\nDROP FUNCTION IF EXISTS execution.dw_context_capacity_fixture(uuid,uuid);",
                self.baseline.def
            ));
            self.finished = true;
        }
    }
}

pub fn join_bounded<T>(worker: JoinHandle<T>, bound: Duration) -> T {
    let start = Instant::now();
    while !worker.is_finished() {
        assert!(
            start.elapsed() < bound,
            "snapshot worker completion exceeded {bound:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    worker
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// A relay that forwards the first connection (e.g. for operation allocation)
/// and closes/refuses the second connection (triggering a reader connect error).
pub struct TwoConnectionRelay {
    port: u16,
    stop: Arc<AtomicBool>,
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl TwoConnectionRelay {
    pub fn start(target: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let threads: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));

        let stop_clone = stop.clone();
        let threads_clone = threads.clone();
        let accept_thread = std::thread::spawn(move || {
            let mut conn_count = 0;
            listener.set_nonblocking(true).unwrap();
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((client, _)) => {
                        conn_count += 1;
                        if conn_count == 1 {
                            let stop = stop_clone.clone();
                            let h = std::thread::spawn(move || {
                                forward_conn(client, target, stop);
                            });
                            threads_clone.lock().unwrap().push(h);
                        } else {
                            let _ = client.shutdown(Shutdown::Both);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        });
        threads.lock().unwrap().push(accept_thread);

        Self {
            port,
            stop,
            threads,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for TwoConnectionRelay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        for h in self.threads.lock().unwrap().drain(..) {
            let _ = h.join();
        }
    }
}

fn forward_conn(client: TcpStream, target: SocketAddr, stop: Arc<AtomicBool>) {
    let server = match TcpStream::connect(target) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = client.set_nonblocking(true);
    let _ = server.set_nonblocking(true);

    let (mut c_in, mut c_out) = (client.try_clone().unwrap(), client);
    let (mut s_in, mut s_out) = (server.try_clone().unwrap(), server);

    let stop_t1 = stop.clone();
    let t1 = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while !stop_t1.load(Ordering::Relaxed) {
            match c_in.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if s_out.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let _ = s_out.shutdown(Shutdown::Both);
    });

    let stop_t2 = stop.clone();
    let t2 = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while !stop_t2.load(Ordering::Relaxed) {
            match s_in.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if c_out.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let _ = c_out.shutdown(Shutdown::Both);
    });

    let _ = t1.join();
    let _ = t2.join();
}

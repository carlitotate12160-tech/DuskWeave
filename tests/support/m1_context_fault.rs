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

const ORIGINAL_0011_BODY: &str = r#"
CREATE OR REPLACE FUNCTION execution.inference_capacity_snapshot(e uuid,c uuid)
RETURNS jsonb LANGUAGE sql STABLE SET search_path=pg_catalog AS $$
    SELECT jsonb_build_object('version','m1_inference_capacity_snapshot_v1',
        'epoch',f.epoch,'session_claimed',f.operation_id IS NOT NULL,
        'active_holds',count(h.operation_id),'charged',jsonb_build_object(
            'calls',coalesce(sum((h.capacity->>'calls')::bigint),0),
            'request_bytes',coalesce(sum((h.capacity->>'request_bytes')::bigint),0),
            'response_bytes',coalesce(sum((h.capacity->>'response_bytes')::bigint),0),
            'output_token_capacity',coalesce(sum((h.capacity->>'output_token_capacity')::bigint),0),
            'wall_time_ms',coalesce(sum((h.capacity->>'wall_time_ms')::bigint),0)))
    FROM execution.session_fences f LEFT JOIN execution.inference_reservations h
    ON (h.engagement_id,h.campaign_id)=(f.engagement_id,f.campaign_id)
    AND h.record_revision=1 AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x
        WHERE (x.engagement_id,x.campaign_id,x.operation_id)=
              (h.engagement_id,h.campaign_id,h.operation_id) AND x.record_revision=2)
    WHERE f.engagement_id=e AND f.campaign_id=c GROUP BY f.epoch,f.operation_id
$$;
REVOKE ALL ON FUNCTION execution.inference_capacity_snapshot(uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.inference_capacity_snapshot(uuid,uuid) TO dw_m1_broker;
"#;

static BARRIER_MUTEX: Mutex<()> = Mutex::new(());

pub struct CapacityBarrierFault {
    _lock: std::sync::MutexGuard<'static, ()>,
    admin: Client,
    released: bool,
}

impl CapacityBarrierFault {
    pub fn install(mut admin: Client) -> Self {
        let lock = BARRIER_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        // Acquire advisory lock
        admin
            .query_one(
                "SELECT pg_advisory_lock($1::int, $2::int)",
                &[&(BARRIER_CLASS_ID as i32), &(BARRIER_LOCK_ID as i32)],
            )
            .unwrap();

        // Install hooked function body that attempts to acquire the advisory lock
        let hooked = format!(
            r#"
            CREATE OR REPLACE FUNCTION execution.inference_capacity_snapshot(e uuid,c uuid)
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
                SELECT jsonb_build_object('version','m1_inference_capacity_snapshot_v1',
                    'epoch',f.epoch,'session_claimed',f.operation_id IS NOT NULL,
                    'active_holds',count(h.operation_id),'charged',jsonb_build_object(
                        'calls',coalesce(sum((h.capacity->>'calls')::bigint),0),
                        'request_bytes',coalesce(sum((h.capacity->>'request_bytes')::bigint),0),
                        'response_bytes',coalesce(sum((h.capacity->>'response_bytes')::bigint),0),
                        'output_token_capacity',coalesce(sum((h.capacity->>'output_token_capacity')::bigint),0),
                        'wall_time_ms',coalesce(sum((h.capacity->>'wall_time_ms')::bigint),0)))
                INTO res
                FROM execution.session_fences f LEFT JOIN execution.inference_reservations h
                ON (h.engagement_id,h.campaign_id)=(f.engagement_id,f.campaign_id)
                AND h.record_revision=1 AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x
                    WHERE (x.engagement_id,x.campaign_id,x.operation_id)=
                          (h.engagement_id,h.campaign_id,h.operation_id) AND x.record_revision=2)
                WHERE f.engagement_id=e AND f.campaign_id=c GROUP BY f.epoch,f.operation_id;
                RETURN res;
            END;
            $$;
            REVOKE ALL ON FUNCTION execution.inference_capacity_snapshot(uuid,uuid) FROM PUBLIC;
            GRANT EXECUTE ON FUNCTION execution.inference_capacity_snapshot(uuid,uuid) TO dw_m1_broker;
            "#,
            class_id = BARRIER_CLASS_ID,
            lock_id = BARRIER_LOCK_ID
        );

        admin.batch_execute(&hooked).unwrap();
        Self {
            _lock: lock,
            admin,
            released: false,
        }
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

    pub fn release(&mut self) {
        if !self.released {
            let _ = self.admin.query_one(
                "SELECT pg_advisory_unlock($1::int, $2::int)",
                &[&(BARRIER_CLASS_ID as i32), &(BARRIER_LOCK_ID as i32)],
            );
            self.released = true;
        }
    }
}

impl Drop for CapacityBarrierFault {
    fn drop(&mut self) {
        self.release();
        let _ = self.admin.batch_execute(ORIGINAL_0011_BODY);
    }
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
                            // Forward connection 1
                            let stop = stop_clone.clone();
                            let h = std::thread::spawn(move || {
                                forward_conn(client, target, stop);
                            });
                            threads_clone.lock().unwrap().push(h);
                        } else {
                            // Drop connection 2 immediately
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

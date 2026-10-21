//! Transport-local revocation and monotonic lease. Reading never calls an owner or waits.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Permit(Arc<State>);
struct State {
    origin: Instant,
    deadline: Instant,
    expires_ms: AtomicU64,
    stopped: AtomicBool,
}
impl Permit {
    pub fn new(deadline: Instant) -> Self {
        Self(Arc::new(State {
            origin: Instant::now(),
            deadline,
            expires_ms: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        }))
    }
    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::Release);
    }
    pub fn stopped(&self) -> bool {
        self.0.stopped.load(Ordering::Acquire)
    }
    pub fn expiry(&self) -> Instant {
        self.0.origin + Duration::from_millis(self.0.expires_ms.load(Ordering::Acquire))
    }
    pub fn current(&self) -> bool {
        !self.stopped() && Instant::now() < self.expiry().min(self.0.deadline)
    }
    /// A slow check cannot manufacture a fresh lease at completion.
    pub fn publish(&self, started: Instant, valid_until: Instant) -> bool {
        let end = (started + Duration::from_millis(2000))
            .min(valid_until)
            .min(self.0.deadline);
        let expired =
            self.0.expires_ms.load(Ordering::Acquire) != 0 && Instant::now() >= self.expiry();
        if self.stopped() || expired || Instant::now() >= end {
            self.stop();
            return false;
        }
        let ms = end.saturating_duration_since(self.0.origin).as_millis() as u64;
        self.0.expires_ms.store(ms, Ordering::Release);
        self.current()
    }
}

/// Independent fallback for a blocked resolver/callback/coordinator. Never joins an I/O worker.
pub struct TransportGuard(Sender<()>);
impl TransportGuard {
    pub fn arm(permit: Permit, call_end: Instant) -> Result<Self, &'static str> {
        let (done, rx) = channel();
        std::thread::Builder::new()
            .name("inference_permit_guard".into())
            .spawn(move || {
                let mut cancelled_at = None;
                loop {
                    match rx.recv_timeout(Duration::from_millis(5)) {
                        Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    let now = Instant::now();
                    if permit.stopped() {
                        cancelled_at.get_or_insert(now);
                    }
                    let hard_end = cancelled_at
                        .map_or(call_end, |at| {
                            call_end.min(at + Duration::from_millis(2000))
                        })
                        .min(permit.expiry());
                    // Keep scheduling/reap margin inside the original bound, never after it.
                    if hard_end.saturating_duration_since(now) <= Duration::from_millis(50) {
                        permit.stop();
                        // No grace extends an expired permit or a call's original deadline.
                        std::process::exit(124);
                    }
                }
            })
            .map_err(|_| "inference_control_unavailable")?;
        Ok(Self(done))
    }
}
impl Drop for TransportGuard {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_reads_use_only_the_atomic_permit() {
        let end = Instant::now() + Duration::from_secs(10);
        let p = Permit::new(end);
        assert!(p.publish(Instant::now(), end));
        for _ in 0..1000 {
            assert!(p.current());
        }
        p.stop();
        assert!(!p.current());
    }

    #[test]
    fn check_start_not_completion_anchors_the_lease_and_revocation_is_monotonic() {
        let p = Permit::new(Instant::now() + Duration::from_secs(10));
        assert!(!p.current(), "a cached/unissued permit cannot release work");
        let start = Instant::now() - Duration::from_millis(1500);
        assert!(p.publish(start, Instant::now() + Duration::from_secs(10)));
        assert!(p.expiry() <= start + Duration::from_secs(2));
        p.stop();
        assert!(!p.publish(Instant::now(), Instant::now() + Duration::from_secs(10)));
        assert!(!p.current());
    }

    #[test]
    fn stale_success_and_original_deadline_never_renew() {
        let now = Instant::now();
        let p = Permit::new(now + Duration::from_secs(10));
        assert!(!p.publish(now - Duration::from_secs(3), now + Duration::from_secs(10)));
        let p = Permit::new(now + Duration::from_millis(20));
        assert!(p.publish(now, now + Duration::from_secs(10)));
        assert!(p.expiry() <= now + Duration::from_millis(20));
    }
}

//! Transport-local revocation and monotonic lease. Reading never calls an owner or waits.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, SyncSender, channel};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportedGuard {
    TransportHardDeadline,
    DecisionEnvelope,
    PermitExpiry,
    CommandWatchdog,
}
impl ReportedGuard {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TransportHardDeadline => "transport_hard_deadline",
            Self::DecisionEnvelope => "decision_envelope",
            Self::PermitExpiry => "permit_expiry",
            Self::CommandWatchdog => "command_watchdog",
        }
    }
}

pub fn report_guard(sink: Option<&SyncSender<ReportedGuard>>, reason: ReportedGuard) {
    if let Some(sink) = sink {
        let _ = sink.try_send(reason);
    }
}

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
        Self::arm_selected(permit, None, call_end, None)
    }
    pub fn arm_successor(
        permit: Permit,
        soft: Instant,
        hard: Instant,
        sink: Option<SyncSender<ReportedGuard>>,
    ) -> Result<Self, &'static str> {
        Self::arm_selected(permit, Some(soft), hard, sink)
    }
    fn arm_selected(
        permit: Permit,
        soft: Option<Instant>,
        call_end: Instant,
        sink: Option<SyncSender<ReportedGuard>>,
    ) -> Result<Self, &'static str> {
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
                    if soft.is_some_and(|end| now >= end) {
                        permit.stop();
                    }
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
                        terminate_guard(&permit, call_end, sink.as_ref());
                    }
                }
            })
            .map_err(|_| "inference_control_unavailable")?;
        Ok(Self(done))
    }
}
fn terminate_guard(
    permit: &Permit,
    call_end: Instant,
    sink: Option<&SyncSender<ReportedGuard>>,
) -> ! {
    let reason = if permit.expiry() <= call_end {
        ReportedGuard::PermitExpiry
    } else {
        ReportedGuard::TransportHardDeadline
    };
    report_guard(sink, reason);
    std::process::exit(124);
}

/// Anchored once at prepare/handoff, before marker/final release and I/O.
pub(super) struct CallClock {
    pub origin: Instant,
    pub soft: Instant,
    pub hard: Instant,
    _guard: TransportGuard,
}
impl CallClock {
    pub fn remaining(&self, permit: &Permit, shared: Instant) -> Result<Duration, &'static str> {
        if self.hard > shared || !permit.current() {
            return Err("current_authority_unavailable");
        }
        self.soft
            .checked_duration_since(Instant::now())
            .ok_or("inference_deadline")
    }
    pub fn arm(
        permit: Permit,
        shared: Instant,
        sink: Option<SyncSender<ReportedGuard>>,
    ) -> Result<Self, &'static str> {
        let origin = Instant::now();
        let timing = &crate::m1_inference_profile::REASONING_BOUNDED;
        let hard = origin + Duration::from_millis(timing.hard_ms);
        if hard > shared || !permit.current() {
            return Err("inference_deadline");
        }
        let soft = origin + Duration::from_millis(timing.soft_ms);
        let guard = TransportGuard::arm_successor(permit, soft, hard, sink)?;
        Ok(Self {
            origin,
            soft,
            hard,
            _guard: guard,
        })
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
    fn guard_frame_values_and_durable_json_use_the_same_closed_vocabulary() {
        for (guard, name) in [
            (
                ReportedGuard::TransportHardDeadline,
                "transport_hard_deadline",
            ),
            (ReportedGuard::DecisionEnvelope, "decision_envelope"),
            (ReportedGuard::PermitExpiry, "permit_expiry"),
            (ReportedGuard::CommandWatchdog, "command_watchdog"),
        ] {
            assert_eq!(guard.as_str(), name);
            assert_eq!(serde_json::to_value(guard).unwrap(), name);
            assert_eq!(
                serde_json::from_value::<ReportedGuard>(serde_json::json!(name)).unwrap(),
                guard
            );
        }
    }

    #[test]
    fn successor_handoff_clock_never_resets_and_refuses_insufficient_shared_remainder() {
        let shared = Instant::now() + Duration::from_millis(124000);
        let permit = Permit::new(shared);
        assert!(permit.publish(Instant::now(), shared));
        assert!(
            CallClock::arm(
                permit.clone(),
                Instant::now() + Duration::from_millis(61999),
                None
            )
            .is_err()
        );
        let clock = CallClock::arm(permit.clone(), shared, None).unwrap();
        assert_eq!(clock.soft - clock.origin, Duration::from_millis(60000));
        assert_eq!(clock.hard - clock.origin, Duration::from_millis(62000));
        let first = clock.remaining(&permit, shared).unwrap();
        assert!(clock.remaining(&permit, shared).unwrap() <= first);
        assert!(
            clock
                .remaining(&permit, clock.hard - Duration::from_millis(1))
                .is_err()
        );
        permit.stop();
        assert!(clock.remaining(&permit, shared).is_err());
    }

    #[test]
    fn full_or_disconnected_reporter_never_blocks_guard_progress() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        report_guard(Some(&tx), ReportedGuard::PermitExpiry);
        let start = Instant::now();
        for _ in 0..1000 {
            report_guard(Some(&tx), ReportedGuard::TransportHardDeadline);
        }
        assert!(start.elapsed() < Duration::from_millis(500));
        assert_eq!(rx.recv().unwrap(), ReportedGuard::PermitExpiry);
        drop(rx);
        report_guard(Some(&tx), ReportedGuard::DecisionEnvelope);
        report_guard(None, ReportedGuard::CommandWatchdog);
    }

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

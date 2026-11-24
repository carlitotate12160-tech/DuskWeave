use super::*;
use crate::m1_effect_session::EffectPhase;
use crate::m1_effect_session::attempt::{HttpsAttemptSession, HttpsAttemptStore};
use crate::m1_https::ipc::{HttpsLimits, HttpsOutcome, HttpsPhase, HttpsSafeResult};
use crate::m1_https::result::LAB_SCOPE_LIMITATION;
use crate::m1_https::{RedirectDisposition, VERSION_HTTPS_IPC};
use crate::m1_inference_profile::{
    HTTPS_HEAD_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER,
    HTTPS_LAB_RUN_PROFILE,
};
use std::collections::VecDeque;

struct TimedStore {
    session: HttpsAttemptSession,
    may_start_delay: Duration,
    recorded: Option<HttpsSafeResult>,
    closed_disposed: bool,
}

impl TimedStore {
    fn new(base_wall_ms: i64, max_duration_ms: u32, may_start_delay: Duration) -> Self {
        let dl = base_wall_ms + i64::from(max_duration_ms);
        let id = Uuid::from_u128(0x9001);
        let req = HttpsIpcRequest {
            schema: VERSION_HTTPS_IPC.into(),
            kind: "REQUEST".into(),
            attempt: id,
            session_generation: 1,
            sequence: 0,
            engagement_id: id,
            campaign_id: id,
            decision_id: id,
            run_id: id,
            step_index: 2,
            attempt_kind: "head".into(),
            result_id: id,
            profile: HTTPS_HEAD_PROFILE.into(),
            execution_mode: "live".into(),
            run_profile: HTTPS_LAB_RUN_PROFILE.into(),
            release_sha256: "a".repeat(64),
            qualification_ref: id,
            asset_ref: id,
            endpoint_ref: "endpoint_0".into(),
            hostname: HTTPS_LAB_HOSTNAME.into(),
            ipv4: HTTPS_LAB_IPV4.into(),
            port: 443,
            approved_path: "/".into(),
            method: "HEAD".into(),
            parent_claim_id: id,
            parent_claim_revision: 1,
            resolver_ipv4: HTTPS_LAB_RESOLVER.into(),
            vantage_ref: id,
            permission_id: id,
            expected_mission_revision: 1,
            authority_evaluated_at_ms: base_wall_ms - 10,
            claim_valid_until_s: (base_wall_ms / 1000) + 60,
            episode_deadline_at_ms: dl + 5000,
            deadline_at_ms: dl,
            max_duration_ms,
            limits: HttpsLimits::fixed(),
        };
        let session = HttpsAttemptSession {
            run_id: req.run_id,
            step_index: req.step_index,
            attempt_id: req.attempt,
            result_ref: req.result_id,
            attempt_deadline_at_ms: dl,
            request: req,
            phase: EffectPhase::Reserved,
            possibly_started: false,
            granted_phase: None,
            result: None,
            disposal_confirmed: false,
        };
        Self {
            session,
            may_start_delay,
            recorded: None,
            closed_disposed: false,
        }
    }
}

impl HttpsAttemptStore for TimedStore {
    fn reserve_head(&mut self, _: Uuid, _: u32) -> Res<HttpsAttemptSession> {
        Ok(self.session.clone())
    }
    fn inspect_head(&mut self, _: Uuid) -> Res<HttpsAttemptSession> {
        Ok(self.session.clone())
    }
    fn current_deadline(&mut self, _: &HttpsAttemptSession) -> Res<i64> {
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn may_start(&mut self, _: &HttpsAttemptSession) -> Res<i64> {
        if !self.may_start_delay.is_zero() {
            std::thread::sleep(self.may_start_delay);
        }
        self.session.phase = EffectPhase::MayStart;
        self.session.possibly_started = true;
        self.session.granted_phase = Some(HttpsPhase::Connect);
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn advance_phase(&mut self, _: &HttpsAttemptSession, phase: HttpsPhase) -> Res<i64> {
        self.session.granted_phase = Some(phase);
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn record_result(
        &mut self,
        _: &HttpsAttemptSession,
        result: &HttpsSafeResult,
        _: &HttpsHeadResult,
    ) -> Res<()> {
        self.recorded = Some(result.clone());
        Ok(())
    }
    fn stop(&mut self, _: &HttpsAttemptSession) -> Res<()> {
        Ok(())
    }
    fn close_disposed(&mut self, _: &HttpsAttemptSession) -> Res<()> {
        self.closed_disposed = true;
        Ok(())
    }
}

struct ScheduledEvent {
    at: Duration,
    event: HttpsEvent,
}

struct TimedBoundary {
    origin: Option<Instant>,
    events: VecDeque<ScheduledEvent>,
    stop_calls: usize,
}

impl TimedBoundary {
    fn new(events: Vec<ScheduledEvent>) -> Self {
        Self {
            origin: None,
            events: events.into(),
            stop_calls: 0,
        }
    }
}

impl HttpsBoundary for TimedBoundary {
    fn ready_connect(&mut self, _: &HttpsIpcRequest) -> Res<()> {
        self.origin = Some(Instant::now());
        Ok(())
    }
    fn start_phase(&mut self, _: &HttpsIpcRequest, _: HttpsPhase, _: i64) -> Res<()> {
        Ok(())
    }
    fn poll(&mut self) -> Res<Option<HttpsEvent>> {
        let elapsed = self.origin.expect("ready_connect sets origin").elapsed();
        if self.events.front().is_some_and(|e| elapsed >= e.at) {
            return Ok(self.events.pop_front().map(|e| e.event));
        }
        Ok(None)
    }
    fn renew(&mut self, _: &HttpsIpcRequest, _: i64) -> Res<()> {
        Ok(())
    }
    fn acknowledge(&mut self, _: &HttpsIpcRequest) -> Res<bool> {
        Ok(true)
    }
    fn stop(&mut self) -> Res<bool> {
        self.stop_calls += 1;
        Ok(true)
    }
}

fn make_result(q: &HttpsIpcRequest, outcome: HttpsOutcome, observed_at_ms: i64) -> HttpsSafeResult {
    let is_head = outcome == HttpsOutcome::HeadResponse;
    let second = if is_head {
        "head_response_does_not_prove_access_or_objective_completion"
    } else {
        "single_endpoint_timeout_not_whole_host"
    };
    HttpsSafeResult {
        schema: "m1_https_ipc_v1".into(),
        kind: "RESULT".into(),
        attempt: q.attempt,
        session_generation: q.session_generation,
        sequence: 7,
        outcome,
        observed_at_ms,
        network_bytes: 128,
        header_bytes: if is_head { 64 } else { 0 },
        status_code: if is_head { 200 } else { 0 },
        tls_category: "none".into(),
        redirect: RedirectDisposition::None,
        limitations: vec![LAB_SCOPE_LIMITATION.into(), second.into()],
    }
}

#[test]
fn delayed_release_avoids_double_deduction_and_admits_timeout_at_cutoff_plus_750ms() {
    let base_wall = 1_700_000_000_000i64;
    let mut store = TimedStore::new(base_wall, 500, Duration::from_millis(350));
    let dl = store.session.attempt_deadline_at_ms;
    let res_to = make_result(&store.session.request, HttpsOutcome::TimedOut, dl + 700);
    let mut boundary = TimedBoundary::new(vec![
        ScheduledEvent {
            at: Duration::from_millis(360),
            event: HttpsEvent::Ready(HttpsPhase::Tls),
        },
        ScheduledEvent {
            at: Duration::from_millis(1250),
            event: HttpsEvent::Result(res_to),
        },
    ]);
    let start_mono = Instant::now();
    let mut clock = || Ok(base_wall + start_mono.elapsed().as_millis() as i64);
    let run_id = store.session.run_id;
    acquire_with_clock(&mut store, &mut boundary, run_id, 2, &mut clock)
        .expect("timeout at original cutoff+750ms must not fail from double deduction");
    assert_eq!(boundary.stop_calls, 0);
    assert!(store.closed_disposed);
    assert_eq!(store.recorded.unwrap().observed_at_ms, dl + 700);
}

fn run_head_case(
    base_wall: i64,
    max_duration_ms: u32,
    obs_offset_ms: i64,
    deliver_at: Duration,
    grant_head: bool,
) -> (Res<()>, TimedStore, TimedBoundary) {
    let mut store = TimedStore::new(base_wall, max_duration_ms, Duration::ZERO);
    let dl = store.session.attempt_deadline_at_ms;
    let res = make_result(
        &store.session.request,
        HttpsOutcome::HeadResponse,
        dl + obs_offset_ms,
    );
    let mut events = vec![ScheduledEvent {
        at: Duration::from_millis(10),
        event: HttpsEvent::Ready(HttpsPhase::Tls),
    }];
    if grant_head {
        events.push(ScheduledEvent {
            at: Duration::from_millis(20),
            event: HttpsEvent::Ready(HttpsPhase::Head),
        });
    }
    events.push(ScheduledEvent {
        at: deliver_at,
        event: HttpsEvent::Result(res),
    });
    let mut boundary = TimedBoundary::new(events);
    let t0 = Instant::now();
    let mut clk = || Ok(base_wall + t0.elapsed().as_millis() as i64);
    let run_id = store.session.run_id;
    let out = acquire_with_clock(&mut store, &mut boundary, run_id, 2, &mut clk);
    (out, store, boundary)
}

#[test]
fn terminal_window_rejects_late_head_success_and_results_past_1000ms() {
    let base_wall = 1_700_000_000_000i64;
    // Pre-cutoff success delivered at cutoff + 750ms (inside +1000ms window) is accepted.
    let (ok_res, ok_store, ok_boundary) =
        run_head_case(base_wall, 200, -10, Duration::from_millis(950), true);
    ok_res.expect("pre-cutoff success delivered at cutoff+750ms must be accepted");
    assert_eq!(ok_boundary.stop_calls, 0);
    assert!(ok_store.closed_disposed);
    assert_eq!(
        ok_store.recorded.unwrap().observed_at_ms,
        ok_store.session.attempt_deadline_at_ms - 10
    );

    // Negative: success observed after cutoff rejects.
    let (err_after, _, b_after) =
        run_head_case(base_wall, 200, 10, Duration::from_millis(250), true);
    assert_eq!(err_after.unwrap_err(), Fail::Input("https_result_refused"));
    assert_eq!(b_after.stop_calls, 1);

    // Negative: future observation (observed_at_ms > wall_now) rejects.
    let (err_future, _, b_future) =
        run_head_case(base_wall, 200, -10, Duration::from_millis(30), true);
    assert_eq!(err_future.unwrap_err(), Fail::Input("https_result_refused"));
    assert_eq!(b_future.stop_calls, 1);

    // Negative: invalid phase (HeadResponse when only Tls granted) rejects.
    let (err_phase, _, b_phase) =
        run_head_case(base_wall, 200, -10, Duration::from_millis(50), false);
    assert_eq!(err_phase.unwrap_err(), Fail::Input("https_result_refused"));
    assert_eq!(b_phase.stop_calls, 1);

    let mut store_over = TimedStore::new(base_wall, 150, Duration::ZERO);
    let dl_over = store_over.session.attempt_deadline_at_ms;
    let over_to = make_result(
        &store_over.session.request,
        HttpsOutcome::TimedOut,
        dl_over + 900,
    );
    let mut b_over = TimedBoundary::new(vec![ScheduledEvent {
        at: Duration::from_millis(1220),
        event: HttpsEvent::Result(over_to),
    }]);
    let t1 = Instant::now();
    let mut clk1 = || Ok(base_wall + t1.elapsed().as_millis() as i64);
    let run_id_over = store_over.session.run_id;
    let err_over =
        acquire_with_clock(&mut store_over, &mut b_over, run_id_over, 2, &mut clk1).unwrap_err();
    assert_eq!(err_over, Fail::State("live_effect_deadline"));
    assert_eq!(b_over.stop_calls, 1);
}

#[test]
fn backward_wall_clock_jump_cannot_extend_marker_bound() {
    let base_wall = 1_700_000_000_000i64;
    let mut store = TimedStore::new(base_wall, 200, Duration::from_millis(180));
    let dl = store.session.attempt_deadline_at_ms;
    // Delayed result at marker + 1250ms (> 200 + 1000) must fail even if wall clock stayed frozen at base_wall.
    let late_to = make_result(&store.session.request, HttpsOutcome::TimedOut, dl + 50);
    let mut boundary = TimedBoundary::new(vec![ScheduledEvent {
        at: Duration::from_millis(1250),
        event: HttpsEvent::Result(late_to),
    }]);
    let mut frozen_wall = || Ok(base_wall);
    let run_id = store.session.run_id;
    let err =
        acquire_with_clock(&mut store, &mut boundary, run_id, 2, &mut frozen_wall).unwrap_err();
    assert_eq!(err, Fail::State("live_effect_deadline"));
    assert_eq!(boundary.stop_calls, 1);
}

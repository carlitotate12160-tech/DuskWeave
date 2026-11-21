use crate::lab_support::{sample_https_ipc_request, sample_https_safe_result, uid};
use duskweave::m1_effect_control::https::{HttpsBoundary, HttpsEvent, acquire, acquire_with_clock};
use duskweave::m1_effect_session::EffectPhase;
use duskweave::m1_effect_session::attempt::{AttemptKind, HttpsAttemptSession, HttpsAttemptStore};
use duskweave::m1_https::HttpsHeadResult;
use duskweave::m1_https::ipc::{HttpsIpcRequest, HttpsOutcome, HttpsPhase, HttpsSafeResult};
use duskweave::{Fail, Res};
use std::collections::VecDeque;
use uuid::Uuid;

struct MockAttemptStore {
    session: HttpsAttemptSession,
    phases_granted: Vec<HttpsPhase>,
    recorded: Option<(HttpsSafeResult, HttpsHeadResult)>,
    stopped: bool,
    closed_disposed: bool,
    fail_stop: bool,
}

impl MockAttemptStore {
    fn new(now_ms: i64) -> Self {
        let req = sample_https_ipc_request(now_ms);
        let session = HttpsAttemptSession {
            run_id: req.run_id,
            step_index: req.step_index,
            attempt_id: req.attempt,
            result_ref: req.result_id,
            attempt_deadline_at_ms: req.deadline_at_ms,
            request: req,
            phase: EffectPhase::Reserved,
            possibly_started: false,
            granted_phase: None,
            result: None,
            disposal_confirmed: false,
        };
        Self {
            session,
            phases_granted: Vec::new(),
            recorded: None,
            stopped: false,
            closed_disposed: false,
            fail_stop: false,
        }
    }
}

impl HttpsAttemptStore for MockAttemptStore {
    fn reserve_head(&mut self, run: Uuid, step_index: u32) -> Res<HttpsAttemptSession> {
        assert_eq!(run, self.session.run_id);
        assert_eq!(step_index, self.session.step_index);
        Ok(self.session.clone())
    }
    fn inspect_head(&mut self, attempt: Uuid) -> Res<HttpsAttemptSession> {
        assert_eq!(attempt, self.session.attempt_id);
        Ok(self.session.clone())
    }
    fn current_deadline(&mut self, _session: &HttpsAttemptSession) -> Res<i64> {
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn may_start(&mut self, _session: &HttpsAttemptSession) -> Res<i64> {
        self.session.phase = EffectPhase::MayStart;
        self.session.possibly_started = true;
        self.session.granted_phase = Some(HttpsPhase::Connect);
        self.phases_granted.push(HttpsPhase::Connect);
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn advance_phase(&mut self, _session: &HttpsAttemptSession, phase: HttpsPhase) -> Res<i64> {
        self.session.granted_phase = Some(phase);
        self.phases_granted.push(phase);
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn record_result(
        &mut self,
        _session: &HttpsAttemptSession,
        result: &HttpsSafeResult,
        head_result: &HttpsHeadResult,
    ) -> Res<()> {
        self.session.phase = EffectPhase::ResultRecorded;
        self.session.result = Some(result.clone());
        self.recorded = Some((result.clone(), head_result.clone()));
        Ok(())
    }
    fn stop(&mut self, _session: &HttpsAttemptSession) -> Res<()> {
        if self.fail_stop {
            return Err(Fail::Store("stop_store_failed"));
        }
        self.session.phase = EffectPhase::Stopped;
        self.stopped = true;
        Ok(())
    }
    fn close_disposed(&mut self, _session: &HttpsAttemptSession) -> Res<()> {
        if self.fail_stop {
            return Err(Fail::Store("close_store_failed"));
        }
        self.session.phase = EffectPhase::Closed;
        self.session.disposal_confirmed = true;
        self.closed_disposed = true;
        Ok(())
    }
}

struct MockBoundary {
    events: VecDeque<Res<Option<HttpsEvent>>>,
    started_phases: Vec<(HttpsPhase, i64)>,
    renewals: Vec<i64>,
    ack_ok: bool,
    stop_disposed: bool,
    stop_calls: usize,
}

impl MockBoundary {
    fn new(events: Vec<HttpsEvent>) -> Self {
        Self {
            events: events.into_iter().map(|e| Ok(Some(e))).collect(),
            started_phases: Vec::new(),
            renewals: Vec::new(),
            ack_ok: true,
            stop_disposed: true,
            stop_calls: 0,
        }
    }
}

impl HttpsBoundary for MockBoundary {
    fn ready_connect(&mut self, _q: &HttpsIpcRequest) -> Res<()> {
        Ok(())
    }
    fn start_phase(
        &mut self,
        _q: &HttpsIpcRequest,
        phase: HttpsPhase,
        deadline_at_ms: i64,
    ) -> Res<()> {
        self.started_phases.push((phase, deadline_at_ms));
        Ok(())
    }
    fn poll(&mut self) -> Res<Option<HttpsEvent>> {
        self.events.pop_front().unwrap_or(Ok(None))
    }
    fn renew(&mut self, _q: &HttpsIpcRequest, deadline_at_ms: i64) -> Res<()> {
        self.renewals.push(deadline_at_ms);
        Ok(())
    }
    fn acknowledge(&mut self, _q: &HttpsIpcRequest) -> Res<bool> {
        Ok(self.ack_ok)
    }
    fn stop(&mut self) -> Res<bool> {
        self.stop_calls += 1;
        Ok(self.stop_disposed)
    }
}

#[test]
fn attempt_session_history_validation_and_launchable_invariants() {
    assert_eq!(AttemptKind::Dns.as_str(), "dns");
    assert_eq!(AttemptKind::RefreshDns.as_str(), "refresh_dns");
    assert_eq!(AttemptKind::Head.as_str(), "head");

    let now = 1_700_000_000_000i64;
    let store = MockAttemptStore::new(now);
    let mut s = store.session.clone();
    assert!(s.launchable());
    s.validate_history().unwrap();

    // Closed session requires disposal_confirmed and possibly_started.
    s.phase = EffectPhase::Closed;
    s.possibly_started = true;
    s.disposal_confirmed = false;
    assert!(s.validate_history().is_err());
    s.disposal_confirmed = true;
    s.validate_history().unwrap();
    assert!(!s.launchable());

    // Result present requires matching granted_phase.
    let res = sample_https_safe_result(
        &s.request,
        HttpsOutcome::HeadResponse,
        now + 500,
        200,
        "none",
        None,
    );
    s.result = Some(res);
    s.granted_phase = Some(HttpsPhase::Connect);
    assert!(s.validate_history().is_err());
    s.granted_phase = Some(HttpsPhase::Head);
    s.validate_history().unwrap();
}

#[test]
fn phased_connect_tls_head_progression_and_f3_early_failures() {
    let now = 1_700_000_000_000i64;
    let mut store = MockAttemptStore::new(now);
    let res_200 = sample_https_safe_result(
        &store.session.request,
        HttpsOutcome::HeadResponse,
        now + 1500,
        200,
        "none",
        None,
    );
    let mut boundary = MockBoundary::new(vec![
        HttpsEvent::Ready(HttpsPhase::Tls),
        HttpsEvent::Ready(HttpsPhase::Head),
        HttpsEvent::Result(res_200.clone()),
    ]);
    let (run_id, step_idx) = (store.session.run_id, store.session.step_index);
    let mut clock = || Ok(now + 1600);
    acquire_with_clock(&mut store, &mut boundary, run_id, step_idx, &mut clock).unwrap();
    assert_eq!(
        store.phases_granted,
        vec![HttpsPhase::Connect, HttpsPhase::Tls, HttpsPhase::Head]
    );
    assert_eq!(boundary.started_phases.len(), 3);
    assert!(store.closed_disposed);
    assert_eq!(boundary.stop_calls, 0);
    store.session.validate_history().unwrap();

    // F3: ConnectionFailed at Head phase is accepted cleanly as an endpoint limitation.
    let mut store_cf = MockAttemptStore::new(now);
    let res_cf = sample_https_safe_result(
        &store_cf.session.request,
        HttpsOutcome::ConnectionFailed,
        now + 1100,
        0,
        "none",
        None,
    );
    let mut boundary_cf = MockBoundary::new(vec![
        HttpsEvent::Ready(HttpsPhase::Tls),
        HttpsEvent::Ready(HttpsPhase::Head),
        HttpsEvent::Result(res_cf),
    ]);
    acquire_with_clock(
        &mut store_cf,
        &mut boundary_cf,
        run_id,
        step_idx,
        &mut || Ok(now + 1200),
    )
    .unwrap();
    assert!(store_cf.closed_disposed);

    // Forged HeadResponse while only Tls was granted must refuse and STOP.
    let mut store_forged = MockAttemptStore::new(now);
    let mut boundary_forged = MockBoundary::new(vec![
        HttpsEvent::Ready(HttpsPhase::Tls),
        HttpsEvent::Result(res_200),
    ]);
    boundary_forged.stop_disposed = false;
    let err = acquire_with_clock(
        &mut store_forged,
        &mut boundary_forged,
        run_id,
        step_idx,
        &mut || Ok(now + 1600),
    )
    .unwrap_err();
    assert_eq!(err, Fail::Input("https_result_refused"));
    assert_eq!(boundary_forged.stop_calls, 1);
    assert!(store_forged.stopped);
    assert!(store_forged.recorded.is_none());
}

#[test]
fn out_of_order_phase_f4_terminal_timeout_and_unconfirmed_disposal() {
    let now = 1_700_000_000_000i64;

    // Skipping Tls (Connect -> Ready(Head)) refuses and stops.
    let mut store_skip = MockAttemptStore::new(now);
    let (run_id, step_idx) = (store_skip.session.run_id, store_skip.session.step_index);
    let mut boundary_skip = MockBoundary::new(vec![HttpsEvent::Ready(HttpsPhase::Head)]);
    let err = acquire_with_clock(
        &mut store_skip,
        &mut boundary_skip,
        run_id,
        step_idx,
        &mut || Ok(now + 100),
    )
    .unwrap_err();
    assert_eq!(err, Fail::State("live_host_protocol_refused"));
    assert_eq!(boundary_skip.stop_calls, 1);
    assert!(store_skip.closed_disposed);

    // F4: TimedOut observed after deadline (deadline + 400 <= deadline + 1000) succeeds.
    let mut store_to = MockAttemptStore::new(now);
    let deadline = store_to.session.attempt_deadline_at_ms;
    let res_to = sample_https_safe_result(
        &store_to.session.request,
        HttpsOutcome::TimedOut,
        deadline + 400,
        0,
        "none",
        None,
    );
    let mut boundary_to = MockBoundary::new(vec![
        HttpsEvent::Ready(HttpsPhase::Tls),
        HttpsEvent::Result(res_to),
    ]);
    let mut to_calls = 0usize;
    acquire_with_clock(
        &mut store_to,
        &mut boundary_to,
        run_id,
        step_idx,
        &mut || {
            to_calls += 1;
            Ok(if to_calls == 1 {
                now + 100
            } else {
                deadline + 450
            })
        },
    )
    .unwrap();
    assert!(store_to.closed_disposed);
    assert_eq!(
        store_to.recorded.as_ref().unwrap().0.observed_at_ms,
        deadline + 400
    );

    // Unconfirmed disposal on ACK returns Unresolved("live_disposal_unconfirmed") and calls stop().
    let mut store_ack = MockAttemptStore::new(now);
    let res_conn = sample_https_safe_result(
        &store_ack.session.request,
        HttpsOutcome::ConnectionFailed,
        now + 200,
        0,
        "none",
        None,
    );
    let mut boundary_ack = MockBoundary::new(vec![HttpsEvent::Result(res_conn)]);
    boundary_ack.ack_ok = false;
    boundary_ack.stop_disposed = false;
    let ack_err = acquire_with_clock(
        &mut store_ack,
        &mut boundary_ack,
        run_id,
        step_idx,
        &mut || Ok(now + 250),
    )
    .unwrap_err();
    assert_eq!(ack_err, Fail::Unresolved("live_disposal_unconfirmed"));
    assert_eq!(boundary_ack.stop_calls, 1);
    assert!(store_ack.stopped);

    // Non-launchable session refuses before boundary.ready_connect; failed stop bookkeeping returns live_effect_stop_pending.
    let mut store_charged = MockAttemptStore::new(now);
    store_charged.session.possibly_started = true;
    let mut boundary_unused = MockBoundary::new(vec![]);
    assert_eq!(
        acquire(&mut store_charged, &mut boundary_unused, run_id, step_idx).unwrap_err(),
        Fail::State("live_effect_already_charged")
    );

    let mut store_stop_fail = MockAttemptStore::new(now);
    store_stop_fail.fail_stop = true;
    let mut boundary_fail = MockBoundary::new(vec![HttpsEvent::Ready(HttpsPhase::Head)]);
    assert_eq!(
        acquire_with_clock(
            &mut store_stop_fail,
            &mut boundary_fail,
            run_id,
            step_idx,
            &mut || Ok(now + 100),
        )
        .unwrap_err(),
        Fail::Unresolved("live_effect_stop_pending")
    );
    let _ = uid(1);
}

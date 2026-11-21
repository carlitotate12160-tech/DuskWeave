//! Broker-owned phased HTTPS control loop (`connect` -> `tls` -> `head`) over single attempt budget.
use crate::m1_effect_session::attempt::{HttpsAttemptSession, HttpsAttemptStore};
use crate::m1_https::HttpsHeadResult;
use crate::m1_https::ipc::{HttpsIpcRequest, HttpsPhase, HttpsSafeResult};
use crate::{Fail, Res};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpsEvent {
    Ready(HttpsPhase),
    Result(HttpsSafeResult),
}

pub trait HttpsBoundary {
    fn ready_connect(&mut self, q: &HttpsIpcRequest) -> Res<()>;
    fn start_phase(
        &mut self,
        q: &HttpsIpcRequest,
        phase: HttpsPhase,
        deadline_at_ms: i64,
    ) -> Res<()>;
    fn poll(&mut self) -> Res<Option<HttpsEvent>>;
    fn renew(&mut self, q: &HttpsIpcRequest, deadline_at_ms: i64) -> Res<()>;
    fn acknowledge(&mut self, q: &HttpsIpcRequest) -> Res<bool>;
    fn stop(&mut self) -> Res<bool>;
}

pub fn acquire(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    run: Uuid,
    step_index: u32,
) -> Res<()> {
    acquire_with_clock(store, boundary, run, step_index, &mut now_unix_ms)
}

pub fn acquire_with_clock(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    run: Uuid,
    step_index: u32,
    now_fn: &mut dyn FnMut() -> Res<i64>,
) -> Res<()> {
    let session = store.reserve_head(run, step_index)?;
    if !session.launchable() {
        return Err(Fail::State("live_effect_already_charged"));
    }
    let outcome = complete(store, boundary, &session, now_fn);
    if let Err(error) = outcome {
        // Local STOP precedes every database operation, including failed bookkeeping.
        let disposed = boundary.stop().unwrap_or(false);
        let recorded = if disposed {
            store.close_disposed(&session)
        } else {
            store.stop(&session)
        };
        return recorded
            .map_err(|_| Fail::Unresolved("live_effect_stop_pending"))
            .and(Err(error));
    }
    Ok(())
}

fn complete(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
    now_fn: &mut dyn FnMut() -> Res<i64>,
) -> Res<()> {
    let (charged, marker_origin) = release_connect(store, boundary, session)?;
    let (safe, head) = receive_result(store, boundary, &charged, marker_origin, now_fn)?;
    store.record_result(&charged, &safe, &head)?;
    if !boundary.acknowledge(&charged.request)? {
        return Err(Fail::Unresolved("live_disposal_unconfirmed"));
    }
    store.close_disposed(&charged)
}

fn release_connect(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
) -> Res<(HttpsAttemptSession, Instant)> {
    let q = &session.request;
    boundary.ready_connect(q)?;
    let marker_origin = Instant::now();
    store.may_start(session)?;
    let charged = store.inspect_head(session.attempt_id)?;
    let deadline = store.current_deadline(&charged)?;
    boundary.start_phase(q, HttpsPhase::Connect, deadline)?;
    Ok((charged, marker_origin))
}

fn attempt_windows(
    session: &HttpsAttemptSession,
    marker_origin: Instant,
    now_fn: &mut dyn FnMut() -> Res<i64>,
) -> Res<(Instant, Instant)> {
    let wall_now = now_fn()?;
    let remaining_ms = session
        .attempt_deadline_at_ms
        .saturating_sub(wall_now)
        .clamp(0, i64::from(session.request.max_duration_ms)) as u64;
    let attempt_end = marker_origin + Duration::from_millis(remaining_ms);
    Ok((attempt_end, attempt_end + Duration::from_millis(1000)))
}

fn poll_terminal_event(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
    granted_phase: &mut HttpsPhase,
    attempt_end: Instant,
    now_fn: &mut dyn FnMut() -> Res<i64>,
) -> Res<Option<(HttpsSafeResult, HttpsHeadResult)>> {
    let Some(event) = boundary.poll()? else {
        return Ok(None);
    };
    handle_event(
        store,
        boundary,
        session,
        granted_phase,
        attempt_end,
        event,
        now_fn,
    )
}

#[allow(clippy::too_many_arguments)]
fn receive_step(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
    granted_phase: &mut HttpsPhase,
    renewal: &mut Instant,
    renewal_count: &mut u8,
    attempt_end: Instant,
    now_fn: &mut dyn FnMut() -> Res<i64>,
) -> Res<Option<(HttpsSafeResult, HttpsHeadResult)>> {
    if let Some(terminal) =
        poll_terminal_event(store, boundary, session, granted_phase, attempt_end, now_fn)?
    {
        return Ok(Some(terminal));
    }
    renew(
        store,
        boundary,
        session,
        renewal,
        renewal_count,
        attempt_end,
    )?;
    std::thread::sleep(Duration::from_millis(5));
    Ok(None)
}

fn receive_result(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
    marker_origin: Instant,
    now_fn: &mut dyn FnMut() -> Res<i64>,
) -> Res<(HttpsSafeResult, HttpsHeadResult)> {
    let mut granted_phase = HttpsPhase::Connect;
    let mut renewal = marker_origin + Duration::from_millis(500);
    let mut renewal_count = 0u8;
    let (attempt_end, handle_end) = attempt_windows(session, marker_origin, now_fn)?;
    loop {
        if Instant::now() >= handle_end {
            return Err(Fail::State("live_effect_deadline"));
        }
        if let Some(terminal) = receive_step(
            store,
            boundary,
            session,
            &mut granted_phase,
            &mut renewal,
            &mut renewal_count,
            attempt_end,
            now_fn,
        )? {
            return Ok(terminal);
        }
    }
}

fn handle_event(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
    granted_phase: &mut HttpsPhase,
    attempt_end: Instant,
    event: HttpsEvent,
    now_fn: &mut dyn FnMut() -> Res<i64>,
) -> Res<Option<(HttpsSafeResult, HttpsHeadResult)>> {
    match event {
        HttpsEvent::Ready(next_phase) => {
            advance(
                store,
                boundary,
                session,
                granted_phase,
                next_phase,
                attempt_end,
            )?;
            Ok(None)
        }
        HttpsEvent::Result(safe) => {
            safe.validate_for(&session.request, *granted_phase, now_fn()?)?;
            let head = safe.into_head_result(&session.request)?;
            Ok(Some((safe, head)))
        }
    }
}

fn advance(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
    granted_phase: &mut HttpsPhase,
    next_phase: HttpsPhase,
    attempt_end: Instant,
) -> Res<()> {
    let valid_next = matches!(
        (*granted_phase, next_phase),
        (HttpsPhase::Connect, HttpsPhase::Tls) | (HttpsPhase::Tls, HttpsPhase::Head)
    );
    if !valid_next || Instant::now() >= attempt_end {
        return Err(Fail::State("live_host_protocol_refused"));
    }
    let deadline = store.advance_phase(session, next_phase)?;
    boundary.start_phase(&session.request, next_phase, deadline)?;
    *granted_phase = next_phase;
    Ok(())
}

fn renew(
    store: &mut dyn HttpsAttemptStore,
    boundary: &mut dyn HttpsBoundary,
    session: &HttpsAttemptSession,
    renewal: &mut Instant,
    renewal_count: &mut u8,
    attempt_end: Instant,
) -> Res<()> {
    let now = Instant::now();
    if now >= attempt_end || now < *renewal || *renewal_count >= 10 {
        return Ok(());
    }
    let deadline = store.current_deadline(session)?;
    boundary.renew(&session.request, deadline)?;
    *renewal += Duration::from_millis(500);
    *renewal_count += 1;
    Ok(())
}

fn now_unix_ms() -> Res<i64> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Fail::State("live_effect_deadline"))?;
    i64::try_from(d.as_millis()).map_err(|_| Fail::State("live_effect_deadline"))
}

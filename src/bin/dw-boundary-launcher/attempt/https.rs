//! Root-owned HTTPS LAB attempt session state and monotonic connect->tls->head grants.
use super::{
    Guard, blocked_ids, create_dir, dir, owned_root, qualify_session_meta, save_json,
    session_guard_ids,
};
use duskweave::m1_https::ipc::{HttpsIpcRequest, HttpsPhase};
use duskweave::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct HttpsSession {
    #[serde(skip)]
    pub preparation_deadline: Option<Instant>,
    pub run_id: Uuid,
    pub request: Box<HttpsIpcRequest>,
    pub granted_phase: Option<HttpsPhase>,
    pub opened_at_ms: i64,
    pub ready_at_ms: Option<i64>,
    pub attempt_deadline_at_ms: Option<i64>,
    pub handling_deadline_at_ms: i64,
}

impl HttpsSession {
    pub fn max_watchdog_ms(&self) -> u64 {
        13000
    }
}

pub(in super::super) fn record(attempt: Uuid) -> Res<HttpsSession> {
    let path = dir(attempt).join("session.json");
    let meta =
        std::fs::symlink_metadata(&path).map_err(|_| Fail::Config("live_host_unqualified"))?;
    if !qualify_session_meta(&meta) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let file = File::open(path).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let session: HttpsSession = serde_json::from_reader(file.take(16385))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if session.request.attempt != attempt {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(session)
}

pub(in super::super) fn load(attempt: Uuid, generation: u64) -> Res<HttpsSession> {
    let session = record(attempt)?;
    if session.request.session_generation != generation {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(session)
}

pub(in super::super) fn stop_ids(attempt: Uuid) -> Option<(Uuid, Uuid, Uuid, u64)> {
    let s = record(attempt).ok()?;
    let q = &s.request;
    Some((
        s.run_id,
        q.engagement_id,
        q.campaign_id,
        q.session_generation,
    ))
}

fn guard_session(s: &HttpsSession) -> Res<(Guard, Guard)> {
    session_guard_ids(
        s.run_id,
        s.request.engagement_id,
        s.request.campaign_id,
        Instant::now() + Duration::from_millis(1000),
    )
}

fn is_blocked(s: &HttpsSession) -> bool {
    blocked_ids(
        s.request.engagement_id,
        s.request.campaign_id,
        s.request.session_generation,
        s.run_id,
    )
}

fn validate_open(run_id: Uuid, request: &HttpsIpcRequest) -> Res<i64> {
    owned_root()?;
    let now = super::super::qualification::now_ms()?;
    request.validate(now)?;
    if run_id.is_nil() {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(now)
}

fn load_guarded(attempt: Uuid, generation: u64) -> Res<(HttpsSession, (Guard, Guard), i64)> {
    let s = load(attempt, generation)?;
    let g = guard_session(&s)?;
    let now = super::super::qualification::now_ms()?;
    Ok((s, g, now))
}

fn finish_open(session: HttpsSession) -> Res<HttpsSession> {
    create_dir(session.request.attempt)?;
    save_json(session.request.attempt, &session)?;
    Ok(session)
}

pub(in super::super) fn open(run_id: Uuid, request: &HttpsIpcRequest) -> Res<HttpsSession> {
    let monotonic = Instant::now();
    let now = validate_open(run_id, request)?;
    let prep_ms = (request.deadline_at_ms - now).clamp(1, 5000) as u64;
    let session = HttpsSession {
        preparation_deadline: Some(monotonic + Duration::from_millis(prep_ms)),
        run_id,
        request: Box::new(request.clone()),
        granted_phase: None,
        opened_at_ms: now,
        ready_at_ms: None,
        attempt_deadline_at_ms: None,
        handling_deadline_at_ms: request.deadline_at_ms.min(now + 5000),
    };
    let _guard = session_guard_ids(
        session.run_id,
        session.request.engagement_id,
        session.request.campaign_id,
        monotonic + Duration::from_millis(1000),
    )?;
    if is_blocked(&session) {
        return Err(Fail::State("live_effect_stopped"));
    }
    finish_open(session)
}

pub(in super::super) fn ready_connect(attempt: Uuid, generation: u64) -> Res<()> {
    let (mut s, _guard, now) = load_guarded(attempt, generation)?;
    if is_blocked(&s) || now >= s.handling_deadline_at_ms || s.ready_at_ms.is_some() {
        return Err(Fail::State("live_effect_stopped"));
    }
    s.ready_at_ms = Some(now);
    s.handling_deadline_at_ms = s.request.deadline_at_ms.min(now + 1000);
    save_json(attempt, &s)
}

fn can_start_connect(s: &HttpsSession, now: i64, deadline: i64) -> bool {
    [
        !is_blocked(s),
        s.ready_at_ms.is_some(),
        now < s.handling_deadline_at_ms,
        deadline > now,
        deadline == s.request.deadline_at_ms,
        deadline <= now + i64::from(s.request.max_duration_ms),
        s.attempt_deadline_at_ms.is_none(),
        s.granted_phase.is_none(),
    ]
    .into_iter()
    .all(std::convert::identity)
}

pub(in super::super) fn start_connect(
    attempt: Uuid,
    generation: u64,
    deadline: i64,
    release: impl FnOnce() -> Res<()>,
) -> Res<()> {
    let (mut s, _guard, now) = load_guarded(attempt, generation)?;
    if !can_start_connect(&s, now, deadline) {
        return Err(Fail::State("live_effect_start_refused"));
    }
    charge_connect(&mut s, deadline)?;
    super::super::network::grant_https(attempt, deadline)?;
    release().inspect_err(|_| {
        let _ = super::super::network::revoke(attempt);
    })
}

fn charge_connect(s: &mut HttpsSession, deadline: i64) -> Res<()> {
    let _one_use = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dir(s.request.attempt).join("started"))
        .map_err(|_| Fail::State("live_effect_already_charged"))?;
    s.granted_phase = Some(HttpsPhase::Connect);
    s.attempt_deadline_at_ms = Some(deadline);
    s.handling_deadline_at_ms = deadline.saturating_add(1000);
    save_json(s.request.attempt, s)
}

fn can_advance_phase(s: &HttpsSession, phase: HttpsPhase, now: i64, deadline: i64) -> bool {
    let phase_ok = matches!(
        (s.granted_phase, phase),
        (Some(HttpsPhase::Connect), HttpsPhase::Tls) | (Some(HttpsPhase::Tls), HttpsPhase::Head)
    );
    s.attempt_deadline_at_ms.is_some_and(|orig| {
        [
            !is_blocked(s),
            phase_ok,
            now < orig,
            deadline > now,
            deadline == orig,
        ]
        .into_iter()
        .all(std::convert::identity)
    })
}

pub(in super::super) fn advance_phase(
    attempt: Uuid,
    generation: u64,
    phase: HttpsPhase,
    deadline: i64,
    release: impl FnOnce() -> Res<()>,
) -> Res<()> {
    let (mut s, _guard, now) = load_guarded(attempt, generation)?;
    if !can_advance_phase(&s, phase, now, deadline) {
        return Err(Fail::State("live_effect_start_refused"));
    }
    s.granted_phase = Some(phase);
    save_json(attempt, &s)?;
    super::super::network::grant_https(attempt, deadline)?;
    release().inspect_err(|_| {
        let _ = super::super::network::revoke(attempt);
    })
}

pub(in super::super) fn renew(attempt: Uuid, generation: u64, deadline: i64) -> Res<()> {
    let (s, _guard, now) = load_guarded(attempt, generation)?;
    let original = s
        .attempt_deadline_at_ms
        .ok_or(Fail::State("live_effect_start_refused"))?;
    let valid = [
        !is_blocked(&s),
        now < original,
        deadline > now,
        deadline <= original,
    ]
    .into_iter()
    .all(std::convert::identity);
    if !valid {
        return Err(Fail::State("live_effect_stopped"));
    }
    super::super::network::grant_https(attempt, deadline)
}

#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/launcher_https_attempt.rs"]
pub(in super::super) mod tests;

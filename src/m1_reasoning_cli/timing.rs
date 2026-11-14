//! Profile-selected autonomous decision window enforcement.
use super::{Owners, known_identity, open_stores, owners, store_input_binding};
use duskweave::m1_inference_profile::{Timing, lookup};
use duskweave::m1_reasoning::inference::Scope;
use duskweave::{Fail, Res};
use postgres::Client;
use std::time::{Duration, Instant};

pub(super) fn autonomous_deadline(
    c: &mut Client,
    s: &Scope,
    timing: Option<&'static Timing>,
) -> Res<Instant> {
    let requested = Instant::now();
    let ms: i64 = c
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .map_err(|_| Fail::State("inference_authority_unavailable"))?
        .try_get(0)
        .map_err(|_| Fail::Store("run_decode"))?;
    let max_ms = timing.map_or(40000, |t| t.shared_ms);
    let ms = u64::try_from(ms)
        .ok()
        .filter(|v| *v > 0 && *v <= max_ms)
        .ok_or(Fail::State("inference_deadline"))?;
    Ok(if timing.is_some() {
        requested
    } else {
        Instant::now()
    } + Duration::from_millis(ms))
}

pub(super) fn autonomous_owners(
    q: &super::ContextRequest,
    s: &Scope,
    inference_profile: Option<&str>,
) -> Res<Owners> {
    let timing = inference_profile
        .map(lookup)
        .transpose()?
        .map(|profile| profile.timing);
    let (mut c, store) = open_stores(q)?;
    if known_identity(&mut c, s, &store.input_hash)? {
        return Err(Fail::State("run_decision_already_started"));
    }
    // This private phase reserves stdout for the readiness frame. The same
    // durable identity binding remains mandatory; the public command announces it.
    store_input_binding(&mut c, s, &store.input_hash)?;
    let deadline = autonomous_deadline(&mut c, s, timing)?;
    owners(c, q, deadline, store)
}

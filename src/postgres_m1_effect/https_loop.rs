//! Restricted attempt-keyed Postgres adapters for `m1_https_loop_v1` LAB DNS & HEAD effects.
use crate::m1_dns::DnsResult;
use crate::m1_effect_session::attempt::{HttpsAttemptSession, HttpsAttemptStore};
use crate::m1_effect_session::{EffectSession, EffectStore};
use crate::m1_https::HttpsHeadResult;
use crate::m1_https::ipc::{HttpsPhase, HttpsSafeResult};
use crate::{Fail, Res};
use postgres::Client;
use serde_json::Value;
use uuid::Uuid;

pub struct PgAttemptDnsStore {
    client: Client,
    step_index: u32,
    attempt_kind: &'static str,
    attempt_id: Option<Uuid>,
}

impl PgAttemptDnsStore {
    pub fn new(client: Client, step_index: u32, attempt_kind: &'static str) -> Self {
        Self {
            client,
            step_index,
            attempt_kind,
            attempt_id: None,
        }
    }

    fn decode_session(&self, raw: Value) -> Res<EffectSession> {
        let session: EffectSession =
            serde_json::from_value(raw).map_err(|_| Fail::Store("live_effect_decode"))?;
        session.validate_history()?;
        Ok(session)
    }

    fn deadline(&mut self, session: &EffectSession, may_start: bool) -> Res<i64> {
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let query = if may_start {
            "SELECT execution.live_attempt_may_start($1,$2)"
        } else {
            "SELECT execution.live_attempt_current($1,$2)"
        };
        self.client
            .query_one(query, &[&session.request.attempt, &generation])
            .map_err(|_| Fail::State("live_effect_release_refused"))?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))
    }

    fn terminal(&mut self, session: &EffectSession, disposed: bool) -> Res<()> {
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        self.client
            .execute(
                "SELECT execution.live_attempt_stop($1,$2,$3)",
                &[&session.request.attempt, &generation, &disposed],
            )
            .map_err(|_| Fail::Unresolved("live_effect_stop_pending"))?;
        Ok(())
    }
}

impl EffectStore for PgAttemptDnsStore {
    fn reserve(&mut self, run: Uuid) -> Res<EffectSession> {
        let step = i32::try_from(self.step_index).map_err(|_| Fail::Input("invalid_run_steps"))?;
        let raw: Value = self
            .client
            .query_one(
                "SELECT execution.live_https_loop_dns_reserve($1,$2,$3)",
                &[&run, &step, &self.attempt_kind],
            )
            .map_err(classify_live_reserve_error)?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let session = self.decode_session(raw)?;
        if session.run_id != run {
            return Err(Fail::Store("live_effect_decode"));
        }
        self.attempt_id = Some(session.request.attempt);
        Ok(session)
    }

    fn inspect(&mut self, _run: Uuid) -> Res<EffectSession> {
        let attempt = self
            .attempt_id
            .ok_or(Fail::State("live_effect_unavailable"))?;
        let raw: Value = self
            .client
            .query_one("SELECT execution.live_attempt_record($1)", &[&attempt])
            .map_err(|_| Fail::State("live_effect_unavailable"))?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let session = self.decode_session(raw)?;
        if session.request.attempt != attempt {
            return Err(Fail::Store("live_effect_decode"));
        }
        Ok(session)
    }

    fn current_deadline(&mut self, session: &EffectSession) -> Res<i64> {
        self.deadline(session, false)
    }

    fn may_start(&mut self, session: &EffectSession) -> Res<i64> {
        self.deadline(session, true)
    }

    fn record_result(&mut self, session: &EffectSession, result: &DnsResult) -> Res<()> {
        result.validate(&session.request, session.attempt_deadline_at_ms)?;
        let raw = serde_json::to_value(result).map_err(|_| Fail::Store("live_effect_decode"))?;
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let mut tx = self
            .client
            .transaction()
            .map_err(|_| Fail::Unresolved("live_result_commit_unknown"))?;
        tx.execute(
            "SELECT execution.live_attempt_dns_result($1,$2,$3)",
            &[&session.request.attempt, &generation, &raw],
        )
        .map_err(|_| Fail::Unresolved("live_result_commit_unknown"))?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("live_result_commit_unknown"))
    }

    fn stop(&mut self, session: &EffectSession) -> Res<()> {
        self.terminal(session, false)
    }

    fn close_disposed(&mut self, session: &EffectSession) -> Res<()> {
        self.terminal(session, true)
    }
}

pub struct PgHttpsAttemptStore {
    client: Client,
}

impl PgHttpsAttemptStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn decode_head(raw: Value) -> Res<HttpsAttemptSession> {
        let session: HttpsAttemptSession =
            serde_json::from_value(raw).map_err(|_| Fail::Store("live_effect_decode"))?;
        session.validate_history()?;
        Ok(session)
    }

    fn deadline(&mut self, session: &HttpsAttemptSession, may_start: bool) -> Res<i64> {
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let query = if may_start {
            "SELECT execution.live_attempt_may_start($1,$2)"
        } else {
            "SELECT execution.live_attempt_current($1,$2)"
        };
        self.client
            .query_one(query, &[&session.attempt_id, &generation])
            .map_err(|_| Fail::State("live_effect_release_refused"))?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))
    }

    fn terminal(&mut self, session: &HttpsAttemptSession, disposed: bool) -> Res<()> {
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        self.client
            .execute(
                "SELECT execution.live_attempt_stop($1,$2,$3)",
                &[&session.attempt_id, &generation, &disposed],
            )
            .map_err(|_| Fail::Unresolved("live_effect_stop_pending"))?;
        Ok(())
    }
}

impl HttpsAttemptStore for PgHttpsAttemptStore {
    fn reserve_head(&mut self, run: Uuid, step_index: u32) -> Res<HttpsAttemptSession> {
        let step = i32::try_from(step_index).map_err(|_| Fail::Input("invalid_run_steps"))?;
        let raw: Value = self
            .client
            .query_one(
                "SELECT execution.live_https_loop_head_reserve($1,$2)",
                &[&run, &step],
            )
            .map_err(classify_live_reserve_error)?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let session = Self::decode_head(raw)?;
        if (session.run_id, session.step_index) != (run, step_index) {
            return Err(Fail::Store("live_effect_decode"));
        }
        Ok(session)
    }

    fn inspect_head(&mut self, attempt: Uuid) -> Res<HttpsAttemptSession> {
        let raw: Value = self
            .client
            .query_one("SELECT execution.live_attempt_head_record($1)", &[&attempt])
            .map_err(|_| Fail::State("live_effect_unavailable"))?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let session = Self::decode_head(raw)?;
        if session.attempt_id != attempt {
            return Err(Fail::Store("live_effect_decode"));
        }
        Ok(session)
    }

    fn current_deadline(&mut self, session: &HttpsAttemptSession) -> Res<i64> {
        self.deadline(session, false)
    }

    fn may_start(&mut self, session: &HttpsAttemptSession) -> Res<i64> {
        self.deadline(session, true)
    }

    fn advance_phase(&mut self, session: &HttpsAttemptSession, phase: HttpsPhase) -> Res<i64> {
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        self.client
            .query_one(
                "SELECT execution.live_attempt_head_phase($1,$2,$3)",
                &[&session.attempt_id, &generation, &phase.as_str()],
            )
            .map_err(|_| Fail::State("live_effect_release_refused"))?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))
    }

    fn record_result(
        &mut self,
        session: &HttpsAttemptSession,
        result: &HttpsSafeResult,
        head_result: &HttpsHeadResult,
    ) -> Res<()> {
        head_result.validate()?;
        let raw_safe =
            serde_json::to_value(result).map_err(|_| Fail::Store("live_effect_decode"))?;
        let raw_head =
            serde_json::to_value(head_result).map_err(|_| Fail::Store("live_effect_decode"))?;
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let mut tx = self
            .client
            .transaction()
            .map_err(|_| Fail::Unresolved("live_result_commit_unknown"))?;
        tx.execute(
            "SELECT execution.live_attempt_head_result($1,$2,$3,$4)",
            &[&session.attempt_id, &generation, &raw_safe, &raw_head],
        )
        .map_err(|_| Fail::Unresolved("live_result_commit_unknown"))?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("live_result_commit_unknown"))
    }

    fn stop(&mut self, session: &HttpsAttemptSession) -> Res<()> {
        self.terminal(session, false)
    }

    fn close_disposed(&mut self, session: &HttpsAttemptSession) -> Res<()> {
        self.terminal(session, true)
    }
}

const KNOWN_LIVE_RESERVE_REFUSALS: &[(&str, &str)] = &[
    ("live_policy_denied", "live_policy_denied"),
    ("live_budget_exhausted", "live_budget_exhausted"),
    ("live_time_insufficient", "live_time_insufficient"),
    ("live_premise_ineligible", "live_premise_ineligible"),
    ("live_refresh_exhausted", "live_refresh_exhausted"),
    ("live_head_exhausted", "live_head_exhausted"),
    ("live_dns_exhausted", "live_dns_exhausted"),
    ("live_effect_expired", "live_effect_expired"),
    (
        "live_qualification_unavailable",
        "live_qualification_unavailable",
    ),
    ("live_sender_not_closed", "live_sender_not_closed"),
    ("live_effect_scope_refused", "live_effect_scope_refused"),
    ("live_effect_session_busy", "live_effect_session_busy"),
    ("live_effect_refused", "live_effect_refused"),
];

fn classify_live_reserve_error(err: postgres::Error) -> Fail {
    let Some(db) = err.as_db_error().filter(|d| d.code().code() == "P0001") else {
        return Fail::State("live_effect_unavailable");
    };
    KNOWN_LIVE_RESERVE_REFUSALS
        .iter()
        .find_map(|(k, v)| (*k == db.message()).then_some(Fail::State(v)))
        .unwrap_or(Fail::State("live_effect_unavailable"))
}

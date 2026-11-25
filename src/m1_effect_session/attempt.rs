//! Attempt-scoped session state and traits for multi-attempt `m1_https_loop_v1` LAB runs.
use super::EffectPhase;
use crate::m1_https::HttpsHeadResult;
use crate::m1_https::ipc::{HttpsIpcRequest, HttpsPhase, HttpsSafeResult};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttemptKind {
    Dns,
    RefreshDns,
    Head,
}

impl AttemptKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::RefreshDns => "refresh_dns",
            Self::Head => "head",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsAttemptSession {
    pub run_id: Uuid,
    pub step_index: u32,
    pub attempt_id: Uuid,
    pub request: HttpsIpcRequest,
    pub result_ref: Uuid,
    pub phase: EffectPhase,
    pub possibly_started: bool,
    pub granted_phase: Option<HttpsPhase>,
    pub attempt_deadline_at_ms: i64,
    pub result: Option<HttpsSafeResult>,
    pub disposal_confirmed: bool,
}

impl HttpsAttemptSession {
    pub fn validate_history(&self) -> Res<()> {
        self.request.validate_shape()?;
        let valid = [
            !self.run_id.is_nil(),
            (1..=4).contains(&self.step_index),
            self.step_index == self.request.step_index,
            !self.attempt_id.is_nil(),
            self.attempt_id == self.request.attempt,
            !self.result_ref.is_nil(),
            self.result_ref == self.request.result_id,
            self.attempt_deadline_at_ms > 0,
            self.attempt_deadline_at_ms <= self.request.episode_deadline_at_ms,
            self.phase != EffectPhase::Closed || self.disposal_confirmed,
            self.phase == EffectPhase::Reserved || self.possibly_started,
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Store("live_effect_decode"));
        }
        self.validate_stored_result()
    }

    fn validate_stored_result(&self) -> Res<()> {
        if let Some(result) = &self.result {
            let granted = self
                .granted_phase
                .ok_or(Fail::Store("live_effect_decode"))?;
            result
                .validate_historical_for(&self.request, granted)
                .map_err(|_| Fail::Store("live_effect_decode"))?;
            result
                .into_head_result(&self.request)
                .map_err(|_| Fail::Store("live_effect_decode"))?;
        }
        Ok(())
    }

    pub fn launchable(&self) -> bool {
        [
            self.phase == EffectPhase::Reserved,
            !self.possibly_started,
            self.granted_phase.is_none(),
            self.result.is_none(),
            !self.disposal_confirmed,
        ]
        .into_iter()
        .all(|v| v)
    }
}

pub trait HttpsAttemptStore {
    fn reserve_head(&mut self, run: Uuid, step_index: u32) -> Res<HttpsAttemptSession>;
    fn inspect_head(&mut self, attempt: Uuid) -> Res<HttpsAttemptSession>;
    fn current_deadline(&mut self, session: &HttpsAttemptSession) -> Res<i64>;
    fn may_start(&mut self, session: &HttpsAttemptSession) -> Res<i64>;
    fn advance_phase(&mut self, session: &HttpsAttemptSession, phase: HttpsPhase) -> Res<i64>;
    fn record_result(
        &mut self,
        session: &HttpsAttemptSession,
        result: &HttpsSafeResult,
        head_result: &HttpsHeadResult,
    ) -> Res<()>;
    fn stop(&mut self, session: &HttpsAttemptSession) -> Res<()>;
    fn close_disposed(&mut self, session: &HttpsAttemptSession) -> Res<()>;
}

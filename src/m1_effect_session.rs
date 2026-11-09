//! Broker-owned live attempt accounting. Historical may-start is never transmission proof.
use crate::m1_dns::{DnsRequest, DnsResult};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectPhase {
    Reserved,
    MayStart,
    ResultRecorded,
    Stopped,
    Closed,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectSession {
    pub run_id: Uuid,
    pub request: DnsRequest,
    pub result_ref: Uuid,
    pub phase: EffectPhase,
    pub possibly_started: bool,
    pub attempt_deadline_at_ms: i64,
    pub result: Option<DnsResult>,
    pub disposal_confirmed: bool,
}
impl EffectSession {
    pub fn validate_history(&self) -> Res<()> {
        self.request.validate(0)?;
        let valid = [
            !self.run_id.is_nil(),
            !self.result_ref.is_nil(),
            self.attempt_deadline_at_ms > 0,
            self.attempt_deadline_at_ms <= self.request.deadline_at_ms,
            self.phase != EffectPhase::Closed || self.disposal_confirmed,
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Store("live_effect_decode"));
        }
        if let Some(result) = &self.result {
            result.validate(&self.request, self.attempt_deadline_at_ms)?;
            if !self.possibly_started {
                return Err(Fail::Store("live_effect_decode"));
            }
        }
        Ok(())
    }
    pub fn launchable(&self) -> bool {
        [
            self.phase == EffectPhase::Reserved,
            !self.possibly_started,
            self.result.is_none(),
            !self.disposal_confirmed,
        ]
        .into_iter()
        .all(|v| v)
    }
}

pub trait EffectStore {
    fn reserve(&mut self, run: Uuid) -> Res<EffectSession>;
    fn inspect(&mut self, run: Uuid) -> Res<EffectSession>;
    fn current_deadline(&mut self, session: &EffectSession) -> Res<i64>;
    fn may_start(&mut self, session: &EffectSession) -> Res<i64>;
    fn record_result(&mut self, session: &EffectSession, result: &DnsResult) -> Res<()>;
    fn stop(&mut self, session: &EffectSession) -> Res<()>;
    fn close_disposed(&mut self, session: &EffectSession) -> Res<()>;
}

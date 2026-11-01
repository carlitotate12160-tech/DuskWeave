//! Restricted Broker bookkeeping and Mission stop-only SQL adapters.
use crate::m1_dns::DnsResult;
use crate::m1_effect_session::{EffectSession, EffectStore};
use crate::mission::OperationId;
use crate::registration::OperationAllocator;
use crate::withdrawal::{MissionAuthorityWithdrawn, WithdrawalRequest, WithdrawalStore};
use crate::{Fail, Res};
use postgres::Client;
use serde_json::Value;
use uuid::Uuid;

pub struct PgEffectStore {
    client: Client,
}
impl PgEffectStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
    fn record(&mut self, query: &'static str, run: Uuid) -> Res<EffectSession> {
        let raw: Value = self
            .client
            .query_one(query, &[&run])
            .map_err(|_| Fail::State("live_effect_unavailable"))?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let session: EffectSession =
            serde_json::from_value(raw).map_err(|_| Fail::Store("live_effect_decode"))?;
        session.validate_history()?;
        if session.run_id != run {
            return Err(Fail::Store("live_effect_decode"));
        }
        Ok(session)
    }
    fn deadline(&mut self, session: &EffectSession, may_start: bool) -> Res<i64> {
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        let query = if may_start {
            "SELECT execution.live_effect_may_start($1,$2)"
        } else {
            "SELECT execution.live_effect_current($1,$2)"
        };
        self.client
            .query_one(query, &[&session.run_id, &generation])
            .map_err(|_| Fail::State("live_effect_release_refused"))?
            .try_get(0)
            .map_err(|_| Fail::Store("live_effect_decode"))
    }
    fn terminal(&mut self, session: &EffectSession, disposed: bool) -> Res<()> {
        let generation = i64::try_from(session.request.session_generation)
            .map_err(|_| Fail::Store("live_effect_decode"))?;
        self.client
            .execute(
                "SELECT execution.live_effect_stop($1,$2,$3)",
                &[&session.run_id, &generation, &disposed],
            )
            .map_err(|_| Fail::Unresolved("live_effect_stop_pending"))?;
        Ok(())
    }
}
impl EffectStore for PgEffectStore {
    fn reserve(&mut self, run: Uuid) -> Res<EffectSession> {
        self.record("SELECT execution.live_effect_reserve($1)", run)
    }
    fn inspect(&mut self, run: Uuid) -> Res<EffectSession> {
        self.record("SELECT execution.live_effect_record($1)", run)
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
            "SELECT execution.live_dns_result($1,$2,$3)",
            &[&session.run_id, &generation, &raw],
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

/// Only the restrictive Mission function is callable with this connection.
pub struct PgStopWriter {
    client: Client,
    generation: i64,
}
impl PgStopWriter {
    pub fn new(client: Client, generation: i64) -> Self {
        Self { client, generation }
    }
}
impl PgStopWriter {
    pub fn latest(mut client: Client, e: Uuid, c: Uuid) -> Res<Option<Self>> {
        let generation: Option<i64> = client
            .query_one("SELECT execution.live_stop_generation($1,$2)", &[&e, &c])
            .map_err(|_| Fail::Unresolved("live_stop_metadata_pending"))?
            .try_get(0)
            .map_err(|_| Fail::Store("contract_decode"))?;
        Ok(generation.map(|generation| Self { client, generation }))
    }
    fn record(
        &mut self,
        q: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        q.validate()?;
        let raw = serde_json::to_value(q).map_err(|_| Fail::Input("invalid_input"))?;
        let row = self
            .client
            .query_one(
                "SELECT mission.live_stop_withdrawal($1,$2,$3,$4)",
                &[&raw, &operation.0, &self.generation, &recover],
            )
            .map_err(|error| {
                if error.as_db_error().is_some() {
                    Fail::State("live_withdrawal_refused")
                } else {
                    Fail::Store("commit_unknown")
                }
            })?;
        let value: Option<Value> = row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?;
        value
            .map(|value| {
                let event: MissionAuthorityWithdrawn =
                    serde_json::from_value(value).map_err(|_| Fail::Store("contract_decode"))?;
                event.validate()?;
                if event.request != *q || event.operation_id != operation {
                    return Err(Fail::Store("contract_decode"));
                }
                Ok(event)
            })
            .transpose()
    }
}

impl WithdrawalStore for PgStopWriter {
    fn withdraw(
        &mut self,
        q: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
        _allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        self.record(q, operation, recover)
    }
}
impl crate::m1_effect_control::authority::RestrictedWithdrawal for PgStopWriter {
    fn withdraw(
        &mut self,
        q: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        self.record(q, operation, recover)
    }
}

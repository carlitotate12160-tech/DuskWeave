//! Fixed owner-local Serializable mutation and strictly read-only recovery.
//! Decoding follows COMMIT: a failed reply/decode never proves rollback.
use crate::m1_prepared_withdrawal::{
    PreparedWithdrawalRecord, PreparedWithdrawalRequest, PreparedWithdrawalStore,
};
use crate::mission::{MissionRegistered, OperationId};
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel, Row};

pub struct PgPreparedWithdrawalStore {
    client: Client,
}

impl PgPreparedWithdrawalStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

fn error(e: &postgres::Error) -> Fail {
    match e.code().map(postgres::error::SqlState::code) {
        Some("40001" | "40P01") => Fail::Store("serialization_retry"),
        Some("P0001") => Fail::State("prepared_withdrawal_refused"),
        Some("42501") => Fail::Config("unqualified_session_writer"),
        _ => Fail::Unresolved("prepared_withdrawal_unknown"),
    }
}

// A COMMIT error has a different claim boundary from a guarded query refusal.
fn commit_error(e: &postgres::Error) -> Fail {
    match e.code().map(postgres::error::SqlState::code) {
        Some("40001" | "40P01") => Fail::Store("serialization_retry"),
        _ => Fail::Unresolved("prepared_withdrawal_unknown"),
    }
}

// LEFT JOIN keeps a corrupt link visible; missing owner is never absence.
const RECOVER: &str = "SELECT to_jsonb(p)-'contract' ||
    jsonb_build_object('writer_oid',p.writer_oid::bigint,'event',w.contract),
    COALESCE(w.contract=p.contract
        AND w.event_id::text=p.contract->>'event_id'
        AND w.registration_operation_id::text=p.contract->>'registration_operation_id'
        AND w.owner_revision=2 AND w.producer='mission' AND w.version=1
        AND w.kind='mission_authority_withdrawn'
        AND w.publication_obligation='trajectory.withdrawal_history.v1',false)
    FROM execution.prepared_withdrawals p LEFT JOIN mission.withdrawals w
    ON (w.engagement_id,w.campaign_id,w.operation_id)
        =(p.engagement_id,p.campaign_id,p.operation_id)
    WHERE p.engagement_id=$1 AND p.campaign_id=$2 AND p.operation_id=$3";
const SUBMIT: &str = "SELECT to_jsonb(p)-'contract' ||
    jsonb_build_object('writer_oid',p.writer_oid::bigint,'event',p.contract),true
    FROM execution.withdraw_prepared_session($1,$2,$3,$4,$5,$6,$7,$8,$9) p";

fn decode(row: Row) -> Res<PreparedWithdrawalRecord> {
    let fail = || Fail::Unresolved("prepared_withdrawal_decode");
    if !row.try_get::<_, bool>(1).map_err(|_| fail())? {
        return Err(fail());
    }
    let raw = row.try_get::<_, serde_json::Value>(0).map_err(|_| fail())?;
    serde_json::from_value(raw).map_err(|_| fail())
}

impl PreparedWithdrawalStore for PgPreparedWithdrawalStore {
    fn execute(
        &mut self,
        r: &PreparedWithdrawalRequest,
        op: OperationId,
        recover: bool,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<PreparedWithdrawalRecord>> {
        let source = serde_json::to_value(source).map_err(|_| Fail::Store("encode"))?;
        let reason =
            serde_json::to_value(r.withdrawal.reason).map_err(|_| Fail::Store("encode"))?;
        let reason = reason
            .as_str()
            .ok_or(Fail::Input("invalid_withdrawal_reason"))?;
        let revision = r.withdrawal.expected_mission_revision as i64;
        let params: [&(dyn postgres::types::ToSql + Sync); 9] = [
            &r.withdrawal.engagement_id.0,
            &r.withdrawal.campaign_id.0,
            &op.0,
            &r.session_operation_id.0,
            &r.expected_session_generation,
            &r.withdrawal.operator_ref.0,
            &revision,
            &reason,
            &source,
        ];
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| error(&e))?;
        let row = tx
            .query_opt(
                if recover { RECOVER } else { SUBMIT },
                &params[..if recover { 3 } else { 9 }],
            )
            .map_err(|e| error(&e))?;
        tx.commit().map_err(|e| commit_error(&e))?;
        row.map(decode).transpose()
    }
}

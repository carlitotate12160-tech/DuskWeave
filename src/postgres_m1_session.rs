//! PostgreSQL adapter for the Broker's prepared/no-effects session port.
//! Guarded SQLSTATEs map to confirmed refusals and known serialization
//! aborts; every other failure — transport, commit or acknowledgment loss —
//! stays unknown for recovery, never a claimed rollback.
use crate::m1_session::{SessionAction, SessionRecord, SessionRequest, SessionStore};
use crate::mission::{MissionRegistered, OperationId};
use crate::{Fail, Res};
use postgres::Client;

pub struct PgSessionStore {
    client: Client,
}

impl PgSessionStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn committed_query(
        &mut self,
        sql: &str,
        params: &[&(dyn postgres::types::ToSql + Sync)],
    ) -> Res<Option<postgres::Row>> {
        // A commit whose reply is lost is unknown, never a claimed
        // rollback; the record is only returned after COMMIT lands.
        let mut tx = self.client.transaction().map_err(|e| session_err(&e))?;
        let row = tx.query_opt(sql, params).map_err(|e| session_err(&e))?;
        tx.commit().map_err(|e| session_err(&e))?;
        Ok(row)
    }

    fn recover_row(&mut self, r: &SessionRequest, op: OperationId) -> Res<Option<postgres::Row>> {
        self.client
            .query_opt(
                "SELECT to_jsonb(h) FROM execution.session_history h \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 \
                 ORDER BY (kind='released_no_effects') DESC LIMIT 1",
                &[&r.engagement_id.0, &r.campaign_id.0, &op.0],
            )
            .map_err(|e| session_err(&e))
    }

    fn prepare_row(
        &mut self,
        r: &SessionRequest,
        op: OperationId,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<postgres::Row>> {
        let original = serde_json::to_value(source.ok_or(Fail::Store("session_contract_decode"))?)
            .map_err(|_| Fail::Store("session_contract_decode"))?;
        self.committed_query(
            "SELECT to_jsonb(execution.prepare_session($1,$2,$3,$4,$5,$6))",
            &[
                &r.engagement_id.0,
                &r.campaign_id.0,
                &op.0,
                &r.operator_ref.0,
                &r.expected_mission_revision,
                &original,
            ],
        )
    }

    fn release_row(&mut self, r: &SessionRequest, op: OperationId) -> Res<Option<postgres::Row>> {
        self.committed_query(
            "SELECT to_jsonb(execution.release_prepared_session($1,$2,$3,$4,$5))",
            &[
                &r.engagement_id.0,
                &r.campaign_id.0,
                &op.0,
                &r.operator_ref.0,
                &r.expected_mission_revision,
            ],
        )
    }
}

fn session_err(e: &postgres::Error) -> Fail {
    match e.code().map(postgres::error::SqlState::code) {
        Some("P0001") => Fail::State("session_refused"),
        Some("42501") => Fail::Config("unqualified_session_writer"),
        Some("40001" | "40P01") => Fail::Store("serialization_retry"),
        _ => Fail::Unresolved("session_outcome_unknown"),
    }
}

fn decode(row: postgres::Row) -> Res<SessionRecord> {
    serde_json::from_value(row.get(0)).map_err(|_| Fail::Unresolved("session_contract_decode"))
}

impl SessionStore for PgSessionStore {
    fn execute(
        &mut self,
        action: SessionAction,
        r: &SessionRequest,
        op: OperationId,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<SessionRecord>> {
        let row = match action {
            SessionAction::Recover => self.recover_row(r, op)?,
            SessionAction::Prepare => self.prepare_row(r, op, source)?,
            SessionAction::Release => self.release_row(r, op)?,
        };
        row.map(decode).transpose()
    }
}

use crate::m1_session::{FenceOutcome, SessionClaim, SessionFence, SessionRecord, SessionRequest};
use crate::mission::OperationId;
use crate::{Fail, Res};
use postgres::Transaction;

pub struct PostgresSessionFence<'a, 'b> {
    pub tx: &'a mut Transaction<'b>,
}

impl<'a, 'b> SessionFence for PostgresSessionFence<'a, 'b> {
    fn prepare(&mut self, claim: &SessionClaim) -> Res<FenceOutcome> {
        let raw_claim = serde_json::to_value(claim).map_err(|_| Fail::Store("encode_claim"))?;
        
        let query = "SELECT execution.prepare_m1_session($1)";
        let row = match self.tx.query_opt(query, &[&raw_claim]) {
            Ok(Some(row)) => row,
            Ok(None) => return Err(Fail::Store("missing_return")),
            Err(e) => return classify_error(e),
        };

        let result: serde_json::Value = row.get(0);
        if let Some(obj) = result.as_object() {
            if let Some(refusal) = obj.get("refused") {
                if let Some(reason) = refusal.as_str() {
                    return Ok(FenceOutcome::Refused(crate::m1_session::decode_refusal(reason)));
                }
            }
        }

        match SessionRecord::decode(result) {
            Ok(record) => Ok(FenceOutcome::Durable(record)),
            Err(Fail::Store("contract_decode")) => Ok(FenceOutcome::Unknown),
            Err(e) => Err(e),
        }
    }

    fn release(&mut self, request: &SessionRequest, operation: OperationId) -> Res<FenceOutcome> {
        let query = "SELECT execution.release_m1_session($1, $2, $3, $4, $5)";
        let row = match self.tx.query_opt(
            query,
            &[
                &request.engagement_id.0,
                &request.campaign_id.0,
                &operation.0,
                &request.operator_ref.0,
                &(request.expected_mission_revision as i64),
            ],
        ) {
            Ok(Some(row)) => row,
            Ok(None) => return Err(Fail::Store("missing_return")),
            Err(e) => return classify_error(e),
        };

        let result: serde_json::Value = row.get(0);
        if let Some(obj) = result.as_object() {
            if let Some(refusal) = obj.get("refused") {
                if let Some(reason) = refusal.as_str() {
                    return Ok(FenceOutcome::Refused(crate::m1_session::decode_refusal(reason)));
                }
            }
        }

        match SessionRecord::decode(result) {
            Ok(record) => Ok(FenceOutcome::Durable(record)),
            Err(Fail::Store("contract_decode")) => Ok(FenceOutcome::Unknown),
            Err(e) => Err(e),
        }
    }

    fn recover(&mut self, request: &SessionRequest, operation: OperationId) -> Res<FenceOutcome> {
        let query = r#"
            SELECT row_to_json(h.*)
            FROM execution.session_history h
            WHERE engagement_id = $1 AND campaign_id = $2 AND operation_id = $3
            ORDER BY recorded_at DESC
            LIMIT 1
        "#;
        let row = match self.tx.query_opt(query, &[&request.engagement_id.0, &request.campaign_id.0, &operation.0]) {
            Ok(Some(row)) => row,
            Ok(None) => return Ok(FenceOutcome::Missing),
            Err(e) => return classify_error(e),
        };

        let result: serde_json::Value = row.get(0);
        match SessionRecord::decode(result) {
            Ok(record) => Ok(FenceOutcome::Durable(record)),
            Err(Fail::Store("contract_decode")) => Ok(FenceOutcome::Unknown),
            Err(e) => Err(e),
        }
    }
}

fn classify_error(e: postgres::Error) -> Res<FenceOutcome> {
    if let Some(db_err) = e.as_db_error() {
        if db_err.code() == &postgres::error::SqlState::T_R_SERIALIZATION_FAILURE
            || db_err.code() == &postgres::error::SqlState::T_R_DEADLOCK_DETECTED
        {
            return Err(Fail::Store("serialization_retry"));
        }
    }
    Err(Fail::Store("commit_unknown"))
}

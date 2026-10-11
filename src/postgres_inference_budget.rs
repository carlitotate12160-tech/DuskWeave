//! Broker's guarded PostgreSQL reservation adapter; no authority SQL is copied here.
use crate::m1_inference_budget::{
    InferenceReservationStore, ReservationAction, ReservationRecord, ReservationRequest,
};
use crate::mission::{MissionRegistered, OperationId};
use crate::{Fail, Res};
use postgres::Client;

pub struct PgInferenceReservationStore {
    client: Client,
}
impl PgInferenceReservationStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

fn reservation_err(e: &postgres::Error) -> Fail {
    match e.code().map(postgres::error::SqlState::code) {
        Some("P0001") => Fail::State("inference_reservation_refused"),
        Some("42501") => Fail::Config("unqualified_inference_writer"),
        Some("40001" | "40P01") => Fail::Store("serialization_retry"),
        _ => Fail::Unresolved("inference_reservation_unknown"),
    }
}

impl InferenceReservationStore for PgInferenceReservationStore {
    fn execute(
        &mut self,
        action: ReservationAction,
        request: &ReservationRequest,
        op: OperationId,
        source: Option<&MissionRegistered>,
    ) -> Res<Option<ReservationRecord>> {
        // These closed types contain only strings, integers, UUIDs and enums.
        let q = serde_json::json!(request);
        let source = serde_json::json!(source);
        let mut tx = self.client.transaction().map_err(|e| reservation_err(&e))?;
        let row = tx
            .query_one(
                "SELECT to_jsonb(execution.inference_reservation($1,$2,$3,$4))",
                &[&action.name(), &op.0, &q, &source],
            )
            .map_err(|e| reservation_err(&e))?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("inference_reservation_unknown"))?;
        let raw: Option<serde_json::Value> = row
            .try_get(0)
            .map_err(|_| Fail::Unresolved("inference_record_decode"))?;
        raw.map(serde_json::from_value)
            .transpose()
            .map_err(|_| Fail::Unresolved("inference_record_decode"))
    }
}

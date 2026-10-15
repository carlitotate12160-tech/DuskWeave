//! Reasoning publication consumed by the existing Trajectory journal.
use super::PgTrajectory;
use crate::m1_reasoning::inference::{Decision, ReasoningHistory};
use crate::m1_reasoning::postgres::decode;
use crate::mission::{CampaignId, EngagementId, EventId, OperationId};
use crate::postgres_trajectory_journal::{self as journal, JournalRecord, JournalTable};
use crate::trajectory::Delivered;
use crate::{Fail, Res};
use serde_json::Value;
fn record(v: Value) -> Res<JournalRecord> {
    let d = decode(v.clone()).map_err(Fail::Store)?;
    Ok(JournalRecord {
        engagement_id: EngagementId(d.engagement),
        campaign_id: CampaignId(d.campaign),
        event_id: EventId(d.event),
        operation_id: OperationId(d.decision),
        producer: d.producer.into(),
        kind: "reasoning_decision".into(),
        version: d.version,
        obligation: d.publication_obligation.into(),
        contract: v,
    })
}
fn value(d: &Decision) -> Res<JournalRecord> {
    record(serde_json::to_value(d).map_err(|_| Fail::Store("inference_decision_encode"))?)
}
fn status(outcome: Delivered) -> &'static str {
    match outcome {
        Delivered::Completed | Delivered::Duplicate => "completed",
        Delivered::Anomaly => "anomaly",
        Delivered::Unresolved(_) => "pending",
    }
}
impl ReasoningHistory for PgTrajectory {
    fn accept_committed_source(&mut self, d: &Decision) -> Result<&'static str, &'static str> {
        let r = value(d).map_err(|_| "inference_decision_integrity")?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(postgres::IsolationLevel::Serializable)
            .start()
            .map_err(|_| "inference_history_unknown")?;
        // Identity order and source checks share the same short transaction.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!("{}:{}", d.engagement, d.campaign)],
        )
        .map_err(|_| "inference_history_unknown")?;
        let outcome = append_source(&mut tx, d, &r)?;
        tx.commit()
            .map_err(|_| "inference_history_commit_unknown")?;
        Ok(outcome)
    }

    fn inspect(&mut self, d: &Decision) -> Result<&'static str, &'static str> {
        let r = value(d).map_err(|_| "inference_decision_integrity")?;
        Ok(
            journal::existing(&mut self.client, JournalTable::Reasoning, &r, record)
                .map_err(|_| "inference_history_integrity")?
                .map(status)
                .unwrap_or("pending"),
        )
    }
}

fn append_source(
    tx: &mut postgres::Transaction<'_>,
    d: &Decision,
    r: &JournalRecord,
) -> Result<&'static str, &'static str> {
    let matched: bool = tx
        .query_one(
            "SELECT cognition.reasoning_source_matches($1,$2)",
            &[&d.event, &r.contract],
        )
        .map_err(|_| "inference_source_unavailable")?
        .get(0);
    if !matched {
        return Err("inference_source_unavailable");
    }
    let previous = journal::existing(tx, JournalTable::Reasoning, r, record)
        .map_err(|_| "inference_history_integrity")?;
    if matches!(previous, Some(Delivered::Anomaly)) {
        journal::append_anomaly(tx, JournalTable::Reasoning, r)
            .map_err(|_| "inference_history_unknown")?;
        return Ok("anomaly");
    }
    if previous.is_none() {
        return journal::append_accepted(tx, JournalTable::Reasoning, r)
            .map(|()| "completed")
            .map_err(|_| "inference_history_unknown");
    }
    Ok("completed")
}

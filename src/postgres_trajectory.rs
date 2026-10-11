//! CampaignTrajectory owner's PostgreSQL adapter. Queries only
//! trajectory.* tables and a narrow read-only Mission publication port;
//! history insertion and completion commit atomically.
#[path = "postgres_withdrawal_history.rs"]
mod postgres_withdrawal_history;

use crate::mission::{CampaignId, EngagementId, EventId, MissionRegistered};
use crate::planning::PlanningAssessed;
use crate::planning_history::PlanningHistoryPort;
use crate::postgres_planning_history as planning;
use crate::postgres_trajectory_history::{self, RegistrationRecord, store_err};
use crate::registration::TrajectoryPort;
use crate::trajectory::{Delivered, HistoryStatus, check_event};
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel};

const REGISTRATION_OBLIGATION: &str = "trajectory.registration_history.v1";

pub struct PgTrajectory {
    client: Client,
}

impl PgTrajectory {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

/// Existing identity conflicts stay journal anomalies; first v3 acceptance
/// additionally requires the exact committed publication from the Mission port.
fn registration_publication_ready(
    tx: &mut postgres::Transaction,
    event: &MissionRegistered,
) -> Res<bool> {
    if event.version != 3 {
        return Ok(true);
    }
    let known = tx
        .query_opt(
            "SELECT 1 FROM trajectory.registration_history WHERE engagement_id=$1
         AND campaign_id=$2 AND event_id=$3 LIMIT 1",
            &[
                &event.engagement_id.0,
                &event.campaign_id.0,
                &event.event_id.0,
            ],
        )
        .map_err(|e| store_err(&e))?
        .is_some();
    if known {
        return Ok(true);
    }
    crate::postgres_mission::source_matches(tx, event)
}

/// Trajectory owns the short acceptance/completion transaction.
fn commit_registration_history(
    client: &mut Client,
    ev: &MissionRegistered,
    record: &RegistrationRecord<'_>,
) -> Res<Delivered> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::Serializable)
        .start()
        .map_err(|e| store_err(&e))?;
    if !registration_publication_ready(&mut tx, ev)? {
        return Ok(Delivered::Unresolved("missing_committed_publication"));
    }
    let outcome = postgres_trajectory_history::append(&mut tx, record)?;
    tx.commit().map_err(|e| store_err(&e))?;
    Ok(outcome)
}

impl TrajectoryPort for PgTrajectory {
    fn deliver(&mut self, ev: &MissionRegistered) -> Res<Delivered> {
        if let Err(cat) = check_event(ev) {
            return Ok(Delivered::Unresolved(cat));
        }
        let contract = serde_json::to_value(ev).map_err(|_| Fail::Store("encode"))?;
        let record = RegistrationRecord {
            engagement_id: ev.engagement_id.0,
            campaign_id: ev.campaign_id.0,
            producer: &ev.producer,
            operation_id: ev.operation_id.0,
            event_id: ev.event_id.0,
            obligation: REGISTRATION_OBLIGATION,
            contract: &contract,
        };
        if ev.version == 3 {
            planning::qualify(&mut self.client, true)?;
        }
        commit_registration_history(&mut self.client, ev, &record)
    }

    fn registration_status(&mut self, event: &MissionRegistered) -> Res<HistoryStatus> {
        if event.version != 3 {
            return self.status(event.engagement_id, event.campaign_id, event.event_id);
        }
        planning::qualify(&mut self.client, false)?;
        postgres_trajectory_history::bound_status(&mut self.client, event)
    }

    fn status(&mut self, e: EngagementId, c: CampaignId, ev: EventId) -> Res<HistoryStatus> {
        postgres_trajectory_history::status(&mut self.client, e.0, c.0, ev.0)
    }
}

impl PlanningHistoryPort for PgTrajectory {
    fn publish(&mut self, event: &PlanningAssessed) -> Res<Delivered> {
        event.validate()?;
        planning::qualify(&mut self.client, true)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
        let outcome = planning::append(&mut tx, event)?;
        tx.commit().map_err(|e| planning::commit_error(&e))?;
        Ok(outcome)
    }

    fn inspect(&mut self, event: &PlanningAssessed) -> Res<Delivered> {
        event.validate()?;
        planning::qualify(&mut self.client, false)?;
        Ok(planning::existing(&mut self.client, event)?
            .unwrap_or(Delivered::Unresolved("not_recorded")))
    }
}

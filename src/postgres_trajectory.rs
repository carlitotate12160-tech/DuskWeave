//! CampaignTrajectory owner's PostgreSQL adapter. Queries only
//! trajectory.* tables; history insertion and completion commit atomically.

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
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
        let outcome = postgres_trajectory_history::append(&mut tx, &record)?;
        tx.commit().map_err(|e| store_err(&e))?;
        Ok(outcome)
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

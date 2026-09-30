//! CampaignTrajectory owner: sourced acceptance of the published Mission
//! contract. Never imports Mission's aggregate or the application layer.

use crate::mission::{CONTRACT_KIND, CONTRACT_VERSION, MissionRegistered, PRODUCER};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivered {
    Completed,
    Duplicate,
    Anomaly,
    Unresolved(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryStatus {
    Completed,
    Pending,
    /// Integrity anomaly for this event identity; blocks completion even
    /// if an accepted row exists.
    Anomaly,
}

fn declared(ev: &MissionRegistered) -> bool {
    ev.producer == PRODUCER
        && ev.kind == CONTRACT_KIND
        && ev.version == CONTRACT_VERSION
        && ev.owner_revision == 1
}

fn scoped(ev: &MissionRegistered) -> bool {
    ev.affected_entity == ev.campaign_id
        && ev.causation_id == ev.operation_id
        && ev.correlation_id == ev.operation_id
}

pub fn check_event(ev: &MissionRegistered) -> Result<(), &'static str> {
    if !declared(ev) {
        return Err("unsupported_contract");
    }
    if !scoped(ev) {
        return Err("scope_violation");
    }
    ev.fields.validate().map_err(|_| "invalid_fields")
}
